use actix_web::{web, HttpRequest, HttpResponse};
use std::sync::Arc;

use crate::models::{
    CreateCronJobRequest, CronJobResponse, HeartbeatConfigResponse,
    UpdateCronJobRequest, UpdateHeartbeatConfigRequest,
};
use crate::scheduler::Scheduler;
use crate::AppState;

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
            return Err(HttpResponse::Unauthorized().json(CronJobResponse {
                success: false,
                job: None,
                jobs: None,
                error: Some("No authorization token provided".to_string()),
            }));
        }
    };

    match state.db.validate_session(&token) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(HttpResponse::Unauthorized().json(CronJobResponse {
            success: false,
            job: None,
            jobs: None,
            error: Some("Invalid or expired session".to_string()),
        })),
        Err(e) => {
            log::error!("Failed to validate session: {}", e);
            Err(HttpResponse::InternalServerError().json(CronJobResponse {
                success: false,
                job: None,
                jobs: None,
                error: Some("Internal server error".to_string()),
            }))
        }
    }
}

/// Configure cron routes
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/cron")
            .route("/jobs", web::get().to(list_jobs))
            .route("/jobs", web::post().to(create_job))
            .route("/jobs/{id}", web::get().to(get_job))
            .route("/jobs/{id}", web::put().to(update_job))
            .route("/jobs/{id}", web::delete().to(delete_job))
            .route("/jobs/{id}/run", web::post().to(run_job))
            .route("/jobs/{id}/runs", web::get().to(get_job_runs))
            .route("/jobs/{id}/pause", web::post().to(pause_job))
            .route("/jobs/{id}/resume", web::post().to(resume_job)),
    );

    cfg.service(
        web::scope("/api/heartbeat")
            .route("/config", web::get().to(get_heartbeat_config))
            .route("/config", web::put().to(update_heartbeat_config))
            .route("/config/{channel_id}", web::get().to(get_channel_heartbeat_config))
            .route("/config/{channel_id}", web::put().to(update_channel_heartbeat_config))
            .route("/pulse_once", web::post().to(pulse_heartbeat)),
    );
}

/// List all cron jobs
async fn list_jobs(state: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    match state.db.list_cron_jobs() {
        Ok(jobs) => HttpResponse::Ok().json(CronJobResponse {
            success: true,
            job: None,
            jobs: Some(jobs),
            error: None,
        }),
        Err(e) => HttpResponse::InternalServerError().json(CronJobResponse {
            success: false,
            job: None,
            jobs: None,
            error: Some(format!("Database error: {}", e)),
        }),
    }
}

/// Create a new cron job
async fn create_job(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<CreateCronJobRequest>,
) -> HttpResponse {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    // Validate schedule type
    let valid_types = ["at", "every", "cron"];
    if !valid_types.contains(&body.schedule_type.to_lowercase().as_str()) {
        return HttpResponse::BadRequest().json(CronJobResponse {
            success: false,
            job: None,
            jobs: None,
            error: Some("Invalid schedule_type. Valid options: at, every, cron".to_string()),
        });
    }

    // Validate cron expression if type is cron
    if body.schedule_type.to_lowercase() == "cron" {
        use cron::Schedule;
        use std::str::FromStr;

        if Schedule::from_str(&body.schedule_value).is_err() {
            return HttpResponse::BadRequest().json(CronJobResponse {
                success: false,
                job: None,
                jobs: None,
                error: Some(format!("Invalid cron expression: {}", body.schedule_value)),
            });
        }
    }

    // Validate session mode
    let valid_modes = ["main", "isolated"];
    if !valid_modes.contains(&body.session_mode.to_lowercase().as_str()) {
        return HttpResponse::BadRequest().json(CronJobResponse {
            success: false,
            job: None,
            jobs: None,
            error: Some("Invalid session_mode. Valid options: main, isolated".to_string()),
        });
    }

    match state.db.create_cron_job(
        &body.name,
        body.description.as_deref(),
        &body.schedule_type,
        &body.schedule_value,
        body.timezone.as_deref(),
        &body.session_mode,
        body.message.as_deref(),
        body.system_event.as_deref(),
        body.channel_id,
        body.deliver_to.as_deref(),
        body.deliver,
        body.model_override.as_deref(),
        body.thinking_level.as_deref(),
        body.timeout_seconds,
        body.delete_after_run,
    ) {
        Ok(job) => HttpResponse::Created().json(CronJobResponse {
            success: true,
            job: Some(job),
            jobs: None,
            error: None,
        }),
        Err(e) => HttpResponse::InternalServerError().json(CronJobResponse {
            success: false,
            job: None,
            jobs: None,
            error: Some(format!("Failed to create job: {}", e)),
        }),
    }
}

