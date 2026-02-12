//! Module/plugin system for StarkBot
//!
//! Modules are optional features that can be installed on demand.
//! Each module can provide database tables, tools, and background workers.

pub mod discord_tipping;
pub mod registry;
pub mod wallet_monitor;

use crate::channels::MessageDispatcher;
use crate::db::Database;
use crate::gateway::events::EventBroadcaster;
use crate::tools::registry::Tool;
use rusqlite::Connection;
use serde_json::Value;
use std::sync::Arc;

pub use registry::ModuleRegistry;

/// Trait that all modules must implement
pub trait Module: Send + Sync {
    /// Unique module name (used as identifier)
    fn name(&self) -> &'static str;
    /// Human-readable description
    fn description(&self) -> &'static str;
    /// Semantic version
    fn version(&self) -> &'static str;
    /// API keys this module requires to function
    fn required_api_keys(&self) -> Vec<&'static str>;
    /// Whether this module creates database tables
    fn has_db_tables(&self) -> bool;
    /// Whether this module provides tools
    fn has_tools(&self) -> bool;
    /// Whether this module runs a background worker
    fn has_worker(&self) -> bool;

    /// Create DB tables (idempotent, uses CREATE IF NOT EXISTS)
    fn init_tables(&self, conn: &Connection) -> rusqlite::Result<()>;

    /// Return tool instances to register
    fn create_tools(&self) -> Vec<Arc<dyn Tool>>;

    /// Spawn background worker (if has_worker). Returns a JoinHandle.
    fn spawn_worker(
        &self,
        db: Arc<Database>,
        broadcaster: Arc<EventBroadcaster>,
        dispatcher: Arc<MessageDispatcher>,
    ) -> Option<tokio::task::JoinHandle<()>>;

    /// Optional: skill markdown content to install
    fn skill_content(&self) -> Option<&'static str> {
        None
    }

    /// Whether this module has a dashboard UI
    fn has_dashboard(&self) -> bool {
        false
    }

    /// Return dashboard data as JSON (module-specific)
    fn dashboard_data(&self, _db: &Database) -> Option<Value> {
        None
    }

    /// Return data to include in cloud backup (module-specific)
    fn backup_data(&self, _db: &Database) -> Option<Value> {
        None
    }

    /// Restore module data from a cloud backup
    fn restore_data(&self, _db: &Database, _data: &Value) -> Result<(), String> {
        Ok(())
    }
}
