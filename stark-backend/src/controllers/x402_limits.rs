use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;
use crate::AppState;
use crate::x402::payment_limits;

/// Validate session token from request
fn validate_session(
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

/// GET /api/x402-limits — return all configured payment limits
pub async fn get_x402_limits(
    state: web::Data<AppState>,
    req: HttpRequest,
) -> impl Responder {
    if let Err(resp) = validate_session(&state, &req) {
        return resp;
    }

    let limits = payment_limits::get_all_limits();
    let entries: Vec<serde_json::Value> = limits
        .into_iter()
        .map(|(asset, limit)| {
            serde_json::json!({
                "asset": asset,
                "max_amount": limit.max_amount,
                "decimals": limit.decimals,
                "display_name": limit.display_name,
                "address": limit.address,
            })
        })
        .collect();

    HttpResponse::Ok().json(serde_json::json!({ "limits": entries }))
}

#[derive(Debug, Deserialize)]
pub struct UpdateLimitRequest {
    pub asset: String,
    pub max_amount: String,
    #[serde(default = "default_decimals")]
    pub decimals: u8,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub address: Option<String>,
}

fn default_decimals() -> u8 {
    6
}

/// PUT /api/x402-limits — update a single payment limit
pub async fn update_x402_limit(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<UpdateLimitRequest>,
) -> impl Responder {
    if let Err(resp) = validate_session(&state, &req) {
        return resp;
    }

    let r = body.into_inner();
    let asset = r.asset.to_uppercase();
    let display_name = r.display_name.unwrap_or_else(|| asset.clone());

    // Validate max_amount is a valid integer
    if r.max_amount.parse::<u128>().is_err() {
        return HttpResponse::BadRequest().json(serde_json::json!({
            "error": "max_amount must be a valid non-negative integer string"
        }));
    }

    // Persist to DB
    if let Err(e) = state.db.set_x402_payment_limit(&asset, &r.max_amount, r.decimals, &display_name, r.address.as_deref()) {
        log::error!("Failed to save x402 payment limit: {}", e);
        return HttpResponse::InternalServerError().json(serde_json::json!({
            "error": format!("Database error: {}", e)
        }));
    }

    // Update in-memory global
    payment_limits::set_limit(&asset, &r.max_amount, r.decimals, &display_name, r.address.as_deref());

    log::info!(
        "[x402_limits] Updated limit: {} max_amount={} decimals={}",
        asset, r.max_amount, r.decimals
    );

    HttpResponse::Ok().json(serde_json::json!({
        "asset": asset,
        "max_amount": r.max_amount,
        "decimals": r.decimals,
        "display_name": display_name,
        "address": r.address,
    }))
}

/// Configure routes
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/x402-limits")
            .route("", web::get().to(get_x402_limits))
            .route("", web::put().to(update_x402_limit))
    );
}