/// Get a cron job by ID
async fn get_job(state: web::Data<AppState>, req: HttpRequest, path: web::Path<i64>) -> HttpResponse {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    let id = path.into_inner();

    match state.db.get_cron_job(id) {
        Ok(Some(job)) => HttpResponse::Ok().json(CronJobResponse {
            success: true,
            job: Some(job),
            jobs: None,
            error: None,
        }),
        Ok(None) => HttpResponse::NotFound().json(CronJobResponse {
            success: false,
            job: None,
            jobs: None,
            error: Some("Job not found".to_string()),
        }),
        Err(e) => HttpResponse::InternalServerError().json(CronJobResponse {
            success: false,
            job: None,
            jobs: None,
            error: Some(format!("Database error: {}", e)),
        }),
    }
}

/// Update a cron job
async fn update_job(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<UpdateCronJobRequest>,
) -> HttpResponse {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    let id = path.into_inner();

    // Validate cron expression if updating schedule
    if let (Some(schedule_type), Some(schedule_value)) =
        (&body.schedule_type, &body.schedule_value)
    {
        if schedule_type.to_lowercase() == "cron" {
            use cron::Schedule;
            use std::str::FromStr;

            if Schedule::from_str(schedule_value).is_err() {
                return HttpResponse::BadRequest().json(CronJobResponse {
                    success: false,
                    job: None,
                    jobs: None,
                    error: Some(format!("Invalid cron expression: {}", schedule_value)),
                });
            }
        }
    }

    match state.db.update_cron_job(
        id,
        body.name.as_deref(),
        body.description.as_deref(),
        body.schedule_type.as_deref(),
        body.schedule_value.as_deref(),
        body.timezone.as_deref(),
        body.session_mode.as_deref(),
        body.message.as_deref(),
        body.system_event.as_deref(),
        body.channel_id,
        body.deliver_to.as_deref(),
        body.deliver,
        body.model_override.as_deref(),
        body.thinking_level.as_deref(),
        body.timeout_seconds,
        body.delete_after_run,
        body.status.as_deref(),
    ) {
        Ok(job) => HttpResponse::Ok().json(CronJobResponse {
            success: true,
            job: Some(job),
            jobs: None,
            error: None,
        }),
        Err(e) => HttpResponse::InternalServerError().json(CronJobResponse {
            success: false,
            job: None,
            jobs: None,
            error: Some(format!("Failed to update job: {}", e)),
        }),
    }
}

/// Delete a cron job
async fn delete_job(state: web::Data<AppState>, req: HttpRequest, path: web::Path<i64>) -> HttpResponse {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    let id = path.into_inner();

    match state.db.delete_cron_job(id) {
        Ok(true) => HttpResponse::Ok().json(CronJobResponse {
            success: true,
            job: None,
            jobs: None,
            error: None,
        }),
        Ok(false) => HttpResponse::NotFound().json(CronJobResponse {
            success: false,
            job: None,
            jobs: None,
            error: Some("Job not found".to_string()),
        }),
        Err(e) => HttpResponse::InternalServerError().json(CronJobResponse {
            success: false,
            job: None,
            jobs: None,
            error: Some(format!("Failed to delete job: {}", e)),
        }),
    }
}

/// Manually run a cron job
async fn run_job(
    state: web::Data<AppState>,
    req: HttpRequest,
    scheduler: web::Data<Arc<Scheduler>>,
    path: web::Path<i64>,
) -> HttpResponse {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    let id = path.into_inner();

    // Get the job first
    let job = match state.db.get_cron_job(id) {
        Ok(Some(job)) => job,
        Ok(None) => {
            return HttpResponse::NotFound().json(CronJobResponse {
                success: false,
                job: None,
                jobs: None,
                error: Some("Job not found".to_string()),
            });
        }
        Err(e) => {
            return HttpResponse::InternalServerError().json(CronJobResponse {
                success: false,
                job: None,
                jobs: None,
                error: Some(format!("Database error: {}", e)),
            });
        }
    };

    match scheduler.run_job_now(&job.job_id).await {
        Ok(_) => HttpResponse::Ok().json(CronJobResponse {
            success: true,
            job: Some(job),
            jobs: None,
            error: None,
        }),
        Err(e) => HttpResponse::InternalServerError().json(CronJobResponse {
            success: false,
            job: None,
            jobs: None,
            error: Some(e),
        }),
    }
}

