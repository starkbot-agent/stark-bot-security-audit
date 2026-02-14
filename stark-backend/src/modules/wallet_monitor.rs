//! Wallet Monitor module — tracks ETH wallet activity and flags large trades
//!
//! Delegates to the standalone wallet-monitor-service via RPC.
//! The service must be running separately on WALLET_MONITOR_URL (default: http://127.0.0.1:9100).

use async_trait::async_trait;
use crate::db::Database;
use crate::integrations::wallet_monitor_client::WalletMonitorClient;
use crate::tools::builtin::cryptocurrency::wallet_monitor::{
    WalletActivityTool, WalletMonitorControlTool, WalletWatchlistTool,
};
use crate::tools::registry::Tool;
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
        "1.1.0"
    }

    fn default_port(&self) -> u16 {
        9100
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
        let client = Self::make_client();
        vec![
            Arc::new(WalletWatchlistTool::new(client.clone())),
            Arc::new(WalletActivityTool::new(client.clone())),
            Arc::new(WalletMonitorControlTool::new(client)),
        ]
    }

    fn skill_content(&self) -> Option<&'static str> {
        Some(WALLET_MONITOR_SKILL)
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
}

const WALLET_MONITOR_SKILL: &str = r#"---
name: wallet_monitor
description: "Monitor ETH wallets for on-chain activity, detect whale trades, and track transaction history on Ethereum Mainnet and Base"
version: 1.1.0
author: starkbot
tags: [crypto, defi, monitoring, wallets, whale, alerts]
requires_tools: [wallet_watchlist, wallet_activity, wallet_monitor_control, dexscreener, token_lookup]
---

# Wallet Monitor Skill

You are helping the user manage their wallet monitoring setup. This skill tracks on-chain activity for watched wallets using Alchemy Enhanced APIs, detecting transfers, swaps, and large trades on Ethereum Mainnet and Base.

The wallet monitor runs as a separate microservice. All tool calls communicate with it via RPC.

## Available Tools

1. **wallet_watchlist** — Manage the list of watched wallets
   - `add`: Add a new wallet to monitor (requires address, optional label/chain/threshold)
   - `remove`: Remove a wallet by ID
   - `list`: Show all watched wallets
   - `update`: Modify wallet settings (label, threshold, enable/disable)

2. **wallet_activity** — Query logged on-chain activity
   - `recent`: Show recent transactions across all watched wallets
   - `large_trades`: Show only large trades (above threshold)
   - `search`: Filter by address, chain, activity type
   - `stats`: Overview statistics

3. **wallet_monitor_control** — Control the background worker
   - `status`: Check if the monitor service is running, wallet counts, uptime
   - `trigger`: Verify worker is active (polls every 60s automatically)

## Workflow

1. First check status: `wallet_monitor_control(action="status")`
2. Add wallets: `wallet_watchlist(action="add", address="0x...", label="Whale Alpha", chain="mainnet", threshold_usd=50000)`
3. The background worker automatically polls every 60 seconds
4. Query activity: `wallet_activity(action="recent")` or `wallet_activity(action="large_trades")`

## Important Notes

- The wallet monitor runs as a standalone service (wallet-monitor-service)
- Dashboard available at http://127.0.0.1:9100/
- Supported chains: "mainnet" (Ethereum) and "base" (Base)
- Each wallet has its own large_trade_threshold_usd (default $10,000)
- Swap detection: transactions with both outgoing and incoming ERC-20 transfers are classified as swaps
- USD values are estimated using DexScreener price data (cached 60s)
- The worker uses block-number cursors for gap-free incremental polling
"#;
