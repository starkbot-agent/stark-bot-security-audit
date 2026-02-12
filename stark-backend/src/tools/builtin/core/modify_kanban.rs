//! Kanban board management tool for the AI agent
//!
//! Allows the agent to manage kanban board items:
//! - List items (optionally filtered by status)
//! - Pick the next ready task (atomically moves to in_progress)
//! - Update item status
//! - Add notes to items
//! - Create new items

use crate::db::tables::kanban::{CreateKanbanItemRequest, UpdateKanbanItemRequest};
use crate::tools::registry::Tool;
use crate::tools::types::{
    PropertySchema, ToolContext, ToolDefinition, ToolGroup, ToolInputSchema, ToolResult,
};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct ModifyKanbanTool {
    definition: ToolDefinition,
}

impl ModifyKanbanTool {
    pub fn new() -> Self {
        let mut properties = HashMap::new();

        properties.insert(
            "action".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "The action to perform: 'list' (show items), 'pick_task' (grab highest-priority ready task), 'update_status' (move item between columns), 'add_note' (append to result), 'create' (new item)".to_string(),
                default: None,
                items: None,
                enum_values: Some(vec![
                    "list".to_string(),
                    "pick_task".to_string(),
                    "update_status".to_string(),
                    "add_note".to_string(),
                    "create".to_string(),
                ]),
            },
        );

