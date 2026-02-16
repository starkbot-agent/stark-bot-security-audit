use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::models::{
    GetOrCreateIdentityRequest, IdentityResponse, LinkIdentityRequest, LinkedAccountInfo,
};
use crate::AppState;

/// Validate session token from request
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
            return Err(HttpResponse::Unauthorized().json(serde_json::json!({
                "error": "No authorization token provided"
            })));
        }
    };

    match state.db.validate_session(&token) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(HttpResponse::Unauthorized().json(serde_json::json!({
            "error": "Invalid or expired session"
        }))),
        Err(e) => {
            log::error!("Session validation error: {}", e);
            Err(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Internal server error"
            })))
        }
    }
}

/// List all identities
async fn list_identities(
    data: web::Data<AppState>,
    req: HttpRequest,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }

    match data.db.list_identities() {
        Ok(links) => {
            // Group by identity_id and return unique identities
            let mut seen = std::collections::HashSet::new();
            let responses: Vec<serde_json::Value> = links
                .into_iter()
                .filter(|link| seen.insert(link.identity_id.clone()))
                .map(|link| {
                    serde_json::json!({
                        "id": link.identity_id,
                        "name": link.platform_user_name.unwrap_or_else(|| link.platform_user_id.clone()),
                        "channel_type": link.channel_type,
                        "platform_user_id": link.platform_user_id,
                        "created_at": link.created_at.to_rfc3339()
                    })
                })
                .collect();
            HttpResponse::Ok().json(responses)
        }
        Err(e) => {
            log::error!("Failed to list identities: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Database error: {}", e)
            }))
        }
    }
}

/// Get or create an identity for a platform user
async fn get_or_create_identity(
    data: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<GetOrCreateIdentityRequest>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }
    match data.db.get_or_create_identity(
        &body.channel_type,
        &body.platform_user_id,
        body.platform_user_name.as_deref(),
    ) {
        Ok(link) => {
            // Get all linked accounts for this identity
            let linked_accounts = match data.db.get_linked_identities(&link.identity_id) {
                Ok(links) => links.iter().map(LinkedAccountInfo::from).collect(),
                Err(_) => vec![LinkedAccountInfo::from(&link)],
            };

            HttpResponse::Ok().json(IdentityResponse {
                identity_id: link.identity_id,
                linked_accounts,
                created_at: link.created_at,
            })
        }
        Err(e) => {
            log::error!("Failed to get or create identity: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Database error: {}", e)
            }))
        }
    }
}

/// Get identity by platform credentials
#[derive(Deserialize)]
struct GetIdentityQuery {
    channel_type: String,
    platform_user_id: String,
}

async fn get_identity(
    data: web::Data<AppState>,
    req: HttpRequest,
    query: web::Query<GetIdentityQuery>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }
    match data
        .db
        .get_identity_by_platform(&query.channel_type, &query.platform_user_id)
    {
        Ok(Some(link)) => {
            // Get all linked accounts for this identity
            let linked_accounts = match data.db.get_linked_identities(&link.identity_id) {
                Ok(links) => links.iter().map(LinkedAccountInfo::from).collect(),
                Err(_) => vec![LinkedAccountInfo::from(&link)],
            };

            HttpResponse::Ok().json(IdentityResponse {
                identity_id: link.identity_id,
                linked_accounts,
                created_at: link.created_at,
            })
        }
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Identity not found"
        })),
        Err(e) => {
            log::error!("Failed to get identity: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Database error: {}", e)
            }))
        }
    }
}

/// Link an existing identity to another platform
async fn link_identity(
    data: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<LinkIdentityRequest>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }
    // First check if this platform/user already has an identity
    if let Ok(Some(_)) = data
        .db
        .get_identity_by_platform(&body.channel_type, &body.platform_user_id)
    {
        return HttpResponse::Conflict().json(serde_json::json!({
            "error": "This platform user is already linked to an identity"
        }));
    }

    match data.db.link_identity(
        &body.identity_id,
        &body.channel_type,
        &body.platform_user_id,
        body.platform_user_name.as_deref(),
    ) {
        Ok(link) => {
            // Get all linked accounts for this identity
            let linked_accounts = match data.db.get_linked_identities(&link.identity_id) {
                Ok(links) => links.iter().map(LinkedAccountInfo::from).collect(),
                Err(_) => vec![LinkedAccountInfo::from(&link)],
            };

            HttpResponse::Created().json(IdentityResponse {
                identity_id: link.identity_id,
                linked_accounts,
                created_at: link.created_at,
            })
        }
        Err(e) => {
            log::error!("Failed to link identity: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Database error: {}", e)
            }))
        }
    }
}

