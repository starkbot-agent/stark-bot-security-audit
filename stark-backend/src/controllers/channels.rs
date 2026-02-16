use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::Serialize;

use crate::models::{
    get_settings_for_channel_type, ChannelResponse, ChannelSettingsResponse,
    ChannelSettingsSchemaResponse, ChannelType, CreateChannelRequest, CreateSafeModeChannelRequest,
    UpdateChannelRequest, UpdateChannelSettingsRequest,
};
use crate::AppState;

#[derive(Serialize)]
pub struct ChannelsListResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channels: Option<Vec<ChannelResponse>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Serialize)]
pub struct ChannelOperationResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<ChannelResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Response for safe mode channel creation with rate limit info
#[derive(Serialize)]
pub struct SafeModeChannelResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<ChannelResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Current queue length (how many pending requests)
    pub queue_length: usize,
    /// Milliseconds until next slot available
    pub next_slot_ms: u64,
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/channels")
            .route("", web::get().to(list_channels))
            .route("", web::post().to(create_channel))
            .route("/safe-mode", web::post().to(create_safe_mode_channel))
            .route("/safe-mode/status", web::get().to(safe_mode_rate_limit_status))
            .route("/settings/schema/{channel_type}", web::get().to(get_settings_schema))
            .route("/{id}", web::get().to(get_channel))
            .route("/{id}", web::put().to(update_channel))
            .route("/{id}", web::delete().to(delete_channel))
            .route("/{id}/start", web::post().to(start_channel))
            .route("/{id}/stop", web::post().to(stop_channel))
            .route("/{id}/settings", web::get().to(get_channel_settings))
            .route("/{id}/settings", web::put().to(update_channel_settings)),
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
            return Err(HttpResponse::Unauthorized().json(ChannelsListResponse {
                success: false,
                channels: None,
                error: Some("No authorization token provided".to_string()),
            }));
        }
    };

    match state.db.validate_session(&token) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(HttpResponse::Unauthorized().json(ChannelsListResponse {
            success: false,
            channels: None,
            error: Some("Invalid or expired session".to_string()),
        })),
        Err(e) => {
            log::error!("Failed to validate session: {}", e);
            Err(HttpResponse::InternalServerError().json(ChannelsListResponse {
                success: false,
                channels: None,
                error: Some("Internal server error".to_string()),
            }))
        }
    }
}

async fn list_channels(state: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    match state.db.list_channels() {
        Ok(channels) => {
            let channel_manager = state.gateway.channel_manager();
            let responses: Vec<ChannelResponse> = channels
                .into_iter()
                .map(|ch| {
                    let running = channel_manager.is_running(ch.id);
                    ChannelResponse::from(ch).with_running(running)
                })
                .collect();

            HttpResponse::Ok().json(ChannelsListResponse {
                success: true,
                channels: Some(responses),
                error: None,
            })
        }
        Err(e) => {
            log::error!("Failed to list channels: {}", e);
            HttpResponse::InternalServerError().json(ChannelsListResponse {
                success: false,
                channels: None,
                error: Some("Failed to retrieve channels".to_string()),
            })
        }
    }
}

async fn get_channel(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<i64>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    let id = path.into_inner();

    match state.db.get_channel(id) {
        Ok(Some(channel)) => {
            let channel_manager = state.gateway.channel_manager();
            let running = channel_manager.is_running(channel.id);
            let response = ChannelResponse::from(channel).with_running(running);

            HttpResponse::Ok().json(ChannelOperationResponse {
                success: true,
                channel: Some(response),
                error: None,
            })
        }
        Ok(None) => HttpResponse::NotFound().json(ChannelOperationResponse {
            success: false,
            channel: None,
            error: Some("Channel not found".to_string()),
        }),
        Err(e) => {
            log::error!("Failed to get channel: {}", e);
            HttpResponse::InternalServerError().json(ChannelOperationResponse {
                success: false,
                channel: None,
                error: Some("Failed to retrieve channel".to_string()),
            })
        }
    }
}

