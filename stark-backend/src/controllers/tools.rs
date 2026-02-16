use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};

use crate::tools::{ToolConfig, ToolDefinition, ToolExecution, ToolGroup, ToolProfile};
use crate::AppState;

#[derive(Serialize)]
pub struct ToolsListResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<ToolInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub group: String,
    pub enabled: bool,
    pub safety_level: String,
}

#[derive(Serialize)]
pub struct GroupsListResponse {
    pub success: bool,
    pub groups: Vec<GroupInfo>,
}

#[derive(Serialize)]
pub struct GroupInfo {
    pub key: String,
    pub label: String,
    pub description: String,
}

#[derive(Serialize)]
pub struct ProfilesListResponse {
    pub success: bool,
    pub profiles: Vec<ProfileInfo>,
}

#[derive(Serialize)]
pub struct ProfileInfo {
    pub name: String,
    pub description: String,
    pub allowed_groups: Vec<String>,
}

#[derive(Serialize)]
pub struct ConfigResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub config: Option<ToolConfigResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct ToolConfigResponse {
    pub profile: String,
    pub allow_list: Vec<String>,
    pub deny_list: Vec<String>,
    pub allowed_groups: Vec<String>,
    pub denied_groups: Vec<String>,
}

impl From<ToolConfig> for ToolConfigResponse {
    fn from(config: ToolConfig) -> Self {
        let profile_str = match config.profile {
            ToolProfile::None => "none",
            ToolProfile::Minimal => "minimal",
            ToolProfile::Standard => "standard",
            ToolProfile::Messaging => "messaging",
            ToolProfile::Finance => "finance",
            ToolProfile::Developer => "developer",
            ToolProfile::Secretary => "secretary",
            ToolProfile::Full => "full",
            ToolProfile::Custom => "custom",
            ToolProfile::SafeMode => "safemode",
        };
        ToolConfigResponse {
            profile: profile_str.to_string(),
            allow_list: config.allow_list,
            deny_list: config.deny_list,
            allowed_groups: config.allowed_groups,
            denied_groups: config.denied_groups,
        }
    }
}

#[derive(Deserialize)]
pub struct UpdateConfigRequest {
    pub profile: Option<String>,
    pub allow_list: Option<Vec<String>>,
    pub deny_list: Option<Vec<String>>,
    pub allowed_groups: Option<Vec<String>>,
    pub denied_groups: Option<Vec<String>>,
}

#[derive(Serialize)]
pub struct HistoryResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub executions: Option<Vec<ToolExecution>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct HistoryQuery {
    pub channel_id: Option<i64>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/tools")
            .route("", web::get().to(list_tools))
            .route("/groups", web::get().to(list_groups))
            .route("/profiles", web::get().to(list_profiles))
            .route("/config", web::get().to(get_global_config))
            .route("/config", web::put().to(update_global_config))
            .route("/config/{channel_id}", web::get().to(get_channel_config))
            .route("/config/{channel_id}", web::put().to(update_channel_config))
            .route("/history", web::get().to(get_history)),
    );
}

fn validate_session_from_request(
    state: &web::Data<AppState>,
    req: &HttpRequest,
) -> Result<(), HttpResponse> {
    let token = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.trim_start_matches("Bearer ").to_string());

    let token = match token {
        Some(t) => t,
        None => {
            return Err(HttpResponse::Unauthorized().json(ToolsListResponse {
                success: false,
                tools: None,
                error: Some("No authorization token provided".to_string()),
            }));
        }
    };

    match state.db.validate_session(&token) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(HttpResponse::Unauthorized().json(ToolsListResponse {
            success: false,
            tools: None,
            error: Some("Invalid or expired session".to_string()),
        })),
        Err(e) => {
            log::error!("Failed to validate session: {}", e);
            Err(HttpResponse::InternalServerError().json(ToolsListResponse {
                success: false,
                tools: None,
                error: Some("Internal server error".to_string()),
            }))
        }
    }
}

