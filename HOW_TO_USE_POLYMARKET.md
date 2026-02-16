# How to Use Polymarket with Starkbot

This guide explains how to trade on Polymarket prediction markets using Starkbot.

## Prerequisites

### 1. Set Up Your Wallet

You need a wallet with USDC on the **Polygon network**.

```bash
# Set your private key in the environment
export BURNER_WALLET_BOT_PRIVATE_KEY="0x..."
```

This is the same wallet used for other Starkbot crypto operations (send_eth, web3_function_call, etc.).

### 2. Fund Your Wallet

1. Get USDC on Polygon (bridge from Ethereum or buy on an exchange)
2. Send USDC to your burner wallet address
3. Keep some MATIC for gas (though most Polymarket operations are gasless)

### 3. Approve Tokens (One-Time Setup)

Before placing your first order, you need to approve the CTF Exchange to spend your USDC:

```
You: "Set up my wallet for Polymarket trading"
Bot: Uses polymarket_trade action=get_balance to show approval info
```

Or manually approve via `web3_function_call` on Polygon:
- USDC Contract: `0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174`
- CTF Exchange: `0xC5d563A36AE78145C45a50134d48A1215220f80a`

---

## Basic Usage

### Browse Markets (No Wallet Needed!)

Market discovery works without any wallet setup:

```
You: "Show me trending prediction markets"
Bot: Uses polymarket_trade action=trending_markets
```

```
You: "Find markets about Bitcoin"
Bot: Uses polymarket_trade action=search_markets query="bitcoin"
```

```
You: "What crypto markets are available?"
Bot: Uses polymarket_trade action=search_markets tag="crypto"
```

### Get Market Details

```
You: "Tell me about the Bitcoin 100k market"
Bot: Uses polymarket_trade action=get_market slug="will-bitcoin-hit-100k"
```

### Get Current Prices

```
You: "What's the current price on that market?"
Bot: Uses polymarket_trade action=get_price token_id="..."
```

Returns midpoint price, best bid/ask, spread, and orderbook depth.

### Check Your Balance (Requires Wallet)

```
You: "What's my Polymarket balance?"
Bot: Uses polymarket_trade action=get_balance
```

Returns your USDC balance and token allowances on Polygon.

### Place a Bet

```
You: "Bet $20 on YES for the Bitcoin $100k market at 0.45"
Bot:
  1. Finds the market and token_id
  2. Calculates shares: $20 / $0.45 = ~44 shares
  3. Places limit order via polymarket_trade
```

### Check Your Orders

```
You: "Show my open orders"
Bot: Uses polymarket_trade with action=get_orders
```

### Cancel Orders

```
You: "Cancel all my open orders"
Bot: Uses polymarket_trade with action=cancel_all
```

```
You: "Cancel order abc123"
Bot: Uses polymarket_trade with action=cancel_order order_id=abc123
```

### Check Positions

```
You: "What positions do I have on Polymarket?"
Bot: Uses polymarket_trade with action=get_positions
```

---

## Understanding Prices

Polymarket prices represent **implied probability**:

| Price | Meaning |
|-------|---------|
| 0.65 | 65% implied chance of YES |
| 0.35 | 35% implied chance of YES (or 65% chance of NO) |
| 0.01 | Very unlikely (1%) |
| 0.99 | Almost certain (99%) |

### Calculating Costs and Payouts

| Action | Cost | Payout if Correct |
|--------|------|-------------------|
| Buy 100 YES @ $0.65 | $65 | $100 |
| Buy 100 NO @ $0.35 | $35 | $100 |
| Buy 50 YES @ $0.40 | $20 | $50 |

**Profit = Payout - Cost**

---

## Tool Reference

### polymarket_trade

The main tool for market discovery and trading.

#### Discovery Actions (No Wallet Required)

| Action | Parameters | Description |
|--------|-----------|-------------|
| `search_markets` | query, tag?, limit? | Search markets by keyword |
| `trending_markets` | tag?, limit? | Get high-volume/popular markets |
| `get_market` | slug | Get market details by URL slug |
| `get_price` | token_id | Get current price/orderbook |

#### Trading Actions (Wallet Required)

| Action | Parameters | Description |
|--------|-----------|-------------|
| `place_order` | token_id, side, price, size | Place a limit order |
| `cancel_order` | order_id | Cancel specific order |
| `cancel_all` | - | Cancel all open orders |
| `get_orders` | - | List open orders |
| `get_positions` | - | Get current holdings |
| `get_balance` | - | Get USDC balance and allowances |

#### Discovery Parameters

| Param | Type | Description |
|-------|------|-------------|
| `query` | string | Search keyword (e.g., "bitcoin", "election") |
| `slug` | string | Market URL slug (e.g., "will-bitcoin-hit-100k") |
| `tag` | string | Category filter: politics, crypto, sports, finance, science, entertainment, world |
| `limit` | integer | Max results (default: 10, max: 50) |

#### Trading Parameters

| Param | Type | Description |
|-------|------|-------------|
| `token_id` | string | Market outcome token ID (from market data) |
| `side` | string | `buy` or `sell` |
| `price` | number | 0.01 to 0.99 (probability/price per share) |
| `size` | number | Number of shares |
| `order_type` | string | `GTC` (default), `FOK`, or `GTD` |

#### Example: Search Markets

```json
{
  "tool": "polymarket_trade",
  "action": "search_markets",
  "query": "bitcoin",
  "limit": 5
}
```

