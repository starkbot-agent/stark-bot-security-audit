use crate::tools::registry::Tool;
use crate::tools::types::{
    PropertySchema, ToolContext, ToolDefinition, ToolGroup, ToolInputSchema, ToolResult,
};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Tool for reading recent cryptocurrency transactions
pub struct ReadRecentTransactionsTool {
    definition: ToolDefinition,
}

impl ReadRecentTransactionsTool {
    pub fn new() -> Self {
        let mut properties = HashMap::new();

        properties.insert(
            "limit".to_string(),
            PropertySchema {
                schema_type: "integer".to_string(),
                description: "Maximum number of transactions to return (default 10, max 50)"
                    .to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "status".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Filter by transaction status".to_string(),
                default: None,
                items: None,
                enum_values: Some(vec![
                    "broadcast".to_string(),
                    "confirmed".to_string(),
                    "failed".to_string(),
                ]),
            },
        );

        properties.insert(
            "network".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Filter by network (e.g. 'base', 'polygon', 'mainnet')".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        ReadRecentTransactionsTool {
            definition: ToolDefinition {
                name: "read_recent_transactions".to_string(),
                description: "Read recent cryptocurrency transactions with optional filtering by status and network. Returns transaction details including hash, value, and explorer links.".to_string(),
                input_schema: ToolInputSchema {
                    schema_type: "object".to_string(),
                    properties,
                    required: vec![],
                },
                group: ToolGroup::System,
                hidden: false,
            },
        }
    }
}

impl Default for ReadRecentTransactionsTool {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Deserialize)]
struct ReadRecentTransactionsParams {
    limit: Option<i64>,
    status: Option<String>,
    network: Option<String>,
}

#[async_trait]
impl Tool for ReadRecentTransactionsTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, params: Value, context: &ToolContext) -> ToolResult {
        let params: ReadRecentTransactionsParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid parameters: {}", e)),
        };

        let db = match &context.database {
            Some(db) => db,
            None => return ToolResult::error("Database not available"),
        };

        let limit = params.limit.unwrap_or(10).min(50).max(1) as usize;

        match db.list_broadcasted_transactions(
            params.status.as_deref(),
            params.network.as_deref(),
            None, // broadcast_mode
            Some(limit),
        ) {
            Ok(transactions) => {
                if transactions.is_empty() {
                    return ToolResult::success("No transactions found matching the criteria.")
                        .with_metadata(json!({ "count": 0 }));
                }

                let mut lines = Vec::new();
                let mut tx_data = Vec::new();

                for tx in &transactions {
                    let date = tx.broadcast_at.format("%Y-%m-%d %H:%M UTC").to_string();
                    let hash_display = tx
                        .tx_hash
                        .as_deref()
                        .map(|h| {
                            if h.len() > 14 {
                                format!("{}...{}", &h[..8], &h[h.len() - 4..])
                            } else {
                                h.to_string()
                            }
                        })
                        .unwrap_or_else(|| "pending".to_string());

                    let from_short = if tx.from_address.len() > 12 {
                        format!(
                            "{}...{}",
                            &tx.from_address[..6],
                            &tx.from_address[tx.from_address.len() - 4..]
                        )
                    } else {
                        tx.from_address.clone()
                    };
                    let to_short = if tx.to_address.len() > 12 {
                        format!(
                            "{}...{}",
                            &tx.to_address[..6],
                            &tx.to_address[tx.to_address.len() - 4..]
                        )
                    } else {
                        tx.to_address.clone()
                    };

                    lines.push(format!(
                        "{} | {} | {} → {} | {} | {} | {}",
                        date,
                        tx.network,
                        from_short,
                        to_short,
                        tx.value_formatted,
                        tx.status,
                        tx.explorer_url.as_deref().unwrap_or(&hash_display),
                    ));

                    tx_data.push(json!({
                        "uuid": tx.uuid,
                        "network": tx.network,
                        "from": tx.from_address,
                        "to": tx.to_address,
                        "value": tx.value,
                        "value_formatted": tx.value_formatted,
                        "tx_hash": tx.tx_hash,
                        "explorer_url": tx.explorer_url,
                        "status": tx.status.to_string(),
                        "broadcast_mode": tx.broadcast_mode.to_string(),
                        "broadcast_at": tx.broadcast_at.to_rfc3339(),
                    }));
                }

                ToolResult::success(format!(
                    "Recent transactions ({}):\n\n{}",
                    transactions.len(),
                    lines.join("\n")
                ))
                .with_metadata(json!({
                    "count": transactions.len(),
                    "transactions": tx_data
                }))
            }
            Err(e) => ToolResult::error(format!("Failed to query transactions: {}", e)),
        }
    }
}