async fn list_tools(state: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    let tool_config = state.db.get_effective_tool_config(None).unwrap_or_default();

    let tools: Vec<ToolInfo> = state
        .tool_registry
        .list()
        .iter()
        .map(|tool| {
            let def = tool.definition();
            let group = tool.group();
            ToolInfo {
                name: def.name.clone(),
                description: def.description.clone(),
                group: group.as_str().to_string(),
                enabled: tool_config.is_tool_allowed(&def.name, group),
                safety_level: tool.safety_level().as_str().to_string(),
            }
        })
        .collect();

    HttpResponse::Ok().json(ToolsListResponse {
        success: true,
        tools: Some(tools),
        error: None,
    })
}

/// List all tool groups with labels and descriptions
async fn list_groups(state: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    let groups: Vec<GroupInfo> = ToolGroup::all()
        .into_iter()
        .map(|g| GroupInfo {
            key: g.as_str().to_string(),
            label: g.label().to_string(),
            description: g.description().to_string(),
        })
        .collect();

    HttpResponse::Ok().json(GroupsListResponse {
        success: true,
        groups,
    })
}

async fn list_profiles(state: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    let profiles = vec![
        ProfileInfo {
            name: "none".to_string(),
            description: "No tools enabled".to_string(),
            allowed_groups: vec![],
        },
        ProfileInfo {
            name: "minimal".to_string(),
            description: "Web tools only".to_string(),
            allowed_groups: vec!["web".to_string()],
        },
        ProfileInfo {
            name: "standard".to_string(),
            description: "Web, filesystem, and exec tools".to_string(),
            allowed_groups: vec!["web".to_string(), "filesystem".to_string(), "exec".to_string()],
        },
        ProfileInfo {
            name: "messaging".to_string(),
            description: "Standard plus messaging tools".to_string(),
            allowed_groups: vec![
                "web".to_string(),
                "filesystem".to_string(),
                "exec".to_string(),
                "messaging".to_string(),
            ],
        },
        ProfileInfo {
            name: "finance".to_string(),
            description: "Finance/DeFi specialist - Web, filesystem, and finance tools".to_string(),
            allowed_groups: vec![
                "web".to_string(),
                "filesystem".to_string(),
                "finance".to_string(),
                "system".to_string(),
            ],
        },
        ProfileInfo {
            name: "developer".to_string(),
            description: "Developer specialist - Web, filesystem, exec, and development tools".to_string(),
            allowed_groups: vec![
                "web".to_string(),
                "filesystem".to_string(),
                "exec".to_string(),
                "development".to_string(),
                "system".to_string(),
            ],
        },
        ProfileInfo {
            name: "secretary".to_string(),
            description: "Secretary specialist - Web, filesystem, messaging, and social tools".to_string(),
            allowed_groups: vec![
                "web".to_string(),
                "filesystem".to_string(),
                "messaging".to_string(),
                "social".to_string(),
                "system".to_string(),
            ],
        },
        ProfileInfo {
            name: "full".to_string(),
            description: "All tools enabled".to_string(),
            allowed_groups: ToolGroup::all()
                .iter()
                .map(|g| g.as_str().to_string())
                .collect(),
        },
        ProfileInfo {
            name: "custom".to_string(),
            description: "Custom configuration with explicit allow/deny lists".to_string(),
            allowed_groups: vec![],
        },
    ];

    HttpResponse::Ok().json(ProfilesListResponse {
        success: true,
        profiles,
    })
}

async fn get_global_config(state: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    let config = state.db.get_effective_tool_config(None).unwrap_or_default();

    HttpResponse::Ok().json(ConfigResponse {
        success: true,
        config: Some(config.into()),
        error: None,
    })
}

