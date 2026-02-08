---
name: swap
description: "Swap ERC20 tokens on Base using 0x DEX aggregator via quoter.defirelay.com"
version: 8.4.0
author: starkbot
homepage: https://0x.org
metadata: {"requires_auth": false, "clawdbot":{"emoji":"🔄"}}
tags: [crypto, defi, swap, dex, base, trading, 0x]
requires_tools: [token_lookup, to_raw_amount, decode_calldata, web3_preset_function_call, x402_fetch, x402_rpc, list_queued_web3_tx, broadcast_web3_tx, verify_tx_broadcast, select_web3_network, define_tasks]
---

# Token Swap Skill

## CRITICAL RULES

1. **ONE TASK AT A TIME.** Only do the work described in the CURRENT task. Do NOT work ahead.
2. **Do NOT call `say_to_user` with `finished_task: true` until the current task is truly done.** Using `finished_task: true` advances the task queue — if you use it prematurely, tasks get skipped.
3. **Use `say_to_user` WITHOUT `finished_task`** for progress updates. Only set `finished_task: true` OR call `task_fully_completed` when ALL steps in the current task are done.
4. **Sequential tool calls only.** Never call two tools in parallel when the second depends on the first (e.g., never call `swap_execute` and `decode_calldata` in the same response).
5. **Use exact parameter values shown.** Especially `cache_as: "swap"` — not "swap_params", not "swap_data", exactly `"swap"`.

## Step 1: Define the five tasks

Call `define_tasks` with all 5 tasks in order:

```json
{"tool": "define_tasks", "tasks": [
  "TASK 1 — Prepare: select network, look up sell+buy tokens, check Permit2 allowance. Call task_fully_completed when done. See swap skill 'Task 1'.",
  "TASK 2 — Approve Permit2 (SKIP if allowance sufficient): call erc20_approve_permit2, broadcast, wait for confirmation. See swap skill 'Task 2'.",
  "TASK 3 — Quote+Decode: call to_raw_amount, then x402_fetch swap_quote, then decode_calldata with cache_as 'swap'. ALL THREE steps required before completing. See swap skill 'Task 3'.",
  "TASK 4 — Execute: call swap_execute preset THEN broadcast_web3_tx. Exactly 2 sequential tool calls. Do NOT call decode_calldata here. See swap skill 'Task 4'.",
  "TASK 5 — Verify: call verify_tx_broadcast, report result. See swap skill 'Task 5'."
]}
```

---

## Task 1: Prepare — look up tokens, check balances, check allowance

### 1a. Select network (if user specified one)

```json
{"tool": "select_web3_network", "network": "<network>"}
```

### 1b. Look up SELL token

```json
{"tool": "token_lookup", "symbol": "<SELL_TOKEN>", "cache_as": "sell_token"}
```

**If selling ETH:** use WETH as the sell token instead:
1. Lookup WETH: `{"tool": "token_lookup", "symbol": "WETH", "cache_as": "sell_token"}`
2. Check WETH balance: `{"tool": "web3_preset_function_call", "preset": "weth_balance", "network": "<network>", "call_only": true}`
3. Check ETH balance: `{"tool": "x402_rpc", "preset": "get_balance", "network": "<network>"}`
4. If WETH insufficient, wrap:
   - `{"tool": "to_raw_amount", "amount": "<human_amount>", "decimals": 18, "cache_as": "wrap_amount"}`
   - `{"tool": "web3_preset_function_call", "preset": "weth_deposit", "network": "<network>"}`
   - Broadcast the wrap tx and wait for confirmation

### 1c. Look up BUY token

```json
{"tool": "token_lookup", "symbol": "<BUY_TOKEN>", "cache_as": "buy_token"}
```

### 1d. Check Permit2 allowance

```json
{"tool": "web3_preset_function_call", "preset": "erc20_allowance_permit2", "network": "<network>", "call_only": true}
```

