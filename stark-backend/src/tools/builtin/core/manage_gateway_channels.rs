use crate::tools::registry::Tool;
use crate::tools::types::{
    PropertySchema, ToolContext, ToolDefinition, ToolGroup, ToolInputSchema, ToolResult,
};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Tool for managing gateway channels (add/edit/delete messaging channels)
pub struct ManageGatewayChannelsTool {
    definition: ToolDefinition,
}

impl ManageGatewayChannelsTool {
    pub fn new() -> Self {
        let mut properties = HashMap::new();

        properties.insert(
            "action".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Action to perform: 'list' (show all channels), 'get' (show one channel), 'create' (add new), 'update' (edit existing), 'delete' (remove channel)".to_string(),
                default: None,
                items: None,
                enum_values: Some(vec![
                    "list".to_string(),
                    "get".to_string(),
                    "create".to_string(),
                    "update".to_string(),
                    "delete".to_string(),
                ]),
            },
        );

        properties.insert(
            "channel_id".to_string(),
            PropertySchema {
                schema_type: "integer".to_string(),
                description: "Channel ID (required for get/update/delete). Use 'list' to find IDs.".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "channel_type".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Channel type (required for create)".to_string(),
                default: None,
                items: None,
                enum_values: Some(vec![
                    "telegram".to_string(),
                    "slack".to_string(),
                    "discord".to_string(),
                    "twitter".to_string(),
                    "external_channel".to_string(),
                ]),
            },
        );

        properties.insert(
            "name".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Channel display name (for create/update)".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "enabled".to_string(),
            PropertySchema {
                schema_type: "boolean".to_string(),
                description: "Whether the channel is enabled (for update)".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "bot_token".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Bot token / API key for the channel (for create/update)".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "app_token".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "App-level token (for Slack socket mode, optional for other types)".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        ManageGatewayChannelsTool {
            definition: ToolDefinition {
                name: "manage_gateway_channels".to_string(),
                description: "Manage messaging gateway channels: list, view, create, update, or delete channels (Telegram, Slack, Discord, Twitter, External).".to_string(),
                input_schema: ToolInputSchema {
                    schema_type: "object".to_string(),
                    properties,
                    required: vec!["action".to_string()],
                },
                group: ToolGroup::System,
                hidden: false,
            },
        }
    }
}

impl Default for ManageGatewayChannelsTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct ManageGatewayChannelsParams {
    action: String,
    channel_id: Option<i64>,
    channel_type: Option<String>,
    name: Option<String>,
    enabled: Option<bool>,
    bot_token: Option<String>,
    app_token: Option<String>,
}

fn format_channel(ch: &crate::models::Channel) -> String {
    format!(
        "Channel #{} [{}]\n  Name: {}\n  Enabled: {}\n  Safe mode: {}",
        ch.id,
        ch.channel_type,
        ch.name,
        if ch.enabled { "YES" } else { "NO" },
        if ch.safe_mode { "YES" } else { "NO" },
    )
}

fn channel_to_json(ch: &crate::models::Channel) -> Value {
    json!({
        "id": ch.id,
        "channel_type": ch.channel_type,
        "name": ch.name,
        "enabled": ch.enabled,
        "safe_mode": ch.safe_mode,
    })
}

const VALID_CHANNEL_TYPES: &[&str] = &[
    "telegram",
    "slack",
    "discord",
    "twitter",
    "external_channel",
];

