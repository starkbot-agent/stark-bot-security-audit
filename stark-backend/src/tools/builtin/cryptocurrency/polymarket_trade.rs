//! Polymarket CLOB Trading Tool
//!
//! Enables trading on Polymarket prediction markets using the polymarket-client-sdk.
//! Uses the burner wallet private key for signing EIP-712 orders.
//!
//! ## Actions
//! - `place_order`: Place a limit order on a market
//! - `cancel_order`: Cancel a specific order by ID
//! - `cancel_all`: Cancel all open orders
//! - `get_orders`: List open orders
//! - `get_positions`: Get current positions and balances
//! - `get_balance`: Get USDC balance and allowances on Polygon
//! - `approve_tokens`: One-time approval for USDC and CTF tokens

use crate::tools::registry::Tool;
use crate::tools::types::{
    PropertySchema, ToolContext, ToolDefinition, ToolGroup, ToolInputSchema, ToolResult,
};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;

// Polymarket SDK imports - use SDK's re-exports
use polymarket_client_sdk::auth::state::Authenticated;
use polymarket_client_sdk::auth::{LocalSigner, Normal, Signer};
use polymarket_client_sdk::clob::types::request::OrdersRequest;
use polymarket_client_sdk::clob::types::{OrderType, Side};
use polymarket_client_sdk::clob::{Client, Config as ClobConfig};
use polymarket_client_sdk::types::{Decimal, U256};
use polymarket_client_sdk::POLYGON;

/// Type alias for authenticated CLOB client
type AuthenticatedClient = Client<Authenticated<Normal>>;

/// Polymarket CLOB API base URL
const CLOB_API_URL: &str = "https://clob.polymarket.com";

/// Polymarket trading tool
pub struct PolymarketTradeTool {
    definition: ToolDefinition,
    /// Cached authenticated client (lazily initialized)
    client_cache: Arc<Mutex<Option<CachedClient>>>,
}

/// Cached authenticated client
/// Note: We don't cache the signer since creating it is cheap
struct CachedClient {
    client: AuthenticatedClient,
}

impl PolymarketTradeTool {
    pub fn new() -> Self {
        let mut properties = HashMap::new();

        properties.insert(
            "action".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Action to perform: place_order, cancel_order, cancel_all, get_orders, get_positions, get_balance".to_string(),
                default: None,
                items: None,
                enum_values: Some(vec![
                    "place_order".to_string(),
                    "cancel_order".to_string(),
                    "cancel_all".to_string(),
                    "get_orders".to_string(),
                    "get_positions".to_string(),
                    "get_balance".to_string(),
                ]),
            },
        );

