---
name: swap
description: "Swap ERC20 tokens on Base using 0x DEX aggregator via quoter.defirelay.com"
version: 6.1.0
author: starkbot
homepage: https://0x.org
metadata: {"requires_auth": false, "clawdbot":{"emoji":"🔄"}}
tags: [crypto, defi, swap, dex, base, trading, 0x]
requires_tools: [token_lookup, register_set, decode_calldata, web3_preset_function_call, web3_function_call, x402_fetch, x402_rpc, list_queued_web3_tx, broadcast_web3_tx, select_web3_network]
---

# Token Swap Skill
---

## The Swap Execution Pipeline (EXACT JSON — use verbatim)

Every swap ends with these 5 calls. All data flows through internal registers automatically. You do NOT touch, read, copy, or forward any hex data.

**Call 1 — Set sell amount (use `to_raw_amount` to convert human amounts to wei):**
```json
{"tool": "to_raw_amount", "amount": "<human_amount>", "decimals_register": "sell_token_decimals", "cache_as": "sell_amount"}
```
`token_lookup` with `cache_as: "sell_token"` automatically sets `sell_token_decimals`, so `to_raw_amount` can read it.
If you already know the exact wei value, you can use `register_set` instead:
```json
{"tool": "register_set", "key": "sell_amount", "value": "<amount_in_wei>"}
```

**Call 2 — Fetch quote (data is stored internally in the `swap_quote` register — do NOT extract anything from the response):**
```json
{"tool": "x402_fetch", "preset": "swap_quote", "cache_as": "swap_quote", "network": "<network>"}
```

**Call 3 — Decode quote (reads from `swap_quote` register, auto-sets `swap_param_0`–`swap_param_4`, `swap_contract`, `swap_value`, `swap_function`):**
```json
{"tool": "decode_calldata", "abi": "0x_settler", "calldata_register": "swap_quote", "cache_as": "swap"}
```

**Call 4 — Execute swap (reads from registers set by decode_calldata — pass ONLY preset and network, nothing else):**
```json
{"tool": "web3_preset_function_call", "preset": "swap_execute", "network": "<network>"}
```

**Call 5 — Broadcast:**
```json
{"tool": "broadcast_web3_tx", "uuid": "<uuid_from_call_4>"}
```

If x402_fetch fails after automatic retries, STOP. Tell the user the swap cannot be completed.

---

## Before the Pipeline: Preparation

Before running Calls 1-5, you need to set up tokens and allowances.

### Network selection (if user specified one)

```json
{"tool": "select_web3_network", "network": "<network>"}
```

### Token lookup

Sell token and buy token must be set via `token_lookup` (NOT `register_set`):
```json
{"tool": "token_lookup", "symbol": "<SELL_TOKEN>", "cache_as": "sell_token"}
```
```json
{"tool": "token_lookup", "symbol": "<BUY_TOKEN>", "cache_as": "buy_token"}
```

### If selling ETH: wrap to WETH first

The swap uses WETH, not native ETH. If selling ETH:
1. Lookup WETH as sell_token: `{"tool": "token_lookup", "symbol": "WETH", "cache_as": "sell_token"}`
2. Check WETH balance: `{"tool": "web3_preset_function_call", "preset": "weth_balance", "network": "<network>", "call_only": true}`
3. Check ETH balance: `{"tool": "x402_rpc", "preset": "get_balance", "network": "<network>"}`
4. If WETH insufficient, set wrap amount and wrap:
   - `{"tool": "to_raw_amount", "amount": "<human_amount>", "decimals": 18, "cache_as": "wrap_amount"}`
   - `{"tool": "web3_preset_function_call", "preset": "weth_deposit", "network": "<network>"}`
   - Broadcast the wrap tx and wait for confirmation

### Allowance check (REQUIRED before every swap)

The sell token must be approved for Permit2. This is the #1 cause of reverted swaps.

1. Set token address: `{"tool": "register_set", "key": "token_address", "value": "<sell_token_address>"}`
2. Set spender: `{"tool": "register_set", "key": "spender_address", "value": "0x000000000022D473030F116dDEE9F6B43aC78BA3"}`
3. Check allowance: `{"tool": "web3_preset_function_call", "preset": "erc20_allowance", "network": "<network>", "call_only": true}`
4. If allowance < sell amount, approve:
   - `{"tool": "register_set", "key": "approve_amount", "value": "115792089237316195423570985008687907853269984665640564039457584007913129639935"}`
   - `{"tool": "web3_preset_function_call", "preset": "erc20_approve", "network": "<network>"}`
   - Broadcast and wait for confirmation

WETH is especially prone to zero allowance after wrapping — always check!

---

## Which preparation path?

| Selling | Steps |
|---------|-------|
| ETH | Lookup WETH as sell_token → check balances → wrap if needed → check allowance → approve if needed → lookup buy_token → **run pipeline** |
| Any token (USDC, WETH, etc.) | Lookup sell_token → check balance → check allowance → approve if needed → lookup buy_token → **run pipeline** |

---

## Converting Amounts

Use `to_raw_amount` to convert human-readable amounts to wei. Do NOT calculate wei manually.

```json
{"tool": "to_raw_amount", "amount": "1.5", "decimals_register": "sell_token_decimals", "cache_as": "sell_amount"}
```

`token_lookup` automatically sets `sell_token_decimals` (or `buy_token_decimals`), so `to_raw_amount` reads the correct decimals. Common decimals: ETH/WETH = 18, USDC = 6.

---
