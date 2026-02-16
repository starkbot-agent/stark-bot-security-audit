//! Transaction confirmation API endpoints
//!
//! Handles confirm/cancel requests from the frontend for pending blockchain transactions.

use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct ConfirmationRequest {
    pub channel_id: i64,
}

#[derive(Debug, Serialize)]
pub struct ConfirmationResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct PendingConfirmationResponse {
    pub has_pending: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmation: Option<PendingConfirmationInfo>,
}

#[derive(Debug, Serialize)]
pub struct PendingConfirmationInfo {
    pub id: String,
    pub channel_id: i64,
    pub tool_name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/confirmation")
            .route("/pending/{channel_id}", web::get().to(get_pending))
            .route("/confirm", web::post().to(confirm))
            .route("/cancel", web::post().to(cancel))
    );
}

/// Check if there's a pending confirmation for a channel
async fn get_pending(
    state: web::Data<AppState>,
    path: web::Path<i64>,
    req: HttpRequest,
) -> impl Responder {
    // Validate auth
    if let Err(resp) = validate_auth(&state, &req) {
        return resp;
    }

    let channel_id = path.into_inner();

    match state.dispatcher.get_pending_confirmation(channel_id) {
        Some(pending) => {
            HttpResponse::Ok().json(PendingConfirmationResponse {
                has_pending: true,
                confirmation: Some(PendingConfirmationInfo {
                    id: pending.id,
                    channel_id: pending.channel_id,
                    tool_name: pending.tool_name,
                    description: pending.description,
                    parameters: pending.arguments,
                }),
            })
        }
        None => {
            HttpResponse::Ok().json(PendingConfirmationResponse {
                has_pending: false,
                confirmation: None,
            })
        }
    }
}

/// Confirm a pending transaction
async fn confirm(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<ConfirmationRequest>,
) -> impl Responder {
    // Validate auth
    if let Err(resp) = validate_auth(&state, &req) {
        return resp;
    }

    match state.dispatcher.api_confirm_transaction(body.channel_id).await {
        Ok(result) => {
            HttpResponse::Ok().json(ConfirmationResponse {
                success: true,
                message: Some("Transaction confirmed and executed".to_string()),
                error: None,
                result: Some(result),
            })
        }
        Err(error) => {
            HttpResponse::BadRequest().json(ConfirmationResponse {
                success: false,
                message: None,
                error: Some(error),
                result: None,
            })
        }
    }
}

/// Cancel a pending transaction
async fn cancel(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<ConfirmationRequest>,
) -> impl Responder {
    // Validate auth
    if let Err(resp) = validate_auth(&state, &req) {
        return resp;
    }

    match state.dispatcher.api_cancel_transaction(body.channel_id) {
        Ok(description) => {
            HttpResponse::Ok().json(ConfirmationResponse {
                success: true,
                message: Some(format!("Transaction cancelled: {}", description)),
                error: None,
                result: None,
            })
        }
        Err(error) => {
            HttpResponse::BadRequest().json(ConfirmationResponse {
                success: false,
                message: None,
                error: Some(error),
                result: None,
            })
        }
    }
}

/// Validate authorization header
fn validate_auth(state: &web::Data<AppState>, req: &HttpRequest) -> Result<(), HttpResponse> {
    let token = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.trim_start_matches("Bearer ").to_string());

    let token = match token {
        Some(t) => t,
        None => {
            return Err(HttpResponse::Unauthorized().json(ConfirmationResponse {
                success: false,
                message: None,
                error: Some("No authorization token provided".to_string()),
                result: None,
            }));
        }
    };

    match state.db.validate_session(&token) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => {
            Err(HttpResponse::Unauthorized().json(ConfirmationResponse {
                success: false,
                message: None,
                error: Some("Invalid or expired session".to_string()),
                result: None,
            }))
        }
        Err(e) => {
            log::error!("Failed to validate session: {}", e);
            Err(HttpResponse::InternalServerError().json(ConfirmationResponse {
                success: false,
                message: None,
                error: Some("Internal server error".to_string()),
                result: None,
            }))
        }
    }
}
