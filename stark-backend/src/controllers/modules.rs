//! HTTP API endpoints for the module/plugin system

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
    has_db_tables: bool,
    has_tools: bool,
    has_worker: bool,
    has_dashboard: bool,
    required_api_keys: Vec<String>,
    api_keys_met: bool,
    installed_at: Option<String>,
}

#[derive(Deserialize)]
struct ModuleActionRequest {
    action: String, // "install", "uninstall", "enable", "disable"
}

/// Activate a module at runtime: register its tools and spawn its worker.
/// Called after install or enable succeeds.
async fn activate_module(data: &web::Data<AppState>, module_name: &str) {
    let registry = crate::modules::ModuleRegistry::new();
    let module = match registry.get(module_name) {
        Some(m) => m,
        None => {
            log::warn!("[MODULE] activate_module: unknown module '{}'", module_name);
            return;
        }
    };

    // Register tools into the shared tool registry (RwLock, no restart needed)
    if module.has_tools() {
        for tool in module.create_tools() {
            log::info!("[MODULE] Hot-registered tool: {} (from {})", tool.name(), module_name);
            data.tool_registry.register(tool);
        }
    }

    // Spawn background worker and track its handle
    if module.has_worker() {
        if let Some(handle) = module.spawn_worker(
            data.db.clone(),
            data.broadcaster.clone(),
            data.dispatcher.clone(),
        ) {
            log::info!("[MODULE] Hot-started worker for: {}", module_name);
            data.module_workers.lock().await.insert(module_name.to_string(), handle);
        }
    }
}

/// Deactivate a module at runtime: unregister its tools and abort its worker.
/// Called before/after disable or uninstall.
async fn deactivate_module(data: &web::Data<AppState>, module_name: &str) {
    let registry = crate::modules::ModuleRegistry::new();
    let module = match registry.get(module_name) {
        Some(m) => m,
        None => {
            log::warn!("[MODULE] deactivate_module: unknown module '{}'", module_name);
            return;
        }
    };

    // Unregister tools
    if module.has_tools() {
        for tool in module.create_tools() {
            let name = tool.name();
            if data.tool_registry.unregister(&name) {
                log::info!("[MODULE] Unregistered tool: {} (from {})", name, module_name);
            }
        }
    }

    // Abort worker
    if let Some(handle) = data.module_workers.lock().await.remove(module_name) {
        handle.abort();
        log::info!("[MODULE] Stopped worker for: {}", module_name);
    }
}

