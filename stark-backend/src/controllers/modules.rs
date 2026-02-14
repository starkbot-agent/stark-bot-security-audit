//! HTTP API endpoints for the module/plugin system
//!
//! Modules are standalone microservices. This controller manages their
//! install/uninstall/enable/disable state in the bot's database and
//! hot-registers/unregisters their tools at runtime.

use actix_web::{web, HttpResponse};
use serde::{Deserialize, Serialize};
use crate::AppState;

#[derive(Serialize)]
struct ModuleInfo {
    name: String,
    description: String,
    version: String,
    installed: bool,
    enabled: bool,
    has_tools: bool,
    has_dashboard: bool,
    service_url: String,
    service_port: u16,
    installed_at: Option<String>,
}

#[derive(Deserialize)]
struct ModuleActionRequest {
    action: String, // "install", "uninstall", "enable", "disable"
}

/// Activate a module at runtime: register its tools.
async fn activate_module(data: &web::Data<AppState>, module_name: &str) {
    let registry = crate::modules::ModuleRegistry::new();
    let module = match registry.get(module_name) {
        Some(m) => m,
        None => {
            log::warn!("[MODULE] activate_module: unknown module '{}'", module_name);
            return;
        }
    };

    if module.has_tools() {
        for tool in module.create_tools() {
            log::info!("[MODULE] Hot-registered tool: {} (from {})", tool.name(), module_name);
            data.tool_registry.register(tool);
        }
    }
}

/// Deactivate a module at runtime: unregister its tools.
async fn deactivate_module(data: &web::Data<AppState>, module_name: &str) {
    let registry = crate::modules::ModuleRegistry::new();
    let module = match registry.get(module_name) {
        Some(m) => m,
        None => {
            log::warn!("[MODULE] deactivate_module: unknown module '{}'", module_name);
            return;
        }
    };

    if module.has_tools() {
        for tool in module.create_tools() {
            let name = tool.name();
            if data.tool_registry.unregister(&name) {
                log::info!("[MODULE] Unregistered tool: {} (from {})", name, module_name);
            }
        }
    }
}

/// GET /api/modules — list all available modules with install status
async fn list_modules(data: web::Data<AppState>) -> HttpResponse {
    let registry = crate::modules::ModuleRegistry::new();
    let installed = data.db.list_installed_modules().unwrap_or_default();

    let mut modules = Vec::new();
    for module in registry.available_modules() {
        let installed_entry = installed.iter().find(|m| m.module_name == module.name());

        modules.push(ModuleInfo {
            name: module.name().to_string(),
            description: module.description().to_string(),
            version: module.version().to_string(),
            installed: installed_entry.is_some(),
            enabled: installed_entry.map(|e| e.enabled).unwrap_or(false),
            has_tools: module.has_tools(),
            has_dashboard: module.has_dashboard(),
            service_url: module.service_url(),
            service_port: module.default_port(),
            installed_at: installed_entry.map(|e| e.installed_at.to_rfc3339()),
        });
    }

    HttpResponse::Ok().json(modules)
}

/// POST /api/modules/{name} — install, uninstall, enable, or disable a module
async fn module_action(
    data: web::Data<AppState>,
    name: web::Path<String>,
    body: web::Json<ModuleActionRequest>,
) -> HttpResponse {
    let name = name.into_inner();
    let action = &body.action;

    match action.as_str() {
        "install" => {
            if data.db.is_module_installed(&name).unwrap_or(false) {
                return HttpResponse::Conflict().json(serde_json::json!({
                    "error": format!("Module '{}' is already installed", name)
                }));
            }

            let registry = crate::modules::ModuleRegistry::new();
            let module = match registry.get(&name) {
                Some(m) => m,
                None => return HttpResponse::NotFound().json(serde_json::json!({
                    "error": format!("Unknown module: '{}'", name)
                })),
            };

            match data.db.install_module(
                &name,
                module.description(),
                module.version(),
                module.has_tools(),
                module.has_dashboard(),
            ) {
                Ok(_) => {
                    // Install skill if provided
                    if let Some(skill_md) = module.skill_content() {
                        let _ = data.skill_registry.create_skill_from_markdown(skill_md);
                    }

                    // Hot-activate: register tools immediately
                    activate_module(&data, &name).await;

                    HttpResponse::Ok().json(serde_json::json!({
                        "status": "installed",
                        "message": format!("Module '{}' installed and activated.", name),
                        "service_url": module.service_url(),
                    }))
                }
                Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Install failed: {}", e)
                })),
            }
        }

        "uninstall" => {
            deactivate_module(&data, &name).await;

            match data.db.uninstall_module(&name) {
                Ok(true) => HttpResponse::Ok().json(serde_json::json!({
                    "status": "uninstalled",
                    "message": format!("Module '{}' deactivated and uninstalled.", name)
                })),
                Ok(false) => HttpResponse::NotFound().json(serde_json::json!({
                    "error": format!("Module '{}' is not installed", name)
                })),
                Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Uninstall failed: {}", e)
                })),
            }
        }

        "enable" => {
            // Auto-install if not already installed
            let already_installed = data.db.is_module_installed(&name).unwrap_or(false);
            if !already_installed {
                let registry = crate::modules::ModuleRegistry::new();
                let module = match registry.get(&name) {
                    Some(m) => m,
                    None => return HttpResponse::NotFound().json(serde_json::json!({
                        "error": format!("Unknown module: '{}'", name)
                    })),
                };

                if let Err(e) = data.db.install_module(
                    &name,
                    module.description(),
                    module.version(),
                    module.has_tools(),
                    module.has_dashboard(),
                ) {
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": format!("Install failed: {}", e)
                    }));
                }

                // Install skill if provided
                if let Some(skill_md) = module.skill_content() {
                    let _ = data.skill_registry.create_skill_from_markdown(skill_md);
                }
            }

            match data.db.set_module_enabled(&name, true) {
                Ok(true) => {
                    activate_module(&data, &name).await;
                    HttpResponse::Ok().json(serde_json::json!({
                        "status": "enabled",
                        "message": format!("Module '{}' enabled.", name)
                    }))
                }
                Ok(false) => HttpResponse::NotFound().json(serde_json::json!({
                    "error": format!("Module '{}' not found", name)
                })),
                Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Enable failed: {}", e)
                })),
            }
        }

        "disable" => {
            deactivate_module(&data, &name).await;
            match data.db.set_module_enabled(&name, false) {
                Ok(true) => HttpResponse::Ok().json(serde_json::json!({
                    "status": "disabled",
                    "message": format!("Module '{}' deactivated and disabled.", name)
                })),
                Ok(false) => HttpResponse::NotFound().json(serde_json::json!({
                    "error": format!("Module '{}' is not installed", name)
                })),
                Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Disable failed: {}", e)
                })),
            }
        }

        _ => HttpResponse::BadRequest().json(serde_json::json!({
            "error": format!("Unknown action: '{}'. Use 'install', 'uninstall', 'enable', or 'disable'.", action)
        })),
    }
}

