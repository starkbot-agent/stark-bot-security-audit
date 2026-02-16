# Polymarket Trading Implementation Plan

## Executive Summary

To enable starkbot to **place bets on Polymarket**, we need to implement order signing and API authentication. The current `web_fetch` tool can make HTTP requests, but Polymarket requires cryptographically signed orders using EIP-712 and authenticated API calls using HMAC-SHA256.

---

## Current State Analysis

### What Starkbot Already Has

| Capability | Tool/Location | Status |
|------------|---------------|--------|
| HTTP Requests | `web_fetch` | Works for reading market data |
| EIP-712 Signing | `x402/signer.rs` | Exists but for different domain (USDC/EIP-3009) |
| Wallet Management | `BURNER_WALLET_BOT_PRIVATE_KEY` | Ready to use |
| EVM Transactions | `web3_tx` | Works for on-chain txs |
| ethers-rs | Backend dependency | Available |

### What's Missing for Trading

| Requirement | Description | Complexity |
|-------------|-------------|------------|
| **Polymarket EIP-712 Signing** | Sign orders with Polymarket's specific domain/struct | Medium |
| **HMAC-SHA256 Auth (L2)** | API key authentication for faster requests | Low |
| **API Credential Management** | Derive/store API keys, secrets, passphrases | Medium |
| **Order Construction** | Build correct order structs with all required fields | Medium |
| **Token Approvals** | One-time USDC/ERC1155 approvals on Polygon | Low (use web3_tx) |

---

## Polymarket Order Signing Requirements

### EIP-712 Domain (Different from x402)
```rust
{
    name: "ClobAuthDomain",  // NOT "USD Coin"
    version: "1",
    chainId: 137,            // Polygon, NOT Base
    verifyingContract: "0x..." // CTF Exchange address
}
```

### Order Struct to Sign
```rust
Order {
    salt: U256,           // Random unique ID
    maker: Address,       // Your wallet address
    signer: Address,      // Same as maker for EOA
    taker: Address,       // Usually 0x0
    tokenId: U256,        // Market outcome token ID
    makerAmount: U256,    // Amount you're offering
    takerAmount: U256,    // Amount you want
    expiration: U256,     // Unix timestamp
    nonce: U256,          // For on-chain cancellations
    feeRateBps: U256,     // Fee rate (usually 0)
    side: U8,             // 0=BUY, 1=SELL
    signatureType: U8,    // 0=EOA, 1=Email, 2=Proxy
}
```

### L2 Authentication (HMAC-SHA256)
```
Headers:
  POLY_API_KEY: <uuid>
  POLY_SIGNATURE: HMAC-SHA256(timestamp + method + path + body, secret)
  POLY_TIMESTAMP: <unix_timestamp>
  POLY_PASSPHRASE: <passphrase>
```

---

## Implementation Options

### Option A: Native Rust Tool (Recommended)

**Create a new `polymarket_trade` tool in Rust**

**Pros:**
- Native integration, no external dependencies
- Fast execution
- Consistent with existing codebase
- Full control over implementation

**Cons:**
- More development work upfront
- Need to maintain Polymarket-specific code

**Estimated Effort:** 2-3 days

**Files to Create/Modify:**
```
stark-backend/src/tools/builtin/
├── polymarket_trade.rs     (NEW - main trading tool)
├── polymarket_auth.rs      (NEW - L2 auth + credential management)
└── mod.rs                  (add new modules)

stark-backend/src/x402/
└── polymarket_signer.rs    (NEW - EIP-712 for Polymarket orders)

config/
└── polymarket.ron          (NEW - contract addresses, endpoints)
```

**Tool API Design:**
```json
{
  "tool": "polymarket_trade",
  "action": "place_order",
  "token_id": "0x...",
  "side": "buy",
  "price": 0.55,
  "size": 100,
  "order_type": "GTC"
}
```

---

### Option B: Python SDK Wrapper

**Use official `py-clob-client` via exec tool**

**Pros:**
- Official SDK, maintained by Polymarket
- Handles all signing complexity
- Quick to prototype

**Cons:**
- External Python dependency
- Less integrated experience
- Slower execution
- Need to manage Python environment

**Estimated Effort:** 1 day

**Files to Create:**
```
scripts/
└── polymarket_trade.py     (NEW - wrapper script)

skills/
└── polymarket.md           (UPDATE - add trading instructions)
```

**Usage:**
```json
{
  "tool": "exec",
  "command": "python scripts/polymarket_trade.py place_order --token_id=0x... --side=buy --price=0.55 --size=100"
}
```

---

### Option C: TypeScript SDK Wrapper

**Use official `@polymarket/clob-client` via Node.js**

Similar to Option B but with TypeScript/Node.js.

**Pros:**
- Official SDK
- Strong typing

**Cons:**
- Node.js dependency
- Same integration issues as Python

---

### Option D: Hybrid Approach (Recommended Path)

**Phase 1: Quick Start with Python SDK (1 day)**
- Install py-clob-client
- Create wrapper script
- Update polymarket skill
- Start trading immediately

**Phase 2: Native Rust Tool (2-3 days)**
- Implement native tool for production use
- Better integration and performance
- Remove Python dependency

---

## Recommended Implementation Plan

### Phase 1: Immediate (Python SDK)

#### Step 1: Install Python SDK
```bash
pip install py-clob-client
```

