//! MiniMax Archetype - Native tool calling with think-block stripping
//!
//! MiniMax M2.5 uses OpenAI-compatible tool calling but wraps chain-of-thought
//! reasoning in <think>...</think> tags. This archetype strips those before
//! returning the response content.

use super::{AgentResponse, ArchetypeId, ModelArchetype};
use crate::tools::ToolDefinition;

pub struct MiniMaxArchetype;

impl MiniMaxArchetype {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MiniMaxArchetype {
    fn default() -> Self {
        Self::new()
    }
}

/// Strip <think>...</think> blocks from content
fn strip_think_blocks(content: &str) -> String {
    let mut result = String::with_capacity(content.len());
    let mut remaining = content;

    while let Some(start) = remaining.find("<think>") {
        result.push_str(&remaining[..start]);
        if let Some(end) = remaining[start..].find("</think>") {
            remaining = &remaining[start + end + "</think>".len()..];
        } else {
            // Unclosed <think> tag — strip everything after it
            remaining = "";
            break;
        }
    }
    result.push_str(remaining);
    result.trim().to_string()
}

impl ModelArchetype for MiniMaxArchetype {
    fn id(&self) -> ArchetypeId {
        ArchetypeId::MiniMax
    }

    fn uses_native_tool_calling(&self) -> bool {
        true
    }

    fn default_model(&self) -> &'static str {
        "MiniMax-M2.5"
    }

    fn enhance_system_prompt(&self, base_prompt: &str, _tools: &[ToolDefinition]) -> String {
        base_prompt.to_string()
    }

    fn clean_content(&self, content: &str) -> String {
        strip_think_blocks(content)
    }

    fn requires_single_system_message(&self) -> bool {
        true
    }

    fn parse_response(&self, content: &str) -> Option<AgentResponse> {
        let cleaned = strip_think_blocks(content);
        Some(AgentResponse {
            body: if cleaned.is_empty() { content.to_string() } else { cleaned },
            tool_call: None,
        })
    }

    fn format_tool_followup(&self, _tool_name: &str, _tool_result: &str, _success: bool) -> String {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_think_blocks() {
        assert_eq!(
            strip_think_blocks("<think>\nThe user said hi.\n</think>\n\nHey! What's up?"),
            "Hey! What's up?"
        );
    }

    #[test]
    fn test_no_think_blocks() {
        assert_eq!(strip_think_blocks("Hello world"), "Hello world");
    }

    #[test]
    fn test_empty_think_block() {
        assert_eq!(strip_think_blocks("<think></think>Hello"), "Hello");
    }
}
