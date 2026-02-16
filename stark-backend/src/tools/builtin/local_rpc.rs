//! Local RPC tool — generic HTTP client restricted to localhost
//!
//! Inverse security boundary from `web_fetch`: this tool ONLY allows
//! requests to localhost (127.0.0.1, ::1, localhost) and rejects
//! everything else.  Designed for calling local microservices
//! (e.g. wallet-monitor-service on port 9100) without requiring
//! dedicated tool structs for each endpoint.

use crate::tools::registry::Tool;
use crate::tools::types::{
    PropertySchema, ToolContext, ToolDefinition, ToolGroup, ToolInputSchema, ToolResult,
    ToolSafetyLevel,
};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct LocalRpcTool {
    definition: ToolDefinition,
}

impl LocalRpcTool {
    pub fn new() -> Self {
        let mut properties = HashMap::new();

        properties.insert(
            "url".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Full URL including path (must be localhost, 127.0.0.1, or ::1)"
                    .to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "method".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "HTTP method (default: GET)".to_string(),
                default: Some(json!("GET")),
                items: None,
                enum_values: Some(vec![
                    "GET".to_string(),
                    "POST".to_string(),
                    "PUT".to_string(),
                    "PATCH".to_string(),
                    "DELETE".to_string(),
                ]),
            },
        );

        properties.insert(
            "body".to_string(),
            PropertySchema {
                schema_type: "object".to_string(),
                description: "JSON request body (for POST/PUT/PATCH)".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        LocalRpcTool {
            definition: ToolDefinition {
                name: "local_rpc".to_string(),
                description: "Call a localhost HTTP endpoint. Only allows 127.0.0.1 / localhost / ::1. Use for local microservice APIs.".to_string(),
                input_schema: ToolInputSchema {
                    schema_type: "object".to_string(),
                    properties,
                    required: vec!["url".to_string()],
                },
                group: ToolGroup::System,
                hidden: false,
            },
        }
    }
}

impl Default for LocalRpcTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct LocalRpcParams {
    url: String,
    method: Option<String>,
    body: Option<Value>,
}

/// Check whether a URL host is localhost.
fn is_localhost_url(url: &url::Url) -> bool {
    match url.host_str() {
        Some(host) => {
            let h = host.to_lowercase();
            h == "localhost" || h == "127.0.0.1" || h == "::1" || h == "[::1]"
        }
        None => false,
    }
}

#[async_trait]
impl Tool for LocalRpcTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, params: Value, _context: &ToolContext) -> ToolResult {
        let params: LocalRpcParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid parameters: {}", e)),
        };

        // Validate URL scheme
        if !params.url.starts_with("http://") && !params.url.starts_with("https://") {
            return ToolResult::error("URL must start with http:// or https://");
        }

        // Parse URL
        let url = match url::Url::parse(&params.url) {
            Ok(u) => u,
            Err(e) => return ToolResult::error(format!("Invalid URL: {}", e)),
        };

        // Security: only allow localhost
        if !is_localhost_url(&url) {
            return ToolResult::error(format!(
                "local_rpc only allows localhost URLs. Got host: '{}'",
                url.host_str().unwrap_or("(none)")
            ));
        }

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(15))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        let method = params.method.as_deref().unwrap_or("GET").to_uppercase();

        let mut request = match method.as_str() {
            "POST" => client.post(&params.url),
            "PUT" => client.put(&params.url),
            "PATCH" => client.patch(&params.url),
            "DELETE" => client.delete(&params.url),
            _ => client.get(&params.url),
        };

        // Default to JSON content type for all requests
        request = request.header("Content-Type", "application/json");

        // Attach JSON body for write methods
        if let Some(ref body) = params.body {
            if matches!(method.as_str(), "POST" | "PUT" | "PATCH") {
                request = request.body(
                    serde_json::to_string(body)
                        .unwrap_or_else(|_| body.to_string()),
                );
            }
        }

        let response = match request.send().await {
            Ok(r) => r,
            Err(e) => {
                return ToolResult::error(format!("Request failed: {}", e));
            }
        };

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        if !status.is_success() {
            let truncated = if body.len() > 2000 {
                format!("{}...", &body[..2000])
            } else {
                body
            };
            return ToolResult::error(format!(
                "HTTP {} from {}\n{}",
                status, params.url, truncated
            ));
        }

        ToolResult::success(body)
    }

    fn safety_level(&self) -> ToolSafetyLevel {
        ToolSafetyLevel::Standard
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_localhost_urls_allowed() {
        let cases = vec![
            ("http://127.0.0.1:9100/api/watchlist", true),
            ("http://localhost:9100/api/watchlist", true),
            ("http://[::1]:9100/api/status", true),
            ("http://127.0.0.1/health", true),
            ("http://localhost/health", true),
        ];
        for (raw, expected) in cases {
            let url = url::Url::parse(raw).unwrap();
            assert_eq!(
                is_localhost_url(&url),
                expected,
                "Expected is_localhost_url({}) = {}",
                raw,
                expected
            );
        }
    }

    #[test]
    fn test_non_localhost_urls_rejected() {
        let cases = vec![
            "http://example.com/api",
            "http://8.8.8.8/dns",
            "http://192.168.1.1/admin",
            "http://10.0.0.1/internal",
            "https://google.com",
        ];
        for raw in cases {
            let url = url::Url::parse(raw).unwrap();
            assert!(
                !is_localhost_url(&url),
                "Expected is_localhost_url({}) = false",
                raw
            );
        }
    }
}
