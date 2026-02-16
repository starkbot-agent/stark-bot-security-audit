//! Llama Archetype - Text-based JSON tool calling
//!
//! This archetype is used for models that don't support native tool calling
//! through the API (generic Llama endpoints, local models, etc.).
//!
//! The AI responds with JSON containing:
//! - `body`: The text response to the user
//! - `tool_call`: Optional tool invocation with name and parameters

use super::{AgentResponse, ArchetypeId, ModelArchetype, TextToolCall};
use crate::tools::ToolDefinition;
use regex::Regex;
use serde_json::Value;

/// Llama archetype for text-based JSON tool calling
pub struct LlamaArchetype {
    // Pre-compiled regex for extracting JSON from markdown
    json_block_pattern: Regex,
}

impl LlamaArchetype {
    pub fn new() -> Self {
        Self {
            json_block_pattern: Regex::new(r"```(?:json)?\s*\n?([\s\S]*?)\n?```").unwrap(),
        }
    }

    /// Try to extract JSON from various formats in the response
    fn extract_json(&self, content: &str) -> Option<AgentResponse> {
        let content = content.trim();

        // Try direct JSON parse first
        if let Ok(response) = serde_json::from_str::<AgentResponse>(content) {
            return Some(response);
        }

        // Try to parse as typed JSON response
        // {"type": "message", "content": "..."} or {"type": "function", ...}
        if let Ok(json) = serde_json::from_str::<Value>(content) {
            if let Some(result) = self.try_parse_typed_json(&json) {
                return Some(result);
            }
        }

        // Try emoji-prefixed tool call format:
        // 🔧 Tool Call: tool_name
        // json { ... }
        if let Some(result) = self.try_parse_emoji_tool_call(content) {
            return Some(result);
        }

        // Try to extract JSON from markdown code blocks
        if let Some(captures) = self.json_block_pattern.captures(content) {
            if let Some(json_match) = captures.get(1) {
                let json_str = json_match.as_str().trim();
                if let Ok(response) = serde_json::from_str::<AgentResponse>(json_str) {
                    return Some(response);
                }
                // Also try typed JSON format in code blocks
                if let Ok(json) = serde_json::from_str::<Value>(json_str) {
                    if let Some(result) = self.try_parse_typed_json(&json) {
                        return Some(result);
                    }
                }
            }
        }

        // Try to find JSON object anywhere in the content
        if let Some(start) = content.find('{') {
            if let Some(extracted) = self.extract_balanced_json(content, start) {
                if let Ok(response) = serde_json::from_str::<AgentResponse>(&extracted) {
                    return Some(response);
                }
                // Also try typed JSON format
                if let Ok(json) = serde_json::from_str::<Value>(&extracted) {
                    if let Some(result) = self.try_parse_typed_json(&json) {
                        return Some(result);
                    }
                }
            }
        }

        None
    }

