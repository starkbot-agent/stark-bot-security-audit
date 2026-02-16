// Session authentication middleware
// This module provides middleware for validating session tokens on protected routes.
// Currently, authentication is handled directly in controllers, but this module
// can be extended to provide a reusable middleware wrapper for protected endpoints.

use actix_web::{HttpRequest, HttpResponse};
use std::sync::Arc;

use crate::db::Database;

pub fn extract_token(req: &HttpRequest) -> Option<String> {
    req.headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.trim_start_matches("Bearer ").to_string())
}

pub async fn validate_request(db: &Arc<Database>, req: &HttpRequest) -> Result<(), HttpResponse> {
    let token = extract_token(req).ok_or_else(|| {
        HttpResponse::Unauthorized().json(serde_json::json!({
            "error": "No authorization token provided"
        }))
    })?;

    match db.validate_session(&token) {
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
