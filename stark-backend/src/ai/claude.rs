use crate::ai::types::{
    AiError, AiResponse, ClaudeContentBlock, ClaudeMessage as TypedClaudeMessage,
    ClaudeMessageContent, ClaudeTool, ThinkingLevel, ToolCall, ToolResponse,
};
use crate::ai::{Message, MessageRole};
use crate::gateway::events::EventBroadcaster;
use crate::gateway::protocol::GatewayEvent;
use crate::tools::ToolDefinition;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

pub struct ClaudeClient {
    client: Client,
    auth_headers: header::HeaderMap,
    endpoint: String,
    model: String,
    /// Thinking budget in tokens (0 = disabled)
    thinking_budget: AtomicU32,
    /// Optional broadcaster for emitting retry events
    broadcaster: Option<Arc<EventBroadcaster>>,
    /// Channel ID for events
    channel_id: Option<i64>,
}

impl Clone for ClaudeClient {
    fn clone(&self) -> Self {
        ClaudeClient {
            client: self.client.clone(),
            auth_headers: self.auth_headers.clone(),
            endpoint: self.endpoint.clone(),
            model: self.model.clone(),
            thinking_budget: AtomicU32::new(self.thinking_budget.load(Ordering::SeqCst)),
            broadcaster: self.broadcaster.clone(),
            channel_id: self.channel_id,
        }
    }
}

/// Extended thinking configuration for Claude
#[derive(Debug, Clone, Serialize)]
struct ThinkingConfig {
    #[serde(rename = "type")]
    thinking_type: String,
    budget_tokens: u32,
}

#[derive(Debug, Serialize)]
struct ClaudeCompletionRequest {
    model: String,
    messages: Vec<SimpleClaudeMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ThinkingConfig>,
}

#[derive(Debug, Serialize)]
struct SimpleClaudeMessage {
    role: String,
    content: String,
}

/// Tool choice options for Claude API
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
#[serde(rename_all = "snake_case")]
enum ToolChoice {
    /// Model decides whether to use tools
    Auto,
    /// Model MUST use a tool
    Any,
    /// Model MUST use the specified tool
    #[allow(dead_code)]
    Tool { name: String },
}

#[derive(Debug, Serialize)]
struct ClaudeToolRequest {
    model: String,
    messages: Vec<TypedClaudeMessage>,
    max_tokens: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    system: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<ClaudeTool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_choice: Option<ToolChoice>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<ThinkingConfig>,
}

#[derive(Debug, Deserialize)]
struct ClaudeCompletionResponse {
    content: Vec<ClaudeResponseContent>,
    #[serde(default)]
    stop_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClaudeResponseContent {
    #[serde(rename = "type")]
    content_type: String,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    input: Option<Value>,
}

#[derive(Debug, Deserialize)]
struct ClaudeErrorResponse {
    error: ClaudeError,
}

#[derive(Debug, Deserialize)]
struct ClaudeError {
    message: String,
}

impl ClaudeClient {
    pub fn new(api_key: &str, endpoint: Option<&str>, model: Option<&str>) -> Result<Self, String> {
        let mut auth_headers = header::HeaderMap::new();
        auth_headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );

        let auth_value = header::HeaderValue::from_str(api_key)
            .map_err(|e| format!("Invalid API key format: {}", e))?;
        auth_headers.insert("x-api-key", auth_value);
        auth_headers.insert(
            "anthropic-version",
            header::HeaderValue::from_static("2023-06-01"),
        );

        Ok(Self {
            client: crate::http::shared_client().clone(),
            auth_headers,
            endpoint: endpoint
                .unwrap_or("https://api.anthropic.com/v1/messages")
                .to_string(),
            model: model.unwrap_or("claude-sonnet-4-20250514").to_string(),
            thinking_budget: AtomicU32::new(0),
            broadcaster: None,
            channel_id: None,
        })
    }

    /// Set the broadcaster for emitting retry events
    pub fn with_broadcaster(mut self, broadcaster: Arc<EventBroadcaster>, channel_id: i64) -> Self {
        self.broadcaster = Some(broadcaster);
        self.channel_id = Some(channel_id);
        self
    }