async fn create_channel(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<CreateChannelRequest>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    // Validate channel type
    if ChannelType::from_str(&body.channel_type).is_none() {
        return HttpResponse::BadRequest().json(ChannelOperationResponse {
            success: false,
            channel: None,
            error: Some("Invalid channel type. Valid options: telegram, slack, discord, twitter, external_channel".to_string()),
        });
    }

    // Validate name is not empty
    if body.name.trim().is_empty() {
        return HttpResponse::BadRequest().json(ChannelOperationResponse {
            success: false,
            channel: None,
            error: Some("Channel name cannot be empty".to_string()),
        });
    }

    let bot_token = body.bot_token.as_deref().unwrap_or("");

    // Safe mode is controlled per-channel via channel settings (default: off for external channels)
    let safe_mode = false;

    match state.db.create_channel_with_safe_mode(
        &body.channel_type,
        &body.name,
        bot_token,
        body.app_token.as_deref(),
        safe_mode,
    ) {
        Ok(channel) => HttpResponse::Created().json(ChannelOperationResponse {
            success: true,
            channel: Some(channel.into()),
            error: None,
        }),
        Err(e) => {
            log::error!("Failed to create channel: {}", e);

            // Check for unique constraint violation
            let error_msg = if e.to_string().contains("UNIQUE constraint failed") {
                "A channel with this type and name already exists".to_string()
            } else {
                "Failed to create channel".to_string()
            };

            HttpResponse::BadRequest().json(ChannelOperationResponse {
                success: false,
                channel: None,
                error: Some(error_msg),
            })
        }
    }
}

/// Create a safe mode channel with rate limiting
///
/// Enforces two rate limits:
/// 1. Global: Max 1 channel creation per second (queue-based)
/// 2. Per-user: Max X queries per 10 minutes (configurable in Bot Settings)
async fn create_safe_mode_channel(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<CreateSafeModeChannelRequest>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    // Validate channel type
    if ChannelType::from_str(&body.channel_type).is_none() {
        return HttpResponse::BadRequest().json(SafeModeChannelResponse {
            success: false,
            channel: None,
            error: Some("Invalid channel type. Valid options: telegram, slack, discord, twitter".to_string()),
            queue_length: state.safe_mode_rate_limiter.queue_len(),
            next_slot_ms: state.safe_mode_rate_limiter.time_until_available_ms(),
        });
    }

    // Validate name is not empty
    if body.name.trim().is_empty() {
        return HttpResponse::BadRequest().json(SafeModeChannelResponse {
            success: false,
            channel: None,
            error: Some("Channel name cannot be empty".to_string()),
            queue_length: state.safe_mode_rate_limiter.queue_len(),
            next_slot_ms: state.safe_mode_rate_limiter.time_until_available_ms(),
        });
    }

    // Validate user_id is not empty
    if body.user_id.trim().is_empty() {
        return HttpResponse::BadRequest().json(SafeModeChannelResponse {
            success: false,
            channel: None,
            error: Some("user_id cannot be empty".to_string()),
            queue_length: state.safe_mode_rate_limiter.queue_len(),
            next_slot_ms: state.safe_mode_rate_limiter.time_until_available_ms(),
        });
    }

    // Validate platform is not empty
    if body.platform.trim().is_empty() {
        return HttpResponse::BadRequest().json(SafeModeChannelResponse {
            success: false,
            channel: None,
            error: Some("platform cannot be empty".to_string()),
            queue_length: state.safe_mode_rate_limiter.queue_len(),
            next_slot_ms: state.safe_mode_rate_limiter.time_until_available_ms(),
        });
    }

    let bot_token = body.bot_token.as_deref().unwrap_or("");

    log::info!(
        "[SAFE_MODE_CHANNEL] Creating safe mode channel '{}' (type: {}) for user {} on {}, queue_len: {}",
        body.name,
        body.channel_type,
        body.user_id,
        body.platform,
        state.safe_mode_rate_limiter.queue_len()
    );

    // Use the rate limiter to create the channel (may queue if rate limited)
    match state.safe_mode_rate_limiter.create_safe_mode_channel(
        &body.channel_type,
        &body.name,
        bot_token,
        body.app_token.as_deref(),
        &body.user_id,
        &body.platform,
    ).await {
        Ok(channel) => {
            log::info!(
                "[SAFE_MODE_CHANNEL] Successfully created safe mode channel '{}' (id: {}) for user {} on {}",
                channel.name,
                channel.id,
                body.user_id,
                body.platform
            );
            HttpResponse::Created().json(SafeModeChannelResponse {
                success: true,
                channel: Some(channel.into()),
                error: None,
                queue_length: state.safe_mode_rate_limiter.queue_len(),
                next_slot_ms: state.safe_mode_rate_limiter.time_until_available_ms(),
            })
        }
        Err(e) => {
            log::warn!("[SAFE_MODE_CHANNEL] Failed to create safe mode channel: {}", e);

            // Check for queue full or rate limit error
            if e.contains("queue full") || e.contains("Rate limit exceeded") {
                return HttpResponse::TooManyRequests().json(SafeModeChannelResponse {
                    success: false,
                    channel: None,
                    error: Some(e),
                    queue_length: state.safe_mode_rate_limiter.queue_len(),
                    next_slot_ms: state.safe_mode_rate_limiter.time_until_available_ms(),
                });
            }

            HttpResponse::BadRequest().json(SafeModeChannelResponse {
                success: false,
                channel: None,
                error: Some(e),
                queue_length: state.safe_mode_rate_limiter.queue_len(),
                next_slot_ms: state.safe_mode_rate_limiter.time_until_available_ms(),
            })
        }
    }
}

