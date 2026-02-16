use crate::gateway::protocol::GatewayEvent;
use crate::tools::registry::Tool;
use crate::tools::types::{
    PropertySchema, ToolContext, ToolDefinition, ToolGroup, ToolInputSchema, ToolResult,
};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Tool for changing the UI theme accent color
pub struct SetThemeAccentTool {
    definition: ToolDefinition,
}

impl SetThemeAccentTool {
    pub fn new() -> Self {
        let mut properties = HashMap::new();
        properties.insert(
            "color".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Accent color to set. Use a color name like 'blue', 'purple', 'red', 'green', 'pink', 'cyan', or 'default' to reset to the default orange.".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        SetThemeAccentTool {
            definition: ToolDefinition {
                name: "set_theme_accent".to_string(),
                description: "Change the UI theme accent color. Use color names like 'blue', 'purple', 'red', or 'default' to reset.".to_string(),
                input_schema: ToolInputSchema {
                    schema_type: "object".to_string(),
                    properties,
                    required: vec!["color".to_string()],
                },
                group: ToolGroup::System,
                hidden: false,
            },
        }
    }
}

impl Default for SetThemeAccentTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct SetThemeAccentParams {
    color: String,
}

#[async_trait]
impl Tool for SetThemeAccentTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, params: Value, context: &ToolContext) -> ToolResult {
        let params: SetThemeAccentParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid parameters: {}", e)),
        };

        let db = match &context.database {
            Some(db) => db,
            None => return ToolResult::error("Database not available"),
        };

        // Normalize: "default" or empty string clears the accent
        let theme_accent = if params.color.is_empty()
            || params.color.eq_ignore_ascii_case("default")
            || params.color.eq_ignore_ascii_case("orange")
        {
            None
        } else {
            Some(params.color.to_lowercase())
        };

        let accent_str = theme_accent.as_deref();

        match db.update_bot_settings_full(
            None, None, None, None, None, None, None, None, None, None, None,
            accent_str,
            None, None,
        ) {
            Ok(settings) => {
                let display_color = settings
                    .theme_accent
                    .as_deref()
                    .unwrap_or("orange (default)");

                // Broadcast settings-changed event for live UI update
                if let Some(ref broadcaster) = context.broadcaster {
                    broadcaster.broadcast(GatewayEvent::new(
                        "settings.changed",
                        json!({
                            "theme_accent": display_color
                        }),
                    ));
                }

                ToolResult::success(format!("Theme accent color set to: {}", display_color))
                    .with_metadata(json!({
                        "theme_accent": display_color
                    }))
            }
            Err(e) => ToolResult::error(format!("Failed to update theme accent: {}", e)),
        }
    }
}
