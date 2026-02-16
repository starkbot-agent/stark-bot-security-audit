use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;
use crate::x402::X402PaymentInfo;

/// AI API error with status code information
#[derive(Debug, Clone)]
pub struct AiError {
    /// Error message
    pub message: String,
    /// HTTP status code if available
    pub status_code: Option<u16>,
}

impl AiError {
    pub fn new(message: impl Into<String>) -> Self {
        AiError {
            message: message.into(),
            status_code: None,
        }
    }

    pub fn with_status(message: impl Into<String>, status_code: u16) -> Self {
        AiError {
            message: message.into(),
            status_code: Some(status_code),
        }
    }

    /// Check if this is a client error (4xx status code)
    /// These errors indicate something wrong with the request that the AI might be able to fix
    pub fn is_client_error(&self) -> bool {
        self.status_code.map(|c| c >= 400 && c < 500).unwrap_or(false)
    }

    /// Check if this is a server error (5xx status code)
    pub fn is_server_error(&self) -> bool {
        self.status_code.map(|c| c >= 500).unwrap_or(false)
    }

    /// Check if this error indicates the context/input is too large
    pub fn is_context_too_large(&self) -> bool {
        let msg = self.message.to_lowercase();
        msg.contains("too large")
            || msg.contains("exceeds maximum")
            || msg.contains("input tokens")
            || msg.contains("context length")
    }
}

impl fmt::Display for AiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(code) = self.status_code {
            write!(f, "[HTTP {}] {}", code, self.message)
        } else {
            write!(f, "{}", self.message)
        }
    }
}

impl std::error::Error for AiError {}

impl From<String> for AiError {
    fn from(s: String) -> Self {
        AiError::new(s)
    }
}

impl From<&str> for AiError {
    fn from(s: &str) -> Self {
        AiError::new(s)
    }
}

/// Thinking level for Claude extended thinking
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThinkingLevel {
    /// Disable extended thinking
    Off,
    /// Minimal thinking budget (~1K tokens)
    Minimal,
    /// Low thinking budget (~4K tokens) - default for reasoning models
    #[default]
    Low,
    /// Medium thinking budget (~10K tokens)
    Medium,
    /// High thinking budget (~32K tokens)
    High,
    /// Extra high thinking budget (~64K tokens)
    XHigh,
}

impl ThinkingLevel {
    /// Parse thinking level from string (used for /think: directives)
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().trim() {
            "off" | "none" | "disable" | "disabled" => Some(ThinkingLevel::Off),
            "minimal" | "think" | "min" => Some(ThinkingLevel::Minimal),
            "low" | "basic" => Some(ThinkingLevel::Low),
            "medium" | "med" | "harder" => Some(ThinkingLevel::Medium),
            "high" | "max" | "ultrathink" => Some(ThinkingLevel::High),
            "xhigh" | "ultra" | "ultrathink+" | "maximum" => Some(ThinkingLevel::XHigh),
            _ => None,
        }
    }

    /// Get the budget token count for this thinking level
    pub fn budget_tokens(&self) -> Option<u32> {
        match self {
            ThinkingLevel::Off => None,
            ThinkingLevel::Minimal => Some(1024),
            ThinkingLevel::Low => Some(4096),
            ThinkingLevel::Medium => Some(10240),
            ThinkingLevel::High => Some(32768),
            ThinkingLevel::XHigh => Some(65536),
        }
    }

    /// Check if thinking is enabled
    pub fn is_enabled(&self) -> bool {
        !matches!(self, ThinkingLevel::Off)
    }

    /// Get human-readable name
    pub fn as_str(&self) -> &'static str {
        match self {
            ThinkingLevel::Off => "off",
            ThinkingLevel::Minimal => "minimal",
            ThinkingLevel::Low => "low",
            ThinkingLevel::Medium => "medium",
            ThinkingLevel::High => "high",
            ThinkingLevel::XHigh => "xhigh",
        }
    }
}

