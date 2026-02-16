//! Figma API tool for inspecting designs, components, styles, and exporting images.
//!
//! Provides access to the Figma REST API for:
//! - Getting file structure and node details
//! - Exporting design elements as images (PNG, SVG, PDF, JPG)
//! - Listing comments on a file
//! - Getting components and styles
//! - Getting design variables and variable collections
//! - Listing team projects and project files

use crate::tools::registry::Tool;
use crate::tools::types::{
    PropertySchema, ToolContext, ToolDefinition, ToolGroup, ToolInputSchema, ToolResult,
};
use crate::tools::ToolSafetyLevel;
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

const FIGMA_API: &str = "https://api.figma.com/v1";

pub struct FigmaTool {
    definition: ToolDefinition,
}

impl FigmaTool {
    pub fn new() -> Self {
        let mut properties = HashMap::new();

        properties.insert(
            "action".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Action to perform: 'get_file' (file tree), 'get_nodes' (specific nodes with full detail), 'get_images' (export as PNG/SVG), 'get_comments', 'get_components', 'get_styles', 'get_variables', 'list_projects' (team projects), 'list_files' (project files)".to_string(),
                default: None,
                items: None,
                enum_values: Some(vec![
                    "get_file".to_string(),
                    "get_nodes".to_string(),
                    "get_images".to_string(),
                    "get_comments".to_string(),
                    "get_components".to_string(),
                    "get_styles".to_string(),
                    "get_variables".to_string(),
                    "list_projects".to_string(),
                    "list_files".to_string(),
                ]),
            },
        );

        properties.insert(
            "file_key".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Figma file key (from URL: figma.com/design/<FILE_KEY>/...). Required for most actions.".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "node_ids".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Comma-separated node IDs (e.g. '1:2,1:3'). Required for get_nodes and get_images.".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "format".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Image export format for get_images (default: png)".to_string(),
                default: Some(json!("png")),
                items: None,
                enum_values: Some(vec![
                    "png".to_string(),
                    "svg".to_string(),
                    "jpg".to_string(),
                    "pdf".to_string(),
                ]),
            },
        );

        properties.insert(
            "scale".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Image export scale for get_images (0.01 to 4, default: 1)".to_string(),
                default: Some(json!("1")),
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "team_id".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Team ID for list_projects action".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "project_id".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Project ID for list_files action".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "depth".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Traversal depth for get_file (default: 2). Use 1 for top-level pages only, higher for more detail.".to_string(),
                default: Some(json!("2")),
                items: None,
                enum_values: None,
            },
        );

        FigmaTool {
            definition: ToolDefinition {
                name: "figma".to_string(),
                description: r#"Interact with Figma designs via the Figma REST API. Requires FIGMA_ACCESS_TOKEN API key.

ACTIONS:
- get_file: Get file structure (pages, frames, components). Use depth param to control detail.
- get_nodes: Get full details for specific nodes by ID (geometry, styles, properties).
- get_images: Export design nodes as PNG/SVG/JPG/PDF. Returns download URLs.
- get_comments: List all comments on a file.
- get_components: List published components in a file.
- get_styles: List published styles (colors, text, effects) in a file.
- get_variables: Get design variables and variable collections.
- list_projects: List projects in a team (requires team_id).
- list_files: List files in a project (requires project_id).

FILE KEY: Extract from Figma URL — figma.com/design/<FILE_KEY>/...
NODE IDS: Found in get_file response or Figma URL after ?node-id=

EXAMPLES:
- Browse file: {"action":"get_file","file_key":"abc123","depth":"1"}
- Inspect node: {"action":"get_nodes","file_key":"abc123","node_ids":"1:2"}
- Export PNG: {"action":"get_images","file_key":"abc123","node_ids":"1:2","format":"png","scale":"2"}
- Get styles: {"action":"get_styles","file_key":"abc123"}"#.to_string(),
                input_schema: ToolInputSchema {
                    schema_type: "object".to_string(),
                    properties,
                    required: vec!["action".to_string()],
                },
                group: ToolGroup::Web,
                hidden: true, // Activated by the figma skill
            },
        }
    }

    /// Get the Figma access token from context
    fn get_token(context: &ToolContext) -> Option<String> {
        context.get_api_key("FIGMA_ACCESS_TOKEN")
    }

    /// Make an authenticated GET request to the Figma API
    async fn figma_get(context: &ToolContext, path: &str) -> Result<Value, String> {
        let token = Self::get_token(context)
            .ok_or_else(|| "FIGMA_ACCESS_TOKEN not set. Install it with install_api_key.".to_string())?;

        let client = context.http_client();
        let url = format!("{}{}", FIGMA_API, path);

        let resp = client
            .get(&url)
            .header("X-Figma-Token", &token)
            .timeout(std::time::Duration::from_secs(30))
            .send()
            .await
            .map_err(|e| format!("Figma API request failed: {}", e))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("Figma API error {}: {}", status, truncate(&body, 500)));
        }

        resp.json::<Value>()
            .await
            .map_err(|e| format!("Failed to parse Figma response: {}", e))
    }
}