/// Get runs for a cron job
async fn get_job_runs(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<i64>,
    query: web::Query<LimitQuery>,
) -> HttpResponse {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    let id = path.into_inner();
    let limit = query.limit.unwrap_or(20);

    match state.db.get_cron_job_runs(id, limit) {
        Ok(runs) => HttpResponse::Ok().json(serde_json::json!({
            "success": true,
            "runs": runs
        })),
        Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
            "success": false,
            "error": format!("Database error: {}", e)
        })),
    }
}

#[derive(serde::Deserialize)]
struct LimitQuery {
    limit: Option<i32>,
}

/// Pause a cron job
async fn pause_job(state: web::Data<AppState>, req: HttpRequest, path: web::Path<i64>) -> HttpResponse {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    let id = path.into_inner();

    match state.db.update_cron_job(
        id,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        Some("paused"),
    ) {
        Ok(job) => HttpResponse::Ok().json(CronJobResponse {
            success: true,
            job: Some(job),
            jobs: None,
            error: None,
        }),
        Err(e) => HttpResponse::InternalServerError().json(CronJobResponse {
            success: false,
            job: None,
            jobs: None,
            error: Some(format!("Failed to pause job: {}", e)),
        }),
    }
}

/// Resume a paused cron job
async fn resume_job(state: web::Data<AppState>, req: HttpRequest, path: web::Path<i64>) -> HttpResponse {
    if let Err(resp) = validate_session_from_request(&state, &req) {
        return resp;
    }

    let id = path.into_inner();

    match state.db.update_cron_job(
        id,
        None, None, None, None, None, None, None, None, None, None, None, None, None, None, None,
        Some("active"),
    ) {
        Ok(job) => HttpResponse::Ok().json(CronJobResponse {
            success: true,
            job: Some(job),
            jobs: None,
            error: None,
        }),
        Err(e) => HttpResponse::InternalServerError().json(CronJobResponse {
            success: false,
            job: None,
            jobs: None,
            error: Some(format!("Failed to resume job: {}", e)),
        }),
    }
}

fn validate_session_for_heartbeat(
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
            return Err(HttpResponse::Unauthorized().json(HeartbeatConfigResponse {
                success: false,
                config: None,
                error: Some("No authorization token provided".to_string()),
            }));
        }
    };

    match state.db.validate_session(&token) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(HttpResponse::Unauthorized().json(HeartbeatConfigResponse {
            success: false,
            config: None,
            error: Some("Invalid or expired session".to_string()),
        })),
        Err(e) => {
            log::error!("Failed to validate session: {}", e);
            Err(HttpResponse::InternalServerError().json(HeartbeatConfigResponse {
                success: false,
                config: None,
                error: Some("Internal server error".to_string()),
            }))
        }
    }
}

/// Get global heartbeat config
async fn get_heartbeat_config(state: web::Data<AppState>, req: HttpRequest) -> HttpResponse {
    if let Err(resp) = validate_session_for_heartbeat(&state, &req) {
        return resp;
    }

    match state.db.get_or_create_heartbeat_config(None) {
        Ok(config) => HttpResponse::Ok().json(HeartbeatConfigResponse {
            success: true,
            config: Some(config),
            error: None,
        }),
        Err(e) => HttpResponse::InternalServerError().json(HeartbeatConfigResponse {
            success: false,
            config: None,
            error: Some(format!("Database error: {}", e)),
        }),
    }
}

/// Update global heartbeat config
async fn update_heartbeat_config(
    state: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<UpdateHeartbeatConfigRequest>,
) -> HttpResponse {
    log::info!("[HEARTBEAT] Update requested: enabled={:?}", body.enabled);

    if let Err(resp) = validate_session_for_heartbeat(&state, &req) {
        log::warn!("[HEARTBEAT] Session validation failed for update");
        return resp;
    }

    // Get or create first
    let config = match state.db.get_or_create_heartbeat_config(None) {
        Ok(c) => {
            log::info!("[HEARTBEAT] Got existing config id={}", c.id);
            c
        }
        Err(e) => {
            log::error!("[HEARTBEAT] Failed to get/create config: {}", e);
            return HttpResponse::InternalServerError().json(HeartbeatConfigResponse {
                success: false,
                config: None,
                error: Some(format!("Database error: {}", e)),
            });
        }
    };

    match state.db.update_heartbeat_config(
        config.id,
        body.interval_minutes,
        body.target.as_deref(),
        body.active_hours_start.as_deref(),
        body.active_hours_end.as_deref(),
        body.active_days.as_deref(),
        body.enabled,
    ) {
        Ok(updated) => {
            log::info!("[HEARTBEAT] Config updated successfully, enabled={}", updated.enabled);
            HttpResponse::Ok().json(HeartbeatConfigResponse {
                success: true,
                config: Some(updated),
                error: None,
            })
        }
        Err(e) => {
            log::error!("[HEARTBEAT] Failed to update config: {}", e);
            HttpResponse::InternalServerError().json(HeartbeatConfigResponse {
                success: false,
                config: None,
                error: Some(format!("Failed to update config: {}", e)),
            })
        }
    }
}