impl std::fmt::Display for ThinkingLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Represents a tool call made by the AI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    /// Unique identifier for this tool call
    pub id: String,
    /// Name of the tool to call
    pub name: String,
    /// Arguments to pass to the tool as JSON
    pub arguments: Value,
}

/// Represents the result of a tool execution to send back to the AI
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResponse {
    /// ID of the tool call this responds to
    pub tool_call_id: String,
    /// Content of the tool response
    pub content: String,
    /// Whether the tool execution resulted in an error
    pub is_error: bool,
}

impl ToolResponse {
    pub fn success(tool_call_id: String, content: String) -> Self {
        ToolResponse {
            tool_call_id,
            content,
            is_error: false,
        }
    }

    pub fn error(tool_call_id: String, error: String) -> Self {
        ToolResponse {
            tool_call_id,
            content: error,
            is_error: true,
        }
    }
}

/// Provider-agnostic tool history entry
/// Stores a round of tool calls and their responses for continuing conversations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolHistoryEntry {
    /// The tool calls made by the AI
    pub tool_calls: Vec<ToolCall>,
    /// The responses from executing those tool calls
    pub tool_responses: Vec<ToolResponse>,
}

impl ToolHistoryEntry {
    pub fn new(tool_calls: Vec<ToolCall>, tool_responses: Vec<ToolResponse>) -> Self {
        ToolHistoryEntry {
            tool_calls,
            tool_responses,
        }
    }
}

/// Handle context overflow by clearing tool history and creating a recovery entry.
/// Returns a ToolHistoryEntry with a summary of cleared work and recovery guidance.
pub fn handle_context_overflow(
    tool_history: &mut Vec<ToolHistoryEntry>,
    iteration_id: &str,
) -> ToolHistoryEntry {
    // Build a summary of what was accomplished before clearing
    let work_summary: Vec<String> = tool_history
        .iter()
        .flat_map(|entry| entry.tool_calls.iter())
        .map(|tc| format!("- {} ({})", tc.name, tc.id))
        .collect();

    let cleared_count = work_summary.len();

    // Clear the tool history to reduce context
    tool_history.clear();

    // Create recovery guidance
    let recovery_guidance = format!(
        "CONTEXT OVERFLOW: The previous context exceeded the model's token limit.\n\n\
         Work completed before reset:\n{}\n\n\
         The tool history has been cleared to allow you to continue.\n\
         Please proceed with a more focused approach - read smaller portions of files and be more selective.",
        if work_summary.is_empty() {
            "None".to_string()
        } else {
            work_summary.join("\n")
        }
    );

    ToolHistoryEntry::new(
        vec![ToolCall {
            id: format!("context_reset_{}", iteration_id),
            name: "system_feedback".to_string(),
            arguments: serde_json::json!({"type": "context_overflow", "cleared_entries": cleared_count}),
        }],
        vec![ToolResponse {
            tool_call_id: format!("context_reset_{}", iteration_id),
            content: recovery_guidance,
            is_error: true,
        }],
    )
}

/// Create a simple error feedback entry for non-context-overflow errors
pub fn create_error_feedback(
    error: &AiError,
    iteration_id: &str,
) -> ToolHistoryEntry {
    let error_guidance = format!(
        "ERROR: The AI API returned an error: \"{}\"\n\n\
         Please adjust your approach and try again.",
        error
    );

    ToolHistoryEntry::new(
        vec![ToolCall {
            id: format!("api_error_{}", iteration_id),
            name: "system_feedback".to_string(),
            arguments: serde_json::json!({"type": "api_error", "status_code": error.status_code}),
        }],
        vec![ToolResponse {
            tool_call_id: format!("api_error_{}", iteration_id),
            content: error_guidance,
            is_error: true,
        }],
    )
}

