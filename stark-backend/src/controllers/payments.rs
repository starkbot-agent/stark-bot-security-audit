//! x402 Payment History API endpoints

use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};

use crate::AppState;

#[derive(Debug, Serialize)]
struct PaymentResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    payments: Option<Vec<PaymentInfo>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    total: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct PaymentInfo {
    id: i64,
    channel_id: Option<i64>,
    tool_name: Option<String>,
    resource: Option<String>,
    amount: String,
    amount_formatted: String,
    asset: String,
    pay_to: String,
    tx_hash: Option<String>,
    status: String,
    feedback_submitted: bool,
    created_at: String,
}

#[derive(Debug, Serialize)]
struct PaymentSummary {
    total_payments: i64,
    total_usdc_spent: String,
    payments_with_feedback: i64,
    payments_without_feedback: i64,
}

#[derive(Debug, Deserialize)]
pub struct PaymentListQuery {
    channel_id: Option<i64>,
    limit: Option<i64>,
    offset: Option<i64>,
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/payments")
            .route("", web::get().to(list_payments))
            .route("/summary", web::get().to(get_summary))
            .route("/{id}", web::get().to(get_payment))
    );
}

/// List x402 payments
async fn list_payments(
    state: web::Data<AppState>,
    req: HttpRequest,
    query: web::Query<PaymentListQuery>,
) -> impl Responder {
    // Validate auth
    if let Err(resp) = validate_auth(&state, &req) {
        return resp;
    }

    let conn = state.db.conn();
    let limit = query.limit.unwrap_or(50).min(100);
    let offset = query.offset.unwrap_or(0);

    // Build query based on filters
    let (sql, params): (&str, Vec<Box<dyn rusqlite::ToSql>>) = if let Some(channel_id) = query.channel_id {
        (
            "SELECT id, channel_id, tool_name, resource, amount, amount_formatted, asset, pay_to, tx_hash, status, feedback_submitted, created_at
             FROM x402_payments WHERE channel_id = ?1
             ORDER BY created_at DESC LIMIT ?2 OFFSET ?3",
            vec![Box::new(channel_id), Box::new(limit), Box::new(offset)]
        )
    } else {
        (
            "SELECT id, channel_id, tool_name, resource, amount, amount_formatted, asset, pay_to, tx_hash, status, feedback_submitted, created_at
             FROM x402_payments
             ORDER BY created_at DESC LIMIT ?1 OFFSET ?2",
            vec![Box::new(limit), Box::new(offset)]
        )
    };

    let mut stmt = match conn.prepare(sql) {
        Ok(s) => s,
        Err(e) => {
            return HttpResponse::InternalServerError().json(PaymentResponse {
                success: false,
                payments: None,
                total: None,
                error: Some(format!("Database error: {}", e)),
            });
        }
    };

    let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

    let payments: Vec<PaymentInfo> = match stmt.query_map(params_refs.as_slice(), |row| {
        Ok(PaymentInfo {
            id: row.get(0)?,
            channel_id: row.get(1)?,
            tool_name: row.get(2)?,
            resource: row.get(3)?,
            amount: row.get(4)?,
            amount_formatted: row.get(5)?,
            asset: row.get(6)?,
            pay_to: row.get(7)?,
            tx_hash: row.get(8)?,
            status: row.get::<_, String>(9).unwrap_or_else(|_| "pending".to_string()),
            feedback_submitted: row.get::<_, i64>(10)? != 0,
            created_at: row.get(11)?,
        })
    }) {
        Ok(rows) => rows.filter_map(|r| r.ok()).collect(),
        Err(e) => {
            return HttpResponse::InternalServerError().json(PaymentResponse {
                success: false,
                payments: None,
                total: None,
                error: Some(format!("Query error: {}", e)),
            });
        }
    };

    // Get total count
    let total: i64 = if query.channel_id.is_some() {
        conn.query_row(
            "SELECT COUNT(*) FROM x402_payments WHERE channel_id = ?1",
            [query.channel_id.unwrap()],
            |row| row.get(0),
        )
        .unwrap_or(0)
    } else {
        conn.query_row("SELECT COUNT(*) FROM x402_payments", [], |row| row.get(0))
            .unwrap_or(0)
    };

    HttpResponse::Ok().json(PaymentResponse {
        success: true,
        payments: Some(payments),
        total: Some(total),
        error: None,
    })
}

/// Get payment summary
async fn get_summary(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> impl Responder {
    // Validate auth
    if let Err(resp) = validate_auth(&state, &req) {
        return resp;
    }

    let conn = state.db.conn();

    let total_payments: i64 = conn
        .query_row("SELECT COUNT(*) FROM x402_payments", [], |row| row.get(0))
        .unwrap_or(0);

    // Sum all amounts (they're stored as strings, so we need to handle this carefully)
    let total_usdc: f64 = conn
        .query_row(
            "SELECT COALESCE(SUM(CAST(amount_formatted AS REAL)), 0) FROM x402_payments WHERE asset = 'USDC'",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0.0);

    let payments_with_feedback: i64 = conn
        .query_row(
            "SELECT COUNT(*) FROM x402_payments WHERE feedback_submitted = 1",
            [],
            |row| row.get(0),
        )
        .unwrap_or(0);

    HttpResponse::Ok().json(serde_json::json!({
        "success": true,
        "summary": {
            "total_payments": total_payments,
            "total_usdc_spent": format!("{:.6}", total_usdc),
            "payments_with_feedback": payments_with_feedback,
            "payments_without_feedback": total_payments - payments_with_feedback
        }
    }))
}

/// Get single payment
async fn get_payment(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<i64>,
) -> impl Responder {
    // Validate auth
    if let Err(resp) = validate_auth(&state, &req) {
        return resp;
    }

    let payment_id = path.into_inner();
    let conn = state.db.conn();

    let payment = conn.query_row(
        "SELECT id, channel_id, tool_name, resource, amount, amount_formatted, asset, pay_to, tx_hash, status, feedback_submitted, created_at
         FROM x402_payments WHERE id = ?1",
        [payment_id],
        |row| {
            Ok(PaymentInfo {
                id: row.get(0)?,
                channel_id: row.get(1)?,
                tool_name: row.get(2)?,
                resource: row.get(3)?,
                amount: row.get(4)?,
                amount_formatted: row.get(5)?,
                asset: row.get(6)?,
                pay_to: row.get(7)?,
                tx_hash: row.get(8)?,
                status: row.get::<_, String>(9).unwrap_or_else(|_| "pending".to_string()),
                feedback_submitted: row.get::<_, i64>(10)? != 0,
                created_at: row.get(11)?,
            })
        },
    );

    match payment {
        Ok(p) => HttpResponse::Ok().json(serde_json::json!({
            "success": true,
            "payment": p
        })),
        Err(_) => HttpResponse::NotFound().json(serde_json::json!({
            "success": false,
            "error": "Payment not found"
        })),
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