        properties.insert(
            "status".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Filter for 'list' action, or new status for 'update_status'. Values: 'ready', 'in_progress', 'complete'".to_string(),
                default: None,
                items: None,
                enum_values: Some(vec![
                    "ready".to_string(),
                    "in_progress".to_string(),
                    "complete".to_string(),
                ]),
            },
        );

        properties.insert(
            "item_id".to_string(),
            PropertySchema {
                schema_type: "integer".to_string(),
                description: "Kanban item ID (required for 'update_status' and 'add_note')".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "title".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Title for new kanban item (required for 'create')".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "description".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Description for 'create', or note text for 'add_note'".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "priority".to_string(),
            PropertySchema {
                schema_type: "integer".to_string(),
                description: "Priority for 'create': 0=normal, 1=high, 2=urgent".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        ModifyKanbanTool {
            definition: ToolDefinition {
                name: "modify_kanban".to_string(),
                description: "Manage the kanban board: list tasks, pick the next ready task, update status, add notes, or create new tasks. Use this to work through the kanban board autonomously.".to_string(),
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

impl Default for ModifyKanbanTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct ModifyKanbanParams {
    action: String,
    status: Option<String>,
    item_id: Option<i64>,
    title: Option<String>,
    description: Option<String>,
    priority: Option<i32>,
}

#[async_trait]
impl Tool for ModifyKanbanTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, params: Value, context: &ToolContext) -> ToolResult {
        let params: ModifyKanbanParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid parameters: {}", e)),
        };

        let db = match &context.database {
            Some(db) => db,
            None => return ToolResult::error("Database not available"),
        };

        match params.action.as_str() {
            "list" => {
                let items = if let Some(ref status) = params.status {
                    match db.list_kanban_items_by_status(status) {
                        Ok(items) => items,
                        Err(e) => return ToolResult::error(format!("Database error: {}", e)),
                    }
                } else {
                    match db.list_kanban_items() {
                        Ok(items) => items,
                        Err(e) => return ToolResult::error(format!("Database error: {}", e)),
                    }
                };

                if items.is_empty() {
                    let filter_msg = params.status.as_deref().map(|s| format!(" with status '{}'", s)).unwrap_or_default();
                    return ToolResult::success(format!("No kanban items found{}.", filter_msg));
                }

                let mut output = String::new();
                for item in &items {
                    let priority_label = match item.priority {
                        2 => " [URGENT]",
                        1 => " [HIGH]",
                        _ => "",
                    };
                    output.push_str(&format!(
                        "#{} [{}]{} — {}\n",
                        item.id, item.status, priority_label, item.title
                    ));
                    if !item.description.is_empty() {
                        output.push_str(&format!("  Description: {}\n", item.description));
                    }
                    if let Some(ref result) = item.result {
                        output.push_str(&format!("  Notes: {}\n", result));
                    }
                }

                ToolResult::success(output)
                    .with_metadata(json!({ "count": items.len() }))
            }

            "pick_task" => {
                match db.pick_next_kanban_task() {
                    Ok(Some(item)) => {
                        // Set session_id if available
                        if let Some(session_id) = context.session_id {
                            let _ = db.update_kanban_item(item.id, &UpdateKanbanItemRequest {
                                session_id: Some(session_id),
                                ..Default::default()
                            });
                        }

                        let priority_label = match item.priority {
                            2 => "URGENT",
                            1 => "HIGH",
                            _ => "NORMAL",
                        };

                        ToolResult::success(format!(
                            "Picked task #{} (priority: {}): {}\n\nDescription: {}\n\nThis task is now IN PROGRESS. Work on it and update status to 'complete' when done.",
                            item.id, priority_label, item.title, item.description
                        )).with_metadata(json!({
                            "item_id": item.id,
                            "title": item.title,
                            "priority": item.priority,
                        }))
                    }
                    Ok(None) => ToolResult::success("No ready tasks on the kanban board. All tasks are either in progress or complete."),
                    Err(e) => ToolResult::error(format!("Database error: {}", e)),
                }
            }

            "update_status" => {
                let item_id = match params.item_id {
                    Some(id) => id,
                    None => return ToolResult::error("'item_id' is required for 'update_status' action"),
                };
                let status = match params.status {
                    Some(s) => s,
                    None => return ToolResult::error("'status' is required for 'update_status' action"),
                };

                if !["ready", "in_progress", "complete"].contains(&status.as_str()) {
                    return ToolResult::error("Invalid status. Must be 'ready', 'in_progress', or 'complete'");
                }

                match db.update_kanban_item(item_id, &UpdateKanbanItemRequest {
                    status: Some(status.clone()),
                    ..Default::default()
                }) {
                    Ok(Some(item)) => {
                        ToolResult::success(format!(
                            "Task #{} '{}' moved to status: {}", item.id, item.title, status
                        ))
                    }
                    Ok(None) => ToolResult::error(format!("Kanban item #{} not found", item_id)),
                    Err(e) => ToolResult::error(format!("Database error: {}", e)),
                }
            }

            "add_note" => {
                let item_id = match params.item_id {
                    Some(id) => id,
                    None => return ToolResult::error("'item_id' is required for 'add_note' action"),
                };
                let note = match params.description {
                    Some(d) => d,
                    None => return ToolResult::error("'description' is required for 'add_note' action (the note text)"),
                };

                // Get existing item to append note
                let existing = match db.get_kanban_item(item_id) {
                    Ok(Some(item)) => item,
                    Ok(None) => return ToolResult::error(format!("Kanban item #{} not found", item_id)),
                    Err(e) => return ToolResult::error(format!("Database error: {}", e)),
                };

                let new_result = match existing.result {
                    Some(ref existing_notes) => format!("{}\n---\n{}", existing_notes, note),
                    None => note.clone(),
                };

                match db.update_kanban_item(item_id, &UpdateKanbanItemRequest {
                    result: Some(new_result),
                    ..Default::default()
                }) {
                    Ok(Some(_)) => ToolResult::success(format!("Note added to task #{}", item_id)),
                    Ok(None) => ToolResult::error(format!("Kanban item #{} not found", item_id)),
                    Err(e) => ToolResult::error(format!("Database error: {}", e)),
                }
            }

            "create" => {
                let title = match params.title {
                    Some(t) => t,
                    None => return ToolResult::error("'title' is required for 'create' action"),
                };

                let request = CreateKanbanItemRequest {
                    title,
                    description: params.description,
                    priority: params.priority,
                };

                match db.create_kanban_item(&request) {
                    Ok(item) => {
                        ToolResult::success(format!(
                            "Created kanban item #{}: '{}'", item.id, item.title
                        )).with_metadata(json!({
                            "item_id": item.id,
                            "title": item.title,
                        }))
                    }
                    Err(e) => ToolResult::error(format!("Database error: {}", e)),
                }
            }

            _ => ToolResult::error(format!(
                "Unknown action: '{}'. Valid actions: list, pick_task, update_status, add_note, create",
                params.action
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_definition() {
        let tool = ModifyKanbanTool::new();
        let def = tool.definition();
        assert_eq!(def.name, "modify_kanban");
        assert_eq!(def.group, ToolGroup::System);
    }
}
