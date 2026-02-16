---
name: polymarket_trading
description: "Explore and trade on Polymarket - search markets, check prices, place bets, manage orders."
version: 2.2.0
author: starkbot
homepage: https://docs.polymarket.com/
metadata: {"clawdbot":{"emoji":"🎲"}}
requires_tools: [polymarket_trade, select_web3_network]
tags: [polymarket, prediction-markets, trading, betting, crypto, defi, polygon]
arguments:
  action:
    description: "Action: search, trending, bet, positions, orders, cancel"
    required: false
  market:
    description: "Market question, slug, or token ID"
    required: false
  amount:
    description: "Amount in USD to bet"
    required: false
  outcome:
    description: "Outcome to bet on (YES/NO or specific outcome name)"
    required: false
---

# Polymarket Trading Guide

You can explore and trade on Polymarket prediction markets using the `polymarket_trade` tool. This includes market discovery (no wallet needed) and trading operations (requires wallet).

## Prerequisites (for Trading)

1. **Wallet Setup**: `BURNER_WALLET_BOT_PRIVATE_KEY` must be configured
2. **USDC on Polygon**: The wallet needs USDC on Polygon network for betting
3. **Token Approvals**: One-time approval needed

## 🚨 FIRST: Select the Polygon Network

**Before ANY Polymarket operation, you MUST select the Polygon network:**

```json
{"tool": "select_web3_network", "network": "polygon"}
```

This sets the `network_name` register to "polygon" (chain ID 137) which is required for all Polymarket operations. Polymarket operates exclusively on Polygon.

> **Note**: Market discovery (search, trending, get_market, get_price) works without a wallet!

---

## Market Discovery (No Wallet Needed)

> **Important**: `search_markets` and `trending_markets` return **lightweight summaries** (title, slug, description, volume) to keep context small. Use `get_market` with the slug to get **full details including outcomes and token_ids** for trading.

### Step 1: Browse Markets

**Search by keyword:**
```json
{
  "tool": "polymarket_trade",
  "action": "search_markets",
  "query": "bitcoin",
  "limit": 10
}
```

**Get trending/popular markets:**
```json
{
  "tool": "polymarket_trade",
  "action": "trending_markets",
  "limit": 10
}
```

**Filter by category:**
```json
{
  "tool": "polymarket_trade",
  "action": "search_markets",
  "tag": "crypto",
  "limit": 10
}
```

Available tags: `politics`, `crypto`, `sports`, `finance`, `science`, `entertainment`, `world`

**Pagination:** Use `offset` to page through results (default limit: 10, max: 20)
```json
{
  "tool": "polymarket_trade",
  "action": "trending_markets",
  "limit": 10,
  "offset": 10
}
```

### Step 2: Get Market Details (Required Before Trading)

Use the `slug` from search results to get full details including outcomes and token_ids:

```json
{
  "tool": "polymarket_trade",
  "action": "get_market",
  "slug": "will-bitcoin-hit-100k-in-2025"
}
```

This returns the `token_id` values needed for `place_order` and `get_price`.

### Step 3: Get Current Price for a Token

```json
{
  "tool": "polymarket_trade",
  "action": "get_price",
  "token_id": "1234567890..."
}
```

Returns midpoint, best bid/ask, spread, and orderbook depth.

---

## Trading Workflow

### Step 1: Find a Market

Search or browse for markets:

```json
{
  "tool": "polymarket_trade",
  "action": "search_markets",
  "query": "election"
}
```

### Step 2: Get Market Details

Use `get_market` with the slug to get outcomes and token_ids:

```json
{
  "tool": "polymarket_trade",
  "action": "get_market",
  "slug": "will-trump-win-2024"
}
```

### Step 3: Get Price Details

Check current price for the token_id you want to trade:

```json
{
  "tool": "polymarket_trade",
  "action": "get_price",
  "token_id": "<TOKEN_ID>"
}
```

### Step 4: Place Order

Use the `polymarket_trade` tool:

```json
{
  "tool": "polymarket_trade",
  "action": "place_order",
  "token_id": "0x...",
  "side": "buy",
  "price": 0.65,
  "size": 100,
  "order_type": "GTC"
}
```

**Parameters:**
- `token_id`: The outcome token to trade
- `side`: `buy` (bet YES) or `sell` (bet NO / exit position)
- `price`: Limit price 0.01-0.99 (0.65 = 65 cents = 65% implied probability)
- `size`: Number of shares (100 shares @ $0.65 = $65 cost)
- `order_type`: `GTC` (good till cancelled), `FOK` (fill or kill), `GTD` (good till date)

---

## Tool Actions Reference

### Discovery Actions (No Wallet Required)

| Action | Parameters | Description |
|--------|-----------|-------------|
| `search_markets` | query, tag?, limit?, offset? | Search markets (summaries only) |
| `trending_markets` | tag?, limit?, offset? | Get high-volume markets (summaries only) |
| `get_market` | slug | Get full market details with outcomes/token_ids |
| `get_price` | token_id | Get current price and orderbook |