/// Get all linked identities for a given identity_id
async fn get_linked_identities(
    data: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<String>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }
    let identity_id = path.into_inner();

    match data.db.get_linked_identities(&identity_id) {
        Ok(links) if !links.is_empty() => {
            let linked_accounts: Vec<LinkedAccountInfo> =
                links.iter().map(LinkedAccountInfo::from).collect();
            let created_at = links.first().map(|l| l.created_at).unwrap();

            HttpResponse::Ok().json(IdentityResponse {
                identity_id,
                linked_accounts,
                created_at,
            })
        }
        Ok(_) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Identity not found"
        })),
        Err(e) => {
            log::error!("Failed to get linked identities: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Database error: {}", e)
            }))
        }
    }
}

/// Get activity logs for an identity (sessions, tool calls)
async fn get_identity_logs(
    data: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<String>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }
    let identity_id = path.into_inner();

    // Get linked accounts first
    let linked_accounts = match data.db.get_linked_identities(&identity_id) {
        Ok(links) if !links.is_empty() => links,
        Ok(_) => {
            return HttpResponse::NotFound().json(serde_json::json!({
                "error": "Identity not found"
            }));
        }
        Err(e) => {
            log::error!("Failed to get linked identities: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Database error: {}", e)
            }));
        }
    };

    // Get sessions for this identity
    let sessions = match data.db.get_sessions_for_identity(&identity_id) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to get sessions for identity: {}", e);
            return HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Database error: {}", e)
            }));
        }
    };

    // Get message counts for each session
    let mut sessions_with_counts: Vec<serde_json::Value> = vec![];
    for session in &sessions {
        let message_count = data.db.count_session_messages(session.id).unwrap_or(0);
        let initial_query = data.db.get_first_user_message(session.id).ok().flatten();

        sessions_with_counts.push(serde_json::json!({
            "id": session.id,
            "session_key": session.session_key,
            "channel_type": session.channel_type,
            "channel_id": session.channel_id,
            "is_active": session.is_active,
            "completion_status": session.completion_status.as_str(),
            "message_count": message_count,
            "initial_query": initial_query,
            "created_at": session.created_at.to_rfc3339(),
            "last_activity_at": session.last_activity_at.to_rfc3339(),
        }));
    }

    // Get tool stats
    let tool_stats = match data.db.get_tool_stats_for_identity(&identity_id) {
        Ok(s) => s,
        Err(e) => {
            log::error!("Failed to get tool stats for identity: {}", e);
            vec![]
        }
    };

    // Get recent tool executions
    let recent_tools = match data.db.get_recent_tool_executions_for_identity(&identity_id, 50) {
        Ok(t) => t,
        Err(e) => {
            log::error!("Failed to get recent tool executions for identity: {}", e);
            vec![]
        }
    };

    let linked_accounts_info: Vec<LinkedAccountInfo> =
        linked_accounts.iter().map(LinkedAccountInfo::from).collect();

    HttpResponse::Ok().json(serde_json::json!({
        "identity_id": identity_id,
        "linked_accounts": linked_accounts_info,
        "sessions": sessions_with_counts,
        "session_count": sessions.len(),
        "tool_stats": tool_stats.iter().map(|(name, total, successful)| {
            serde_json::json!({
                "tool_name": name,
                "total_calls": total,
                "successful_calls": successful,
            })
        }).collect::<Vec<_>>(),
        "recent_tool_executions": recent_tools.iter().map(|t| {
            serde_json::json!({
                "id": t.id,
                "tool_name": t.tool_name,
                "parameters": t.parameters,
                "success": t.success,
                "result": t.result,
                "duration_ms": t.duration_ms,
                "executed_at": t.executed_at,
            })
        }).collect::<Vec<_>>(),
    }))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/identities")
            .route("", web::get().to(list_identities))
            .route("", web::post().to(get_or_create_identity))
            .route("/lookup", web::get().to(get_identity))
            .route("/link", web::post().to(link_identity))
            .route("/{identity_id}", web::get().to(get_linked_identities))
            .route("/{identity_id}/logs", web::get().to(get_identity_logs)),
    );
}
