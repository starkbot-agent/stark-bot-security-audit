//! OpenAI Archetype - Native tool calling for OpenAI-compatible endpoints
//!
//! Used for GPT models behind defirelay proxies or direct OpenAI API.
//! Tools are passed via the API's `tools` parameter.

use super::{AgentResponse, ArchetypeId, ModelArchetype};
use crate::tools::ToolDefinition;

pub struct OpenAIArchetype;

impl OpenAIArchetype {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OpenAIArchetype {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelArchetype for OpenAIArchetype {
    fn id(&self) -> ArchetypeId {
        ArchetypeId::OpenAI
    }

    fn uses_native_tool_calling(&self) -> bool {
        true
    }

    fn default_model(&self) -> &'static str {
        // Relay endpoints handle model selection server-side.
        // For direct OpenAI API, the client constructor falls back to "gpt-4o".
        ""
    }

    fn enhance_system_prompt(&self, base_prompt: &str, _tools: &[ToolDefinition]) -> String {
        base_prompt.to_string()
    }

    fn parse_response(&self, content: &str) -> Option<AgentResponse> {
        Some(AgentResponse {
            body: content.to_string(),
            tool_call: None,
        })
    }

    fn format_tool_followup(&self, _tool_name: &str, _tool_result: &str, _success: bool) -> String {
        String::new()
    }
}
