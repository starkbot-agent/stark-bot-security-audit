use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::Deserialize;

use crate::db::tables::kanban::{CreateKanbanItemRequest, UpdateKanbanItemRequest};
use crate::gateway::protocol::GatewayEvent;
use crate::AppState;

/// Validate session token from request
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

#[derive(Deserialize)]
struct ListQuery {
    status: Option<String>,
}

/// List all kanban items (optional ?status=ready filter)
async fn list_items(
    data: web::Data<AppState>,
    req: HttpRequest,
    query: web::Query<ListQuery>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }

    let result = if let Some(ref status) = query.status {
        data.db.list_kanban_items_by_status(status)
    } else {
        data.db.list_kanban_items()
    };

    match result {
        Ok(items) => HttpResponse::Ok().json(items),
        Err(e) => {
            log::error!("Failed to list kanban items: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Database error: {}", e)
            }))
        }
    }
}

/// Create a new kanban item
async fn create_item(
    data: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<CreateKanbanItemRequest>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }

    match data.db.create_kanban_item(&body.into_inner()) {
        Ok(item) => {
            // Broadcast event for real-time updates
            data.broadcaster.broadcast(GatewayEvent::new(
                "kanban_item_updated",
                serde_json::json!({ "item": &item }),
            ));
            HttpResponse::Created().json(item)
        }
        Err(e) => {
            log::error!("Failed to create kanban item: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Database error: {}", e)
            }))
        }
    }
}

/// Get a single kanban item
async fn get_item(
    data: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<i64>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }

    let item_id = path.into_inner();

    match data.db.get_kanban_item(item_id) {
        Ok(Some(item)) => HttpResponse::Ok().json(item),
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Item not found"
        })),
        Err(e) => {
            log::error!("Failed to get kanban item: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Database error: {}", e)
            }))
        }
    }
}

/// Update a kanban item
async fn update_item(
    data: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<i64>,
    body: web::Json<UpdateKanbanItemRequest>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }

    let item_id = path.into_inner();

    match data.db.update_kanban_item(item_id, &body.into_inner()) {
        Ok(Some(item)) => {
            // Broadcast event for real-time updates
            data.broadcaster.broadcast(GatewayEvent::new(
                "kanban_item_updated",
                serde_json::json!({ "item": &item }),
            ));
            HttpResponse::Ok().json(item)
        }
        Ok(None) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Item not found"
        })),
        Err(e) => {
            log::error!("Failed to update kanban item: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Database error: {}", e)
            }))
        }
    }
}

/// Delete a kanban item
async fn delete_item(
    data: web::Data<AppState>,
    req: HttpRequest,
    path: web::Path<i64>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }

    let item_id = path.into_inner();

    match data.db.delete_kanban_item(item_id) {
        Ok(true) => {
            // Broadcast event for real-time updates
            data.broadcaster.broadcast(GatewayEvent::new(
                "kanban_item_updated",
                serde_json::json!({ "action": "deleted", "item_id": item_id }),
            ));
            HttpResponse::Ok().json(serde_json::json!({
                "success": true,
                "message": "Item deleted"
            }))
        }
        Ok(false) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Item not found"
        })),
        Err(e) => {
            log::error!("Failed to delete kanban item: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": format!("Database error: {}", e)
            }))
        }
    }
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/kanban")
            .route("/items", web::get().to(list_items))
            .route("/items", web::post().to(create_item))
            .route("/items/{id}", web::get().to(get_item))
            .route("/items/{id}", web::put().to(update_item))
            .route("/items/{id}", web::delete().to(delete_item)),
    );
}
