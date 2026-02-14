//! Kimi Archetype - Native OpenAI-compatible tool calling
//!
//! This archetype is used for models that support native tool calling
//! through the OpenAI-compatible API (Kimi/Moonshot, OpenAI, Azure, etc.).
//!
//! Tools are passed via the API's `tools` parameter, and responses
//! contain `tool_calls` in the message structure.

use super::{AgentResponse, ArchetypeId, ModelArchetype};
use crate::tools::ToolDefinition; // Required by ModelArchetype trait

/// Kimi archetype for native OpenAI-compatible tool calling
pub struct KimiArchetype;

impl KimiArchetype {
    pub fn new() -> Self {
        Self
    }
}

impl Default for KimiArchetype {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelArchetype for KimiArchetype {
    fn id(&self) -> ArchetypeId {
        ArchetypeId::Kimi
    }

    fn uses_native_tool_calling(&self) -> bool {
        true
    }

    fn default_model(&self) -> &'static str {
        "kimi-k2-turbo-preview" // Kimi K2 turbo preview - supports native tool calling per docs
    }

    fn enhance_system_prompt(&self, base_prompt: &str, _tools: &[ToolDefinition]) -> String {
        // Don't list tools in the system prompt - they're passed via the API's `tools` parameter.
        // Listing them as text confuses some models into outputting tool calls as formatted text
        // instead of using the native tool_calls mechanism.
        base_prompt.to_string()
    }

    fn requires_single_system_message(&self) -> bool {
        true
    }

    fn parse_response(&self, content: &str) -> Option<AgentResponse> {
        // Native tool calling uses the API's tool_calls field, not text parsing
        // This is only called if there's text content without tool calls
        Some(AgentResponse {
            body: content.to_string(),
            tool_call: None,
        })
    }

    fn format_tool_followup(&self, _tool_name: &str, _tool_result: &str, _success: bool) -> String {
        // Native tool calling uses the API's message format for tool results
        // This shouldn't be called for native archetypes, but provide a fallback
        String::new()
    }
}