/// Get heartbeat config for a specific channel
async fn get_channel_heartbeat_config(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<i64>,
) -> HttpResponse {
    if let Err(resp) = validate_session_for_heartbeat(&state, &req) {
        return resp;
    }

    let channel_id = path.into_inner();

    match state.db.get_or_create_heartbeat_config(Some(channel_id)) {
        Ok(config) => HttpResponse::Ok().json(HeartbeatConfigResponse {
            success: true,
            config: Some(config),
            error: None,
        }),
        Err(e) => HttpResponse::InternalServerError().json(HeartbeatConfigResponse {
            success: false,
            config: None,
            error: Some(format!("Database error: {}", e)),
        }),
    }
}

/// Manually trigger a heartbeat pulse
async fn pulse_heartbeat(
    state: web::Data<AppState>,
    req: HttpRequest,
    scheduler: web::Data<Arc<Scheduler>>,
) -> HttpResponse {
    log::info!("[HEARTBEAT] Pulse requested");

    if let Err(resp) = validate_session_for_heartbeat(&state, &req) {
        log::warn!("[HEARTBEAT] Session validation failed for pulse");
        return resp;
    }

    // Get or create global heartbeat config
    let config = match state.db.get_or_create_heartbeat_config(None) {
        Ok(c) => {
            log::info!("[HEARTBEAT] Got config id={}, enabled={}", c.id, c.enabled);
            c
        }
        Err(e) => {
            log::error!("[HEARTBEAT] Failed to get config: {}", e);
            return HttpResponse::InternalServerError().json(HeartbeatConfigResponse {
                success: false,
                config: None,
                error: Some(format!("Database error: {}", e)),
            });
        }
    };

    log::info!("[HEARTBEAT] Running heartbeat now for config {}", config.id);

    // Fire and forget - returns immediately, heartbeat runs in background
    match scheduler.get_ref().run_heartbeat_now(config.id) {
        Ok(msg) => {
            log::info!("[HEARTBEAT] {}", msg);
            HttpResponse::Ok().json(HeartbeatConfigResponse {
                success: true,
                config: Some(config),
                error: None,
            })
        }
        Err(e) => {
            log::error!("[HEARTBEAT] Pulse failed to start: {}", e);
            HttpResponse::BadRequest().json(HeartbeatConfigResponse {
                success: false,
                config: Some(config),
                error: Some(e),
            })
        }
    }
}

/// Update heartbeat config for a specific channel
async fn update_channel_heartbeat_config(
    state: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<UpdateHeartbeatConfigRequest>,
) -> HttpResponse {
    if let Err(resp) = validate_session_for_heartbeat(&state, &req) {
        return resp;
    }

    let channel_id = path.into_inner();

    // Get or create first
    let config = match state.db.get_or_create_heartbeat_config(Some(channel_id)) {
        Ok(c) => c,
        Err(e) => {
            return HttpResponse::InternalServerError().json(HeartbeatConfigResponse {
                success: false,
                config: None,
                error: Some(format!("Database error: {}", e)),
            });
        }
    };

    match state.db.update_heartbeat_config(
        config.id,
        body.interval_minutes,
        body.target.as_deref(),
        body.active_hours_start.as_deref(),
        body.active_hours_end.as_deref(),
        body.active_days.as_deref(),
        body.enabled,
    ) {
        Ok(updated) => HttpResponse::Ok().json(HeartbeatConfigResponse {
            success: true,
            config: Some(updated),
            error: None,
        }),
        Err(e) => HttpResponse::InternalServerError().json(HeartbeatConfigResponse {
            success: false,
            config: None,
            error: Some(format!("Failed to update config: {}", e)),
        }),
    }
}
