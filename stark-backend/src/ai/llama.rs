use crate::ai::types::{AiResponse, ToolCall};
use crate::ai::Message;
use crate::gateway::events::EventBroadcaster;
use crate::gateway::protocol::GatewayEvent;
use crate::tools::ToolDefinition;
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;


/// Llama client for Ollama API (with tool support for Llama 3.1+)
#[derive(Clone)]
pub struct LlamaClient {
    client: Client,
    auth_headers: header::HeaderMap,
    endpoint: String,
    model: String,
    /// Optional broadcaster for emitting retry events
    broadcaster: Option<Arc<EventBroadcaster>>,
    /// Channel ID for events
    channel_id: Option<i64>,
}

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    tools: Option<Vec<OllamaTool>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OllamaMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<OllamaToolCall>>,
}

#[derive(Debug, Serialize, Clone)]
struct OllamaTool {
    #[serde(rename = "type")]
    tool_type: String,
    function: OllamaToolFunction,
}

#[derive(Debug, Serialize, Clone)]
struct OllamaToolFunction {
    name: String,
    description: String,
    parameters: Value,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OllamaToolCall {
    #[serde(default)]
    pub id: Option<String>,
    pub function: OllamaFunctionCall,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct OllamaFunctionCall {
    pub name: String,
    pub arguments: Value,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: OllamaResponseMessage,
    #[serde(default)]
    done_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct OllamaResponseMessage {
    #[serde(default)]
    content: String,
    #[serde(default)]
    tool_calls: Option<Vec<OllamaToolCall>>,
}

#[derive(Debug, Deserialize)]
struct OllamaErrorResponse {
    error: String,
}

impl LlamaClient {
    pub fn new(endpoint: Option<&str>, model: Option<&str>) -> Result<Self, String> {
        let mut auth_headers = header::HeaderMap::new();
        auth_headers.insert(
            header::CONTENT_TYPE,
            header::HeaderValue::from_static("application/json"),
        );

        Ok(Self {
            client: crate::http::shared_client().clone(),
            auth_headers,
            endpoint: endpoint
                .unwrap_or("http://localhost:11434/api/chat")
                .to_string(),
            model: model.unwrap_or("llama3.3").to_string(),
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
                "ollama",
            ));
        }
    }

    pub async fn generate_text(&self, messages: Vec<Message>) -> Result<String, String> {
        let api_messages: Vec<OllamaMessage> = messages
            .into_iter()
            .map(|m| OllamaMessage {
                role: m.role.to_string(),
                content: m.content,
                tool_calls: None,
            })
            .collect();

        let request = OllamaChatRequest {
            model: self.model.clone(),
            messages: api_messages,
            stream: false,
            tools: None,
        };

        log::debug!("Sending request to Ollama API: {:?}", request);

        // Retry configuration for transient errors
        const MAX_RETRIES: u32 = 3;
        const BASE_DELAY_MS: u64 = 2000;

        let mut last_error: Option<String> = None;
        let mut response_data_opt: Option<OllamaChatResponse> = None;

        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                let delay_ms = BASE_DELAY_MS * (1 << (attempt - 1));
                let wait_secs = delay_ms / 1000;
                log::warn!(
                    "[OLLAMA] Retry attempt {}/{} after {}ms delay",
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
                .timeout(Duration::from_secs(300))
                .json(&request)
                .send()
                .await;

            let response = match request_result {
                Ok(r) => r,
                Err(e) => {
                    last_error = Some(format!("Ollama API request failed: {}", e));
                    if attempt < MAX_RETRIES {
                        log::warn!("[OLLAMA] Request failed (attempt {}): {}, will retry", attempt + 1, e);
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
                        "[OLLAMA] Received retryable status {} (attempt {}), will retry",
                        status,
                        attempt + 1
                    );
                    last_error = Some(format!("HTTP {}: {}", status, error_text));
                    continue;
                }

                if let Ok(error_response) = serde_json::from_str::<OllamaErrorResponse>(&error_text) {
                    return Err(format!("Ollama API error: {}", error_response.error));
                }

                return Err(format!(
                    "Ollama API returned error status: {}, body: {}",
                    status, error_text
                ));
            }

            response_data_opt = Some(response
                .json()
                .await
                .map_err(|e| format!("Failed to parse Ollama response: {}", e))?);
            break;
        }

        let response_data = response_data_opt.ok_or_else(|| {
            last_error.unwrap_or_else(|| "Max retries exceeded".to_string())
        })?;

        if response_data.message.content.is_empty() {
            return Err("Ollama API returned no content".to_string());
        }

        Ok(response_data.message.content)
    }

    /// Generate a response with tool support (Llama 3.1+ with Ollama)
    pub async fn generate_with_tools(
        &self,
        messages: Vec<Message>,
        tool_messages: Vec<OllamaMessage>,
        tools: Vec<ToolDefinition>,
    ) -> Result<AiResponse, String> {
        // Convert messages to Ollama format
        let mut api_messages: Vec<OllamaMessage> = messages
            .into_iter()
            .map(|m| OllamaMessage {
                role: m.role.to_string(),
                content: m.content,
                tool_calls: None,
            })
            .collect();

        // Add tool conversation history
        api_messages.extend(tool_messages);

        // Convert tool definitions to Ollama format
        let ollama_tools: Vec<OllamaTool> = tools
            .into_iter()
            .map(|t| OllamaTool {
                tool_type: "function".to_string(),
                function: OllamaToolFunction {
                    name: t.name,
                    description: t.description,
                    parameters: serde_json::to_value(t.input_schema).unwrap_or_default(),
                },
            })
            .collect();

        let request = OllamaChatRequest {
            model: self.model.clone(),
            messages: api_messages,
            stream: false,
            tools: if ollama_tools.is_empty() {
                None
            } else {
                Some(ollama_tools)
            },
        };

        log::debug!(
            "Sending tool request to Ollama API: {}",
            serde_json::to_string_pretty(&request).unwrap_or_default()
        );

        // Retry configuration for transient errors
        const MAX_RETRIES: u32 = 3;
        const BASE_DELAY_MS: u64 = 2000;

        let mut last_error: Option<String> = None;
        let mut response_data_opt: Option<OllamaChatResponse> = None;

        for attempt in 0..=MAX_RETRIES {
            if attempt > 0 {
                let delay_ms = BASE_DELAY_MS * (1 << (attempt - 1));
                let wait_secs = delay_ms / 1000;
                log::warn!(
                    "[OLLAMA] Tool request retry attempt {}/{} after {}ms delay",
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
                .timeout(Duration::from_secs(300))
                .json(&request)
                .send()
                .await;

            let response = match request_result {
                Ok(r) => r,
                Err(e) => {
                    last_error = Some(format!("Ollama API request failed: {}", e));
                    if attempt < MAX_RETRIES {
                        log::warn!("[OLLAMA] Tool request failed (attempt {}): {}, will retry", attempt + 1, e);
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
                        "[OLLAMA] Tool request received retryable status {} (attempt {}), will retry",
                        status,
                        attempt + 1
                    );
                    last_error = Some(format!("HTTP {}: {}", status, error_text));
                    continue;
                }

                if let Ok(error_response) = serde_json::from_str::<OllamaErrorResponse>(&error_text) {
                    return Err(format!("Ollama API error: {}", error_response.error));
                }

                return Err(format!(
                    "Ollama API returned error status: {}, body: {}",
                    status, error_text
                ));
            }

            response_data_opt = Some(response
                .json()
                .await
                .map_err(|e| format!("Failed to parse Ollama response: {}", e))?);
            break;
        }

        let response_data = response_data_opt.ok_or_else(|| {
            last_error.unwrap_or_else(|| "Max retries exceeded".to_string())
        })?;

        // Parse tool calls from response
        let mut tool_calls = Vec::new();
        if let Some(calls) = response_data.message.tool_calls {
            for (idx, call) in calls.into_iter().enumerate() {
                tool_calls.push(ToolCall {
                    id: call.id.unwrap_or_else(|| format!("call_{}", idx)),
                    name: call.function.name,
                    arguments: call.function.arguments,
                });
            }
        }

        // Determine stop reason
        let stop_reason = if !tool_calls.is_empty() {
            Some("tool_use".to_string())
        } else {
            response_data.done_reason
        };

        Ok(AiResponse {
            content: response_data.message.content,
            tool_calls,
            stop_reason,
            x402_payment: None, // Llama doesn't use x402 directly (handled by OpenAI-compatible wrapper)
        })
    }

    /// Build tool result messages for continuing conversation after tool execution
    pub fn build_tool_result_messages(
        tool_calls: &[ToolCall],
        tool_responses: &[crate::ai::ToolResponse],
    ) -> Vec<OllamaMessage> {
        let mut messages = Vec::new();

        // Assistant message with tool calls
        let ollama_tool_calls: Vec<OllamaToolCall> = tool_calls
            .iter()
            .map(|tc| OllamaToolCall {
                id: Some(tc.id.clone()),
                function: OllamaFunctionCall {
                    name: tc.name.clone(),
                    arguments: tc.arguments.clone(),
                },
            })
            .collect();

        messages.push(OllamaMessage {
            role: "assistant".to_string(),
            content: String::new(),
            tool_calls: Some(ollama_tool_calls),
        });

        // Tool response messages
        for response in tool_responses {
            messages.push(OllamaMessage {
                role: "tool".to_string(),
                content: response.content.clone(),
                tool_calls: None,
            });
        }

        messages
    }
}

/// Re-export for use in AiClient
pub use OllamaMessage as LlamaMessage;
