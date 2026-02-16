//! Claude Archetype - Native Anthropic API tool calling
//!
//! This archetype is used for models that use the Anthropic API directly.
//! Tools are passed via the API's `tools` parameter with x-api-key authentication.

use super::{AgentResponse, ArchetypeId, ModelArchetype};
use crate::tools::ToolDefinition;

/// Claude archetype for native Anthropic API tool calling
pub struct ClaudeArchetype;

impl ClaudeArchetype {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ClaudeArchetype {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelArchetype for ClaudeArchetype {
    fn id(&self) -> ArchetypeId {
        ArchetypeId::Claude
    }

    fn uses_native_tool_calling(&self) -> bool {
        true
    }

    fn default_model(&self) -> &'static str {
        "claude-sonnet-4-20250514"
    }

    fn enhance_system_prompt(&self, base_prompt: &str, _tools: &[ToolDefinition]) -> String {
        // Don't list tools in the system prompt - they're passed via the API's `tools` parameter
        base_prompt.to_string()
    }

    fn parse_response(&self, content: &str) -> Option<AgentResponse> {
        // Native tool calling uses the API's tool_use blocks, not text parsing
        Some(AgentResponse {
            body: content.to_string(),
            tool_call: None,
        })
    }

    fn format_tool_followup(&self, _tool_name: &str, _tool_result: &str, _success: bool) -> String {
        // Native tool calling uses the API's message format for tool results
        String::new()
    }
}