/// Get rate limiter status for safe mode channels
async fn safe_mode_rate_limit_status(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "queue_length": state.safe_mode_rate_limiter.queue_len(),
        "next_slot_ms": state.safe_mode_rate_limiter.time_until_available_ms(),
        "is_processing": state.safe_mode_rate_limiter.is_processing(),
    }))
}

async fn update_channel(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<UpdateChannelRequest>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    let id = path.into_inner();

    // Validate name if provided
    if let Some(ref name) = body.name {
        if name.trim().is_empty() {
            return HttpResponse::BadRequest().json(ChannelOperationResponse {
                success: false,
                channel: None,
                error: Some("Channel name cannot be empty".to_string()),
            });
        }
    }

    // Handle app_token: None means don't update, Some(value) means set to value
    let app_token_update: Option<Option<&str>> = body.app_token.as_ref().map(|t| Some(t.as_str()));

    match state.db.update_channel(
        id,
        body.name.as_deref(),
        body.enabled,
        body.bot_token.as_deref(),
        app_token_update,
    ) {
        Ok(Some(channel)) => {
            let channel_manager = state.gateway.channel_manager();
            let running = channel_manager.is_running(channel.id);
            let response = ChannelResponse::from(channel).with_running(running);

            HttpResponse::Ok().json(ChannelOperationResponse {
                success: true,
                channel: Some(response),
                error: None,
            })
        }
        Ok(None) => HttpResponse::NotFound().json(ChannelOperationResponse {
            success: false,
            channel: None,
            error: Some("Channel not found".to_string()),
        }),
        Err(e) => {
            log::error!("Failed to update channel: {}", e);
            HttpResponse::InternalServerError().json(ChannelOperationResponse {
                success: false,
                channel: None,
                error: Some("Failed to update channel".to_string()),
            })
        }
    }
}

async fn delete_channel(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<i64>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    let id = path.into_inner();

    // Stop the channel if it's running
    let channel_manager = state.gateway.channel_manager();
    if channel_manager.is_running(id) {
        let _ = channel_manager.stop_channel(id).await;
    }

    match state.db.delete_channel(id) {
        Ok(deleted) => {
            if deleted {
                HttpResponse::Ok().json(ChannelOperationResponse {
                    success: true,
                    channel: None,
                    error: None,
                })
            } else {
                HttpResponse::NotFound().json(ChannelOperationResponse {
                    success: false,
                    channel: None,
                    error: Some("Channel not found".to_string()),
                })
            }
        }
        Err(e) => {
            log::error!("Failed to delete channel: {}", e);
            HttpResponse::InternalServerError().json(ChannelOperationResponse {
                success: false,
                channel: None,
                error: Some("Failed to delete channel".to_string()),
            })
        }
    }
}

async fn start_channel(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<i64>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    let id = path.into_inner();

    // Get channel from database
    let channel = match state.db.get_channel(id) {
        Ok(Some(ch)) => ch,
        Ok(None) => {
            return HttpResponse::NotFound().json(ChannelOperationResponse {
                success: false,
                channel: None,
                error: Some("Channel not found".to_string()),
            });
        }
        Err(e) => {
            log::error!("Failed to get channel: {}", e);
            return HttpResponse::InternalServerError().json(ChannelOperationResponse {
                success: false,
                channel: None,
                error: Some("Failed to retrieve channel".to_string()),
            });
        }
    };

    // Start the channel
    let channel_manager = state.gateway.channel_manager();
    match channel_manager.start_channel(channel.clone()).await {
        Ok(()) => {
            // Update enabled status in database
            let _ = state.db.set_channel_enabled(id, true);

            let response = ChannelResponse::from(channel).with_running(true);
            HttpResponse::Ok().json(ChannelOperationResponse {
                success: true,
                channel: Some(response),
                error: None,
            })
        }
        Err(e) => {
            log::error!("Failed to start channel: {}", e);
            HttpResponse::BadRequest().json(ChannelOperationResponse {
                success: false,
                channel: None,
                error: Some(e),
            })
        }
    }
}

