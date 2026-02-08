---
name: aave
description: "Lend and earn yield on Aave V3 (Base) — supply USDC for APY, check positions, withdraw."
version: 1.0.0
author: starkbot
homepage: https://aave.com
metadata: {"requires_auth": false, "clawdbot":{"emoji":"👻"}}
requires_tools: [token_lookup, to_raw_amount, web3_preset_function_call, list_queued_web3_tx, broadcast_web3_tx, verify_tx_broadcast, select_web3_network, define_tasks, web_fetch]
tags: [crypto, defi, finance, lending, aave, yield, apy, base, usdc]
---

# Aave V3 Lending on Base

Supply USDC (or other tokens) to Aave V3 on Base to earn yield, check your lending positions, and withdraw.

## CRITICAL RULES

1. **ONE TASK AT A TIME.** Only do the work described in the CURRENT task. Do NOT work ahead.
2. **Do NOT call `say_to_user` with `finished_task: true` until the current task is truly done.**
3. **Sequential tool calls only.** Never call two tools in parallel when the second depends on the first.

## Key Addresses (Base)

| Contract | Address |
|----------|---------|
| Aave V3 Pool | `0xA238Dd80C259a72e81d7e4664a9801593F98d1c5` |
| USDC | `0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913` |
| aBasUSDC (aToken) | `0x4e65fE4DbA92790696d040ac24Aa414708F5c0AB` |

---

## Operation A: Check Current APY

Use DeFi Llama to get current USDC supply APY on Aave V3 Base:

```tool:web_fetch
url: https://yields.llama.fi/pools
method: GET
extract_mode: raw
```

From the response, filter for pools where `project` is `"aave-v3"` and `chain` is `"Base"` and `symbol` contains `"USDC"`. Report the `apy` field as the current supply APY.

---

## Operation B: Check Staked/Supplied Balance

To check how much the user has supplied to Aave, query the aToken balance (aBasUSDC for USDC):

### 1. Set the aToken address in register

```json
{"tool": "token_lookup", "symbol": "USDC", "cache_as": "token_address"}
```

Then set the aToken address for balance check:

```json
{"tool": "set_address", "register": "token_address", "address": "0x4e65fE4DbA92790696d040ac24Aa414708F5c0AB"}
```

### 2. Check aToken balance

```tool:web3_preset_function_call
preset: erc20_balance
network: base
call_only: true
```

The aToken balance represents the user's supplied amount plus accrued interest. USDC has 6 decimals, so divide the raw value by 1e6 for the human-readable amount.

---

## Operation C: Supply USDC to Aave

### Define tasks

```json
{"tool": "define_tasks", "tasks": [
  "TASK 1 — Prepare: select Base network, look up USDC, check USDC balance, check Aave Pool allowance. See aave skill 'Task 1'.",
  "TASK 2 — Approve Aave Pool (SKIP if allowance sufficient): approve USDC for Aave Pool, broadcast, wait. See aave skill 'Task 2'.",
  "TASK 3 — Supply: convert amount, call aave_supply preset, broadcast, verify. See aave skill 'Task 3'."
]}
```

### Task 1: Prepare — look up token, check balance, check allowance

#### 1a. Select network

```json
{"tool": "select_web3_network", "network": "base"}
```

#### 1b. Look up USDC

```json
{"tool": "token_lookup", "symbol": "USDC", "cache_as": "token_address"}
```

#### 1c. Check USDC balance

```tool:web3_preset_function_call
preset: erc20_balance
network: base
call_only: true
```

#### 1d. Check Aave Pool allowance

```tool:web3_preset_function_call
preset: aave_allowance_pool
network: base
call_only: true
```

#### 1e. Report and complete

Tell the user their USDC balance and whether approval is needed. Complete with `finished_task: true`.

---

### Task 2: Approve USDC for Aave Pool

**If Task 1 determined allowance is already sufficient, SKIP:**

```json
{"tool": "task_fully_completed", "summary": "Allowance already sufficient — skipping approval."}
```

