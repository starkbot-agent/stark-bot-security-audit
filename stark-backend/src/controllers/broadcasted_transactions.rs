//! Broadcasted transactions API endpoints
//!
//! Provides REST API access to the persistent broadcast history.

use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};

use crate::db::tables::broadcasted_transactions::BroadcastedTransaction;
use crate::AppState;

/// Validate session token from request
fn validate_session(state: &web::Data<AppState>, req: &HttpRequest) -> Result<(), HttpResponse> {
    let token = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.trim_start_matches("Bearer ").to_string());

    let token = match token {
        Some(t) => t,
        None => {
            return Err(HttpResponse::Unauthorized().json(serde_json::json!({
                "success": false,
                "error": "No authorization token provided"
            })));
        }
    };

    match state.db.validate_session(&token) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(HttpResponse::Unauthorized().json(serde_json::json!({
            "success": false,
            "error": "Invalid or expired session"
        }))),
        Err(e) => {
            log::error!("Failed to validate session: {}", e);
            Err(HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": "Internal server error"
            })))
        }
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/broadcasted-transactions")
            .route("", web::get().to(list_broadcasted_transactions)),
    );
}

/// Query parameters for listing broadcasted transactions
#[derive(Debug, Deserialize)]
pub struct ListParams {
    status: Option<String>,
    network: Option<String>,
    broadcast_mode: Option<String>,
    limit: Option<usize>,
}

/// Response for listing broadcasted transactions
#[derive(Debug, Serialize)]
pub struct ListResponse {
    success: bool,
    transactions: Vec<BroadcastedTransaction>,
    total: usize,
}

/// List broadcasted transactions with optional filters
async fn list_broadcasted_transactions(
    state: web::Data<AppState>,
    req: HttpRequest,
    query: web::Query<ListParams>,
) -> impl Responder {
    if let Err(resp) = validate_session(&state, &req) {
        return resp;
    }

    let limit = query.limit.unwrap_or(100).min(500);

    match state.db.list_broadcasted_transactions(
        query.status.as_deref(),
        query.network.as_deref(),
        query.broadcast_mode.as_deref(),
        Some(limit),
    ) {
        Ok(transactions) => {
            let total = transactions.len();
            HttpResponse::Ok().json(ListResponse {
                success: true,
                transactions,
                total,
            })
        }
        Err(e) => {
            log::error!("Failed to list broadcasted transactions: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "success": false,
                "error": "Failed to fetch transactions"
            }))
        }
    }
}
