use actix_cors::Cors;
use actix_files::{Files, NamedFile};
use actix_web::{middleware::Logger, web, App, HttpServer};
use dotenv::dotenv;
use std::sync::Arc;

mod ai;
mod channels;
mod config;
mod controllers;
mod db;
mod execution;
mod gateway;
mod middleware;
mod models;
mod skills;
mod tools;
mod x402;

use channels::MessageDispatcher;
use config::Config;
use db::Database;
use execution::ExecutionTracker;
use gateway::Gateway;
use skills::SkillRegistry;
use tools::ToolRegistry;

pub struct AppState {
    pub db: Arc<Database>,
    pub config: Config,
    pub gateway: Arc<Gateway>,
    pub tool_registry: Arc<ToolRegistry>,
    pub skill_registry: Arc<SkillRegistry>,
    pub dispatcher: Arc<MessageDispatcher>,
    pub execution_tracker: Arc<ExecutionTracker>,
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    dotenv().ok();
    env_logger::init();

    let config = Config::from_env();
    let port = config.port;
    let gateway_port = config.gateway_port;

    log::info!("Initializing database at {}", config.database_url);
    let db = Database::new(&config.database_url).expect("Failed to initialize database");
    let db = Arc::new(db);

    // Initialize Tool Registry with built-in tools
    log::info!("Initializing tool registry");
    let tool_registry = Arc::new(tools::create_default_registry());
    log::info!("Registered {} tools", tool_registry.len());

    // Initialize Skill Registry (database-backed)
    log::info!("Initializing skill registry");
    let skill_registry = Arc::new(skills::create_default_registry(db.clone()));

    // Load file-based skills into database (for backward compatibility)
    let skill_count = skill_registry.load_all().await.unwrap_or_else(|e| {
        log::warn!("Failed to load skills from disk: {}", e);
        0
    });
    log::info!("Loaded {} skills from disk, {} total in database", skill_count, skill_registry.len());

    // Initialize Gateway with tool registry
    log::info!("Initializing Gateway");
    let gateway = Arc::new(Gateway::new_with_tools(db.clone(), tool_registry.clone()));

    // Initialize Execution Tracker for progress display
    log::info!("Initializing execution tracker");
    let execution_tracker = Arc::new(ExecutionTracker::new(gateway.broadcaster().clone()));

    // Create the shared MessageDispatcher for all message processing
    log::info!("Initializing message dispatcher");
    let dispatcher = Arc::new(MessageDispatcher::new_with_wallet(
        db.clone(),
        gateway.broadcaster().clone(),
        tool_registry.clone(),
        execution_tracker.clone(),
        config.burner_wallet_private_key.clone(),
    ));

    // Start Gateway WebSocket server
    let gw = gateway.clone();
    tokio::spawn(async move {
        gw.start(gateway_port).await;
    });

    // Start enabled channels
    log::info!("Starting enabled channels");
    gateway.start_enabled_channels().await;

    log::info!("Starting StarkBot server on port {}", port);
    log::info!("Gateway WebSocket server on port {}", gateway_port);

    let tool_reg = tool_registry.clone();
    let skill_reg = skill_registry.clone();
    let disp = dispatcher.clone();
    let exec_tracker = execution_tracker.clone();

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header()
            .max_age(3600);

        App::new()
            .app_data(web::Data::new(AppState {
                db: Arc::clone(&db),
                config: config.clone(),
                gateway: Arc::clone(&gateway),
                tool_registry: Arc::clone(&tool_reg),
                skill_registry: Arc::clone(&skill_reg),
                dispatcher: Arc::clone(&disp),
                execution_tracker: Arc::clone(&exec_tracker),
            }))
            .wrap(Logger::default())
            .wrap(cors)
            .configure(controllers::health::config)
            .configure(controllers::auth::config)
            .configure(controllers::dashboard::config)
            .configure(controllers::chat::config)
            .configure(controllers::api_keys::config)
            .configure(controllers::channels::config)
            .configure(controllers::agent_settings::configure)
            .configure(controllers::sessions::config)
            .configure(controllers::memories::config)
            .configure(controllers::identity::config)
            .configure(controllers::tools::config)
            .configure(controllers::skills::config)
            // Serve static files, with SPA fallback to index.html for client-side routing
            .service(
                Files::new("/", "./stark-frontend/dist")
                    .index_file("index.html")
                    .default_handler(|req: actix_web::dev::ServiceRequest| {
                        let (http_req, _payload) = req.into_parts();
                        async {
                            let response = NamedFile::open("./stark-frontend/dist/index.html")?
                                .into_response(&http_req);
                            Ok(actix_web::dev::ServiceResponse::new(http_req, response))
                        }
                    })
            )
    })
    .bind(("0.0.0.0", port))?
    .run()
    .await
}
