# How to Bridge USDC with Across Protocol

This guide explains how to use the `bridge_usdc` tool to move USDC between chains using Across Protocol.

## Supported Chains

| Chain | Chain ID | USDC Address |
|-------|----------|--------------|
| Ethereum | 1 | `0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48` |
| Base | 8453 | `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913` |
| Polygon | 137 | `0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359` |
| Arbitrum | 42161 | `0xaf88d065e77c8cC2239327C5EDb3A432268e5831` |
| Optimism | 10 | `0x0b2C639c533813f4Aa9D7837CAf62653d097Ff85` |

## Why Across Protocol?

- **Fast fills**: ~2 second completion via relayer network
- **Low fees**: ~0.1-0.2% for USDC bridges
- **Native CCTP**: Uses Circle's Cross-Chain Transfer Protocol for USDC
- **No API key required**: Public API with optional integrator ID

## Quick Start

### Basic Bridge Command

```json
{
  "tool": "bridge_usdc",
  "params": {
    "from_chain": "base",
    "to_chain": "polygon",
    "amount": "100"
  }
}
```

This queues transactions to bridge 100 USDC from Base to Polygon.

## Complete Bridge Flow

### Step 1: Initiate Bridge

Call the `bridge_usdc` tool:

```json
{
  "tool": "bridge_usdc",
  "params": {
    "from_chain": "base",
    "to_chain": "polygon",
    "amount": "100"
  }
}
```

**Response:**
```
BRIDGE QUEUED (not yet broadcast)

Route: base → polygon
Amount: 100 USDC
Expected: 99.82 USDC (after fees)
Est. fill time: ~2 seconds
Recipient: 0xYourWallet...

Transactions queued:
approval: a1b2c3d4-...
bridge: e5f6g7h8-...

--- Next Steps ---
To view queued: use `list_queued_web3_tx`
To broadcast: use `broadcast_web3_tx`
```

### Step 2: Review Queued Transactions

```json
{
  "tool": "list_queued_web3_tx",
  "params": {
    "status": "pending"
  }
}
```

### Step 3: Broadcast Approval (if needed)

If this is your first time bridging USDC via Across, you need to approve the spend:

```json
{
  "tool": "broadcast_web3_tx",
  "params": {
    "uuid": "a1b2c3d4-..."
  }
}
```

**Wait for the approval transaction to confirm before proceeding.**

### Step 4: Broadcast Bridge Transaction

```json
{
  "tool": "broadcast_web3_tx",
  "params": {
    "uuid": "e5f6g7h8-..."
  }
}
```

### Step 5: Receive Funds

Across relayers will fill your order on the destination chain in approximately 2 seconds. You'll receive USDC at your wallet address on Polygon.

## Advanced Options

### Bridge to Different Recipient

Send bridged USDC to a different wallet:

```json
{
  "tool": "bridge_usdc",
  "params": {
    "from_chain": "base",
    "to_chain": "arbitrum",
    "amount": "500",
    "recipient": "0x1234567890abcdef1234567890abcdef12345678"
  }
}
```

### Custom Slippage

Adjust slippage tolerance (default is 0.5%):

```json
{
  "tool": "bridge_usdc",
  "params": {
    "from_chain": "ethereum",
    "to_chain": "optimism",
    "amount": "1000",
    "slippage": 0.01
  }
}
```

## Fee Structure

Across fees consist of:

| Fee Type | Description |
|----------|-------------|
| **LP Fee** | Compensates liquidity providers (~0.04%) |
| **Relayer Capital Fee** | Compensates relayers for capital lockup (~0.01%) |
| **Destination Gas** | Gas cost on destination chain (varies) |

**Example for 100 USDC Base → Polygon:**
- Input: 100.00 USDC
- Output: ~99.82 USDC
- Total Fee: ~0.18 USDC (0.18%)

## How Across Protocol Works