/// GET /api/modules — list all available modules with install status
async fn list_modules(data: web::Data<AppState>) -> HttpResponse {
    let registry = crate::modules::ModuleRegistry::new();
    let installed = data.db.list_installed_modules().unwrap_or_default();

    let mut modules = Vec::new();
    for module in registry.available_modules() {
        let installed_entry = installed.iter().find(|m| m.module_name == module.name());
        let required_keys: Vec<String> = module.required_api_keys().iter().map(|s| s.to_string()).collect();
        let api_keys_met = required_keys.iter().all(|key| {
            data.db.get_api_key(key).ok().flatten().is_some()
        });

        modules.push(ModuleInfo {
            name: module.name().to_string(),
            description: module.description().to_string(),
            version: module.version().to_string(),
            installed: installed_entry.is_some(),
            enabled: installed_entry.map(|e| e.enabled).unwrap_or(false),
            has_db_tables: module.has_db_tables(),
            has_tools: module.has_tools(),
            has_worker: module.has_worker(),
            has_dashboard: module.has_dashboard(),
            required_api_keys: required_keys,
            api_keys_met,
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

            // Check API keys
            for key in module.required_api_keys() {
                if data.db.get_api_key(key).ok().flatten().is_none() {
                    return HttpResponse::BadRequest().json(serde_json::json!({
                        "error": format!("Missing required API key: {}", key)
                    }));
                }
            }

            // Create tables
            if module.has_db_tables() {
                let conn = data.db.conn();
                if let Err(e) = module.init_tables(&conn) {
                    return HttpResponse::InternalServerError().json(serde_json::json!({
                        "error": format!("Failed to create tables: {}", e)
                    }));
                }
            }

            let required_keys = module.required_api_keys();
            let key_strs: Vec<&str> = required_keys.iter().copied().collect();
            match data.db.install_module(
                &name,
                module.description(),
                module.version(),
                module.has_db_tables(),
                module.has_tools(),
                module.has_worker(),
                &key_strs,
            ) {
                Ok(_) => {
                    // Install skill if provided
                    if let Some(skill_md) = module.skill_content() {
                        let _ = data.skill_registry.create_skill_from_markdown(skill_md);
                    }

                    // Hot-activate: register tools and spawn worker immediately
                    activate_module(&data, &name).await;

                    HttpResponse::Ok().json(serde_json::json!({
                        "status": "installed",
                        "message": format!("Module '{}' installed and activated.", name)
                    }))
                }
                Err(e) => HttpResponse::InternalServerError().json(serde_json::json!({
                    "error": format!("Install failed: {}", e)
                })),
            }
        }

        "uninstall" => {
            // Deactivate before uninstalling
            deactivate_module(&data, &name).await;

            match data.db.uninstall_module(&name) {
                Ok(true) => HttpResponse::Ok().json(serde_json::json!({
                    "status": "uninstalled",
                    "message": format!("Module '{}' deactivated and uninstalled. Data preserved.", name)
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

                // Check API keys
                for key in module.required_api_keys() {
                    if data.db.get_api_key(key).ok().flatten().is_none() {
                        return HttpResponse::BadRequest().json(serde_json::json!({
                            "error": format!("Missing required API key: {}", key)
                        }));
                    }
                }

                // Create tables
                if module.has_db_tables() {
                    let conn = data.db.conn();
                    if let Err(e) = module.init_tables(&conn) {
                        return HttpResponse::InternalServerError().json(serde_json::json!({
                            "error": format!("Failed to create tables: {}", e)
                        }));
                    }
                }

                let required_keys = module.required_api_keys();
                let key_strs: Vec<&str> = required_keys.iter().copied().collect();
                if let Err(e) = data.db.install_module(
                    &name,
                    module.description(),
                    module.version(),
                    module.has_db_tables(),
                    module.has_tools(),
                    module.has_worker(),
                    &key_strs,
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
                Ok(false) => {
                    // Shouldn't happen since we auto-install above, but handle gracefully
                    HttpResponse::NotFound().json(serde_json::json!({
                        "error": format!("Module '{}' not found", name)
                    }))
                }
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

/// POST /api/modules/reload — full resync of all module tools and workers
async fn reload_modules(data: web::Data<AppState>) -> HttpResponse {
    let module_registry = crate::modules::ModuleRegistry::new();
    let mut activated = Vec::new();
    let mut deactivated = Vec::new();

    // 1. Deactivate all currently tracked module workers
    {
        let mut workers = data.module_workers.lock().await;
        for (name, handle) in workers.drain() {
            handle.abort();
            log::info!("[MODULE] Reload: stopped worker for '{}'", name);
        }
    }

    // 2. Unregister all module tools (iterate available modules, remove any that exist)
    for module in module_registry.available_modules() {
        if module.has_tools() {
            for tool in module.create_tools() {
                data.tool_registry.unregister(&tool.name());
            }
        }
    }

    // 3. Read DB for installed + enabled modules, activate each
    let installed = data.db.list_installed_modules().unwrap_or_default();
    for entry in &installed {
        if entry.enabled {
            if let Some(module) = module_registry.get(&entry.module_name) {
                // Register tools
                if module.has_tools() {
                    for tool in module.create_tools() {
                        log::info!("[MODULE] Reload: registered tool '{}' (from {})", tool.name(), entry.module_name);
                        data.tool_registry.register(tool);
                    }
                }
                // Spawn worker
                if module.has_worker() {
                    if let Some(handle) = module.spawn_worker(
                        data.db.clone(),
                        data.broadcaster.clone(),
                        data.dispatcher.clone(),
                    ) {
                        log::info!("[MODULE] Reload: started worker for '{}'", entry.module_name);
                        data.module_workers.lock().await.insert(entry.module_name.clone(), handle);
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
            .route("/{name}", web::post().to(module_action)),
    );
}
