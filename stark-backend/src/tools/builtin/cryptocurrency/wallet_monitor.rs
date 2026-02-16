//! Wallet monitoring tools — watchlist management, activity queries, and monitor control
//!
//! These tools are only registered when the wallet_monitor module is installed.
//! All operations go through the wallet-monitor-service via RPC.

use crate::integrations::wallet_monitor_client::WalletMonitorClient;
use crate::tools::registry::Tool;
use crate::tools::types::{
    PropertySchema, ToolContext, ToolDefinition, ToolGroup, ToolInputSchema, ToolResult,
    ToolSafetyLevel,
};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;
use wallet_monitor_types::ActivityEntry;

// =====================================================
// WalletWatchlistTool
// =====================================================

pub struct WalletWatchlistTool {
    definition: ToolDefinition,
    client: Arc<WalletMonitorClient>,
}

impl WalletWatchlistTool {
    pub fn new(client: Arc<WalletMonitorClient>) -> Self {
        let mut properties = HashMap::new();

        properties.insert(
            "action".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Action: 'add', 'remove', 'list', 'update'".to_string(),
                default: None,
                items: None,
                enum_values: Some(vec![
                    "add".to_string(),
                    "remove".to_string(),
                    "list".to_string(),
                    "update".to_string(),
                ]),
            },
        );

        properties.insert(
            "address".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Ethereum address (0x + 40 hex chars). Required for 'add'.".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "label".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Human-readable label for the wallet".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "chain".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Chain to monitor: 'mainnet' or 'base'. Default: 'mainnet'".to_string(),
                default: Some(json!("mainnet")),
                items: None,
                enum_values: Some(vec!["mainnet".to_string(), "base".to_string()]),
            },
        );

        properties.insert(
            "threshold_usd".to_string(),
            PropertySchema {
                schema_type: "number".to_string(),
                description: "Large trade threshold in USD. Default: 1000".to_string(),
                default: Some(json!(1000.0)),
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "id".to_string(),
            PropertySchema {
                schema_type: "integer".to_string(),
                description: "Watchlist entry ID. Required for 'remove' and 'update'.".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "notes".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Notes about this wallet".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "monitor_enabled".to_string(),
            PropertySchema {
                schema_type: "boolean".to_string(),
                description: "Enable/disable monitoring for this wallet".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        WalletWatchlistTool {
            definition: ToolDefinition {
                name: "wallet_watchlist".to_string(),
                description: "Manage the wallet watchlist for monitoring on-chain activity. Add, remove, list, or update watched wallets on Ethereum Mainnet and Base.".to_string(),
                input_schema: ToolInputSchema {
                    schema_type: "object".to_string(),
                    properties,
                    required: vec!["action".to_string()],
                },
                group: ToolGroup::Finance,
                hidden: false,
            },
            client,
        }
    }
}

#[derive(Debug, Deserialize)]
struct WatchlistParams {
    action: String,
    address: Option<String>,
    label: Option<String>,
    chain: Option<String>,
    threshold_usd: Option<f64>,
    id: Option<i64>,
    notes: Option<String>,
    monitor_enabled: Option<bool>,
}

fn is_valid_eth_address(addr: &str) -> bool {
    addr.starts_with("0x") && addr.len() == 42 && addr[2..].chars().all(|c| c.is_ascii_hexdigit())
}

#[async_trait]
impl Tool for WalletWatchlistTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, params: Value, _context: &ToolContext) -> ToolResult {
        let params: WatchlistParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid parameters: {}", e)),
        };

        match params.action.as_str() {
            "add" => {
                let address = match params.address {
                    Some(ref a) => a,
                    None => return ToolResult::error("'address' is required for 'add' action"),
                };
                if !is_valid_eth_address(address) {
                    return ToolResult::error("Invalid Ethereum address. Must be 0x + 40 hex characters.");
                }
                let chain = params.chain.as_deref().unwrap_or("mainnet");
                let threshold = params.threshold_usd.unwrap_or(1000.0);

                match self.client.add_wallet(address, params.label.as_deref(), chain, threshold).await {
                    Ok(entry) => ToolResult::success(json!({
                        "status": "added",
                        "id": entry.id,
                        "address": entry.address,
                        "label": entry.label,
                        "chain": entry.chain,
                        "threshold_usd": entry.large_trade_threshold_usd,
                    }).to_string()),
                    Err(e) => ToolResult::error(format!("Failed to add wallet: {}", e)),
                }
            }

            "remove" => {
                let id = match params.id {
                    Some(id) => id,
                    None => return ToolResult::error("'id' is required for 'remove' action"),
                };
                match self.client.remove_wallet(id).await {
                    Ok(_) => ToolResult::success(format!("Removed watchlist entry #{}", id)),
                    Err(e) => ToolResult::error(e),
                }
            }

            "list" => match self.client.list_watchlist().await {
                Ok(entries) => {
                    if entries.is_empty() {
                        return ToolResult::success("No wallets on the watchlist. Use action='add' to start monitoring.");
                    }
                    let mut output = format!("**Wallet Watchlist** ({} entries)\n\n", entries.len());
                    for e in &entries {
                        let label = e.label.as_deref().unwrap_or("(unlabeled)");
                        let status = if e.monitor_enabled { "active" } else { "paused" };
                        let last_block = e.last_checked_block.map(|b| format!("block #{}", b)).unwrap_or_else(|| "not yet checked".to_string());
                        output.push_str(&format!(
                            "#{} | {} | {} | {} | threshold: ${:.0} | {} | {}\n",
                            e.id, label, e.address, e.chain, e.large_trade_threshold_usd, status, last_block
                        ));
                    }
                    ToolResult::success(output)
                }
                Err(e) => ToolResult::error(format!("Failed to list watchlist: {}", e)),
            },

            "update" => {
                let id = match params.id {
                    Some(id) => id,
                    None => return ToolResult::error("'id' is required for 'update' action"),
                };
                match self.client.update_wallet(
                    id,
                    params.label.as_deref(),
                    params.threshold_usd,
                    params.monitor_enabled,
                    params.notes.as_deref(),
                ).await {
                    Ok(_) => ToolResult::success(format!("Updated watchlist entry #{}", id)),
                    Err(e) => ToolResult::error(e),
                }
            }

            _ => ToolResult::error(format!("Unknown action: '{}'. Use 'add', 'remove', 'list', or 'update'.", params.action)),
        }
    }

    fn safety_level(&self) -> ToolSafetyLevel {
        ToolSafetyLevel::Standard
    }
}

