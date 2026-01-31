pub mod archetypes;
pub mod claude;
pub mod llama;
pub mod multi_agent;
pub mod openai;
pub mod streaming;
pub mod types;

pub use claude::ClaudeClient;
pub use llama::{LlamaClient, LlamaMessage};
pub use openai::OpenAIClient;
pub use archetypes::{ArchetypeId, ArchetypeRegistry, ModelArchetype};
pub use types::{
    AiResponse, ClaudeMessage as TypedClaudeMessage, ThinkingLevel, ToolCall, ToolHistoryEntry,
    ToolResponse,
};

use crate::gateway::events::EventBroadcaster;
use crate::gateway::protocol::GatewayEvent;
use crate::models::AgentSettings;
use crate::tools::ToolDefinition;
use crate::x402::X402PaymentInfo;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

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
    ///
    /// Uses ClaudeClient for Claude archetype (requires x-api-key auth),
    /// OpenAI-compatible client for all other archetypes.
    pub fn from_settings_with_wallet(
        settings: &AgentSettings,
        burner_private_key: Option<&str>,
    ) -> Result<Self, String> {
        use crate::x402::is_x402_endpoint;

        // Get archetype to determine client type and default model
        let archetype_id = Self::infer_archetype(settings);
        let registry = ArchetypeRegistry::new();
        let archetype = registry.get(archetype_id).unwrap_or_else(|| registry.default_archetype());
        let model = archetype.default_model();

        // Determine API key: x402 endpoints don't need one, others use secret_key
        let api_key = if is_x402_endpoint(&settings.endpoint) {
            ""  // x402 endpoints use crypto signatures, no API key needed
        } else {
            settings.secret_key.as_deref().unwrap_or("")
        };

        // Use ClaudeClient for Claude archetype (native Anthropic API with x-api-key header)
        if archetype_id == ArchetypeId::Claude {
            let client = ClaudeClient::new(
                api_key,
                Some(&settings.endpoint),
                Some(model),
            )?;
            return Ok(AiClient::Claude(client));
        }

        // All other archetypes use OpenAI-compatible client
        let client = OpenAIClient::new_with_x402_and_tokens(
            api_key,
            Some(&settings.endpoint),
            Some(model),
            burner_private_key,
            Some(settings.max_tokens as u32),
        )?;
        Ok(AiClient::OpenAI(client))
    }

    /// Get the archetype ID from agent settings
    pub fn infer_archetype(settings: &AgentSettings) -> ArchetypeId {
        ArchetypeId::from_str(&settings.model_archetype).unwrap_or(ArchetypeId::Kimi)
    }

    /// Generate text using the configured provider
    pub async fn generate_text(&self, messages: Vec<Message>) -> Result<String, String> {
        match self {
            AiClient::Claude(client) => client.generate_text(messages).await,
            AiClient::OpenAI(client) => client.generate_text(messages).await,
            AiClient::Llama(client) => client.generate_text(messages).await,
        }
    }

    /// Generate text and emit x402 payment event if applicable
    /// Returns (content, optional payment info) so caller can persist the payment
    pub async fn generate_text_with_events(
        &self,
        messages: Vec<Message>,
        broadcaster: &Arc<EventBroadcaster>,
        channel_id: i64,
    ) -> Result<(String, Option<X402PaymentInfo>), String> {
        match self {
            AiClient::OpenAI(client) => {
                let (content, payment) = client.generate_text_with_payment_info(messages).await?;
                // Emit x402 payment event if payment was made
                if let Some(ref payment_info) = payment {
                    broadcaster.broadcast(GatewayEvent::x402_payment(
                        channel_id,
                        &payment_info.amount,
                        &payment_info.amount_formatted,
                        &payment_info.asset,
                        &payment_info.pay_to,
                        payment_info.resource.as_deref(),
                    ));
                }
                Ok((content, payment))
            }
            // Other providers don't support x402
            AiClient::Claude(client) => Ok((client.generate_text(messages).await?, None)),
            AiClient::Llama(client) => Ok((client.generate_text(messages).await?, None)),
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

    /// Check if the current provider supports extended thinking
    pub fn supports_thinking(&self) -> bool {
        matches!(self, AiClient::Claude(_))
    }

    /// Set the thinking level for Claude models
    pub fn set_thinking_level(&self, level: ThinkingLevel) {
        if let AiClient::Claude(client) = self {
            client.set_thinking_level(level);
        }
    }

    /// Set the broadcaster for emitting retry events to the frontend
    pub fn with_broadcaster(self, broadcaster: Arc<EventBroadcaster>, channel_id: i64) -> Self {
        match self {
            AiClient::Claude(client) => {
                AiClient::Claude(client.with_broadcaster(broadcaster, channel_id))
            }
            AiClient::OpenAI(client) => {
                AiClient::OpenAI(client.with_broadcaster(broadcaster, channel_id))
            }
            AiClient::Llama(client) => {
                AiClient::Llama(client.with_broadcaster(broadcaster, channel_id))
            }
        }
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
