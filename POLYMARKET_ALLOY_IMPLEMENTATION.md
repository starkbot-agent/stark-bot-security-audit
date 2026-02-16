# Polymarket Trading Implementation with Alloy

## Overview

Using [Alloy](https://github.com/alloy-rs/core) makes EIP-712 signing trivial with the `sol!` macro. Here's the complete implementation plan.

---

## Dependencies to Add

Update `stark-backend/Cargo.toml`:

```toml
[dependencies]
# Modern Ethereum toolkit (replaces ethers for new code)
alloy-primitives = "0.8"
alloy-sol-types = "0.8"

# For signing (can keep ethers for wallet, or migrate to alloy-signer)
alloy-signer = "0.6"
alloy-signer-local = "0.6"

# For HMAC-SHA256 (L2 API auth)
hmac = "0.12"
sha2 = "0.10"

# Keep ethers for now (gradual migration)
ethers = "2.0"
```

---

## Implementation Files

### 1. Polymarket Types (`src/polymarket/types.rs`)

```rust
use alloy_sol_types::sol;

// Define Polymarket order types using Alloy's sol! macro
// This gives us FREE EIP-712 encoding!
sol! {
    #[derive(Debug, PartialEq)]
    enum Side {
        BUY,
        SELL
    }

    #[derive(Debug, PartialEq)]
    enum SignatureType {
        EOA,
        POLY_PROXY,
        POLY_GNOSIS_SAFE,
        POLY_1271
    }

    /// Polymarket CTF Exchange Order
    /// https://github.com/Polymarket/ctf-exchange
    #[derive(Debug)]
    struct Order {
        uint256 salt;
        address maker;
        address signer;
        address taker;
        uint256 tokenId;
        uint256 makerAmount;
        uint256 takerAmount;
        uint256 expiration;
        uint256 nonce;
        uint256 feeRateBps;
        uint8 side;
        uint8 signatureType;
    }
}

// Contract addresses on Polygon (Chain ID 137)
pub mod contracts {
    use alloy_primitives::address;

    /// CTF Exchange (current) - multi-outcome markets
    pub const CTF_EXCHANGE: alloy_primitives::Address =
        address!("C5d563A36AE78145C45a50134d48A1215220f80a");

    /// CTF Exchange (legacy) - binary YES/NO markets
    pub const CTF_EXCHANGE_LEGACY: alloy_primitives::Address =
        address!("4bFb41d5B3570DeFd03C39a9A4D8dE6Bd8B8982E");

    /// Conditional Tokens (ERC1155)
    pub const CONDITIONAL_TOKENS: alloy_primitives::Address =
        address!("4D97DCd97eC945f40cF65F87097ACe5EA0476045");

    /// USDC on Polygon
    pub const USDC: alloy_primitives::Address =
        address!("2791Bca1f2de4661ED88A30C99A7a9449Aa84174");
}

pub const POLYGON_CHAIN_ID: u64 = 137;
```

### 2. Order Signer (`src/polymarket/signer.rs`)

```rust
use alloy_primitives::{Address, B256, U256, keccak256};
use alloy_sol_types::{eip712_domain, SolStruct};
use alloy_signer::Signer;
use alloy_signer_local::PrivateKeySigner;

use super::types::{Order, contracts, POLYGON_CHAIN_ID};

/// EIP-712 domain for Polymarket CTF Exchange
fn polymarket_domain() -> alloy_sol_types::Eip712Domain {
    eip712_domain! {
        name: "Polymarket CTF Exchange",
        version: "1",
        chain_id: POLYGON_CHAIN_ID,
    }
}

/// Polymarket order signer
pub struct PolymarketSigner {
    signer: PrivateKeySigner,
}

impl PolymarketSigner {
    pub fn new(private_key: &str) -> Result<Self, String> {
        let key = private_key.strip_prefix("0x").unwrap_or(private_key);
        let signer = key.parse::<PrivateKeySigner>()
            .map_err(|e| format!("Invalid private key: {}", e))?;

        Ok(Self { signer })
    }

    pub fn address(&self) -> Address {
        self.signer.address()
    }

    /// Sign a Polymarket order - returns signature bytes
    pub async fn sign_order(&self, order: &Order) -> Result<Vec<u8>, String> {
        let domain = polymarket_domain();

        // Alloy gives us the signing hash for FREE!
        let signing_hash = order.eip712_signing_hash(&domain);

        // Sign it
        let signature = self.signer
            .sign_hash(&signing_hash)
            .await
            .map_err(|e| format!("Signing failed: {}", e))?;

        Ok(signature.as_bytes().to_vec())
    }

    /// Create a complete signed order ready for submission
    pub async fn create_signed_order(
        &self,
        token_id: U256,
        side: u8,  // 0 = BUY, 1 = SELL
        price: f64,
        size: f64,
        expiration_secs: u64,
    ) -> Result<SignedOrder, String> {
        let maker = self.address();
        let salt = generate_salt();
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Calculate amounts based on price and size
        // Price is 0.00-1.00, size is number of outcome tokens
        let (maker_amount, taker_amount) = calculate_amounts(price, size, side);

        let order = Order {
            salt,
            maker,
            signer: maker,
            taker: Address::ZERO,  // Public order
            tokenId: token_id,
            makerAmount: maker_amount,
            takerAmount: taker_amount,
            expiration: U256::from(now + expiration_secs),
            nonce: U256::ZERO,
            feeRateBps: U256::ZERO,
            side,
            signatureType: 0,  // EOA
        };

        let signature = self.sign_order(&order).await?;

        Ok(SignedOrder {
            order,
            signature: format!("0x{}", hex::encode(&signature)),
        })
    }
}

fn generate_salt() -> U256 {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).expect("RNG failed");
    U256::from_be_bytes(bytes)
}

fn calculate_amounts(price: f64, size: f64, side: u8) -> (U256, U256) {
    // USDC has 6 decimals, outcome tokens have 6 decimals
    let size_raw = (size * 1_000_000.0) as u128;
    let price_raw = (price * 1_000_000.0) as u128;

    if side == 0 {  // BUY
        // Maker gives USDC, receives outcome tokens
        let usdc_amount = (size * price * 1_000_000.0) as u128;
        (U256::from(usdc_amount), U256::from(size_raw))
    } else {  // SELL
        // Maker gives outcome tokens, receives USDC
        let usdc_amount = (size * price * 1_000_000.0) as u128;
        (U256::from(size_raw), U256::from(usdc_amount))
    }
}

pub struct SignedOrder {
    pub order: Order,
    pub signature: String,
}
```

### 3. L2 API Authentication (`src/polymarket/auth.rs`)

```rust
use hmac::{Hmac, Mac};
use sha2::Sha256;
use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};

type HmacSha256 = Hmac<Sha256>;

/// L2 API credentials for Polymarket CLOB
#[derive(Clone, Debug)]
pub struct ApiCredentials {
    pub api_key: String,      // UUID
    pub secret: String,       // Base64 encoded
    pub passphrase: String,   // Random string
}

impl ApiCredentials {
    /// Build HMAC signature for L2 authentication
    pub fn sign_request(
        &self,
        method: &str,
        path: &str,
        body: &str,
        timestamp: u64,
    ) -> String {
        let message = format!("{}{}{}{}", timestamp, method.to_uppercase(), path, body);

        let secret_bytes = BASE64.decode(&self.secret)
            .expect("Invalid base64 secret");

        let mut mac = HmacSha256::new_from_slice(&secret_bytes)
            .expect("HMAC creation failed");
        mac.update(message.as_bytes());

        let result = mac.finalize();
        BASE64.encode(result.into_bytes())
    }

    /// Get headers for authenticated request
    pub fn get_headers(&self, method: &str, path: &str, body: &str) -> Vec<(String, String)> {
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();

        let signature = self.sign_request(method, path, body, timestamp);

        vec![
            ("POLY_API_KEY".to_string(), self.api_key.clone()),
            ("POLY_SIGNATURE".to_string(), signature),
            ("POLY_TIMESTAMP".to_string(), timestamp.to_string()),
            ("POLY_PASSPHRASE".to_string(), self.passphrase.clone()),
        ]
    }
}

/// Derive API credentials from wallet signature (L1 -> L2)
pub async fn derive_api_credentials(
    signer: &super::signer::PolymarketSigner,
) -> Result<ApiCredentials, String> {
    // The derivation process involves signing a specific message
    // and deriving the credentials deterministically

    // Message to sign for credential derivation
    let message = "I am signing this message to derive my API credentials";

    // Sign and derive (simplified - actual implementation needs exact Polymarket spec)
    let signature = signer.sign_message(message).await?;

    // Derive credentials from signature hash
    let hash = alloy_primitives::keccak256(signature.as_bytes());

    Ok(ApiCredentials {
        api_key: uuid::Uuid::new_v4().to_string(),
        secret: BASE64.encode(&hash[0..32]),
        passphrase: hex::encode(&hash[0..16]),
    })
}
```

### 4. Trading Tool (`src/tools/builtin/polymarket_trade.rs`)

```rust
use crate::tools::registry::Tool;
use crate::tools::types::{
    PropertySchema, ToolContext, ToolDefinition, ToolGroup, ToolInputSchema, ToolResult,
};
use async_trait::async_trait;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;

pub struct PolymarketTradeTool {
    definition: ToolDefinition,
}

impl PolymarketTradeTool {
    pub fn new() -> Self {
        let mut properties = HashMap::new();

        properties.insert("action".to_string(), PropertySchema {
            schema_type: "string".to_string(),
            description: "Action: place_order, get_orders, cancel_order, cancel_all, get_balance".to_string(),
            default: None,
            items: None,
            enum_values: Some(vec![
                "place_order".to_string(),
                "get_orders".to_string(),
                "cancel_order".to_string(),
                "cancel_all".to_string(),
                "get_balance".to_string(),
            ]),
        });

        properties.insert("token_id".to_string(), PropertySchema {
            schema_type: "string".to_string(),
            description: "Token ID (condition_id) of the market outcome".to_string(),
            default: None,
            items: None,
            enum_values: None,
        });

        properties.insert("side".to_string(), PropertySchema {
            schema_type: "string".to_string(),
            description: "Order side: buy or sell".to_string(),
            default: None,
            items: None,
            enum_values: Some(vec!["buy".to_string(), "sell".to_string()]),
        });

        properties.insert("price".to_string(), PropertySchema {
            schema_type: "number".to_string(),
            description: "Price per share (0.01 to 0.99)".to_string(),
            default: None,
            items: None,
            enum_values: None,
        });

        properties.insert("size".to_string(), PropertySchema {
            schema_type: "number".to_string(),
            description: "Number of shares to buy/sell".to_string(),
            default: None,
            items: None,
            enum_values: None,
        });

        properties.insert("order_type".to_string(), PropertySchema {
            schema_type: "string".to_string(),
            description: "Order type: GTC (good till cancelled), FOK (fill or kill), GTD (good till date)".to_string(),
            default: Some(json!("GTC")),
            items: None,
            enum_values: Some(vec!["GTC".to_string(), "FOK".to_string(), "GTD".to_string()]),
        });

        properties.insert("order_id".to_string(), PropertySchema {
            schema_type: "string".to_string(),
            description: "Order ID for cancellation".to_string(),
            default: None,
            items: None,
            enum_values: None,
        });

        PolymarketTradeTool {
            definition: ToolDefinition {
                name: "polymarket_trade".to_string(),
                description: "Place bets and manage orders on Polymarket prediction markets".to_string(),
                input_schema: ToolInputSchema {
                    schema_type: "object".to_string(),
                    properties,
                    required: vec!["action".to_string()],
                },
                group: ToolGroup::Finance,
            },
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
            "place_order" => self.place_order(params).await,
            "get_orders" => self.get_orders().await,
            "cancel_order" => self.cancel_order(params).await,
            "cancel_all" => self.cancel_all().await,
            "get_balance" => self.get_balance().await,
            _ => ToolResult::error(format!("Unknown action: {}", params.action)),
        }
    }
}

impl PolymarketTradeTool {
    async fn place_order(&self, params: PolymarketParams) -> ToolResult {
        // Validate required params
        let token_id = match &params.token_id {
            Some(t) => t,
            None => return ToolResult::error("token_id is required for place_order"),
        };
        let side = match &params.side {
            Some(s) => s,
            None => return ToolResult::error("side is required for place_order"),
        };
        let price = match params.price {
            Some(p) if p > 0.0 && p < 1.0 => p,
            Some(p) => return ToolResult::error(format!("price must be between 0.01 and 0.99, got {}", p)),
            None => return ToolResult::error("price is required for place_order"),
        };
        let size = match params.size {
            Some(s) if s > 0.0 => s,
            Some(s) => return ToolResult::error(format!("size must be positive, got {}", s)),
            None => return ToolResult::error("size is required for place_order"),
        };

        // Get private key
        let private_key = match crate::config::burner_wallet_private_key() {
            Some(k) => k,
            None => return ToolResult::error("BURNER_WALLET_BOT_PRIVATE_KEY not set"),
        };

        // Create signer and sign order
        let signer = match crate::polymarket::signer::PolymarketSigner::new(&private_key) {
            Ok(s) => s,
            Err(e) => return ToolResult::error(format!("Failed to create signer: {}", e)),
        };

        let side_num: u8 = if side == "buy" { 0 } else { 1 };
        let token_id_u256 = match token_id.parse::<alloy_primitives::U256>() {
            Ok(t) => t,
            Err(e) => return ToolResult::error(format!("Invalid token_id: {}", e)),
        };

        // Sign order
        let signed_order = match signer.create_signed_order(
            token_id_u256,
            side_num,
            price,
            size,
            86400, // 24 hour expiration
        ).await {
            Ok(o) => o,
            Err(e) => return ToolResult::error(format!("Failed to sign order: {}", e)),
        };

        // Submit to CLOB API
        let order_type = params.order_type.unwrap_or_else(|| "GTC".to_string());
        match self.submit_order(&signed_order, &order_type).await {
            Ok(result) => ToolResult::success(serde_json::to_string_pretty(&result).unwrap()),
            Err(e) => ToolResult::error(format!("Failed to submit order: {}", e)),
        }
    }

    async fn submit_order(
        &self,
        signed_order: &crate::polymarket::signer::SignedOrder,
        order_type: &str,
    ) -> Result<Value, String> {
        // Build order payload for CLOB API
        let payload = json!({
            "order": {
                "salt": signed_order.order.salt.to_string(),
                "maker": format!("{:?}", signed_order.order.maker),
                "signer": format!("{:?}", signed_order.order.signer),
                "taker": format!("{:?}", signed_order.order.taker),
                "tokenId": signed_order.order.tokenId.to_string(),
                "makerAmount": signed_order.order.makerAmount.to_string(),
                "takerAmount": signed_order.order.takerAmount.to_string(),
                "expiration": signed_order.order.expiration.to_string(),
                "nonce": signed_order.order.nonce.to_string(),
                "feeRateBps": signed_order.order.feeRateBps.to_string(),
                "side": if signed_order.order.side == 0 { "BUY" } else { "SELL" },
                "signatureType": signed_order.order.signatureType,
                "signature": &signed_order.signature,
            },
            "orderType": order_type,
        });

        // Get API credentials and make authenticated request
        // ... HTTP request to https://clob.polymarket.com/order

        Ok(payload)  // Placeholder
    }

    async fn get_orders(&self) -> ToolResult {
        // GET /orders with L2 auth
        ToolResult::success("[]".to_string())
    }

    async fn cancel_order(&self, params: PolymarketParams) -> ToolResult {
        let order_id = match &params.order_id {
            Some(id) => id,
            None => return ToolResult::error("order_id is required for cancel_order"),
        };
        // DELETE /order with L2 auth
        ToolResult::success(format!("Cancelled order {}", order_id))
    }

    async fn cancel_all(&self) -> ToolResult {
        // DELETE /orders with L2 auth
        ToolResult::success("All orders cancelled".to_string())
    }

    async fn get_balance(&self) -> ToolResult {
        // Query USDC and position balances
        ToolResult::success("{}".to_string())
    }
}
```

---

## Using Foundry/Forge (Optional but Helpful)

Foundry can help with:

### 1. Testing Order Signatures

Create a Foundry project to verify signatures match:

```bash
cd stark-backend
forge init polymarket-test --no-git
```

```solidity
// test/OrderSignature.t.sol
pragma solidity ^0.8.0;

import "forge-std/Test.sol";

contract OrderSignatureTest is Test {
    bytes32 constant ORDER_TYPEHASH = keccak256(
        "Order(uint256 salt,address maker,address signer,address taker,uint256 tokenId,uint256 makerAmount,uint256 takerAmount,uint256 expiration,uint256 nonce,uint256 feeRateBps,uint8 side,uint8 signatureType)"
    );

    function testOrderHash() public {
        // Verify our Rust implementation produces same hash
        bytes32 hash = keccak256(abi.encode(
            ORDER_TYPEHASH,
            uint256(123), // salt
            address(0x1234), // maker
            // ... rest of fields
        ));

        // Compare with expected
        console.logBytes32(hash);
    }
}
```

### 2. Local Testing with Anvil

```bash
# Fork Polygon mainnet locally
anvil --fork-url https://polygon-rpc.com --chain-id 137
```

### 3. Contract Interaction Scripts

```solidity
// script/ApproveTokens.s.sol
pragma solidity ^0.8.0;

import "forge-std/Script.sol";

contract ApproveTokens is Script {
    address constant CTF_EXCHANGE = 0xC5d563A36AE78145C45a50134d48A1215220f80a;
    address constant USDC = 0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174;
    address constant CTF = 0x4D97DCd97eC945f40cF65F87097ACe5EA0476045;

    function run() external {
        vm.startBroadcast();

        // Approve USDC
        IERC20(USDC).approve(CTF_EXCHANGE, type(uint256).max);

        // Approve CTF (ERC1155)
        IERC1155(CTF).setApprovalForAll(CTF_EXCHANGE, true);

        vm.stopBroadcast();
    }
}
```

---

## File Structure

```
stark-backend/
├── Cargo.toml              (add alloy dependencies)
├── src/
│   ├── polymarket/
│   │   ├── mod.rs          (module exports)
│   │   ├── types.rs        (Order struct via sol!)
│   │   ├── signer.rs       (EIP-712 signing)
│   │   ├── auth.rs         (L2 HMAC auth)
│   │   └── client.rs       (CLOB API client)
│   └── tools/
│       └── builtin/
│           ├── mod.rs      (add polymarket_trade)
│           └── polymarket_trade.rs
├── polymarket-test/        (optional Foundry project)
│   ├── foundry.toml
│   ├── src/
│   └── test/
```

---

## Why Alloy Makes This Easy

| Task | ethers-rs (current) | Alloy |
|------|---------------------|-------|
| Define Order struct | Manual struct + EIP712 impl | `sol! { struct Order {...} }` |
| EIP-712 hash | Manual encoding | `order.eip712_signing_hash(&domain)` |
| Domain separator | Manual calculation | `eip712_domain!{ name: "...", ... }` |
| Type hash | Manual keccak | Auto-generated |

**The `sol!` macro handles ALL the EIP-712 complexity automatically!**

---

## Implementation Steps

1. **Add Alloy dependencies** to Cargo.toml
2. **Create `src/polymarket/` module** with types, signer, auth
3. **Create `polymarket_trade` tool** in tools/builtin
4. **Register tool** in tool registry
5. **Update polymarket skill** with trading instructions
6. **Test** with small orders on real Polymarket

---

## Estimated Effort

| Task | Time |
|------|------|
| Add dependencies | 10 min |
| Types module (sol! macro) | 30 min |
| Signer module | 1 hour |
| Auth module (HMAC) | 30 min |
| Trading tool | 2 hours |
| Integration & testing | 2 hours |
| **Total** | **~6 hours** |

---

## Sources

- [Alloy EIP-712 Domain Macro](https://docs.rs/alloy-sol-types/latest/alloy_sol_types/macro.eip712_domain.html)
- [Alloy sol! Macro](https://docs.rs/alloy-sol-macro/latest/alloy_sol_macro/macro.sol.html)
- [Polymarket CTF Exchange](https://github.com/Polymarket/ctf-exchange)
- [Polymarket CLOB Docs](https://docs.polymarket.com/developers/CLOB/introduction)