**Otherwise, approve:**

```tool:web3_preset_function_call
preset: aave_approve_pool
network: base
```

Broadcast and wait for confirmation:

```json
{"tool": "broadcast_web3_tx", "uuid": "<uuid_from_approve>"}
```

After confirmed:

```json
{"tool": "task_fully_completed", "summary": "USDC approved for Aave V3 Pool. Ready to supply."}
```

---

### Task 3: Supply USDC

#### 3a. Convert amount to raw units

```json
{"tool": "to_raw_amount", "amount": "<human_amount>", "decimals": 6, "cache_as": "aave_supply_amount"}
```

#### 3b. Execute supply

```tool:web3_preset_function_call
preset: aave_supply
network: base
```

#### 3c. Broadcast

```json
{"tool": "broadcast_web3_tx", "uuid": "<uuid_from_supply>"}
```

#### 3d. Verify

```json
{"tool": "verify_tx_broadcast"}
```

Report result and complete.

---

## Operation D: Withdraw from Aave

### Define tasks

```json
{"tool": "define_tasks", "tasks": [
  "TASK 1 — Prepare: select Base network, look up USDC, check aToken balance. See aave skill 'Withdraw Task 1'.",
  "TASK 2 — Withdraw: convert amount (or use max), call aave_withdraw, broadcast, verify. See aave skill 'Withdraw Task 2'."
]}
```

### Withdraw Task 1: Prepare

#### 1a. Select network

```json
{"tool": "select_web3_network", "network": "base"}
```

#### 1b. Look up USDC

```json
{"tool": "token_lookup", "symbol": "USDC", "cache_as": "token_address"}
```

#### 1c. Check aToken balance (supplied amount)

```json
{"tool": "set_address", "register": "token_address", "address": "0x4e65fE4DbA92790696d040ac24Aa414708F5c0AB"}
```

```tool:web3_preset_function_call
preset: erc20_balance
network: base
call_only: true
```

Report the supplied balance. Complete with `finished_task: true`.

---

### Withdraw Task 2: Execute withdrawal

#### 2a. Set USDC as token_address (for the withdraw call)

```json
{"tool": "token_lookup", "symbol": "USDC", "cache_as": "token_address"}
```

#### 2b. Convert amount to raw units

For a specific amount:
```json
{"tool": "to_raw_amount", "amount": "<human_amount>", "decimals": 6, "cache_as": "aave_withdraw_amount"}
```

To withdraw ALL, use max uint256:
```json
{"tool": "to_raw_amount", "amount": "115792089237316195423570985008687907853269984665640564039457584007913129639935", "decimals": 0, "cache_as": "aave_withdraw_amount"}
```

#### 2c. Execute withdraw

```tool:web3_preset_function_call
preset: aave_withdraw
network: base
```

#### 2d. Broadcast

```json
{"tool": "broadcast_web3_tx", "uuid": "<uuid_from_withdraw>"}
```

#### 2e. Verify

```json
{"tool": "verify_tx_broadcast"}
```

Report result and complete.

---

## Error Handling

| Error | Cause | Solution |
|-------|-------|----------|
| Insufficient USDC | Not enough USDC to supply | Check balance first, reduce amount |
| Insufficient gas | Not enough ETH for gas | Need ETH on Base for gas |
| Allowance too low | USDC not approved for Pool | Run approval task first |
| Withdraw exceeds balance | Trying to withdraw more than supplied | Use "withdraw all" or reduce amount |

---

## How Aave Lending Works

1. **Supply**: You deposit USDC into the Aave Pool. You receive aBasUSDC (aTokens) representing your deposit.
2. **Earn**: Your aToken balance grows over time as interest accrues. The APY is variable based on utilization.
3. **Withdraw**: Redeem aTokens for the underlying USDC plus earned interest.

Benefits:
- Variable APY on USDC deposits
- No lock-up — withdraw anytime
- Battle-tested protocol (Aave V3)
- Earn yield on idle stablecoins