### Trading Actions (Wallet Required)

| Action | Parameters | Description |
|--------|-----------|-------------|
| `place_order` | token_id, side, price, size | Place limit order |
| `cancel_order` | order_id | Cancel specific order |
| `cancel_all` | - | Cancel all open orders |
| `get_orders` | - | List open orders |
| `get_positions` | - | Get current holdings |
| `get_balance` | - | Get USDC balance |

### Example: Search Markets
```json
{
  "tool": "polymarket_trade",
  "action": "search_markets",
  "query": "fed interest rate",
  "limit": 5
}
```

### Example: Get Price
```json
{
  "tool": "polymarket_trade",
  "action": "get_price",
  "token_id": "1234567890..."
}
```

### Example: Place Order
```json
{
  "tool": "polymarket_trade",
  "action": "place_order",
  "token_id": "1234567890...",
  "side": "buy",
  "price": 0.55,
  "size": 50
}
```

### Example: Get Open Orders
```json
{
  "tool": "polymarket_trade",
  "action": "get_orders"
}
```

### Example: Cancel All Orders
```json
{
  "tool": "polymarket_trade",
  "action": "cancel_all"
}
```

### Example: Get Positions
```json
{
  "tool": "polymarket_trade",
  "action": "get_positions"
}
```

### Example: Get Balance
```json
{
  "tool": "polymarket_trade",
  "action": "get_balance"
}
```

---

## Understanding Prices & Outcomes

### Binary Markets (YES/NO)
- Price represents implied probability
- 0.65 price = 65% implied chance of YES
- YES + NO prices should sum to ~1.00
- Buy YES if you think probability > current price
- Buy NO if you think probability < current price

### Multi-Outcome Markets
- Each outcome has its own token
- Prices represent relative probabilities
- Sum of all outcomes ~1.00

### Calculating Costs & Payouts

| Action | Cost | Max Payout |
|--------|------|------------|
| Buy 100 YES @ $0.65 | $65 | $100 (if YES wins) |
| Buy 100 NO @ $0.35 | $35 | $100 (if NO wins) |

**Profit if correct:** `(1.00 - price) × size`
**Loss if wrong:** `price × size`

---

## Risk Management Rules

1. **Check Spread First**: Wide spreads (>5%) indicate low liquidity
2. **Use Limit Orders**: Avoid market orders to prevent slippage
3. **Position Sizing**: Never bet more than you can afford to lose
4. **Verify Token ID**: Double-check you're trading the right outcome
5. **Monitor Positions**: Check `get_positions` regularly

---

## Example Trading Session

### User: "Bet $20 on YES for the Bitcoin $100k market"

**Agent Workflow:**

1. **Select Polygon network:**
```json
{"tool": "select_web3_network", "network": "polygon"}
```
This MUST be done first for any Polymarket operation.

2. **Search for market:**
```json
{"tool": "polymarket_trade", "action": "search_markets", "query": "bitcoin 100k"}
```
Returns lightweight list with slugs.

3. **Get market details (for token_id):**
```json
{"tool": "polymarket_trade", "action": "get_market", "slug": "will-bitcoin-hit-100k-in-2025"}
```
Returns outcomes with token_ids.

4. **Check price and spread:**
```json
{"tool": "polymarket_trade", "action": "get_price", "token_id": "<TOKEN_ID>"}
```

5. **Place order** (if spread acceptable):
```json
{
  "tool": "polymarket_trade",
  "action": "place_order",
  "token_id": "<TOKEN_ID>",
  "side": "buy",
  "price": 0.45,
  "size": 44
}
```
(44 shares × $0.45 = ~$20)

6. **Confirm to user:**
"Placed order to buy 44 YES shares at $0.45 (45% implied probability). If Bitcoin hits $100k, you'll receive $44 profit. Order ID: xxx"

---

## Contract Addresses (Polygon)

| Contract | Address |
|----------|---------|
| CTF Exchange | `0xC5d563A36AE78145C45a50134d48A1215220f80a` |
| USDC | `0x3c499c542cef5e3811e1192ce70d8cc03d5c3359` |
| Conditional Tokens | `0x4D97DCd97eC945f40cF65F87097ACe5EA0476045` |

---

## API Endpoints

| API | URL | Purpose |
|-----|----------|---------|
| Gamma | `https://gamma-api.polymarket.com` | Market discovery |
| CLOB | `https://clob.polymarket.com` | Prices, orders, trading |
| Data | `https://data-api.polymarket.com` | Positions, history |

---

## Error Handling

| Error | Cause | Solution |
|-------|-------|----------|
| "Insufficient balance" | Not enough USDC | Bridge USDC to Polygon |
| "Token not approved" | Missing approval | Run `approve_tokens` action |
| "Invalid price" | Price outside 0.01-0.99 | Use valid probability price |
| "Order rejected" | Market closed or invalid | Verify market is active |
