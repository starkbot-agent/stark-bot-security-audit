//! DynamicModuleTool — a Tool implementation that proxies calls to a module's
//! HTTP RPC endpoint. Created from a `ToolManifest` at runtime, no compiled
//! request/response types needed.

use crate::tools::registry::Tool;
use crate::tools::types::{
    PropertySchema, ToolContext, ToolDefinition, ToolGroup, ToolInputSchema, ToolResult,
};
use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;

use super::manifest::ToolManifest;

/// A tool that forwards calls to a module microservice via HTTP.
pub struct DynamicModuleTool {
    definition: ToolDefinition,
    /// Full URL to POST/GET (e.g. "http://127.0.0.1:9100/rpc/watchlist")
    rpc_url: String,
    /// HTTP method (POST, GET, etc.)
    rpc_method: String,
}

impl DynamicModuleTool {
    /// Create a DynamicModuleTool from a manifest tool definition and the service base URL.
    pub fn from_manifest(manifest: &ToolManifest, service_base_url: &str) -> Self {
        let mut properties = HashMap::new();

        for (param_name, param) in &manifest.parameters {
            properties.insert(
                param_name.clone(),
                PropertySchema {
                    schema_type: param.param_type.clone(),
                    description: param
                        .description
                        .clone()
                        .unwrap_or_default(),
                    default: param.default.as_ref().map(toml_to_json),
                    items: None,
                    enum_values: param.enum_values.clone(),
                },
            );
        }

        let required = manifest.required_parameters();

        let rpc_url = format!(
            "{}{}",
            service_base_url.trim_end_matches('/'),
            manifest.rpc_endpoint
        );

        DynamicModuleTool {
            definition: ToolDefinition {
                name: manifest.name.clone(),
                description: manifest.description.clone(),
                input_schema: ToolInputSchema {
                    schema_type: "object".to_string(),
                    properties,
                    required,
                },
                group: manifest.tool_group(),
                hidden: false,
            },
            rpc_url,
            rpc_method: manifest.rpc_method.clone(),
        }
    }
}

/// Convert a TOML value to a serde_json Value (for default values).
fn toml_to_json(v: &toml::Value) -> Value {
    match v {
        toml::Value::String(s) => Value::String(s.clone()),
        toml::Value::Integer(i) => Value::Number((*i).into()),
        toml::Value::Float(f) => {
            serde_json::Number::from_f64(*f)
                .map(Value::Number)
                .unwrap_or(Value::Null)
        }
        toml::Value::Boolean(b) => Value::Bool(*b),
        toml::Value::Array(arr) => Value::Array(arr.iter().map(toml_to_json).collect()),
        toml::Value::Table(tbl) => {
            let map = tbl
                .iter()
                .map(|(k, v)| (k.clone(), toml_to_json(v)))
                .collect();
            Value::Object(map)
        }
        toml::Value::Datetime(dt) => Value::String(dt.to_string()),
    }
}

#[async_trait]
impl Tool for DynamicModuleTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, params: Value, _context: &ToolContext) -> ToolResult {
        let client = reqwest::Client::new();

        let request = match self.rpc_method.to_uppercase().as_str() {
            "GET" => client.get(&self.rpc_url).query(
                &params
                    .as_object()
                    .map(|m| {
                        m.iter()
                            .map(|(k, v)| (k.clone(), v.to_string()))
                            .collect::<Vec<_>>()
                    })
                    .unwrap_or_default(),
            ),
            _ => client
                .post(&self.rpc_url)
                .header("Content-Type", "application/json")
                .json(&params),
        };

        match request.send().await {
            Ok(resp) => {
                let status = resp.status();
                match resp.text().await {
                    Ok(body) => {
                        if status.is_success() {
                            // Try to parse as RpcResponse-like JSON
                            if let Ok(json) = serde_json::from_str::<Value>(&body) {
                                // If it has a "data" field, extract it
                                if let Some(data) = json.get("data") {
                                    ToolResult::success(
                                        serde_json::to_string_pretty(data)
                                            .unwrap_or_else(|_| body.clone()),
                                    )
                                } else {
                                    ToolResult::success(
                                        serde_json::to_string_pretty(&json)
                                            .unwrap_or(body),
                                    )
                                }
                            } else {
                                ToolResult::success(body)
                            }
                        } else {
                            ToolResult::error(format!(
                                "Service returned HTTP {}: {}",
                                status, body
                            ))
                        }
                    }
                    Err(e) => ToolResult::error(format!("Failed to read response body: {}", e)),
                }
            }
            Err(e) => ToolResult::error(format!(
                "Failed to reach module service at {}: {}",
                self.rpc_url, e
            )),
        }
    }
}
