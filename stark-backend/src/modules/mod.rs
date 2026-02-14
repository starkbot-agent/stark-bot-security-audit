//! Module/plugin system for StarkBot
//!
//! Modules are standalone microservices that run as separate binaries.
//! Each module has its own database, HTTP server, and dashboard.
//! The main bot communicates with modules via JSON RPC over HTTP.

pub mod discord_tipping;
pub mod dynamic_module;
pub mod dynamic_tool;
pub mod loader;
pub mod manifest;
pub mod registry;
pub mod wallet_monitor;

use async_trait::async_trait;
use crate::db::Database;
use crate::tools::registry::Tool;
use serde_json::Value;
use std::sync::Arc;

pub use registry::ModuleRegistry;

/// Trait that all modules must implement.
///
/// Modules are standalone services — they manage their own database, workers,
/// and dashboard. This trait defines the interface the main bot uses to
/// interact with them: registering tools, fetching dashboard data, and
/// performing backup/restore via RPC.
#[async_trait]
pub trait Module: Send + Sync {
    /// Unique module name (used as identifier)
    fn name(&self) -> &str;
    /// Human-readable description
    fn description(&self) -> &str;
    /// Semantic version
    fn version(&self) -> &str;
    /// Default port the service listens on
    fn default_port(&self) -> u16;

    /// The base URL of the running service (reads from env or falls back to default)
    fn service_url(&self) -> String;

    /// Whether this module provides tools to the bot
    fn has_tools(&self) -> bool;
    /// Whether this module has a standalone dashboard (served by the service itself)
    fn has_dashboard(&self) -> bool;

    /// Return tool instances to register with the bot
    fn create_tools(&self) -> Vec<Arc<dyn Tool>>;

    /// Optional: skill markdown content to install
    fn skill_content(&self) -> Option<&str> {
        None
    }

    /// Return dashboard data as JSON (fetched from the service via RPC)
    async fn dashboard_data(&self, _db: &Database) -> Option<Value> {
        None
    }

    /// Return data to include in cloud backup (fetched from service)
    async fn backup_data(&self, _db: &Database) -> Option<Value> {
        None
    }

    /// Restore module data from a cloud backup (sent to service)
    async fn restore_data(&self, _db: &Database, _data: &Value) -> Result<(), String> {
        Ok(())
    }
}