/// Unified AI response that can contain both text and tool calls
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiResponse {
    /// Text content of the response (may be empty if only tool calls)
    pub content: String,
    /// Tool calls requested by the AI
    pub tool_calls: Vec<ToolCall>,
    /// The reason the AI stopped generating
    pub stop_reason: Option<String>,
    /// x402 payment info if a payment was made for this request
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x402_payment: Option<X402PaymentInfo>,
}

impl AiResponse {
    pub fn text(content: String) -> Self {
        AiResponse {
            content,
            tool_calls: vec![],
            stop_reason: Some("end_turn".to_string()),
            x402_payment: None,
        }
    }

    pub fn with_tools(content: String, tool_calls: Vec<ToolCall>) -> Self {
        AiResponse {
            content,
            tool_calls,
            stop_reason: Some("tool_use".to_string()),
            x402_payment: None,
        }
    }

    /// Add x402 payment info to the response
    pub fn with_x402_payment(mut self, payment: Option<X402PaymentInfo>) -> Self {
        self.x402_payment = payment;
        self
    }

    /// Check if the response contains tool calls
    pub fn has_tool_calls(&self) -> bool {
        !self.tool_calls.is_empty()
    }

    /// Check if the AI wants to use tools
    pub fn is_tool_use(&self) -> bool {
        self.stop_reason.as_deref() == Some("tool_use") || !self.tool_calls.is_empty()
    }
}

/// Tool definition in Claude API format
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeTool {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Content block types in Claude API responses
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ClaudeContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

impl ClaudeContentBlock {
    pub fn text(text: impl Into<String>) -> Self {
        ClaudeContentBlock::Text { text: text.into() }
    }

    pub fn tool_result(tool_use_id: String, content: String, is_error: bool) -> Self {
        ClaudeContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error: if is_error { Some(true) } else { None },
        }
    }
}

/// Message with tool content for Claude API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeMessage {
    pub role: String,
    pub content: ClaudeMessageContent,
}

/// Content can be either a string or array of content blocks
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ClaudeMessageContent {
    Text(String),
    Blocks(Vec<ClaudeContentBlock>),
}

impl ClaudeMessage {
    pub fn user(content: impl Into<String>) -> Self {
        ClaudeMessage {
            role: "user".to_string(),
            content: ClaudeMessageContent::Text(content.into()),
        }
    }

    pub fn assistant(content: impl Into<String>) -> Self {
        ClaudeMessage {
            role: "assistant".to_string(),
            content: ClaudeMessageContent::Text(content.into()),
        }
    }

    pub fn assistant_with_blocks(blocks: Vec<ClaudeContentBlock>) -> Self {
        ClaudeMessage {
            role: "assistant".to_string(),
            content: ClaudeMessageContent::Blocks(blocks),
        }
    }

    pub fn user_with_tool_results(results: Vec<ClaudeContentBlock>) -> Self {
        ClaudeMessage {
            role: "user".to_string(),
            content: ClaudeMessageContent::Blocks(results),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_response_text() {
        let response = AiResponse::text("Hello world".to_string());
        assert_eq!(response.content, "Hello world");
        assert!(response.tool_calls.is_empty());
        assert!(!response.is_tool_use());
    }

    #[test]
    fn test_ai_response_with_tools() {
        let tool_call = ToolCall {
            id: "call_123".to_string(),
            name: "web_fetch".to_string(),
            arguments: serde_json::json!({"url": "https://example.com"}),
        };
        let response = AiResponse::with_tools("Fetching...".to_string(), vec![tool_call]);

        assert!(response.has_tool_calls());
        assert!(response.is_tool_use());
        assert_eq!(response.tool_calls.len(), 1);
    }

    #[test]
    fn test_tool_response() {
        let success = ToolResponse::success("call_123".to_string(), "Result".to_string());
        assert!(!success.is_error);

        let error = ToolResponse::error("call_456".to_string(), "Failed".to_string());
        assert!(error.is_error);
    }
}