    /// Try to parse emoji-prefixed tool call format:
    /// 🔧 Tool Call: tool_name
    /// json { ... }
    /// or
    /// 🔧 Tool Call: tool_name
    /// { ... }
    fn try_parse_emoji_tool_call(&self, content: &str) -> Option<AgentResponse> {
        // Look for the emoji tool call pattern
        let tool_call_marker = "🔧 Tool Call:";
        let alt_marker = "🔧 **Tool Call:**";

        let (marker_pos, marker_len) = if let Some(pos) = content.find(tool_call_marker) {
            (pos, tool_call_marker.len())
        } else if let Some(pos) = content.find(alt_marker) {
            (pos, alt_marker.len())
        } else {
            return None;
        };

        // Extract tool name (rest of line after marker)
        let after_marker = &content[marker_pos + marker_len..];
        let tool_name_end = after_marker.find('\n').unwrap_or(after_marker.len());
        let tool_name = after_marker[..tool_name_end].trim().trim_matches('`').to_string();

        if tool_name.is_empty() {
            return None;
        }

        log::info!("[PARSE] Found emoji tool call format for tool: {}", tool_name);

        // Find the JSON parameters - could be after "json" keyword or just raw JSON
        let params_section = &after_marker[tool_name_end..];

        // Try to find JSON object
        let json_str = if let Some(json_start) = params_section.find('{') {
            // Extract balanced JSON
            if let Some(extracted) = self.extract_balanced_json(params_section, json_start) {
                extracted
            } else {
                return None;
            }
        } else {
            return None;
        };

        // Parse the JSON parameters
        match serde_json::from_str::<Value>(&json_str) {
            Ok(params) => {
                log::info!("[PARSE] Successfully parsed emoji tool call: {} with params", tool_name);
                Some(AgentResponse {
                    body: format!("Executing {}...", tool_name),
                    tool_call: Some(TextToolCall {
                        tool_name,
                        tool_params: params,
                    }),
                })
            }
            Err(e) => {
                log::warn!("[PARSE] Failed to parse JSON params for emoji tool call: {}", e);
                None
            }
        }
    }

    /// Try to parse typed JSON format ({"type": "message"/"function", ...})
    fn try_parse_typed_json(&self, json: &Value) -> Option<AgentResponse> {
        let msg_type = json.get("type").and_then(|v| v.as_str())?;

        match msg_type {
            "message" => {
                // Handle message type - just extract content
                let content_str = json.get("content").and_then(|v| v.as_str())?;
                log::info!("[PARSE] Extracted message content from type:message format");
                Some(AgentResponse {
                    body: content_str.to_string(),
                    tool_call: None,
                })
            }
            "function" => {
                // Handle function call type
                let name = json.get("name").and_then(|v| v.as_str())?;
                let params = json.get("parameters")?;
                log::info!("[PARSE] Converted native function call format: {}", name);
                Some(AgentResponse {
                    body: format!("Executing {}...", name),
                    tool_call: Some(TextToolCall {
                        tool_name: name.to_string(),
                        tool_params: params.clone(),
                    }),
                })
            }
            _ => None,
        }
    }

    /// Extract a balanced JSON object from content starting at given position
    fn extract_balanced_json(&self, content: &str, start: usize) -> Option<String> {
        let mut depth = 0;
        let mut end = start;

        for (i, c) in content[start..].char_indices() {
            match c {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }

        if end > start && depth == 0 {
            Some(content[start..end].to_string())
        } else {
            None
        }
    }
}

impl Default for LlamaArchetype {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelArchetype for LlamaArchetype {
    fn id(&self) -> ArchetypeId {
        ArchetypeId::Llama
    }

    fn uses_native_tool_calling(&self) -> bool {
        false
    }

    fn default_model(&self) -> &'static str {
        "llama3.3" // Default Llama model
    }

    fn enhance_system_prompt(&self, base_prompt: &str, tools: &[ToolDefinition]) -> String {
        let mut prompt = base_prompt.to_string();

        // Add JSON response format instruction
        prompt.push_str("\n\n## RESPONSE FORMAT (CRITICAL)\n\n");
        prompt.push_str("You MUST respond in this JSON format:\n");
        prompt.push_str("```\n");
        prompt.push_str("{\"body\": \"your message\", \"tool_call\": null}\n");
        prompt.push_str("```\n\n");
        prompt.push_str("To call a tool:\n");
        prompt.push_str("```\n");
        prompt.push_str("{\"body\": \"brief status\", \"tool_call\": {\"tool_name\": \"name\", \"tool_params\": {...}}}\n");
        prompt.push_str("```\n\n");

        // Build tools array in OpenAI schema format
        if !tools.is_empty() {
            prompt.push_str("## AVAILABLE TOOLS\n\n");
            prompt.push_str("```json\n");
            prompt.push_str("[\n");

            let tool_entries: Vec<String> = tools
                .iter()
                .map(|tool| {
                    let tool_json = serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": tool.name,
                            "description": tool.description,
                            "parameters": tool.input_schema
                        }
                    });
                    serde_json::to_string_pretty(&tool_json).unwrap_or_default()
                })
                .collect();