#### Example: Get Price

```json
{
  "tool": "polymarket_trade",
  "action": "get_price",
  "token_id": "1234567890..."
}
```

#### Example: Place Order

```json
{
  "tool": "polymarket_trade",
  "action": "place_order",
  "token_id": "1234567890...",
  "side": "buy",
  "price": 0.55,
  "size": 100,
  "order_type": "GTC"
}
```

---

## Skills Reference

### polymarket (Read-Only)

For browsing markets and getting prices without trading.

```
Activate: "Use the polymarket skill"
```

Capabilities:
- Search markets by topic
- Get current prices and orderbooks
- View market details and resolution info

### polymarket_trading

For active trading operations.

```
Activate: "Use the polymarket_trading skill"
```

Capabilities:
- Place and manage orders
- Track positions and P&L
- Full trading workflows

---

## API Endpoints

| API | Base URL | Purpose |
|-----|----------|---------|
| Gamma | `https://gamma-api.polymarket.com` | Market discovery, metadata |
| CLOB | `https://clob.polymarket.com` | Trading, prices, orderbooks |
| Data | `https://data-api.polymarket.com` | Positions, history, user data |

---

## Contract Addresses (Polygon)

| Contract | Address |
|----------|---------|
| CTF Exchange | `0xC5d563A36AE78145C45a50134d48A1215220f80a` |
| USDC | `0x2791Bca1f2de4661ED88A30C99A7a9449Aa84174` |
| Conditional Tokens | `0x4D97DCd97eC945f40cF65F87097ACe5EA0476045` |

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  User: "Bet $20 on YES at 0.45"                            │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  Starkbot Agent                                             │
│  ├── Activates polymarket_trading skill                     │
│  ├── Finds market via Gamma API (web_fetch)                │
│  ├── Gets current price via CLOB API (web_fetch)           │
│  └── Places order via polymarket_trade tool                │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  polymarket_trade Tool                                      │
│  ├── Creates authenticated CLOB client                      │
│  ├── Builds limit order with SDK                           │
│  ├── Signs order (EIP-712) with LocalSigner                │
│  └── Submits to Polymarket CLOB API                        │
└─────────────────────────────────────────────────────────────┘
                            │
                            ▼
┌─────────────────────────────────────────────────────────────┐
│  Polymarket CLOB (clob.polymarket.com)                     │
│  ├── Validates order signature                              │
│  ├── Checks balance/allowance                              │
│  └── Adds to orderbook / executes match                    │
└─────────────────────────────────────────────────────────────┘
```

---

## SDK Integration Details

The tool uses the official `polymarket-client-sdk` Rust crate:

```toml
polymarket-client-sdk = { version = "0.4", features = ["clob", "ws", "data", "gamma", "heartbeats"] }
```

### Order Flow

1. **Build Order**: `client.limit_order().token_id(...).price(...).size(...).build().await`
2. **Sign Order**: `client.sign(&signer, order).await` (EIP-712 typed data)
3. **Submit Order**: `client.post_order(signed_order).await`

### Authentication

The SDK handles L2 authentication automatically:
- Derives API credentials from wallet signature
- Signs requests with HMAC-SHA256
- Manages session state

---

## Troubleshooting

### "Insufficient balance"
- Check USDC balance on Polygon
- Bridge more USDC to Polygon network

### "Token not approved"
- Run approval transaction for USDC to CTF Exchange
- Use `web3_function_call` with `approve(address,uint256)`

### "Order rejected"
- Market may be closed or resolved
- Price may be outside valid range (0.01-0.99)
- Size may exceed available liquidity

### "Authentication failed"
- Check `BURNER_WALLET_BOT_PRIVATE_KEY` is set correctly
- Ensure private key has `0x` prefix or valid hex format

---

## Safety Tips

1. **Start Small**: Test with small amounts first
2. **Check Spreads**: Wide spreads indicate low liquidity
3. **Use Limit Orders**: Avoid slippage with limit orders (GTC)
4. **Verify Token ID**: Always confirm you're trading the right outcome
5. **Monitor Positions**: Check `get_positions` regularly
6. **Understand Resolution**: Know how and when markets resolve

---

## Example Session

```
You: "What's my Polymarket balance?"

Bot: [First selects Polygon network with select_web3_network]
     Your Polymarket wallet (0x1234...abcd) has:
     - USDC Balance: 150.50
     - Allowances: CTF Exchange approved

You: "Find prediction markets about the Fed interest rate"

Bot: [Network already set to Polygon]
     Found 3 active markets:
     1. "Fed rate cut in March 2025" - YES: $0.35, NO: $0.65
     2. "Fed holds rates steady Q1" - YES: $0.72, NO: $0.28
     3. "Fed rate above 5% end of 2025" - YES: $0.15, NO: $0.85

You: "Buy $25 of YES on the March rate cut at 0.35"

Bot: Placing order...
     - Market: Fed rate cut in March 2025
     - Side: BUY YES
     - Price: $0.35
     - Shares: 71.4
     - Total Cost: $25.00
     - Potential Payout: $71.40

     Order placed! ID: 0xabc123...

You: "What are my positions now?"

Bot: Your Polymarket positions:
     - Fed rate cut March 2025 (YES): 71.4 shares @ $0.35 avg
       Current price: $0.38 | P&L: +$2.14 (+8.5%)
```
