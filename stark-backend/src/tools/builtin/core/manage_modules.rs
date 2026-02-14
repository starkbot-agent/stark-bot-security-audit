//! Module management tool — install, uninstall, enable, disable, list, and check status of plugins
//!
//! Modules are standalone microservices. This tool manages the bot's
//! record of which modules are installed/enabled and hot-registers their tools.

use crate::tools::registry::Tool;
use crate::tools::types::{
    PropertySchema, ToolContext, ToolDefinition, ToolGroup, ToolInputSchema, ToolResult,
};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct ManageModulesTool {
    definition: ToolDefinition,
}

impl ManageModulesTool {
    pub fn new() -> Self {
        let mut properties = HashMap::new();

        properties.insert(
            "action".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Action: 'list' available modules, 'install' a module, 'uninstall', 'enable', 'disable', or check 'status'".to_string(),
                default: None,
                items: None,
                enum_values: Some(vec![
                    "list".to_string(),
                    "install".to_string(),
                    "uninstall".to_string(),
                    "enable".to_string(),
                    "disable".to_string(),
                    "status".to_string(),
                ]),
            },
        );

        properties.insert(
            "name".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Module name (required for install, uninstall, enable, disable, status)".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        ManageModulesTool {
            definition: ToolDefinition {
                name: "manage_modules".to_string(),
                description: "Manage StarkBot plugin modules. Each module is a standalone microservice with its own database and dashboard. List available modules, install/uninstall, enable/disable, or check status.".to_string(),
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

#[derive(Debug, Deserialize)]
struct ModuleParams {
    action: String,
    name: Option<String>,
}

#[async_trait]
impl Tool for ManageModulesTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, params: Value, context: &ToolContext) -> ToolResult {
        let params: ModuleParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid parameters: {}", e)),
        };

        let db = match context.database.as_ref() {
            Some(db) => db,
            None => return ToolResult::error("Database not available"),
        };

        match params.action.as_str() {
            "list" => {
                let registry = crate::modules::ModuleRegistry::new();
                let installed = db.list_installed_modules().unwrap_or_default();

                let mut output = String::from("**Available Modules**\n\n");

                for module in registry.available_modules() {
                    let installed_entry = installed.iter().find(|m| m.module_name == module.name());
                    let status = match installed_entry {
                        Some(e) if e.enabled => "installed & enabled",
                        Some(_) => "installed (disabled)",
                        None => "not installed",
                    };

                    output.push_str(&format!(
                        "**{}** v{} — {}\n  Status: {} | Service: {} | Tools: {} | Dashboard: {}\n\n",
                        module.name(),
                        module.version(),
                        module.description(),
                        status,
                        module.service_url(),
                        if module.has_tools() { "yes" } else { "no" },
                        if module.has_dashboard() { "yes" } else { "no" },
                    ));
                }

                ToolResult::success(output)
            }

            "install" => {
                let name = match params.name.as_deref() {
                    Some(n) => n,
                    None => return ToolResult::error("'name' is required for 'install' action"),
                };

                if db.is_module_installed(name).unwrap_or(false) {
                    return ToolResult::error(format!("Module '{}' is already installed. Use 'enable' to re-enable it.", name));
                }

                let registry = crate::modules::ModuleRegistry::new();
                let module = match registry.get(name) {
                    Some(m) => m,
                    None => return ToolResult::error(format!("Unknown module: '{}'. Use action='list' to see available modules.", name)),
                };

                match db.install_module(
                    name,
                    module.description(),
                    module.version(),
                    module.has_tools(),
                    module.has_dashboard(),
                ) {
                    Ok(_entry) => {
                        let mut result_parts = vec![
                            format!("Module '{}' installed successfully!", name),
                            format!("Service URL: {}", module.service_url()),
                        ];

                        if module.has_tools() {
                            result_parts.push("Tools registered (available after restart or on next session).".to_string());
                        }
                        if module.has_dashboard() {
                            result_parts.push(format!("Dashboard: {}/", module.service_url()));
                        }

                        // Install skill if module provides one
                        if let Some(skill_md) = module.skill_content() {
                            if let Some(skill_registry) = context.skill_registry.as_ref() {
                                match skill_registry.create_skill_from_markdown(skill_md) {
                                    Ok(_) => result_parts.push("Skill installed.".to_string()),
                                    Err(e) => result_parts.push(format!("Warning: Failed to install skill: {}", e)),
                                }
                            }
                        }

                        ToolResult::success(result_parts.join("\n"))
                    }
                    Err(e) => ToolResult::error(format!("Failed to install module: {}", e)),
                }
            }

            "uninstall" => {
                let name = match params.name.as_deref() {
                    Some(n) => n,
                    None => return ToolResult::error("'name' is required for 'uninstall' action"),
                };
                match db.uninstall_module(name) {
                    Ok(true) => ToolResult::success(format!(
                        "Module '{}' uninstalled. The service continues running independently.",
                        name
                    )),
                    Ok(false) => ToolResult::error(format!("Module '{}' is not installed", name)),
                    Err(e) => ToolResult::error(format!("Failed to uninstall: {}", e)),
                }
            }

            "enable" => {
                let name = match params.name.as_deref() {
                    Some(n) => n,
                    None => return ToolResult::error("'name' is required for 'enable' action"),
                };
                match db.set_module_enabled(name, true) {
                    Ok(true) => ToolResult::success(format!(
                        "Module '{}' enabled. Tools are now active.",
                        name
                    )),
                    Ok(false) => ToolResult::error(format!("Module '{}' is not installed", name)),
                    Err(e) => ToolResult::error(format!("Failed to enable: {}", e)),
                }
            }

            "disable" => {
                let name = match params.name.as_deref() {
                    Some(n) => n,
                    None => return ToolResult::error("'name' is required for 'disable' action"),
                };
                match db.set_module_enabled(name, false) {
                    Ok(true) => ToolResult::success(format!(
                        "Module '{}' disabled. Tools hidden. Service continues running.",
                        name
                    )),
                    Ok(false) => ToolResult::error(format!("Module '{}' is not installed", name)),
                    Err(e) => ToolResult::error(format!("Failed to disable: {}", e)),
                }
            }

            "status" => {
                let name = match params.name.as_deref() {
                    Some(n) => n,
                    None => return ToolResult::error("'name' is required for 'status' action"),
                };

                let registry = crate::modules::ModuleRegistry::new();
                let module = match registry.get(name) {
                    Some(m) => m,
                    None => return ToolResult::error(format!("Unknown module: '{}'", name)),
                };

                match db.get_installed_module(name) {
                    Ok(Some(m)) => {
                        ToolResult::success(json!({
                            "module": m.module_name,
                            "version": m.version,
                            "enabled": m.enabled,
                            "description": m.description,
                            "has_tools": m.has_tools,
                            "has_dashboard": m.has_dashboard,
                            "service_url": module.service_url(),
                            "installed_at": m.installed_at.to_rfc3339(),
                        }).to_string())
                    }
                    Ok(None) => ToolResult::error(format!("Module '{}' is not installed", name)),
                    Err(e) => ToolResult::error(format!("Failed to get status: {}", e)),
                }
            }

            _ => ToolResult::error(format!(
                "Unknown action: '{}'. Use 'list', 'install', 'uninstall', 'enable', 'disable', or 'status'.",
                params.action
            )),
        }
    }

    fn safety_level(&self) -> crate::tools::types::ToolSafetyLevel {
        crate::tools::types::ToolSafetyLevel::Standard
    }
}