impl Default for FigmaTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct Params {
    action: String,
    file_key: Option<String>,
    node_ids: Option<String>,
    format: Option<String>,
    scale: Option<String>,
    team_id: Option<String>,
    project_id: Option<String>,
    depth: Option<String>,
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() > max {
        format!("{}...", &s[..max])
    } else {
        s.to_string()
    }
}

/// Recursively summarize a Figma document tree to avoid overwhelming output
fn summarize_node(node: &Value, depth: usize, max_depth: usize) -> String {
    let name = node["name"].as_str().unwrap_or("(unnamed)");
    let node_type = node["type"].as_str().unwrap_or("UNKNOWN");
    let id = node["id"].as_str().unwrap_or("");
    let indent = "  ".repeat(depth);

    let mut line = format!("{}{} [{}] id={}", indent, node_type, name, id);

    // Add useful properties
    if let Some(fills) = node.get("fills") {
        if let Some(arr) = fills.as_array() {
            if !arr.is_empty() {
                let colors: Vec<String> = arr
                    .iter()
                    .filter_map(|f| {
                        f.get("color").map(|c| {
                            format!(
                                "rgba({},{},{},{:.1})",
                                (c["r"].as_f64().unwrap_or(0.0) * 255.0) as u8,
                                (c["g"].as_f64().unwrap_or(0.0) * 255.0) as u8,
                                (c["b"].as_f64().unwrap_or(0.0) * 255.0) as u8,
                                c["a"].as_f64().unwrap_or(1.0)
                            )
                        })
                    })
                    .collect();
                if !colors.is_empty() {
                    line.push_str(&format!(" fills=[{}]", colors.join(",")));
                }
            }
        }
    }

    if let Some(chars) = node.get("characters") {
        if let Some(text) = chars.as_str() {
            let preview = truncate(text, 60);
            line.push_str(&format!(" text=\"{}\"", preview));
        }
    }

    if let Some(bbox) = node.get("absoluteBoundingBox") {
        if let (Some(w), Some(h)) = (bbox["width"].as_f64(), bbox["height"].as_f64()) {
            line.push_str(&format!(" {}x{}", w as i32, h as i32));
        }
    }

    let mut out = vec![line];

    if depth < max_depth {
        if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
            for child in children {
                out.push(summarize_node(child, depth + 1, max_depth));
            }
        }
    } else if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        if !children.is_empty() {
            out.push(format!("{}  ... {} children", "  ".repeat(depth), children.len()));
        }
    }

    out.join("\n")
}

fn format_comments(data: &Value) -> String {
    let comments = match data.get("comments").and_then(|c| c.as_array()) {
        Some(c) => c,
        None => return "No comments found.".to_string(),
    };

    if comments.is_empty() {
        return "No comments on this file.".to_string();
    }

    let mut out = format!("{} comments:\n\n", comments.len());
    for c in comments.iter().take(50) {
        let user = c["user"]["handle"].as_str().unwrap_or("unknown");
        let message = c["message"].as_str().unwrap_or("");
        let created = c["created_at"].as_str().unwrap_or("");
        let resolved = c["resolved_at"].as_str();
        let status = if resolved.is_some() { " [resolved]" } else { "" };
        out.push_str(&format!("**{}**{} ({})\n{}\n\n", user, status, created, message));
    }
    out
}