#### Step 2: Create Trading Script
Create `scripts/polymarket/trade.py`:
```python
#!/usr/bin/env python3
"""Polymarket trading script for starkbot"""

import os
import sys
import json
from py_clob_client.client import ClobClient
from py_clob_client.clob_types import OrderArgs, OrderType

def main():
    # Get private key from environment
    private_key = os.environ.get("BURNER_WALLET_BOT_PRIVATE_KEY")
    if not private_key:
        print(json.dumps({"error": "BURNER_WALLET_BOT_PRIVATE_KEY not set"}))
        sys.exit(1)

    # Initialize client
    client = ClobClient(
        host="https://clob.polymarket.com",
        key=private_key,
        chain_id=137,  # Polygon
    )

    # Get or create API credentials
    creds = client.create_or_derive_api_creds()
    client.set_api_creds(creds)

    # Parse command
    action = sys.argv[1] if len(sys.argv) > 1 else "help"

    if action == "place_order":
        # Parse args
        token_id = sys.argv[2]
        side = sys.argv[3]  # "buy" or "sell"
        price = float(sys.argv[4])
        size = float(sys.argv[5])

        order_args = OrderArgs(
            token_id=token_id,
            price=price,
            size=size,
            side=side.upper()
        )

        signed_order = client.create_order(order_args)
        result = client.post_order(signed_order, OrderType.GTC)
        print(json.dumps(result))

    elif action == "get_orders":
        orders = client.get_orders()
        print(json.dumps(orders))

    elif action == "cancel_order":
        order_id = sys.argv[2]
        result = client.cancel(order_id)
        print(json.dumps(result))

    elif action == "cancel_all":
        result = client.cancel_all()
        print(json.dumps(result))

    else:
        print(json.dumps({
            "commands": ["place_order", "get_orders", "cancel_order", "cancel_all"]
        }))

if __name__ == "__main__":
    main()
```

#### Step 3: Update Polymarket Skill
Add trading section to `skills/polymarket.md`

---

### Phase 2: Production (Native Rust)

#### Step 1: Create Polymarket Signer
Implement EIP-712 signing for Polymarket's order structure in Rust.

#### Step 2: Create Trading Tool
New `polymarket_trade` tool with actions:
- `place_order` - Create and post limit/market orders
- `get_orders` - List open orders
- `cancel_order` - Cancel specific order
- `cancel_all` - Cancel all orders
- `get_trades` - Get trade history

#### Step 3: API Credential Management
- Derive credentials from private key
- Store securely (in-memory or encrypted)
- Auto-refresh as needed

---

## Prerequisites Before Trading

### One-Time Setup

1. **Fund Wallet on Polygon**
   - Transfer USDC to your wallet on Polygon
   - Need small amount of MATIC for gas

2. **Approve Token Spending**
   ```json
   // Approve USDC for CTF Exchange (for buying)
   {
     "tool": "web3_function_call",
     "network": "polygon",
     "contract": "0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174", // USDC on Polygon
     "function": "approve",
     "args": ["0xC5d563A36AE78145C45a50134d48A1215220f80a", "unlimited"]
   }

   // Approve outcome tokens (for selling)
   {
     "tool": "web3_function_call",
     "network": "polygon",
     "contract": "0x4d97dcd97ec945f40cf65f87097ace5ea0476045", // CTF
     "function": "setApprovalForAll",
     "args": ["0xC5d563A36AE78145C45a50134d48A1215220f80a", true]
   }
   ```

3. **Verify Wallet Balance**
   Check USDC balance on Polygon before placing orders.

---

## Contract Addresses (Polygon - Chain ID 137)

| Contract | Address | Purpose |
|----------|---------|---------|
| CTF Exchange | `0xC5d563A36AE78145C45a50134d48A1215220f80a` | Order settlement |
| Conditional Tokens | `0x4d97dcd97ec945f40cf65f87097ace5ea0476045` | Outcome tokens (ERC1155) |
| USDC | `0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174` | Payment token |

---

## API Endpoints

| Endpoint | Base URL | Auth |
|----------|----------|------|
| Markets | `https://gamma-api.polymarket.com` | None |
| Trading | `https://clob.polymarket.com` | L1 or L2 |
| User Data | `https://data-api.polymarket.com` | L2 |

---

## Risk Considerations

1. **Financial Risk**: Trading real money - start with small amounts
2. **Smart Contract Risk**: Contracts are audited but not risk-free
3. **API Changes**: Polymarket may update APIs
4. **Network Issues**: Polygon congestion can delay transactions
5. **Private Key Security**: Never expose the burner wallet key

---

## Decision Required

**Which approach should we implement?**

| Option | Time | Integration | Maintenance |
|--------|------|-------------|-------------|
| A. Native Rust | 2-3 days | Best | Medium |
| B. Python SDK | 1 day | Good | Low (uses official SDK) |
| C. TypeScript | 1 day | Good | Low |
| D. Hybrid | 1 day + 2-3 days | Best | Medium |

**Recommendation**: Start with **Option B (Python SDK)** for immediate functionality, then migrate to **Option A (Native Rust)** for production.

---

## Next Steps

1. **Confirm approach** - Python SDK first, then native Rust?
2. **Set up Python environment** - Install py-clob-client
3. **Create trading script** - Wrapper for SDK
4. **Update polymarket skill** - Add trading instructions
5. **Test with small amounts** - Verify everything works
6. **Plan native implementation** - Design Rust tool API