            prompt.push_str(&tool_entries.join(",\n"));
            prompt.push_str("\n]\n```\n\n");

            // Add usage examples
            prompt.push_str("## EXAMPLES\n\n");
            prompt.push_str("Weather query:\n");
            prompt.push_str("```\n{\"body\": \"Checking...\", \"tool_call\": {\"tool_name\": \"exec\", \"tool_params\": {\"command\": \"curl -s 'wttr.in/Ohio?format=3'\"}}}\n```\n\n");

            prompt.push_str("Fetch web page:\n");
            prompt.push_str("```\n{\"body\": \"Fetching...\", \"tool_call\": {\"tool_name\": \"web_fetch\", \"tool_params\": {\"url\": \"https://example.com\"}}}\n```\n\n");

            prompt.push_str("**IMPORTANT**: For weather, news, or live data - USE TOOLS IMMEDIATELY. Do not say you cannot access real-time data.\n\n");

            prompt.push_str("## CRITICAL: NEVER HALLUCINATE TOOL RESULTS\n\n");
            prompt.push_str("- WAIT for actual tool results before reporting them to the user\n");
            prompt.push_str("- Report EXACTLY what tools return - never invent tx hashes, addresses, numbers, or data\n");
            prompt.push_str("- If a tool fails, quote the actual error message verbatim\n");
            prompt.push_str("- If a tool succeeds, report the actual output - do not embellish or guess\n");
            prompt.push_str("- WRONG: Making up a hash like '0x7f4f...5b5b' before getting the real result\n");
            prompt.push_str("- RIGHT: Waiting for tool result and reporting the actual hash returned\n\n");
        }

        prompt
    }

    fn parse_response(&self, content: &str) -> Option<AgentResponse> {
        // Try to extract structured JSON response
        if let Some(response) = self.extract_json(content) {
            return Some(response);
        }

        // If all parsing fails, treat the whole content as body with no tool call
        log::debug!(
            "[PARSE] Could not extract JSON, treating as plain text response"
        );
        Some(AgentResponse {
            body: content.trim().to_string(),
            tool_call: None,
        })
    }

    fn format_tool_followup(&self, tool_name: &str, tool_result: &str, success: bool) -> String {
        if success {
            format!(
                "Tool '{}' returned:\n{}\n\nNow provide your final response to the user based on this result. Remember to respond in JSON format.",
                tool_name, tool_result
            )
        } else {
            // Check if this is a git permission error (can be solved by forking)
            let error_lower = tool_result.to_lowercase();
            let is_git_permission = (error_lower.contains("permission")
                || error_lower.contains("403")
                || error_lower.contains("denied"))
                && (error_lower.contains("git")
                    || error_lower.contains("github")
                    || error_lower.contains("push"));

            if is_git_permission {
                format!(
                    "Tool '{}' FAILED with error:\n{}\n\nYou don't have push access to this repository. To contribute to repos you don't own, use the FORK workflow:\n1. Fork the repo: `gh repo fork OWNER/REPO --clone`\n2. Make changes in the forked repo\n3. Push to YOUR fork\n4. Create PR: `gh pr create --repo OWNER/REPO`\n\nTry the fork workflow. Remember to respond in JSON format.",
                    tool_name, tool_result
                )
            } else {
                format!(
                    "Tool '{}' FAILED with error:\n{}\n\nTry a different approach if possible. Common fixes:\n- If directory exists: cd into it instead of cloning\n- If command not found: try alternative command\n- If permission denied: check if you need to fork the repo first\n\nIf truly impossible, explain why. Remember to respond in JSON format.",
                    tool_name, tool_result
                )
            }
        }
    }
}