        properties.insert(
            "token_id".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Token ID (condition_id) of the market outcome. Required for place_order.".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "side".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Order side: 'buy' or 'sell'. Required for place_order.".to_string(),
                default: None,
                items: None,
                enum_values: Some(vec!["buy".to_string(), "sell".to_string()]),
            },
        );

        properties.insert(
            "price".to_string(),
            PropertySchema {
                schema_type: "number".to_string(),
                description: "Limit price per share (0.01 to 0.99, e.g., 0.65 = 65 cents). Required for place_order.".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "size".to_string(),
            PropertySchema {
                schema_type: "number".to_string(),
                description: "Number of shares to buy/sell (e.g., 100 = $100 max payout). Required for place_order.".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        properties.insert(
            "order_type".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Order type: GTC (good till cancelled), FOK (fill or kill). Default: GTC.".to_string(),
                default: Some(json!("GTC")),
                items: None,
                enum_values: Some(vec!["GTC".to_string(), "FOK".to_string()]),
            },
        );

        properties.insert(
            "order_id".to_string(),
            PropertySchema {
                schema_type: "string".to_string(),
                description: "Order ID for cancellation. Required for cancel_order.".to_string(),
                default: None,
                items: None,
                enum_values: None,
            },
        );

        PolymarketTradeTool {
            definition: ToolDefinition {
                name: "polymarket_trade".to_string(),
                description: "Trade on Polymarket prediction markets. Place bets, manage orders, and check positions. Requires BURNER_WALLET_BOT_PRIVATE_KEY with USDC on Polygon.".to_string(),
                input_schema: ToolInputSchema {
                    schema_type: "object".to_string(),
                    properties,
                    required: vec!["action".to_string()],
                },
                group: ToolGroup::Finance,
            },
            client_cache: Arc::new(Mutex::new(None)),
        }
    }

    /// Get the private key from environment
    fn get_private_key() -> Result<String, String> {
        crate::config::burner_wallet_private_key()
            .ok_or_else(|| "BURNER_WALLET_BOT_PRIVATE_KEY not set. Configure this env var to trade on Polymarket.".to_string())
    }

    /// Get wallet address from private key
    fn get_wallet_address() -> Result<String, String> {
        let pk = Self::get_private_key()?;
        let pk_clean = pk.strip_prefix("0x").unwrap_or(&pk);

        // Use ethers to derive address (already in deps, simpler than alloy for this)
        let wallet: ethers::signers::LocalWallet = pk_clean
            .parse()
            .map_err(|e| format!("Invalid private key: {}", e))?;

        use ethers::signers::Signer as EthersSigner;
        Ok(format!("{:?}", wallet.address()))
    }

    /// Get or create authenticated CLOB client
    async fn get_authenticated_client(&self) -> Result<AuthenticatedClient, String> {
        // Check cache first
        {
            let cache = self.client_cache.lock().await;
            if let Some(cached) = cache.as_ref() {
                return Ok(cached.client.clone());
            }
        }

        // Create new authenticated client
        let pk = Self::get_private_key()?;
        let pk_clean = pk.strip_prefix("0x").unwrap_or(&pk);

        let signer = LocalSigner::from_str(pk_clean)
            .map(|s| s.with_chain_id(Some(POLYGON)))
            .map_err(|e| format!("Invalid private key: {}", e))?;

        let config = ClobConfig::builder()
            .use_server_time(true)
            .build();

        let client = Client::new(CLOB_API_URL, config)
            .map_err(|e| format!("Failed to create CLOB client: {}", e))?
            .authentication_builder(&signer)
            .authenticate()
            .await
            .map_err(|e| format!("Failed to authenticate with CLOB: {}", e))?;

        // Cache for future use
        {
            let mut cache = self.client_cache.lock().await;
            *cache = Some(CachedClient { client: client.clone() });
        }

        Ok(client)
    }

    /// Create a fresh signer for signing operations
    fn create_signer_for_signing() -> Result<impl Signer + Clone, String> {
        let pk = Self::get_private_key()?;
        let pk_clean = pk.strip_prefix("0x").unwrap_or(&pk);

        LocalSigner::from_str(pk_clean)
            .map(|s| s.with_chain_id(Some(POLYGON)))
            .map_err(|e| format!("Invalid private key: {}", e))
    }

    /// Place a limit order on Polymarket
    async fn place_order(&self, params: &PolymarketParams) -> ToolResult {
        // Validate required parameters
        let token_id_str = match &params.token_id {
            Some(t) => t,
            None => return ToolResult::error("token_id is required for place_order"),
        };

        let side_str = match &params.side {
            Some(s) => s.to_lowercase(),
            None => return ToolResult::error("side is required for place_order (buy or sell)"),
        };

        let price = match params.price {
            Some(p) if p > 0.0 && p < 1.0 => p,
            Some(p) => return ToolResult::error(format!(
                "price must be between 0.01 and 0.99, got {}. Price represents probability (0.65 = 65%)",
                p
            )),
            None => return ToolResult::error("price is required for place_order"),
        };

        let size = match params.size {
            Some(s) if s > 0.0 => s,
            Some(s) => return ToolResult::error(format!("size must be positive, got {}", s)),
            None => return ToolResult::error("size is required for place_order"),
        };

        let order_type_str = params.order_type.clone().unwrap_or_else(|| "GTC".to_string());

        // Parse token_id to U256
        let token_id = match U256::from_str(token_id_str) {
            Ok(t) => t,
            Err(e) => return ToolResult::error(format!("Invalid token_id: {}", e)),
        };

        // Parse side
        let side = match side_str.as_str() {
            "buy" => Side::Buy,
            "sell" => Side::Sell,
            _ => return ToolResult::error(format!("Invalid side: {}. Use 'buy' or 'sell'", side_str)),
        };

        // Parse order type
        let order_type = match order_type_str.to_uppercase().as_str() {
            "GTC" => OrderType::GTC,
            "FOK" => OrderType::FOK,
            "GTD" => OrderType::GTD,
            _ => return ToolResult::error(format!("Invalid order_type: {}. Use 'GTC', 'FOK', or 'GTD'", order_type_str)),
        };

        // Convert price and size to Decimal
        let price_decimal = match Decimal::try_from(price) {
            Ok(d) => d,
            Err(e) => return ToolResult::error(format!("Invalid price decimal: {}", e)),
        };

        let size_decimal = match Decimal::try_from(size) {
            Ok(d) => d,
            Err(e) => return ToolResult::error(format!("Invalid size decimal: {}", e)),
        };

        // Get authenticated client and signer
        let client = match self.get_authenticated_client().await {
            Ok(c) => c,
            Err(e) => return ToolResult::error(e),
        };

        let signer = match Self::create_signer_for_signing() {
            Ok(s) => s,
            Err(e) => return ToolResult::error(e),
        };

        // Build the limit order
        let order = match client
            .limit_order()
            .token_id(token_id)
            .price(price_decimal)
            .size(size_decimal)
            .side(side)
            .order_type(order_type)
            .build()
            .await
        {
            Ok(o) => o,
            Err(e) => return ToolResult::error(format!("Failed to build order: {}", e)),
        };

        // Sign the order
        let signed_order = match client.sign(&signer, order).await {
            Ok(s) => s,
            Err(e) => return ToolResult::error(format!("Failed to sign order: {}", e)),
        };

        // Submit the order
        let wallet_address = Self::get_wallet_address().unwrap_or_else(|_| "unknown".to_string());
        match client.post_order(signed_order).await {
            Ok(response) => {
                let usdc_cost = size * price;
                let result = json!({
                    "status": "success",
                    "order_id": response.order_id,
                    "success": response.success,
                    "details": {
                        "token_id": token_id_str,
                        "side": side_str,
                        "price": price,
                        "size": size,
                        "order_type": order_type_str,
                        "usdc_cost": format!("{:.2}", usdc_cost),
                        "potential_payout": format!("{:.2}", size),
                    },
                    "wallet": wallet_address,
                    "network": "polygon"
                });
                ToolResult::success(serde_json::to_string_pretty(&result).unwrap())
            }
            Err(e) => ToolResult::error(format!("Failed to submit order: {}", e))
        }
    }

    /// Cancel a specific order
    async fn cancel_order(&self, params: &PolymarketParams) -> ToolResult {
        let order_id = match &params.order_id {
            Some(id) => id,
            None => return ToolResult::error("order_id is required for cancel_order"),
        };

        let client = match self.get_authenticated_client().await {
            Ok(c) => c,
            Err(e) => return ToolResult::error(e),
        };

        match client.cancel_order(order_id).await {
            Ok(response) => {
                let result = json!({
                    "status": "success",
                    "order_id": order_id,
                    "cancelled": response.canceled,
                    "not_cancelled": response.not_canceled,
                });
                ToolResult::success(serde_json::to_string_pretty(&result).unwrap())
            }
            Err(e) => ToolResult::error(format!("Failed to cancel order: {}", e))
        }
    }

    /// Cancel all open orders
    async fn cancel_all(&self) -> ToolResult {
        let client = match self.get_authenticated_client().await {
            Ok(c) => c,
            Err(e) => return ToolResult::error(e),
        };

        match client.cancel_all_orders().await {
            Ok(response) => {
                let result = json!({
                    "status": "success",
                    "cancelled": response.canceled,
                    "not_cancelled": response.not_canceled,
                });
                ToolResult::success(serde_json::to_string_pretty(&result).unwrap())
            }
            Err(e) => ToolResult::error(format!("Failed to cancel all orders: {}", e))
        }
    }

    /// Get open orders
    async fn get_orders(&self) -> ToolResult {
        let client = match self.get_authenticated_client().await {
            Ok(c) => c,
            Err(e) => return ToolResult::error(e),
        };

        let wallet_address = Self::get_wallet_address().unwrap_or_else(|_| "unknown".to_string());

        let request = OrdersRequest::default();
        match client.orders(&request, None).await {
            Ok(orders) => {
                let orders_json: Vec<Value> = orders.data.iter().map(|o| {
                    json!({
                        "order_id": o.id,
                        "status": format!("{:?}", o.status),
                        "token_id": o.asset_id.to_string(),
                        "side": format!("{:?}", o.side),
                        "original_size": o.original_size.to_string(),
                        "size_matched": o.size_matched.to_string(),
                        "price": o.price.to_string(),
                        "outcome": o.outcome,
                        "created_at": o.created_at,
                    })
                }).collect();

                let result = json!({
                    "status": "success",
                    "count": orders.data.len(),
                    "orders": orders_json,
                    "wallet": wallet_address,
                });
                ToolResult::success(serde_json::to_string_pretty(&result).unwrap())
            }
            Err(e) => ToolResult::error(format!("Failed to fetch orders: {}", e))
        }
    }

    /// Get current positions from Data API
    async fn get_positions(&self) -> ToolResult {
        let wallet_address = match Self::get_wallet_address() {
            Ok(addr) => addr,
            Err(e) => return ToolResult::error(e),
        };

        // Fetch positions from Data API
        let http_client = reqwest::Client::new();
        let url = format!("https://data-api.polymarket.com/positions?user={}", wallet_address);

        match http_client.get(&url).send().await {
            Ok(response) => {
                match response.json::<Value>().await {
                    Ok(positions) => {
                        let result = json!({
                            "status": "success",
                            "wallet": wallet_address,
                            "positions": positions,
                        });
                        ToolResult::success(serde_json::to_string_pretty(&result).unwrap())
                    }
                    Err(e) => ToolResult::error(format!("Failed to parse positions: {}", e))
                }
            }
            Err(e) => ToolResult::error(format!("Failed to fetch positions: {}", e))
        }
    }

    /// Get balance and allowance info
    async fn get_balance(&self) -> ToolResult {
        let client = match self.get_authenticated_client().await {
            Ok(c) => c,
            Err(e) => return ToolResult::error(e),
        };

        let wallet_address = Self::get_wallet_address().unwrap_or_else(|_| "unknown".to_string());

        use polymarket_client_sdk::clob::types::request::BalanceAllowanceRequest;

        match client.balance_allowance(BalanceAllowanceRequest::default()).await {
            Ok(balance_resp) => {
                // Convert allowances HashMap to a JSON-friendly format
                let allowances: serde_json::Map<String, Value> = balance_resp.allowances
                    .iter()
                    .map(|(addr, val)| (format!("{:?}", addr), json!(val)))
                    .collect();

                let result = json!({
                    "status": "success",
                    "wallet": wallet_address,
                    "network": "polygon",
                    "balance": {
                        "usdc": balance_resp.balance.to_string(),
                        "allowances": allowances,
                    },
                    "note": "Balance in USDC (6 decimals). Divide by 1000000 for human-readable amount."
                });
                ToolResult::success(serde_json::to_string_pretty(&result).unwrap())
            }
            Err(e) => ToolResult::error(format!("Failed to fetch balance: {}", e))
        }
    }
}