    /// Emit a retry event if broadcaster is configured
    fn emit_retry_event(&self, attempt: u32, max_attempts: u32, wait_seconds: u64, error: &str) {
        if let (Some(broadcaster), Some(channel_id)) = (&self.broadcaster, self.channel_id) {
            broadcaster.broadcast(GatewayEvent::ai_retrying(
                channel_id,
                attempt,
                max_attempts,
                wait_seconds,
                error,
                "claude",
            ));
        }
    }

    /// Set the thinking level for subsequent requests
    pub fn set_thinking_level(&self, level: ThinkingLevel) {
        let budget = level.budget_tokens().unwrap_or(0);
        self.thinking_budget.store(budget, Ordering::SeqCst);
        log::info!("Claude thinking level set to {} (budget: {} tokens)", level, budget);
    }

    /// Get the current thinking budget
    pub fn get_thinking_budget(&self) -> u32 {
        self.thinking_budget.load(Ordering::SeqCst)
    }

    /// Build thinking config if enabled
    fn build_thinking_config(&self) -> Option<ThinkingConfig> {
        let budget = self.get_thinking_budget();
        if budget > 0 {
            Some(ThinkingConfig {
                thinking_type: "enabled".to_string(),
                budget_tokens: budget,
            })
        } else {
            None
        }
    }

    pub async fn generate_text(&self, messages: Vec<Message>) -> Result<String, String> {
        // Extract system message if present
        let mut system_message = None;
        let filtered_messages: Vec<Message> = messages
            .into_iter()
            .filter(|m| {
                if m.role == MessageRole::System {
                    system_message = Some(m.content.clone());
                    false
                } else {
                    true
                }
            })
            .collect();

        let api_messages: Vec<SimpleClaudeMessage> = filtered_messages
            .into_iter()
            .map(|m| SimpleClaudeMessage {
                role: m.role.to_string(),
                content: m.content,
            })
            .collect();

        let thinking = self.build_thinking_config();
        let request = ClaudeCompletionRequest {
            model: self.model.clone(),
            messages: api_messages,
            max_tokens: 4096,
            system: system_message,
            thinking,
        };

        log::debug!("Sending request to Claude API: {:?}", request);

        // Retry configuration for transient errors
        const MAX_RETRIES: u32 = 3;
        const BASE_DELAY_MS: u64 = 2000;

        let mut last_error: Option<String> = None;
        let mut response_data_opt: Option<ClaudeCompletionResponse> = None;

        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                let delay_ms = BASE_DELAY_MS * (1 << (attempt - 1));
                let wait_secs = delay_ms / 1000;
                log::warn!(
                    "[CLAUDE] Retry attempt {}/{} after {}ms delay",
                    attempt,
                    MAX_RETRIES,
                    delay_ms
                );
                // Emit retry event to frontend
                self.emit_retry_event(
                    attempt,
                    MAX_RETRIES,
                    wait_secs,
                    last_error.as_deref().unwrap_or("Unknown error"),
                );
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }

            let request_result = self
                .client
                .post(&self.endpoint)
                .headers(self.auth_headers.clone())
                .json(&request)
                .send()
                .await;

            let response = match request_result {
                Ok(r) => r,
                Err(e) => {
                    last_error = Some(format!("Claude API request failed: {}", e));
                    if attempt < MAX_RETRIES {
                        log::warn!("[CLAUDE] Request failed (attempt {}): {}, will retry", attempt + 1, e);
                        continue;
                    }
                    return Err(last_error.unwrap());
                }
            };

            let status = response.status();
            let status_code = status.as_u16();
            let is_retryable = matches!(status_code, 429 | 502 | 503 | 504);