// =====================================================
// WalletActivityTool
// =====================================================

pub struct WalletActivityTool {
    definition: ToolDefinition,
    client: Arc<WalletMonitorClient>,
}

impl WalletActivityTool {
    pub fn new(client: Arc<WalletMonitorClient>) -> Self {
        let mut properties = HashMap::new();

        properties.insert(
            "action".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Action: 'recent', 'large_trades', 'search', 'stats'".to_string(),
                default: None,
                items: None,
                enum_values: Some(vec![
                    "recent".to_string(),
                    "large_trades".to_string(),
                    "search".to_string(),
                    "stats".to_string(),
                ]),
            },
        );

        properties.insert(
            "address".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Filter by wallet address".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "activity_type".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Filter by type: 'eth_transfer', 'erc20_transfer', 'swap', 'internal'".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "chain".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Filter by chain: 'mainnet' or 'base'".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "large_only".to_string(),
            PropertySchema {
                schema_type: "boolean".to_string(),
                description: "Only show large trades".to_string(),
                default: Some(json!(false)),
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "limit".to_string(),
            PropertySchema {
                schema_type: "integer".to_string(),
                description: "Max results to return (default 25, max 200)".to_string(),
                default: Some(json!(25)),
                items: None,
                enum_values: None,
            },
        );

        WalletActivityTool {
            definition: ToolDefinition {
                name: "wallet_activity".to_string(),
                description: "Query logged wallet activity from monitored wallets. View recent transactions, large trades, search by filters, or get stats.".to_string(),
                input_schema: ToolInputSchema {
                    schema_type: "object".to_string(),
                    properties,
                    required: vec!["action".to_string()],
                },
                group: ToolGroup::Finance,
                hidden: false,
            },
            client,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ActivityParams {
    action: String,
    address: Option<String>,
    activity_type: Option<String>,
    chain: Option<String>,
    large_only: Option<bool>,
    limit: Option<usize>,
}

#[async_trait]
impl Tool for WalletActivityTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, params: Value, _context: &ToolContext) -> ToolResult {
        let params: ActivityParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid parameters: {}", e)),
        };

        match params.action.as_str() {
            "recent" => {
                let filter = wallet_monitor_types::ActivityFilter {
                    address: params.address,
                    activity_type: params.activity_type,
                    chain: params.chain,
                    large_only: params.large_only.unwrap_or(false),
                    limit: Some(params.limit.unwrap_or(25)),
                    ..Default::default()
                };
                match self.client.query_activity(&filter).await {
                    Ok(entries) => format_activity_list(&entries, "Recent Activity"),
                    Err(e) => ToolResult::error(format!("Query failed: {}", e)),
                }
            }

            "large_trades" => {
                let filter = wallet_monitor_types::ActivityFilter {
                    large_only: true,
                    limit: Some(params.limit.unwrap_or(25)),
                    ..Default::default()
                };
                match self.client.query_activity(&filter).await {
                    Ok(entries) => format_activity_list(&entries, "Large Trades"),
                    Err(e) => ToolResult::error(format!("Query failed: {}", e)),
                }
            }

            "search" => {
                let filter = wallet_monitor_types::ActivityFilter {
                    address: params.address,
                    activity_type: params.activity_type,
                    chain: params.chain,
                    large_only: params.large_only.unwrap_or(false),
                    limit: Some(params.limit.unwrap_or(50)),
                    ..Default::default()
                };
                match self.client.query_activity(&filter).await {
                    Ok(entries) => format_activity_list(&entries, "Search Results"),
                    Err(e) => ToolResult::error(format!("Query failed: {}", e)),
                }
            }

            "stats" => match self.client.get_activity_stats().await {
                Ok(stats) => ToolResult::success(json!({
                    "total_transactions": stats.total_transactions,
                    "large_trades": stats.large_trades,
                    "watched_wallets": stats.watched_wallets,
                    "active_wallets": stats.active_wallets,
                }).to_string()),
                Err(e) => ToolResult::error(format!("Stats query failed: {}", e)),
            },

            _ => ToolResult::error(format!(
                "Unknown action: '{}'. Use 'recent', 'large_trades', 'search', or 'stats'.",
                params.action
            )),
        }
    }

