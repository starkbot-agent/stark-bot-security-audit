//! Wallet Monitor module — tracks ETH wallet activity and flags large trades
//!
//! Monitors wallets on Ethereum Mainnet + Base using Alchemy Enhanced APIs.

use crate::channels::MessageDispatcher;
use crate::db::Database;
use crate::gateway::events::EventBroadcaster;
use crate::integrations::wallet_monitor_worker;
use crate::tools::builtin::cryptocurrency::wallet_monitor::{
    WalletActivityTool, WalletMonitorControlTool, WalletWatchlistTool,
};
use crate::tools::registry::Tool;
use rusqlite::Connection;
use serde_json::{json, Value};
use std::sync::Arc;

pub struct WalletMonitorModule;

impl super::Module for WalletMonitorModule {
    fn name(&self) -> &'static str {
        "wallet_monitor"
    }

    fn description(&self) -> &'static str {
        "Monitor ETH wallets for activity and whale trades (Mainnet + Base)"
    }

    fn version(&self) -> &'static str {
        "1.0.0"
    }

    fn required_api_keys(&self) -> Vec<&'static str> {
        vec!["ALCHEMY_API_KEY"]
    }

    fn has_db_tables(&self) -> bool {
        true
    }

    fn has_tools(&self) -> bool {
        true
    }

    fn has_worker(&self) -> bool {
        true
    }

    fn init_tables(&self, conn: &Connection) -> rusqlite::Result<()> {
        crate::db::tables::wallet_monitor::create_tables(conn)
    }

    fn create_tools(&self) -> Vec<Arc<dyn Tool>> {
        vec![
            Arc::new(WalletWatchlistTool::new()),
            Arc::new(WalletActivityTool::new()),
            Arc::new(WalletMonitorControlTool::new()),
        ]
    }

    fn spawn_worker(
        &self,
        db: Arc<Database>,
        broadcaster: Arc<EventBroadcaster>,
        _dispatcher: Arc<MessageDispatcher>,
    ) -> Option<tokio::task::JoinHandle<()>> {
        Some(tokio::spawn(async move {
            wallet_monitor_worker::run_worker(db, broadcaster).await;
        }))
    }

    fn skill_content(&self) -> Option<&'static str> {
        Some(WALLET_MONITOR_SKILL)
    }

    fn has_dashboard(&self) -> bool {
        true
    }

    fn dashboard_data(&self, db: &Database) -> Option<Value> {
        let watchlist = db.list_watchlist().ok()?;
        let stats = db.get_activity_stats().ok()?;
        let recent = db.query_activity(&crate::db::tables::wallet_monitor::ActivityFilter {
            limit: Some(10),
            ..Default::default()
        }).ok()?;

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
version: 1.0.0
author: starkbot
tags: [crypto, defi, monitoring, wallets, whale, alerts]
requires_tools: [wallet_watchlist, wallet_activity, wallet_monitor_control, dexscreener, token_lookup]
---

# Wallet Monitor Skill

You are helping the user manage their wallet monitoring setup. This skill tracks on-chain activity for watched wallets using Alchemy Enhanced APIs, detecting transfers, swaps, and large trades on Ethereum Mainnet and Base.

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
   - `status`: Check if the monitor is running, API key status, wallet counts
   - `trigger`: Verify worker is active (polls every 60s automatically)

## Workflow

1. First check status: `wallet_monitor_control(action="status")`
2. Add wallets: `wallet_watchlist(action="add", address="0x...", label="Whale Alpha", chain="mainnet", threshold_usd=50000)`
3. The background worker automatically polls every 60 seconds
4. Query activity: `wallet_activity(action="recent")` or `wallet_activity(action="large_trades")`

## Important Notes

- The monitor requires ALCHEMY_API_KEY to be configured
- Supported chains: "mainnet" (Ethereum) and "base" (Base)
- Each wallet has its own large_trade_threshold_usd (default $10,000)
- Swap detection: transactions with both outgoing and incoming ERC-20 transfers are classified as swaps
- USD values are estimated using DexScreener price data (cached 60s)
- The worker uses block-number cursors for gap-free incremental polling
"#;