            if !status.is_success() {
                let error_text = response.text().await.unwrap_or_default();

                // Check if this is a transient 402 error (payment settlement network failure)
                let is_transient_402 = status_code == 402 && (
                    error_text.contains("connection failed") ||
                    error_text.contains("Connection failed") ||
                    error_text.contains("error sending request") ||
                    error_text.contains("timed out") ||
                    error_text.contains("timeout") ||
                    error_text.contains("temporarily unavailable") ||
                    error_text.contains("network error")
                );

                if (is_retryable || is_transient_402) && attempt < MAX_RETRIES {
                    log::warn!(
                        "[CLAUDE] Received retryable status {} (attempt {}), will retry",
                        status,
                        attempt + 1
                    );
                    last_error = Some(format!("HTTP {}: {}", status, error_text));
                    continue;
                }

                if let Ok(error_response) = serde_json::from_str::<ClaudeErrorResponse>(&error_text) {
                    return Err(format!("Claude API error: {}", error_response.error.message));
                }

                return Err(format!(
                    "Claude API returned error status: {}, body: {}",
                    status, error_text
                ));
            }

            response_data_opt = Some(response
                .json()
                .await
                .map_err(|e| format!("Failed to parse Claude response: {}", e))?);
            break;
        }

        let response_data = response_data_opt.ok_or_else(|| {
            last_error.unwrap_or_else(|| "Max retries exceeded".to_string())
        })?;

        // Concatenate all text content from response
        let content: String = response_data
            .content
            .iter()
            .filter(|c| c.content_type == "text")
            .filter_map(|c| c.text.clone())
            .collect();

        if content.is_empty() {
            return Err("Claude API returned no content".to_string());
        }

        Ok(content)
    }

    /// Generate a response with tool support
    pub async fn generate_with_tools(
        &self,
        messages: Vec<Message>,
        tool_messages: Vec<TypedClaudeMessage>,
        tools: Vec<ToolDefinition>,
    ) -> Result<AiResponse, AiError> {
        // Extract system message if present
        let mut system_message = None;
        let filtered_messages: Vec<Message> = messages
            .into_iter()
            .filter(|m| {
                if m.role == MessageRole::System {
                    system_message = Some(m.content.clone());
                    false
                } else {
                    true
                }
            })
            .collect();

        // Convert regular messages to typed messages
        let mut api_messages: Vec<TypedClaudeMessage> = filtered_messages
            .into_iter()
            .map(|m| TypedClaudeMessage {
                role: m.role.to_string(),
                content: ClaudeMessageContent::Text(m.content),
            })
            .collect();

        // Add tool messages (assistant tool_use + user tool_result pairs)
        api_messages.extend(tool_messages);

        // Convert tool definitions to Claude format
        let claude_tools: Vec<ClaudeTool> = tools
            .into_iter()
            .map(|t| ClaudeTool {
                name: t.name,
                description: t.description,
                input_schema: serde_json::to_value(t.input_schema).unwrap_or_default(),
            })
            .collect();

        let thinking = self.build_thinking_config();
        let has_tools = !claude_tools.is_empty();
        let request = ClaudeToolRequest {
            model: self.model.clone(),
            messages: api_messages,
            max_tokens: 4096,
            system: system_message,
            tools: if has_tools {
                Some(claude_tools)
            } else {
                None
            },
            // Force tool use when tools are available
            tool_choice: if has_tools {
                Some(ToolChoice::Any)
            } else {
                None
            },
            thinking,
        };

        log::debug!(
            "Sending tool request to Claude API: {}",
            serde_json::to_string_pretty(&request).unwrap_or_default()
        );

        // Retry configuration for transient errors
        const MAX_RETRIES: u32 = 3;
        const BASE_DELAY_MS: u64 = 2000;

        let mut last_error: Option<(String, Option<u16>)> = None;
        let mut response_data_opt: Option<ClaudeCompletionResponse> = None;

        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                let delay_ms = BASE_DELAY_MS * (1 << (attempt - 1));
                let wait_secs = delay_ms / 1000;
                log::warn!(
                    "[CLAUDE] Tool request retry attempt {}/{} after {}ms delay",
                    attempt,
                    MAX_RETRIES,
                    delay_ms
                );
                // Emit retry event to frontend
                self.emit_retry_event(
                    attempt,
                    MAX_RETRIES,
                    wait_secs,
                    last_error.as_ref().map(|(m, _)| m.as_str()).unwrap_or("Unknown error"),
                );
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            }

            let request_result = self
                .client
                .post(&self.endpoint)
                .headers(self.auth_headers.clone())
                .json(&request)
                .send()
                .await;

            let response = match request_result {
                Ok(r) => r,
                Err(e) => {
                    last_error = Some((format!("Claude API request failed: {}", e), None));
                    if attempt < MAX_RETRIES {
                        log::warn!("[CLAUDE] Tool request failed (attempt {}): {}, will retry", attempt + 1, e);
                        continue;
                    }
                    let (msg, code) = last_error.unwrap();
                    return Err(match code {
                        Some(c) => AiError::with_status(msg, c),
                        None => AiError::new(msg),
                    });
                }
            };

            let status = response.status();
            let status_code = status.as_u16();
            let is_retryable = matches!(status_code, 429 | 502 | 503 | 504);

            if !status.is_success() {
                let error_text = response.text().await.unwrap_or_default();

                // Check if this is a transient 402 error (payment settlement network failure)
                let is_transient_402 = status_code == 402 && (
                    error_text.contains("connection failed") ||
                    error_text.contains("Connection failed") ||
                    error_text.contains("error sending request") ||
                    error_text.contains("timed out") ||
                    error_text.contains("timeout") ||
                    error_text.contains("temporarily unavailable") ||
                    error_text.contains("network error")
                );

                if (is_retryable || is_transient_402) && attempt < MAX_RETRIES {
                    log::warn!(
                        "[CLAUDE] Tool request received retryable status {} (attempt {}), will retry",
                        status,
                        attempt + 1
                    );
                    last_error = Some((format!("HTTP {}: {}", status, error_text), Some(status_code)));
                    continue;
                }

                let error_msg = if let Ok(error_response) = serde_json::from_str::<ClaudeErrorResponse>(&error_text) {
                    format!("Claude API error: {}", error_response.error.message)
                } else {
                    format!("Claude API returned error status: {}, body: {}", status, error_text)
                };

                return Err(AiError::with_status(error_msg, status_code));
            }

            response_data_opt = Some(response
                .json()
                .await
                .map_err(|e| AiError::new(format!("Failed to parse Claude response: {}", e)))?);
            break;
        }

        let response_data = response_data_opt.ok_or_else(|| {
            let (msg, code) = last_error.unwrap_or_else(|| ("Max retries exceeded".to_string(), None));
            match code {
                Some(c) => AiError::with_status(msg, c),
                None => AiError::new(msg),
            }
        })?;

        // Parse the response content
        let mut text_content = String::new();
        let mut tool_calls = Vec::new();

        for content in response_data.content {
            match content.content_type.as_str() {
                "text" => {
                    if let Some(text) = content.text {
                        text_content.push_str(&text);
                    }
                }
                "tool_use" => {
                    if let (Some(id), Some(name), Some(input)) =
                        (content.id, content.name, content.input)
                    {
                        tool_calls.push(ToolCall {
                            id,
                            name,
                            arguments: input,
                        });
                    }
                }
                _ => {}
            }
        }

        Ok(AiResponse {
            content: text_content,
            tool_calls,
            stop_reason: response_data.stop_reason,
            x402_payment: None, // Claude doesn't use x402
        })
    }

    /// Build tool result messages to continue conversation after tool execution
    pub fn build_tool_result_messages(
        tool_calls: &[ToolCall],
        tool_responses: &[ToolResponse],
    ) -> Vec<TypedClaudeMessage> {
        // First message: assistant with tool_use blocks
        let tool_use_blocks: Vec<ClaudeContentBlock> = tool_calls
            .iter()
            .map(|tc| ClaudeContentBlock::ToolUse {
                id: tc.id.clone(),
                name: tc.name.clone(),
                input: tc.arguments.clone(),
            })
            .collect();

        // Second message: user with tool_result blocks
        let tool_result_blocks: Vec<ClaudeContentBlock> = tool_responses
            .iter()
            .map(|tr| ClaudeContentBlock::tool_result(
                tr.tool_call_id.clone(),
                tr.content.clone(),
                tr.is_error,
            ))
            .collect();

        vec![
            TypedClaudeMessage::assistant_with_blocks(tool_use_blocks),
            TypedClaudeMessage::user_with_tool_results(tool_result_blocks),
        ]
    }
}
