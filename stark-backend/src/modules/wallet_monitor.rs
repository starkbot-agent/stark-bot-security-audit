//! Wallet Monitor module — tracks ETH wallet activity and flags large trades
//!
//! Delegates to the standalone wallet-monitor-service via RPC.
//! The service must be running separately on WALLET_MONITOR_URL (default: http://127.0.0.1:9100).

use async_trait::async_trait;
use crate::db::Database;
use crate::integrations::wallet_monitor_client::WalletMonitorClient;
use serde_json::{json, Value};
use std::sync::Arc;

pub struct WalletMonitorModule;

impl WalletMonitorModule {
    fn make_client() -> Arc<WalletMonitorClient> {
        let url = Self::url_from_env();
        Arc::new(WalletMonitorClient::new(&url))
    }

    fn url_from_env() -> String {
        std::env::var("WALLET_MONITOR_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:9100".to_string())
    }
}

#[async_trait]
impl super::Module for WalletMonitorModule {
    fn name(&self) -> &'static str {
        "wallet_monitor"
    }

    fn description(&self) -> &'static str {
        "Monitor ETH wallets for activity and whale trades (Mainnet + Base)"
    }

    fn version(&self) -> &'static str {
        "2.1.0"
    }

    fn default_port(&self) -> u16 {
        9100
    }

    fn service_url(&self) -> String {
        Self::url_from_env()
    }

    fn has_tools(&self) -> bool {
        false
    }

    fn has_dashboard(&self) -> bool {
        true
    }

    fn create_tools(&self) -> Vec<Arc<dyn crate::tools::registry::Tool>> {
        vec![]
    }

    fn skill_content(&self) -> Option<&'static str> {
        Some(include_str!("wallet_monitor.md"))
    }

    async fn dashboard_data(&self, _db: &Database) -> Option<Value> {
        let client = Self::make_client();
        let watchlist = client.list_watchlist().await.ok()?;
        let stats = client.get_activity_stats().await.ok()?;
        let filter = wallet_monitor_types::ActivityFilter {
            limit: Some(10),
            ..Default::default()
        };
        let recent = client.query_activity(&filter).await.ok()?;

        let watchlist_json: Vec<Value> = watchlist.iter().map(|w| {
            json!({
                "id": w.id,
                "address": w.address,
                "label": w.label,
                "chain": w.chain,
                "monitor_enabled": w.monitor_enabled,
                "large_trade_threshold_usd": w.large_trade_threshold_usd,
                "last_checked_at": w.last_checked_at,
            })
        }).collect();

        let recent_activity_json: Vec<Value> = recent.iter().map(|a| {
            json!({
                "chain": a.chain,
                "tx_hash": a.tx_hash,
                "activity_type": a.activity_type,
                "usd_value": a.usd_value,
                "asset_symbol": a.asset_symbol,
                "amount_formatted": a.amount_formatted,
                "is_large_trade": a.is_large_trade,
                "created_at": a.created_at,
            })
        }).collect();

        Some(json!({
            "watched_wallets": stats.watched_wallets,
            "active_wallets": stats.active_wallets,
            "total_transactions": stats.total_transactions,
            "large_trades": stats.large_trades,
            "watchlist": watchlist_json,
            "recent_activity": recent_activity_json,
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
                    "address": e.address,
                    "label": e.label,
                    "chain": e.chain,
                    "monitor_enabled": e.monitor_enabled,
                    "large_trade_threshold_usd": e.large_trade_threshold_usd,
                    "copy_trade_enabled": e.copy_trade_enabled,
                    "copy_trade_max_usd": e.copy_trade_max_usd,
                    "notes": e.notes,
                })
            })
            .collect();
        Some(Value::Array(json_entries))
    }

    async fn restore_data(&self, _db: &Database, data: &Value) -> Result<(), String> {
        let entries = data
            .as_array()
            .ok_or("wallet_monitor restore data must be a JSON array")?;

        if entries.is_empty() {
            return Ok(());
        }

        let backup_entries: Vec<wallet_monitor_types::BackupEntry> = entries
            .iter()
            .filter_map(|e| {
                Some(wallet_monitor_types::BackupEntry {
                    address: e["address"].as_str()?.to_string(),
                    label: e["label"].as_str().map(|s| s.to_string()),
                    chain: e["chain"].as_str().unwrap_or("mainnet").to_string(),
                    monitor_enabled: e["monitor_enabled"].as_bool().unwrap_or(true),
                    large_trade_threshold_usd: e["large_trade_threshold_usd"].as_f64().unwrap_or(1000.0),
                    copy_trade_enabled: e["copy_trade_enabled"].as_bool().unwrap_or(false),
                    copy_trade_max_usd: e["copy_trade_max_usd"].as_f64(),
                    notes: e["notes"].as_str().map(|s| s.to_string()),
                })
            })
            .collect();

        let client = Self::make_client();
        let restored = client.backup_restore(backup_entries).await?;

        log::info!(
            "[wallet_monitor] Restored {} watchlist entries from backup",
            restored
        );
        Ok(())
    }
}