async fn update_global_config(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<UpdateConfigRequest>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    let mut config = state.db.get_effective_tool_config(None).unwrap_or_default();
    config.channel_id = None; // Ensure it's global

    // Update fields if provided
    if let Some(ref profile_str) = body.profile {
        if let Some(profile) = ToolProfile::from_str(profile_str) {
            config.profile = profile;
        } else {
            return HttpResponse::BadRequest().json(ConfigResponse {
                success: false,
                config: None,
                error: Some(format!("Invalid profile: {}", profile_str)),
            });
        }
    }

    if let Some(ref allow_list) = body.allow_list {
        config.allow_list = allow_list.clone();
    }

    if let Some(ref deny_list) = body.deny_list {
        config.deny_list = deny_list.clone();
    }

    if let Some(ref allowed_groups) = body.allowed_groups {
        config.allowed_groups = allowed_groups.clone();
    }

    if let Some(ref denied_groups) = body.denied_groups {
        config.denied_groups = denied_groups.clone();
    }

    match state.db.save_tool_config(&config) {
        Ok(_) => HttpResponse::Ok().json(ConfigResponse {
            success: true,
            config: Some(config.into()),
            error: None,
        }),
        Err(e) => {
            log::error!("Failed to save tool config: {}", e);
            HttpResponse::InternalServerError().json(ConfigResponse {
                success: false,
                config: None,
                error: Some("Failed to save configuration".to_string()),
            })
        }
    }
}

async fn get_channel_config(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<i64>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    let channel_id = path.into_inner();
    let config = state
        .db
        .get_effective_tool_config(Some(channel_id))
        .unwrap_or_default();

    HttpResponse::Ok().json(ConfigResponse {
        success: true,
        config: Some(config.into()),
        error: None,
    })
}

async fn update_channel_config(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<UpdateConfigRequest>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    let channel_id = path.into_inner();

    // Start with existing config or default
    let mut config = state
        .db
        .get_channel_tool_config(channel_id)
        .ok()
        .flatten()
        .unwrap_or_else(|| {
            let mut c = ToolConfig::default();
            c.channel_id = Some(channel_id);
            c
        });

    config.channel_id = Some(channel_id);

    // Update fields if provided
    if let Some(ref profile_str) = body.profile {
        if let Some(profile) = ToolProfile::from_str(profile_str) {
            config.profile = profile;
        } else {
            return HttpResponse::BadRequest().json(ConfigResponse {
                success: false,
                config: None,
                error: Some(format!("Invalid profile: {}", profile_str)),
            });
        }
    }

    if let Some(ref allow_list) = body.allow_list {
        config.allow_list = allow_list.clone();
    }

    if let Some(ref deny_list) = body.deny_list {
        config.deny_list = deny_list.clone();
    }

    if let Some(ref allowed_groups) = body.allowed_groups {
        config.allowed_groups = allowed_groups.clone();
    }

    if let Some(ref denied_groups) = body.denied_groups {
        config.denied_groups = denied_groups.clone();
    }

    match state.db.save_tool_config(&config) {
        Ok(_) => HttpResponse::Ok().json(ConfigResponse {
            success: true,
            config: Some(config.into()),
            error: None,
        }),
        Err(e) => {
            log::error!("Failed to save channel tool config: {}", e);
            HttpResponse::InternalServerError().json(ConfigResponse {
                success: false,
                config: None,
                error: Some("Failed to save configuration".to_string()),
            })
        }
    }
}

async fn get_history(
    state: web::Data<AppState>,
    req: HttpRequest,
    query: web::Query<HistoryQuery>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    let limit = query.limit.unwrap_or(50);
    let offset = query.offset.unwrap_or(0);

    let executions = if let Some(channel_id) = query.channel_id {
        state.db.get_tool_execution_history(channel_id, limit, offset)
    } else {
        state.db.get_all_tool_execution_history(limit, offset)
    };

    match executions {
        Ok(execs) => HttpResponse::Ok().json(HistoryResponse {
            success: true,
            executions: Some(execs),
            error: None,
        }),
        Err(e) => {
            log::error!("Failed to get tool execution history: {}", e);
            HttpResponse::InternalServerError().json(HistoryResponse {
                success: false,
                executions: None,
                error: Some("Failed to retrieve execution history".to_string()),
            })
        }
    }
}