async fn stop_channel(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<i64>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    let id = path.into_inner();

    // Get channel from database
    let channel = match state.db.get_channel(id) {
        Ok(Some(ch)) => ch,
        Ok(None) => {
            return HttpResponse::NotFound().json(ChannelOperationResponse {
                success: false,
                channel: None,
                error: Some("Channel not found".to_string()),
            });
        }
        Err(e) => {
            log::error!("Failed to get channel: {}", e);
            return HttpResponse::InternalServerError().json(ChannelOperationResponse {
                success: false,
                channel: None,
                error: Some("Failed to retrieve channel".to_string()),
            });
        }
    };

    // Stop the channel
    let channel_manager = state.gateway.channel_manager();
    match channel_manager.stop_channel(id).await {
        Ok(()) => {
            // Update enabled status in database
            let _ = state.db.set_channel_enabled(id, false);

            let response = ChannelResponse::from(channel).with_running(false);
            HttpResponse::Ok().json(ChannelOperationResponse {
                success: true,
                channel: Some(response),
                error: None,
            })
        }
        Err(e) => {
            log::error!("Failed to stop channel: {}", e);
            HttpResponse::BadRequest().json(ChannelOperationResponse {
                success: false,
                channel: None,
                error: Some(e),
            })
        }
    }
}

/// Get available settings schema for a channel type
async fn get_settings_schema(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<String>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    let channel_type_str = path.into_inner();
    let channel_type = match ChannelType::from_str(&channel_type_str) {
        Some(ct) => ct,
        None => {
            return HttpResponse::BadRequest().json(ChannelSettingsSchemaResponse {
                success: false,
                channel_type: channel_type_str,
                settings: vec![],
            });
        }
    };

    let settings = get_settings_for_channel_type(channel_type);

    HttpResponse::Ok().json(ChannelSettingsSchemaResponse {
        success: true,
        channel_type: channel_type_str,
        settings,
    })
}

/// Get settings for a channel
async fn get_channel_settings(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<i64>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    let id = path.into_inner();

    // Verify channel exists
    match state.db.get_channel(id) {
        Ok(Some(_)) => {}
        Ok(None) => {
            return HttpResponse::NotFound().json(ChannelSettingsResponse {
                success: false,
                settings: vec![],
            });
        }
        Err(e) => {
            log::error!("Failed to get channel: {}", e);
            return HttpResponse::InternalServerError().json(ChannelSettingsResponse {
                success: false,
                settings: vec![],
            });
        }
    }

    match state.db.get_channel_settings(id) {
        Ok(settings) => HttpResponse::Ok().json(ChannelSettingsResponse {
            success: true,
            settings,
        }),
        Err(e) => {
            log::error!("Failed to get channel settings: {}", e);
            HttpResponse::InternalServerError().json(ChannelSettingsResponse {
                success: false,
                settings: vec![],
            })
        }
    }
}

/// Update settings for a channel
async fn update_channel_settings(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<UpdateChannelSettingsRequest>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    let id = path.into_inner();

    // Verify channel exists
    match state.db.get_channel(id) {
        Ok(Some(_)) => {}
        Ok(None) => {
            return HttpResponse::NotFound().json(ChannelSettingsResponse {
                success: false,
                settings: vec![],
            });
        }
        Err(e) => {
            log::error!("Failed to get channel: {}", e);
            return HttpResponse::InternalServerError().json(ChannelSettingsResponse {
                success: false,
                settings: vec![],
            });
        }
    }

    // Convert to tuple format for bulk update
    let settings_tuples: Vec<(String, String)> = body
        .settings
        .iter()
        .map(|s| (s.key.clone(), s.value.clone()))
        .collect();

    match state.db.update_channel_settings(id, &settings_tuples) {
        Ok(()) => {
            // Return updated settings
            match state.db.get_channel_settings(id) {
                Ok(settings) => HttpResponse::Ok().json(ChannelSettingsResponse {
                    success: true,
                    settings,
                }),
                Err(e) => {
                    log::error!("Failed to get updated channel settings: {}", e);
                    HttpResponse::InternalServerError().json(ChannelSettingsResponse {
                        success: false,
                        settings: vec![],
                    })
                }
            }
        }
        Err(e) => {
            log::error!("Failed to update channel settings: {}", e);
            HttpResponse::InternalServerError().json(ChannelSettingsResponse {
                success: false,
                settings: vec![],
            })
        }
    }
}