#[async_trait]
impl Tool for ManageGatewayChannelsTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, params: Value, context: &ToolContext) -> ToolResult {
        let params: ManageGatewayChannelsParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid parameters: {}", e)),
        };

        let db = match &context.database {
            Some(db) => db,
            None => return ToolResult::error("Database not available"),
        };

        match params.action.as_str() {
            "list" => match db.list_channels() {
                Ok(channels) => {
                    if channels.is_empty() {
                        return ToolResult::success("No channels configured.");
                    }

                    let output: Vec<String> = channels.iter().map(|c| format_channel(c)).collect();
                    let channel_data: Vec<Value> =
                        channels.iter().map(|c| channel_to_json(c)).collect();

                    ToolResult::success(output.join("\n\n")).with_metadata(json!({
                        "count": channels.len(),
                        "channels": channel_data
                    }))
                }
                Err(e) => ToolResult::error(format!("Database error: {}", e)),
            },

            "get" => {
                let id = match params.channel_id {
                    Some(id) => id,
                    None => {
                        return ToolResult::error(
                            "'channel_id' is required for 'get' action. Use 'list' to find IDs.",
                        )
                    }
                };

                match db.get_channel(id) {
                    Ok(Some(ch)) => {
                        ToolResult::success(format_channel(&ch)).with_metadata(channel_to_json(&ch))
                    }
                    Ok(None) => ToolResult::error(format!("Channel #{} not found", id)),
                    Err(e) => ToolResult::error(format!("Database error: {}", e)),
                }
            }

            "create" => {
                let channel_type = match &params.channel_type {
                    Some(ct) => {
                        let ct_lower = ct.to_lowercase();
                        if !VALID_CHANNEL_TYPES.contains(&ct_lower.as_str()) {
                            return ToolResult::error(format!(
                                "Invalid channel type: '{}'. Valid types: {}",
                                ct,
                                VALID_CHANNEL_TYPES.join(", ")
                            ));
                        }
                        ct_lower
                    }
                    None => {
                        return ToolResult::error(
                            "'channel_type' is required for 'create' action.",
                        )
                    }
                };

                let name = params.name.as_deref().unwrap_or(&channel_type);
                let bot_token = params.bot_token.as_deref().unwrap_or("");

                // External channels default to safe_mode
                let safe_mode = channel_type == "external_channel";

                match db.create_channel_with_safe_mode(
                    &channel_type,
                    name,
                    bot_token,
                    params.app_token.as_deref(),
                    safe_mode,
                ) {
                    Ok(ch) => ToolResult::success(format!(
                        "Channel created:\n\n{}",
                        format_channel(&ch)
                    ))
                    .with_metadata(channel_to_json(&ch)),
                    Err(e) => ToolResult::error(format!("Failed to create channel: {}", e)),
                }
            }

            "update" => {
                let id = match params.channel_id {
                    Some(id) => id,
                    None => {
                        return ToolResult::error(
                            "'channel_id' is required for 'update' action.",
                        )
                    }
                };

                // Wrap app_token in Option<Option<&str>> for the DB method
                let app_token_opt = params
                    .app_token
                    .as_ref()
                    .map(|t| Some(t.as_str()));

                match db.update_channel(
                    id,
                    params.name.as_deref(),
                    params.enabled,
                    params.bot_token.as_deref(),
                    app_token_opt,
                ) {
                    Ok(Some(ch)) => ToolResult::success(format!(
                        "Channel updated:\n\n{}",
                        format_channel(&ch)
                    ))
                    .with_metadata(channel_to_json(&ch)),
                    Ok(None) => ToolResult::error(format!("Channel #{} not found", id)),
                    Err(e) => ToolResult::error(format!("Failed to update channel: {}", e)),
                }
            }

            "delete" => {
                let id = match params.channel_id {
                    Some(id) => id,
                    None => {
                        return ToolResult::error(
                            "'channel_id' is required for 'delete' action.",
                        )
                    }
                };

                match db.delete_channel(id) {
                    Ok(true) => ToolResult::success(format!("Channel #{} deleted.", id))
                        .with_metadata(json!({ "deleted_id": id })),
                    Ok(false) => ToolResult::error(format!("Channel #{} not found", id)),
                    Err(e) => ToolResult::error(format!("Failed to delete channel: {}", e)),
                }
            }

            _ => ToolResult::error(format!(
                "Unknown action: '{}'. Valid actions: list, get, create, update, delete",
                params.action
            )),
        }
    }
}