```
┌─────────────────────────────────────────────────────────────────┐
│                        ACROSS BRIDGE FLOW                        │
└─────────────────────────────────────────────────────────────────┘

  BASE (Source Chain)                    POLYGON (Destination Chain)
  ───────────────────                    ─────────────────────────

  1. User deposits USDC                  3. Relayer sends USDC
     to SpokePool                           to user (~2 sec)
         │                                       ▲
         ▼                                       │
  ┌─────────────┐                        ┌─────────────┐
  │  SpokePool  │                        │  SpokePool  │
  │    Base     │                        │   Polygon   │
  └─────────────┘                        └─────────────┘
         │                                       │
         │         ┌─────────────────┐          │
         └────────►│    Relayers     │◄─────────┘
                   │  (fill orders)  │
                   └─────────────────┘
                            │
                   2. Relayer sees deposit,
                      fills on destination
                            │
                            ▼
                   ┌─────────────────┐
                   │   UMA Oracle    │
                   │  (settlement)   │
                   └─────────────────┘
                            │
                   4. Relayer reimbursed
                      from user's deposit
```

### Key Components

1. **SpokePool**: Contract on each chain that receives deposits and processes fills
2. **Relayers**: Third parties that front capital to fill orders quickly
3. **UMA Oracle**: Optimistic oracle that settles disputes and reimburses relayers

## Tracking Your Bridge

After broadcasting, track your bridge at:
- **Across Explorer**: https://across.to/transactions

Or query the status API:
```
GET https://app.across.to/api/deposit/status?depositId={id}&originChainId={chainId}
```

## Error Handling

| Error | Cause | Solution |
|-------|-------|----------|
| "Insufficient USDC balance" | Not enough USDC on source chain | Check balance, reduce amount |
| "Gas estimation failed" | Insufficient ETH for gas | Add ETH to wallet |
| "Across API error" | Route unavailable | Try smaller amount or different route |
| "Same chain" | from_chain equals to_chain | Pick different chains |

## Requirements

- **Wallet**: `BURNER_WALLET_BOT_PRIVATE_KEY` environment variable must be set
- **USDC Balance**: Sufficient USDC on source chain
- **Gas**: ETH (or native token) for transaction fees on source chain

## Example Conversation

**User**: "Bridge 50 USDC from Base to Polygon for trading on Polymarket"

**Agent**:
1. Checks USDC balance on Base
2. Calls `bridge_usdc` with from_chain=base, to_chain=polygon, amount=50
3. Reports queued transactions and expected output
4. User confirms
5. Agent broadcasts approval (if needed), waits for confirmation
6. Agent broadcasts bridge transaction
7. Reports success: "Bridged 50 USDC to Polygon. Expected arrival: ~2 seconds. You should now have ~49.91 USDC on Polygon."

## API Reference

### Across Protocol API

**Base URL**: `https://app.across.to/api`

**Get Quote & Transaction Data**:
```
GET /swap/approval
  ?tradeType=exactInput
  &amount={raw_amount}
  &inputToken={source_usdc_address}
  &originChainId={source_chain_id}
  &outputToken={dest_usdc_address}
  &destinationChainId={dest_chain_id}
  &depositor={wallet_address}
  &recipient={recipient_address}
  &slippage={slippage_pct}
```

**Response Fields**:
- `approvalTxns[]` - Token approval transactions (if needed)
- `swapTx` - Bridge transaction to execute
- `expectedOutputAmount` - Amount after fees (raw)
- `expectedFillTime` - Estimated seconds to fill
- `fees` - Detailed fee breakdown

## Related Tools

- `list_queued_web3_tx` - View pending transactions
- `broadcast_web3_tx` - Send queued transactions
- `web3_function_call` - Check USDC balance (use `erc20_balance` preset)
- `token_lookup` - Get token addresses

## Resources

- [Across Protocol Docs](https://docs.across.to/)
- [Across API Reference](https://docs.across.to/reference/api-reference)
- [Circle CCTP](https://developers.circle.com/cctp)
- [Across Explorer](https://across.to/transactions)
