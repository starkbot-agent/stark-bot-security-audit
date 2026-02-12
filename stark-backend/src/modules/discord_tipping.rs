//! Discord Tipping module — enables tipping Discord users with ERC-20 tokens
//!
//! Manages discord_user_profiles table, provides the discord_resolve_user tool,
//! and exposes a dashboard showing all registered profiles.

use crate::channels::MessageDispatcher;
use crate::db::Database;
use crate::discord_hooks;
use crate::gateway::events::EventBroadcaster;
use crate::tools::registry::Tool;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::sync::Arc;

pub struct DiscordTippingModule;

impl super::Module for DiscordTippingModule {
    fn name(&self) -> &'static str {
        "discord_tipping"
    }

    fn description(&self) -> &'static str {
        "Tip Discord users with tokens. Register wallet addresses and resolve mentions for transfers."
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn required_api_keys(&self) -> Vec<&'static str> {
        vec![] // No API keys required — uses the bot's existing wallet
    }

    fn has_db_tables(&self) -> bool {
        true
    }

    fn has_tools(&self) -> bool {
        true
    }

    fn has_worker(&self) -> bool {
        false
    }

    fn init_tables(&self, conn: &Connection) -> rusqlite::Result<()> {
        discord_hooks::db::init_tables(conn)
    }

    fn create_tools(&self) -> Vec<Arc<dyn Tool>> {
        vec![Arc::new(
            discord_hooks::tools::DiscordResolveUserTool::new(),
        )]
    }

    fn spawn_worker(
        &self,
        _db: Arc<Database>,
        _broadcaster: Arc<EventBroadcaster>,
        _dispatcher: Arc<MessageDispatcher>,
    ) -> Option<tokio::task::JoinHandle<()>> {
        None
    }

    fn skill_content(&self) -> Option<&'static str> {
        Some(include_str!("../../../skills/discord_tipping.md"))
    }

    fn has_dashboard(&self) -> bool {
        true
    }

    fn dashboard_data(&self, db: &Database) -> Option<Value> {
        let all_profiles = discord_hooks::db::list_all_profiles(db).ok()?;
        let registered_profiles: Vec<_> = all_profiles
            .iter()
            .filter(|p| p.registration_status == "registered")
            .collect();
        let total_count = all_profiles.len();
        let registered_count = registered_profiles.len();

        let profiles_json: Vec<Value> = all_profiles
            .iter()
            .map(|p| {
                json!({
                    "discord_user_id": p.discord_user_id,
                    "discord_username": p.discord_username,
                    "public_address": p.public_address,
                    "registration_status": p.registration_status,
                    "registered_at": p.registered_at,
                    "last_interaction_at": p.last_interaction_at,
                })
            })
            .collect();

        Some(json!({
            "total_profiles": total_count,
            "registered_count": registered_count,
            "unregistered_count": total_count - registered_count,
            "profiles": profiles_json,
        }))
    }

    fn backup_data(&self, db: &Database) -> Option<Value> {
        let profiles = discord_hooks::db::list_registered_profiles(db).ok()?;
        if profiles.is_empty() {
            return None;
        }

        let entries: Vec<Value> = profiles
            .iter()
            .filter_map(|p| {
                p.public_address.as_ref().map(|addr| {
                    json!({
                        "discord_user_id": p.discord_user_id,
                        "discord_username": p.discord_username,
                        "public_address": addr,
                        "registered_at": p.registered_at,
                    })
                })
            })
            .collect();

        Some(Value::Array(entries))
    }

    fn restore_data(&self, db: &Database, data: &Value) -> Result<(), String> {
        let entries = data
            .as_array()
            .ok_or("discord_tipping restore data must be a JSON array")?;

        if entries.is_empty() {
            return Ok(());
        }

        // Clear existing registrations before restore
        discord_hooks::db::clear_registrations_for_restore(db)?;

        let mut restored = 0;
        for entry in entries {
            let user_id = entry["discord_user_id"]
                .as_str()
                .ok_or("Missing discord_user_id")?;
            let username = entry["discord_username"]
                .as_str()
                .unwrap_or("unknown");
            let address = entry["public_address"]
                .as_str()
                .ok_or("Missing public_address")?;

            discord_hooks::db::get_or_create_profile(db, user_id, username)?;
            discord_hooks::db::register_address(db, user_id, address)?;
            restored += 1;
        }

        log::info!(
            "[discord_tipping] Restored {} registrations from backup",
            restored
        );
        Ok(())
    }
}
