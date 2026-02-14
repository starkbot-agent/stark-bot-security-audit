//! Discord Tipping module — enables tipping Discord users with ERC-20 tokens
//!
//! Delegates to the standalone discord-tipping-service via RPC.
//! The service must be running separately on DISCORD_TIPPING_URL (default: http://127.0.0.1:9101).

use async_trait::async_trait;
use crate::db::Database;
use crate::integrations::discord_tipping_client::DiscordTippingClient;
use crate::tools::registry::Tool;
use serde_json::{json, Value};
use std::sync::Arc;

pub struct DiscordTippingModule;

impl DiscordTippingModule {
    fn make_client() -> DiscordTippingClient {
        let url = Self::url_from_env();
        DiscordTippingClient::new(&url)
    }

    fn url_from_env() -> String {
        std::env::var("DISCORD_TIPPING_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:9101".to_string())
    }
}

#[async_trait]
impl super::Module for DiscordTippingModule {
    fn name(&self) -> &'static str {
        "discord_tipping"
    }

    fn description(&self) -> &'static str {
        "Tip Discord users with tokens. Register wallet addresses and resolve mentions for transfers."
    }

    fn version(&self) -> &'static str {
        "1.1.0"
    }

    fn default_port(&self) -> u16 {
        9101
    }

    fn service_url(&self) -> String {
        Self::url_from_env()
    }

    fn has_tools(&self) -> bool {
        true
    }

    fn has_dashboard(&self) -> bool {
        true
    }

    fn create_tools(&self) -> Vec<Arc<dyn Tool>> {
        vec![Arc::new(
            crate::discord_hooks::tools::DiscordResolveUserTool::new(),
        )]
    }

    fn skill_content(&self) -> Option<&'static str> {
        Some(include_str!("../../../skills/discord_tipping.md"))
    }

    async fn dashboard_data(&self, _db: &Database) -> Option<Value> {
        let client = Self::make_client();
        let all_profiles = client.list_all_profiles().await.ok()?;
        let registered_count = all_profiles
            .iter()
            .filter(|p| p.registration_status == "registered")
            .count();
        let total_count = all_profiles.len();

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

    async fn backup_data(&self, _db: &Database) -> Option<Value> {
        let client = Self::make_client();
        let entries = client.backup_export().await.ok()?;
        if entries.is_empty() {
            return None;
        }
        let json_entries: Vec<Value> = entries
            .iter()
            .map(|e| {
                json!({
                    "discord_user_id": e.discord_user_id,
                    "discord_username": e.discord_username,
                    "public_address": e.public_address,
                    "registered_at": e.registered_at,
                })
            })
            .collect();
        Some(Value::Array(json_entries))
    }

    async fn restore_data(&self, _db: &Database, data: &Value) -> Result<(), String> {
        let entries = data
            .as_array()
            .ok_or("discord_tipping restore data must be a JSON array")?;

        if entries.is_empty() {
            return Ok(());
        }

        let backup_entries: Vec<discord_tipping_types::BackupEntry> = entries
            .iter()
            .filter_map(|e| {
                Some(discord_tipping_types::BackupEntry {
                    discord_user_id: e["discord_user_id"].as_str()?.to_string(),
                    discord_username: e["discord_username"]
                        .as_str()
                        .map(|s| s.to_string()),
                    public_address: e["public_address"].as_str()?.to_string(),
                    registered_at: e["registered_at"].as_str().map(|s| s.to_string()),
                })
            })
            .collect();

        let client = Self::make_client();
        let restored = client.backup_restore(backup_entries).await?;

        log::info!(
            "[discord_tipping] Restored {} registrations from backup",
            restored
        );
        Ok(())
    }
}
