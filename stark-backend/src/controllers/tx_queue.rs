//! Transaction queue API endpoints
//!
//! Provides REST API access to the transaction queue for the frontend.

use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};

use crate::AppState;
use crate::tx_queue::{QueuedTxStatus, QueuedTxSummary};

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
        web::scope("/api/tx-queue")
            .route("", web::get().to(list_transactions))
            .route("/pending", web::get().to(list_pending))
            .route("/{uuid}", web::get().to(get_transaction)),
    );
}

/// Query parameters for listing transactions
#[derive(Debug, Deserialize)]
pub struct ListParams {
    status: Option<String>,
    limit: Option<usize>,
}

/// Response for listing transactions
#[derive(Debug, Serialize)]
pub struct ListResponse {
    success: bool,
    transactions: Vec<QueuedTxSummary>,
    total: usize,
    pending_count: usize,
    confirmed_count: usize,
    failed_count: usize,
}

/// Response for a single transaction
#[derive(Debug, Serialize)]
pub struct TransactionResponse {
    success: bool,
    transaction: Option<QueuedTxSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// List all transactions with optional filters
async fn list_transactions(
    state: web::Data<AppState>,
    req: HttpRequest,
    query: web::Query<ListParams>,
) -> impl Responder {
    if let Err(resp) = validate_session(&state, &req) {
        return resp;
    }

    let tx_queue = &state.tx_queue;
    let limit = query.limit.unwrap_or(50).min(100);

    // Parse status filter if provided
    let status_filter: Option<QueuedTxStatus> = query.status.as_ref().and_then(|s| {
        match s.to_lowercase().as_str() {
            "pending" => Some(QueuedTxStatus::Pending),
            "broadcasting" => Some(QueuedTxStatus::Broadcasting),
            "broadcast" => Some(QueuedTxStatus::Broadcast),
            "confirmed" => Some(QueuedTxStatus::Confirmed),
            "failed" => Some(QueuedTxStatus::Failed),
            "expired" => Some(QueuedTxStatus::Expired),
            _ => None,
        }
    });

    // Get transactions based on filter
    let transactions = if let Some(status) = status_filter {
        tx_queue.list_by_status(status)
    } else {
        tx_queue.list_recent(limit)
    };

    // Limit results
    let transactions: Vec<_> = transactions.into_iter().take(limit).collect();
    let total = transactions.len();

    // Count by status
    let pending_count = tx_queue.count_by_status(QueuedTxStatus::Pending);
    let confirmed_count = tx_queue.count_by_status(QueuedTxStatus::Confirmed);
    let failed_count = tx_queue.count_by_status(QueuedTxStatus::Failed);

    HttpResponse::Ok().json(ListResponse {
        success: true,
        transactions,
        total,
        pending_count,
        confirmed_count,
        failed_count,
    })
}

/// List only pending transactions
async fn list_pending(state: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    if let Err(resp) = validate_session(&state, &req) {
        return resp;
    }

    let tx_queue = &state.tx_queue;

    let transactions = tx_queue.list_pending();
    let total = transactions.len();

    let pending_count = total;
    let confirmed_count = tx_queue.count_by_status(QueuedTxStatus::Confirmed);
    let failed_count = tx_queue.count_by_status(QueuedTxStatus::Failed);

    HttpResponse::Ok().json(ListResponse {
        success: true,
        transactions,
        total,
        pending_count,
        confirmed_count,
        failed_count,
    })
}

/// Get a specific transaction by UUID
async fn get_transaction(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<String>,
) -> impl Responder {
    if let Err(resp) = validate_session(&state, &req) {
        return resp;
    }

    let uuid = path.into_inner();
    let tx_queue = &state.tx_queue;

    match tx_queue.get_summary(&uuid) {
        Some(transaction) => HttpResponse::Ok().json(TransactionResponse {
            success: true,
            transaction: Some(transaction),
            error: None,
        }),
        None => HttpResponse::NotFound().json(TransactionResponse {
            success: false,
            transaction: None,
            error: Some(format!("Transaction with UUID '{}' not found", uuid)),
        }),
    }
}