#[derive(Debug, Deserialize)]
struct PolymarketParams {
    action: String,
    token_id: Option<String>,
    side: Option<String>,
    price: Option<f64>,
    size: Option<f64>,
    order_type: Option<String>,
    order_id: Option<String>,
}

#[async_trait]
impl Tool for PolymarketTradeTool {
    fn definition(&self) -> ToolDefinition {
        self.definition.clone()
    }

    async fn execute(&self, params: Value, _context: &ToolContext) -> ToolResult {
        let params: PolymarketParams = match serde_json::from_value(params) {
            Ok(p) => p,
            Err(e) => return ToolResult::error(format!("Invalid parameters: {}", e)),
        };

        match params.action.as_str() {
            "place_order" => self.place_order(&params).await,
            "cancel_order" => self.cancel_order(&params).await,
            "cancel_all" => self.cancel_all().await,
            "get_orders" => self.get_orders().await,
            "get_positions" => self.get_positions().await,
            "get_balance" => self.get_balance().await,
            _ => ToolResult::error(format!(
                "Unknown action: '{}'. Valid actions: place_order, cancel_order, cancel_all, get_orders, get_positions, get_balance",
                params.action
            )),
        }
    }
}

impl Default for PolymarketTradeTool {
    fn default() -> Self {
        Self::new()
    }
}

// Make the tool Clone-able for the registry
impl Clone for PolymarketTradeTool {
    fn clone(&self) -> Self {
        Self {
            definition: self.definition.clone(),
            client_cache: Arc::new(Mutex::new(None)), // Fresh cache for clone
        }
    }
}
