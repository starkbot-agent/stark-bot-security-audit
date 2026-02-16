use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::Serialize;

use crate::AppState;

#[derive(Serialize)]
pub struct DashboardData {
    message: String,
    timestamp: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    error: String,
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(web::resource("/api/dashboard").route(web::get().to(get_dashboard)));
}

async fn get_dashboard(state: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    let token = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.trim_start_matches("Bearer ").to_string());

    let token = match token {
        Some(t) => t,
        None => {
            return HttpResponse::Unauthorized().json(ErrorResponse {
                error: "No authorization token provided".to_string(),
            });
        }
    };

    match state.db.validate_session(&token) {
        Ok(Some(_session)) => HttpResponse::Ok().json(DashboardData {
            message: "Welcome to StarkBot Dashboard!".to_string(),
            timestamp: chrono::Utc::now().to_rfc3339(),
        }),
        Ok(None) => HttpResponse::Unauthorized().json(ErrorResponse {
            error: "Invalid or expired session".to_string(),
        }),
        Err(e) => {
            log::error!("Failed to validate session: {}", e);
            HttpResponse::InternalServerError().json(ErrorResponse {
                error: "Internal server error".to_string(),
            })
        }
    }
}
