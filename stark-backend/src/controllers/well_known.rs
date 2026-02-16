use actix_web::{web, HttpResponse, Responder};

use crate::AppState;

/// Serve the agent registration file at /.well-known/agent-registration.json
/// This is a PUBLIC endpoint (no auth) per EIP-8004 for domain verification.
/// Reads identity from the database (single source of truth).
async fn agent_registration(state: web::Data<AppState>) -> impl Responder {
    match state.db.get_agent_identity_full() {
        Some(row) => {
            let reg = row.to_registration_file();
            HttpResponse::Ok()
                .content_type("application/json")
                .json(reg)
        }
        None => {
            HttpResponse::NotFound().json(serde_json::json!({
                "error": "Agent registration not configured"
            }))
        }
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/.well-known")
            .route("/agent-registration.json", web::get().to(agent_registration)),
    );
}