### 1e. Report findings and complete

Tell the user what you found (token addresses, balances, whether approval is needed) using `say_to_user` with `finished_task: true`:

```json
{"tool": "say_to_user", "message": "Found tokens: SELL=0x... BUY=0x...\nAllowance: sufficient/insufficient", "finished_task": true}
```

**Do NOT proceed to approval or quoting in this task. Just report findings.**

---

## Task 2: Approve sell token for Permit2

**If Task 1 determined allowance is already sufficient, SKIP this task:**

```json
{"tool": "task_fully_completed", "summary": "Allowance already sufficient — skipping approval."}
```

**Otherwise, approve:**

```json
{"tool": "web3_preset_function_call", "preset": "erc20_approve_permit2", "network": "<network>"}
```

Broadcast and wait for confirmation:
```json
{"tool": "broadcast_web3_tx", "uuid": "<uuid_from_approve>"}
```

After the approval is confirmed:
```json
{"tool": "task_fully_completed", "summary": "Sell token approved for Permit2. Ready for quote."}
```

 
---

## Task 3: Get swap quote AND decode it

**This task has 3 steps. ALL THREE must complete before calling task_fully_completed.**

### 3a. Convert sell amount to raw units

```json
{"tool": "to_raw_amount", "amount": "<human_amount>", "decimals_register": "sell_token_decimals", "cache_as": "sell_amount"}
```

### 3b. Fetch swap quote

```json
{"tool": "x402_fetch", "preset": "swap_quote", "cache_as": "swap_quote", "network": "<network>"}
```

If this fails after retries, STOP and tell the user.

### 3c. Decode the quote (REQUIRED — do NOT skip this step)

**Use `cache_as: "swap"` exactly — NOT "swap_params", NOT "swap_data", exactly `"swap"`.**
This sets registers: `swap_contract`, `swap_param_0`–`swap_param_4`, `swap_value`.
Task 4 depends on these registers being set.

```json
{"tool": "decode_calldata", "abi": "0x_settler", "calldata_register": "swap_quote", "cache_as": "swap"}
```

**Only after decode_calldata succeeds**, complete the task:
```json
{"tool": "task_fully_completed", "summary": "Quote fetched and decoded. Ready to execute swap."}
```

---

## Task 4: Execute the swap

**Exactly 2 tool calls, SEQUENTIALLY (one at a time, NOT in parallel):**

**Do NOT call `decode_calldata` in this task — it was already done in Task 3.**
The registers `swap_contract`, `swap_param_0`–`swap_param_4`, `swap_value` should already be set.

### 4a. Create the swap transaction (FIRST call)

```json
{"tool": "web3_preset_function_call", "preset": "swap_execute", "network": "<network>"}
```

Wait for the result. Extract the `uuid` from the response.

### 4b. Broadcast it (SECOND call — after 4a succeeds)

```json
{"tool": "broadcast_web3_tx", "uuid": "<uuid_from_4a>"}
```

After broadcast succeeds:
```json
{"tool": "task_fully_completed", "summary": "Swap broadcast. Verifying next."}
```

---

## Task 5: Verify the swap

Call `verify_tx_broadcast` to poll for the receipt, decode token transfer events, and confirm the result matches the user's intent:

```json
{"tool": "verify_tx_broadcast"}
```

Read the output:

- **"TRANSACTION VERIFIED"** → The swap succeeded AND the AI confirmed it matches the user's intent. Report success with tx hash and explorer link.
- **"TRANSACTION CONFIRMED — INTENT MISMATCH"** → Confirmed on-chain but AI flagged a concern. Tell the user to check the explorer.
- **"TRANSACTION REVERTED"** → The swap failed. Tell the user.
- **"CONFIRMATION TIMEOUT"** → Tell the user to check the explorer link.

Call `task_fully_completed` when verify_tx_broadcast returned VERIFIED or CONFIRMED.