#[async_trait]
impl Tool for FigmaTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, params: Value, context: &ToolContext) -> ToolResult {
        let params: Params = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid parameters: {}", e)),
        };

        match params.action.as_str() {
            "get_file" => {
                let file_key = match &params.file_key {
                    Some(k) if !k.is_empty() => k,
                    _ => return ToolResult::error("'file_key' required for get_file"),
                };
                let depth: usize = params.depth.as_deref().unwrap_or("2").parse().unwrap_or(2);

                let path = format!("/files/{}?depth={}", file_key, depth);
                match Self::figma_get(context, &path).await {
                    Ok(data) => {
                        let file_name = data["name"].as_str().unwrap_or("(untitled)");
                        let last_modified = data["lastModified"].as_str().unwrap_or("unknown");
                        let version = data["version"].as_str().unwrap_or("unknown");

                        let mut out = format!(
                            "**{}**\nLast modified: {} | Version: {}\n\n",
                            file_name, last_modified, version
                        );

                        if let Some(document) = data.get("document") {
                            out.push_str(&summarize_node(document, 0, depth + 1));
                        }

                        ToolResult::success(truncate(&out, 30000)).with_metadata(json!({
                            "file_key": file_key,
                            "name": file_name,
                            "last_modified": last_modified,
                        }))
                    }
                    Err(e) => ToolResult::error(e),
                }
            }

            "get_nodes" => {
                let file_key = match &params.file_key {
                    Some(k) if !k.is_empty() => k,
                    _ => return ToolResult::error("'file_key' required"),
                };
                let node_ids = match &params.node_ids {
                    Some(ids) if !ids.is_empty() => ids,
                    _ => return ToolResult::error("'node_ids' required (e.g. '1:2,1:3')"),
                };

                let encoded_ids = urlencoding::encode(node_ids);
                let path = format!("/files/{}/nodes?ids={}", file_key, encoded_ids);
                match Self::figma_get(context, &path).await {
                    Ok(data) => {
                        let nodes = data.get("nodes").cloned().unwrap_or(json!({}));
                        let pretty = serde_json::to_string_pretty(&nodes).unwrap_or_default();
                        ToolResult::success(truncate(&pretty, 30000)).with_metadata(json!({
                            "file_key": file_key,
                            "node_ids": node_ids,
                        }))
                    }
                    Err(e) => ToolResult::error(e),
                }
            }

            "get_images" => {
                let file_key = match &params.file_key {
                    Some(k) if !k.is_empty() => k,
                    _ => return ToolResult::error("'file_key' required"),
                };
                let node_ids = match &params.node_ids {
                    Some(ids) if !ids.is_empty() => ids,
                    _ => return ToolResult::error("'node_ids' required (e.g. '1:2,1:3')"),
                };

                let format = params.format.as_deref().unwrap_or("png");
                let scale = params.scale.as_deref().unwrap_or("1");
                let encoded_ids = urlencoding::encode(node_ids);
                let path = format!(
                    "/images/{}?ids={}&format={}&scale={}",
                    file_key, encoded_ids, format, scale
                );

                match Self::figma_get(context, &path).await {
                    Ok(data) => {
                        let images = data.get("images").cloned().unwrap_or(json!({}));
                        let mut out = format!("Exported {} image(s) as {} @{}x:\n\n",
                            images.as_object().map(|m| m.len()).unwrap_or(0),
                            format, scale
                        );

                        if let Some(map) = images.as_object() {
                            for (node_id, url) in map {
                                if let Some(url_str) = url.as_str() {
                                    out.push_str(&format!("Node {}: {}\n", node_id, url_str));
                                } else {
                                    out.push_str(&format!("Node {}: (export failed)\n", node_id));
                                }
                            }
                        }

                        ToolResult::success(out).with_metadata(json!({
                            "file_key": file_key,
                            "images": images,
                            "format": format,
                            "scale": scale,
                        }))
                    }
                    Err(e) => ToolResult::error(e),
                }
            }

            "get_comments" => {
                let file_key = match &params.file_key {
                    Some(k) if !k.is_empty() => k,
                    _ => return ToolResult::error("'file_key' required"),
                };

                let path = format!("/files/{}/comments", file_key);
                match Self::figma_get(context, &path).await {
                    Ok(data) => {
                        let out = format_comments(&data);
                        ToolResult::success(out).with_metadata(json!({"file_key": file_key}))
                    }
                    Err(e) => ToolResult::error(e),
                }
            }

            "get_components" => {
                let file_key = match &params.file_key {
                    Some(k) if !k.is_empty() => k,
                    _ => return ToolResult::error("'file_key' required"),
                };

                let path = format!("/files/{}/components", file_key);
                match Self::figma_get(context, &path).await {
                    Ok(data) => {
                        let meta = data.get("meta").and_then(|m| m.get("components"));
                        match meta {
                            Some(components) => {
                                let arr = components.as_array();
                                let count = arr.map(|a| a.len()).unwrap_or(0);
                                let mut out = format!("{} published components:\n\n", count);

                                if let Some(arr) = arr {
                                    for comp in arr.iter().take(50) {
                                        let name = comp["name"].as_str().unwrap_or("(unnamed)");
                                        let desc = comp["description"].as_str().unwrap_or("");
                                        let key = comp["key"].as_str().unwrap_or("");
                                        let node_id = comp["node_id"].as_str().unwrap_or("");
                                        out.push_str(&format!("- **{}** (key={}, node={})", name, key, node_id));
                                        if !desc.is_empty() {
                                            out.push_str(&format!("\n  {}", desc));
                                        }
                                        out.push('\n');
                                    }
                                }
                                ToolResult::success(out).with_metadata(json!({"file_key": file_key, "count": count}))
                            }
                            None => ToolResult::success("No published components found."),
                        }
                    }
                    Err(e) => ToolResult::error(e),
                }
            }

            "get_styles" => {
                let file_key = match &params.file_key {
                    Some(k) if !k.is_empty() => k,
                    _ => return ToolResult::error("'file_key' required"),
                };

                let path = format!("/files/{}/styles", file_key);
                match Self::figma_get(context, &path).await {
                    Ok(data) => {
                        let meta = data.get("meta").and_then(|m| m.get("styles"));
                        match meta {
                            Some(styles) => {
                                let arr = styles.as_array();
                                let count = arr.map(|a| a.len()).unwrap_or(0);
                                let mut out = format!("{} published styles:\n\n", count);

                                if let Some(arr) = arr {
                                    for style in arr.iter().take(100) {
                                        let name = style["name"].as_str().unwrap_or("(unnamed)");
                                        let style_type = style["style_type"].as_str().unwrap_or("?");
                                        let desc = style["description"].as_str().unwrap_or("");
                                        let key = style["key"].as_str().unwrap_or("");
                                        out.push_str(&format!("- **{}** [{}] key={}", name, style_type, key));
                                        if !desc.is_empty() {
                                            out.push_str(&format!(" — {}", desc));
                                        }
                                        out.push('\n');
                                    }
                                }
                                ToolResult::success(out).with_metadata(json!({"file_key": file_key, "count": count}))
                            }
                            None => ToolResult::success("No published styles found."),
                        }
                    }
                    Err(e) => ToolResult::error(e),
                }
            }

            "get_variables" => {
                let file_key = match &params.file_key {
                    Some(k) if !k.is_empty() => k,
                    _ => return ToolResult::error("'file_key' required"),
                };

                let path = format!("/files/{}/variables/local", file_key);
                match Self::figma_get(context, &path).await {
                    Ok(data) => {
                        let meta = data.get("meta").cloned().unwrap_or(json!({}));

                        let collections = meta.get("variableCollections")
                            .and_then(|c| c.as_object());
                        let variables = meta.get("variables")
                            .and_then(|v| v.as_object());

                        let mut out = String::new();

                        if let Some(colls) = collections {
                            out.push_str(&format!("{} variable collections:\n\n", colls.len()));
                            for (_, coll) in colls {
                                let name = coll["name"].as_str().unwrap_or("(unnamed)");
                                let modes = coll.get("modes")
                                    .and_then(|m| m.as_array())
                                    .map(|a| a.iter()
                                        .filter_map(|m| m["name"].as_str())
                                        .collect::<Vec<_>>()
                                        .join(", "))
                                    .unwrap_or_default();
                                out.push_str(&format!("### {}\nModes: {}\n\n", name, modes));
                            }
                        }

                        if let Some(vars) = variables {
                            out.push_str(&format!("{} variables:\n\n", vars.len()));
                            for (_, var) in vars.iter().take(100) {
                                let name = var["name"].as_str().unwrap_or("?");
                                let resolved_type = var["resolvedType"].as_str().unwrap_or("?");
                                out.push_str(&format!("- **{}** [{}]", name, resolved_type));
                                if let Some(values) = var.get("valuesByMode").and_then(|v| v.as_object()) {
                                    let vals: Vec<String> = values.values()
                                        .take(3)
                                        .map(|v| format!("{}", v))
                                        .collect();
                                    if !vals.is_empty() {
                                        out.push_str(&format!(" = {}", vals.join(" | ")));
                                    }
                                }
                                out.push('\n');
                            }
                        }

                        if out.is_empty() {
                            out = "No variables found in this file.".to_string();
                        }

                        ToolResult::success(truncate(&out, 30000)).with_metadata(json!({"file_key": file_key}))
                    }
                    Err(e) => ToolResult::error(e),
                }
            }

            "list_projects" => {
                let team_id = match &params.team_id {
                    Some(id) if !id.is_empty() => id,
                    _ => return ToolResult::error("'team_id' required for list_projects"),
                };

                let path = format!("/teams/{}/projects", team_id);
                match Self::figma_get(context, &path).await {
                    Ok(data) => {
                        let projects = data.get("projects").and_then(|p| p.as_array());
                        match projects {
                            Some(arr) => {
                                let mut out = format!("{} projects:\n\n", arr.len());
                                for proj in arr {
                                    let name = proj["name"].as_str().unwrap_or("(unnamed)");
                                    let id = proj["id"].as_str()
                                        .or_else(|| proj["id"].as_i64().map(|_| ""))
                                        .unwrap_or("");
                                    // id might be a number
                                    let id_str = if id.is_empty() {
                                        proj["id"].to_string()
                                    } else {
                                        id.to_string()
                                    };
                                    out.push_str(&format!("- **{}** (id: {})\n", name, id_str));
                                }
                                ToolResult::success(out).with_metadata(json!({"team_id": team_id}))
                            }
                            None => ToolResult::success("No projects found."),
                        }
                    }
                    Err(e) => ToolResult::error(e),
                }
            }

            "list_files" => {
                let project_id = match &params.project_id {
                    Some(id) if !id.is_empty() => id,
                    _ => return ToolResult::error("'project_id' required for list_files"),
                };

                let path = format!("/projects/{}/files", project_id);
                match Self::figma_get(context, &path).await {
                    Ok(data) => {
                        let files = data.get("files").and_then(|f| f.as_array());
                        match files {
                            Some(arr) => {
                                let mut out = format!("{} files:\n\n", arr.len());
                                for file in arr {
                                    let name = file["name"].as_str().unwrap_or("(unnamed)");
                                    let key = file["key"].as_str().unwrap_or("?");
                                    let modified = file["last_modified"].as_str().unwrap_or("");
                                    out.push_str(&format!("- **{}** (key: {}) modified: {}\n", name, key, modified));
                                }
                                ToolResult::success(out).with_metadata(json!({"project_id": project_id}))
                            }
                            None => ToolResult::success("No files found."),
                        }
                    }
                    Err(e) => ToolResult::error(e),
                }
            }

            _ => ToolResult::error(format!(
                "Unknown action '{}'. Use: get_file, get_nodes, get_images, get_comments, get_components, get_styles, get_variables, list_projects, list_files",
                params.action
            )),
        }
    }

    fn safety_level(&self) -> ToolSafetyLevel {
        ToolSafetyLevel::Standard
    }
}