/// GET /api/modules/{name}/dashboard — get module-specific dashboard data
async fn module_dashboard(
    data: web::Data<AppState>,
    name: web::Path<String>,
) -> HttpResponse {
    let name = name.into_inner();

    // Check if module is installed and enabled
    let installed = data.db.list_installed_modules().unwrap_or_default();
    let module_entry = installed.iter().find(|m| m.module_name == name);
    match module_entry {
        None => return HttpResponse::NotFound().json(serde_json::json!({
            "error": format!("Module '{}' is not installed", name)
        })),
        Some(entry) if !entry.enabled => return HttpResponse::BadRequest().json(serde_json::json!({
            "error": format!("Module '{}' is disabled", name)
        })),
        _ => {}
    }

    let registry = crate::modules::ModuleRegistry::new();
    let module = match registry.get(&name) {
        Some(m) => m,
        None => return HttpResponse::NotFound().json(serde_json::json!({
            "error": format!("Unknown module: '{}'", name)
        })),
    };

    if !module.has_dashboard() {
        return HttpResponse::NotFound().json(serde_json::json!({
            "error": format!("Module '{}' does not have a dashboard", name)
        }));
    }

    match module.dashboard_data(&data.db) {
        Some(data) => HttpResponse::Ok().json(data),
        None => HttpResponse::Ok().json(serde_json::json!({})),
    }
}

/// GET /api/modules/{name}/status — proxy health check to the module's service
async fn module_status(
    data: web::Data<AppState>,
    name: web::Path<String>,
) -> HttpResponse {
    let name = name.into_inner();

    let registry = crate::modules::ModuleRegistry::new();
    let module = match registry.get(&name) {
        Some(m) => m,
        None => return HttpResponse::NotFound().json(serde_json::json!({
            "error": format!("Unknown module: '{}'", name)
        })),
    };

    let url = format!("{}/rpc/status", module.service_url());
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap_or_default();

    match client.get(&url).send().await {
        Ok(resp) if resp.status().is_success() => {
            let body = resp.text().await.unwrap_or_default();
            HttpResponse::Ok()
                .content_type("application/json")
                .body(body)
        }
        Ok(_) => HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "unhealthy",
            "error": "Service returned non-200 response"
        })),
        Err(_) => HttpResponse::ServiceUnavailable().json(serde_json::json!({
            "status": "offline",
            "error": "Service unreachable"
        })),
    }
}

/// POST /api/modules/reload — full resync of all module tools
async fn reload_modules(data: web::Data<AppState>) -> HttpResponse {
    let module_registry = crate::modules::ModuleRegistry::new();
    let mut activated = Vec::new();
    let mut deactivated = Vec::new();

    // 1. Unregister all module tools
    for module in module_registry.available_modules() {
        if module.has_tools() {
            for tool in module.create_tools() {
                data.tool_registry.unregister(&tool.name());
            }
        }
    }

    // 2. Read DB for installed + enabled modules, activate each
    let installed = data.db.list_installed_modules().unwrap_or_default();
    for entry in &installed {
        if entry.enabled {
            if let Some(module) = module_registry.get(&entry.module_name) {
                if module.has_tools() {
                    for tool in module.create_tools() {
                        log::info!("[MODULE] Reload: registered tool '{}' (from {})", tool.name(), entry.module_name);
                        data.tool_registry.register(tool);
                    }
                }
                activated.push(entry.module_name.clone());
            }
        } else {
            deactivated.push(entry.module_name.clone());
        }
    }

    log::info!("[MODULE] Reload complete: {} activated, {} inactive", activated.len(), deactivated.len());

    HttpResponse::Ok().json(serde_json::json!({
        "status": "reloaded",
        "activated": activated,
        "deactivated": deactivated,
        "message": format!("Reloaded {} module(s).", activated.len())
    }))
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/modules")
            .route("", web::get().to(list_modules))
            .route("/reload", web::post().to(reload_modules))
            .route("/{name}/dashboard", web::get().to(module_dashboard))
            .route("/{name}/status", web::get().to(module_status))
            .route("/{name}", web::post().to(module_action)),
    );
}