    fn safety_level(&self) -> ToolSafetyLevel {
        ToolSafetyLevel::ReadOnly
    }
}

fn format_activity_list(entries: &[ActivityEntry], title: &str) -> ToolResult {
    if entries.is_empty() {
        return ToolResult::success(format!("**{}**: No activity found.", title));
    }

    let mut output = format!("**{}** ({} entries)\n\n", title, entries.len());
    for e in entries {
        let usd = e
            .usd_value
            .map(|v| format!(" (${:.0})", v))
            .unwrap_or_default();
        let large = if e.is_large_trade { " **LARGE**" } else { "" };
        let asset = e.asset_symbol.as_deref().unwrap_or("ETH");
        let amount = e.amount_formatted.as_deref().unwrap_or("?");

        match e.activity_type.as_str() {
            "swap" => {
                let from_token = e.swap_from_token.as_deref().unwrap_or("?");
                let from_amount = e.swap_from_amount.as_deref().unwrap_or("?");
                let to_token = e.swap_to_token.as_deref().unwrap_or("?");
                let to_amount = e.swap_to_amount.as_deref().unwrap_or("?");
                output.push_str(&format!(
                    "SWAP: {} {} → {} {}{}{} | {} | {}\n",
                    from_amount, from_token, to_amount, to_token, usd, large, e.chain, e.tx_hash
                ));
            }
            _ => {
                output.push_str(&format!(
                    "{}: {} {}{}{} | {} → {} | {} | {}\n",
                    e.activity_type.to_uppercase(),
                    amount,
                    asset,
                    usd,
                    large,
                    &e.from_address[..e.from_address.len().min(10)],
                    &e.to_address[..e.to_address.len().min(10)],
                    e.chain,
                    e.tx_hash
                ));
            }
        }
    }
    ToolResult::success(output)
}

// =====================================================
// WalletMonitorControlTool
// =====================================================

pub struct WalletMonitorControlTool {
    definition: ToolDefinition,
    client: Arc<WalletMonitorClient>,
}

impl WalletMonitorControlTool {
    pub fn new(client: Arc<WalletMonitorClient>) -> Self {
        let mut properties = HashMap::new();

        properties.insert(
            "action".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Action: 'status' to check worker health, 'trigger' to force an immediate poll".to_string(),
                default: None,
                items: None,
                enum_values: Some(vec!["status".to_string(), "trigger".to_string()]),
            },
        );

        WalletMonitorControlTool {
            definition: ToolDefinition {
                name: "wallet_monitor_control".to_string(),
                description: "Control the wallet monitor background worker. Check status or trigger an immediate poll.".to_string(),
                input_schema: ToolInputSchema {
                    schema_type: "object".to_string(),
                    properties,
                    required: vec!["action".to_string()],
                },
                group: ToolGroup::Finance,
                hidden: false,
            },
            client,
        }
    }
}

#[derive(Debug, Deserialize)]
struct ControlParams {
    action: String,
}

#[async_trait]
impl Tool for WalletMonitorControlTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, params: Value, _context: &ToolContext) -> ToolResult {
        let params: ControlParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid parameters: {}", e)),
        };

        match params.action.as_str() {
            "status" => {
                match self.client.get_status().await {
                    Ok(status) => ToolResult::success(json!({
                        "running": status.running,
                        "uptime_secs": status.uptime_secs,
                        "watched_wallets": status.watched_wallets,
                        "active_wallets": status.active_wallets,
                        "total_transactions": status.total_transactions,
                        "large_trades": status.large_trades,
                        "last_tick_at": status.last_tick_at,
                        "poll_interval_secs": status.poll_interval_secs,
                    }).to_string()),
                    Err(e) => ToolResult::error(format!("Wallet monitor service unavailable: {}", e)),
                }
            }

            "trigger" => {
                match self.client.get_status().await {
                    Ok(status) if status.running => {
                        ToolResult::success(format!(
                            "Wallet monitor service is running (uptime: {}s, poll interval: {}s). The next tick will process any pending wallets.",
                            status.uptime_secs, status.poll_interval_secs
                        ))
                    }
                    Ok(_) => ToolResult::error("Wallet monitor service is not running."),
                    Err(e) => ToolResult::error(format!("Wallet monitor service unavailable: {}", e)),
                }
            }

            _ => ToolResult::error(format!(
                "Unknown action: '{}'. Use 'status' or 'trigger'.",
                params.action
            )),
        }
    }

    fn safety_level(&self) -> ToolSafetyLevel {
        ToolSafetyLevel::Standard
    }
}
