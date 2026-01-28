use actix_web::{web, HttpRequest, HttpResponse, Responder};
use chrono::Utc;
use ethers::core::types::Signature;
use ethers::utils::hash_message;
use serde::{Deserialize, Serialize};

use crate::AppState;

const SERVICE_NAME: &str = "StarkBot";

#[derive(Deserialize)]
pub struct GenerateChallengeRequest {
    public_address: String,
}

#[derive(Serialize)]
pub struct ChallengeResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    challenge: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Deserialize)]
pub struct ValidateAuthRequest {
    public_address: String,
    challenge: String,
    signature: String,
}

#[derive(Serialize)]
pub struct LoginResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Deserialize)]
pub struct LogoutRequest {
    token: String,
}

#[derive(Serialize)]
pub struct LogoutResponse {
    success: bool,
}

#[derive(Serialize)]
pub struct ValidateResponse {
    valid: bool,
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/auth")
            .route("/generate_challenge", web::post().to(generate_challenge))
            .route("/validate_auth", web::post().to(validate_auth))
            .route("/logout", web::post().to(logout))
            .route("/validate", web::get().to(validate)),
    );
}

fn generate_challenge_text(public_address: &str, unix_timestamp: i64) -> String {
    format!(
        "Signing in to {} as {} at {}",
        SERVICE_NAME,
        public_address.to_lowercase(),
        unix_timestamp
    )
}

fn recover_address(msg: &str, signature: &str) -> Option<String> {
    let sig_bytes = hex::decode(signature.strip_prefix("0x").unwrap_or(signature)).ok()?;
    let sig = Signature::try_from(sig_bytes.as_slice()).ok()?;

    let msg_hash = hash_message(msg);
    let recovered = sig.recover(msg_hash).ok()?;

    Some(format!("{:?}", recovered).to_lowercase())
}

async fn generate_challenge(
    state: web::Data<AppState>,
    body: web::Json<GenerateChallengeRequest>,
) -> impl Responder {
    let public_address = body.public_address.trim().to_lowercase();

    // Validate it looks like an Ethereum address
    if !public_address.starts_with("0x") || public_address.len() != 42 {
        return HttpResponse::BadRequest().json(ChallengeResponse {
            success: false,
            challenge: None,
            error: Some("Invalid public address".to_string()),
        });
    }

    let unix_timestamp = Utc::now().timestamp();
    let challenge = generate_challenge_text(&public_address, unix_timestamp);

    match state.db.create_or_update_challenge(&public_address, &challenge) {
        Ok(_) => HttpResponse::Ok().json(ChallengeResponse {
            success: true,
            challenge: Some(challenge),
            error: None,
        }),
        Err(e) => {
            log::error!("Failed to create challenge: {}", e);
            HttpResponse::InternalServerError().json(ChallengeResponse {
                success: false,
                challenge: None,
                error: Some("Database error".to_string()),
            })
        }
    }
}

async fn validate_auth(
    state: web::Data<AppState>,
    body: web::Json<ValidateAuthRequest>,
) -> impl Responder {
    let public_address = body.public_address.trim().to_lowercase();
    let challenge = &body.challenge;
    let signature = &body.signature;

    // Validate it looks like an Ethereum address
    if !public_address.starts_with("0x") || public_address.len() != 42 {
        return HttpResponse::BadRequest().json(LoginResponse {
            success: false,
            token: None,
            expires_at: None,
            error: Some("Invalid public address".to_string()),
        });
    }

    // Check that this address is the admin address
    let admin_address = state.config.login_admin_public_address.to_lowercase();
    if public_address != admin_address {
        return HttpResponse::Unauthorized().json(LoginResponse {
            success: false,
            token: None,
            expires_at: None,
            error: Some("Unauthorized wallet address".to_string()),
        });
    }

    // Verify the challenge exists and matches
    match state.db.validate_challenge(&public_address, challenge) {
        Ok(true) => {}
        Ok(false) => {
            return HttpResponse::Unauthorized().json(LoginResponse {
                success: false,
                token: None,
                expires_at: None,
                error: Some("No active challenge found or challenge mismatch".to_string()),
            });
        }
        Err(e) => {
            log::error!("Failed to validate challenge: {}", e);
            return HttpResponse::InternalServerError().json(LoginResponse {
                success: false,
                token: None,
                expires_at: None,
                error: Some("Database error".to_string()),
            });
        }
    }

    // Verify signature
    let recovered_address = recover_address(challenge, signature);
    if recovered_address.as_deref() != Some(public_address.as_str()) {
        return HttpResponse::Unauthorized().json(LoginResponse {
            success: false,
            token: None,
            expires_at: None,
            error: Some("Invalid signature".to_string()),
        });
    }

    // Delete the used challenge
    let _ = state.db.delete_challenge(&public_address);

    // Create session
    match state.db.create_session_for_address(Some(&public_address)) {
        Ok(session) => HttpResponse::Ok().json(LoginResponse {
            success: true,
            token: Some(session.token),
            expires_at: Some(session.expires_at.timestamp()),
            error: None,
        }),
        Err(e) => {
            log::error!("Failed to create session: {}", e);
            HttpResponse::InternalServerError().json(LoginResponse {
                success: false,
                token: None,
                expires_at: None,
                error: Some("Failed to create session".to_string()),
            })
        }
    }
}

async fn logout(state: web::Data<AppState>, body: web::Json<LogoutRequest>) -> impl Responder {
    match state.db.delete_session(&body.token) {
        Ok(_) => HttpResponse::Ok().json(LogoutResponse { success: true }),
        Err(e) => {
            log::error!("Failed to delete session: {}", e);
            HttpResponse::InternalServerError().json(LogoutResponse { success: false })
        }
    }
}

async fn validate(state: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    let token = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.trim_start_matches("Bearer ").to_string());

    let token = match token {
        Some(t) => t,
        None => {
            return HttpResponse::Ok().json(ValidateResponse { valid: false });
        }
    };

    match state.db.validate_session(&token) {
        Ok(Some(_)) => HttpResponse::Ok().json(ValidateResponse { valid: true }),
        Ok(None) => HttpResponse::Ok().json(ValidateResponse { valid: false }),
        Err(e) => {
            log::error!("Failed to validate session: {}", e);
            HttpResponse::Ok().json(ValidateResponse { valid: false })
        }
    }
}
