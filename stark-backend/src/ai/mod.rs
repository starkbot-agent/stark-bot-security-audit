pub mod claude;
pub mod llama;
pub mod openai;
pub mod types;

pub use claude::ClaudeClient;
pub use llama::{LlamaClient, LlamaMessage};
pub use openai::OpenAIClient;
pub use types::{
    AiResponse, ClaudeMessage as TypedClaudeMessage, ToolCall, ToolHistoryEntry, ToolResponse,
};

use crate::models::{AgentSettings, AiProvider};
use crate::tools::ToolDefinition;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
}

impl ToString for MessageRole {
    fn to_string(&self) -> String {
        match self {
            MessageRole::System => "system".to_string(),
            MessageRole::User => "user".to_string(),
            MessageRole::Assistant => "assistant".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
}

/// Unified AI client that works with any configured provider
pub enum AiClient {
    Claude(ClaudeClient),
    OpenAI(OpenAIClient),
    Llama(LlamaClient),
}

impl AiClient {
    /// Create an AI client from agent settings
    pub fn from_settings(settings: &AgentSettings) -> Result<Self, String> {
        Self::from_settings_with_wallet(settings, None)
    }

    /// Create an AI client from agent settings with optional burner wallet for x402
    pub fn from_settings_with_wallet(
        settings: &AgentSettings,
        burner_private_key: Option<&str>,
    ) -> Result<Self, String> {
        let provider = settings.provider_enum().ok_or_else(|| {
            format!("Unknown provider: {}", settings.provider)
        })?;

        match provider {
            AiProvider::Claude => {
                let client = ClaudeClient::new(
                    &settings.api_key,
                    Some(&settings.endpoint),
                    Some(&settings.model),
                )?;
                Ok(AiClient::Claude(client))
            }
            AiProvider::OpenAI | AiProvider::OpenAICompatible => {
                // Both OpenAI and OpenAI-compatible use the same client
                // The endpoint from settings is always used
                let client = OpenAIClient::new_with_x402_and_tokens(
                    &settings.api_key,
                    Some(&settings.endpoint),
                    Some(&settings.model),
                    burner_private_key,
                    Some(settings.max_tokens as u32),
                )?;
                Ok(AiClient::OpenAI(client))
            }
            AiProvider::Llama => {
                // Llama endpoints may also use x402 (like llama.defirelay.com)
                // Use OpenAI-compatible client for x402 support
                let client = OpenAIClient::new_with_x402_and_tokens(
                    "",  // No API key needed for llama endpoints
                    Some(&settings.endpoint),
                    Some(&settings.model),
                    burner_private_key,
                    Some(settings.max_tokens as u32),
                )?;
                Ok(AiClient::OpenAI(client))
            }
        }
    }

    /// Generate text using the configured provider
    pub async fn generate_text(&self, messages: Vec<Message>) -> Result<String, String> {
        match self {
            AiClient::Claude(client) => client.generate_text(messages).await,
            AiClient::OpenAI(client) => client.generate_text(messages).await,
            AiClient::Llama(client) => client.generate_text(messages).await,
        }
    }

    /// Generate response with tool support (Claude, OpenAI, and Llama 3.1+)
    pub async fn generate_with_tools(
        &self,
        messages: Vec<Message>,
        tool_history: Vec<ToolHistoryEntry>,
        tools: Vec<ToolDefinition>,
    ) -> Result<AiResponse, String> {
        match self {
            AiClient::Claude(client) => {
                // Convert tool history to Claude format
                let tool_messages = Self::tool_history_to_claude(&tool_history);
                client
                    .generate_with_tools(messages, tool_messages, tools)
                    .await
            }
            AiClient::OpenAI(client) => {
                // Convert tool history to OpenAI format
                let tool_messages = Self::tool_history_to_openai(&tool_history);
                client
                    .generate_with_tools(messages, tool_messages, tools)
                    .await
            }
            AiClient::Llama(client) => {
                // Convert tool history to Llama/Ollama format
                let tool_messages = Self::tool_history_to_llama(&tool_history);
                client
                    .generate_with_tools(messages, tool_messages, tools)
                    .await
            }
        }
    }

    /// Check if the current provider supports tools
    pub fn supports_tools(&self) -> bool {
        // All providers now support tools
        matches!(self, AiClient::Claude(_) | AiClient::OpenAI(_) | AiClient::Llama(_))
    }

    /// Build a tool history entry from tool calls and responses
    pub fn build_tool_history_entry(
        tool_calls: Vec<ToolCall>,
        tool_responses: Vec<ToolResponse>,
    ) -> ToolHistoryEntry {
        ToolHistoryEntry::new(tool_calls, tool_responses)
    }

    /// Convert tool history to Claude format
    fn tool_history_to_claude(history: &[ToolHistoryEntry]) -> Vec<TypedClaudeMessage> {
        use types::{ClaudeContentBlock, ClaudeMessage, ClaudeMessageContent};

        let mut messages = Vec::new();
        for entry in history {
            // Build assistant message with tool_use blocks
            let tool_use_blocks: Vec<ClaudeContentBlock> = entry
                .tool_calls
                .iter()
                .map(|tc| ClaudeContentBlock::ToolUse {
                    id: tc.id.clone(),
                    name: tc.name.clone(),
                    input: tc.arguments.clone(),
                })
                .collect();

            messages.push(ClaudeMessage {
                role: "assistant".to_string(),
                content: ClaudeMessageContent::Blocks(tool_use_blocks),
            });

            // Build user message with tool_result blocks
            let result_blocks: Vec<ClaudeContentBlock> = entry
                .tool_responses
                .iter()
                .map(|tr| ClaudeContentBlock::tool_result(
                    tr.tool_call_id.clone(),
                    tr.content.clone(),
                    tr.is_error,
                ))
                .collect();

            messages.push(ClaudeMessage::user_with_tool_results(result_blocks));
        }
        messages
    }

    /// Convert tool history to OpenAI format
    fn tool_history_to_openai(
        history: &[ToolHistoryEntry],
    ) -> Vec<openai::OpenAIMessage> {
        let mut messages = Vec::new();
        for entry in history {
            let openai_messages =
                OpenAIClient::build_tool_result_messages(&entry.tool_calls, &entry.tool_responses);
            messages.extend(openai_messages);
        }
        messages
    }

    /// Convert tool history to Llama/Ollama format
    fn tool_history_to_llama(history: &[ToolHistoryEntry]) -> Vec<LlamaMessage> {
        let mut messages = Vec::new();
        for entry in history {
            let llama_messages =
                LlamaClient::build_tool_result_messages(&entry.tool_calls, &entry.tool_responses);
            messages.extend(llama_messages);
        }
        messages
    }
}
