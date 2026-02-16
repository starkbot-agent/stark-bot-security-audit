---
name: eip8004-reputation
description: "Query and submit reputation feedback for EIP-8004 agents"
version: 1.0.0
author: starkbot
homepage: https://eips.ethereum.org/EIPS/eip-8004
metadata: {"requires_auth": true, "clawdbot":{"emoji":"⭐"}}
tags: [crypto, reputation, eip8004, feedback, trust, agent]
---

# EIP-8004 Reputation Management

Query and submit reputation feedback for agents using the EIP-8004 Reputation Registry. Build trust through on-chain feedback with payment proofs.

## Overview

The Reputation Registry enables:
- **Giving Feedback**: Rate agents after interactions
- **Payment Proofs**: Link feedback to x402 payments
- **Querying Reputation**: Check agent trustworthiness
- **Responding**: Agents can respond to feedback

## Contract Addresses (Base Mainnet)

| Contract | Address |
|----------|---------|
| Reputation Registry | `0x...` (TBD) |

---

## Querying Reputation

### Get Reputation Summary

Query an agent's overall reputation score:

**Function**:
```solidity
function getSummary(
    uint256 agentId,
    address[] calldata clientAddresses,
    string tag1,
    string tag2
) external view returns (uint64 count, int128 summaryValue, uint8 summaryValueDecimals)
```

**Using x402_rpc tool**:
```json
{
  "method": "eth_call",
  "params": [{
    "to": "<REPUTATION_REGISTRY>",
    "data": "<encoded_getSummary_call>"
  }, "latest"],
  "network": "base"
}
```

**Returns**:
- `count`: Number of feedback entries
- `summaryValue`: Aggregate score (fixed-point)
- `summaryValueDecimals`: Decimal places for score

### Read Specific Feedback

```solidity
function readFeedback(
    uint256 agentId,
    address clientAddress,
    uint64 feedbackIndex
) external view returns (
    int128 value,
    uint8 valueDecimals,
    string tag1,
    string tag2,
    bool isRevoked
)
```

---

## Giving Feedback

### Step 1: Prepare Feedback File (Optional)

For detailed feedback with payment proof, create a JSON file:

```json
{
  "agentRegistry": "eip155:8453:0x...",
  "agentId": 42,
  "clientAddress": "eip155:8453:0x<your_wallet>",
  "createdAt": "2024-01-15T10:30:00Z",
  "value": 100,
  "valueDecimals": 0,
  "tag1": "swap",
  "tag2": "usdc-eth",
  "endpoint": "https://quoter.defirelay.com",
  "proofOfPayment": {
    "fromAddress": "0x<your_wallet>",
    "toAddress": "0x<agent_wallet>",
    "chainId": "8453",
    "txHash": "0x<payment_tx_hash>"
  }
}
```

### Step 2: Upload to IPFS (Optional)

```bash
# Upload feedback file
curl -X POST "https://api.pinata.cloud/pinning/pinJSONToIPFS" \
  -H "Authorization: Bearer $PINATA_JWT" \
  -H "Content-Type: application/json" \
  -d '{"pinataContent": <feedback_json>}'
```

### Step 3: Compute Hash (if using IPFS)

```bash
# KECCAK-256 hash of the feedback file content
# Use ethers.js or similar: keccak256(toUtf8Bytes(JSON.stringify(feedback)))
```

### Step 4: Submit Feedback On-Chain

**Function**:
```solidity
function giveFeedback(
    uint256 agentId,
    int128 value,           // -1000000 to 1000000 (with decimals)
    uint8 valueDecimals,    // 0-18
    string calldata tag1,   // Category (e.g., "swap", "api", "chat")
    string calldata tag2,   // Subcategory (e.g., "fast", "accurate")
    string calldata endpoint,
    string calldata feedbackURI,  // IPFS URI or empty
    bytes32 feedbackHash         // Keccak256 or 0x0
) external
```

**Using web3_tx tool**:
```json
{
  "to": "<REPUTATION_REGISTRY>",
  "data": "<encoded_giveFeedback_call>",
  "network": "base"
}
```

### Feedback Value Scale

| Value | Meaning |
|-------|---------|
| 100 | Excellent / Highly Recommended |
| 75 | Good |
| 50 | Satisfactory |
| 25 | Below Average |
| 0 | Neutral / No Opinion |
| -25 | Poor |
| -50 | Bad |
| -100 | Terrible / Avoid |

---

## Responding to Feedback

Agents can respond to feedback they've received:

**Function**:
```solidity
function appendResponse(
    uint256 agentId,
    address clientAddress,
    uint64 feedbackIndex,
    string calldata responseURI,
    bytes32 responseHash
) external
```

Only the agent's owner can append responses.

---

## Revoking Feedback

If feedback was given in error, you can revoke it:

```solidity
function revokeFeedback(uint256 agentId, uint64 feedbackIndex) external
```

Only the original feedback giver can revoke.

---

## Auto-Feedback After x402 Payments

The bot can automatically submit positive feedback after successful x402 interactions:

### Workflow

1. **x402 Payment Made**: Bot pays for service via x402
2. **Track Payment**: Store tx_hash, amount, recipient
3. **Evaluate Success**: Check if service worked correctly
4. **Submit Feedback**: Call `giveFeedback` with proofOfPayment
5. **Mark Complete**: Update payment record as feedback_submitted

### Example Auto-Feedback

After a successful swap quote from DeFi Relay:

```json
{
  "agentId": 42,
  "value": 100,
  "valueDecimals": 0,
  "tag1": "api",
  "tag2": "swap-quote",
  "endpoint": "https://quoter.defirelay.com/swap/allowance-holder/quote",
  "feedbackURI": "ipfs://Qm.../feedback.json",
  "proofOfPayment": {
    "txHash": "0x...",
    "chainId": "8453"
  }
}
```

---

## Querying Before Interaction

Before interacting with a new agent, check their reputation:

### Decision Flow

```
1. Get agent's agentId from discovery/URI
2. Call getSummary(agentId, [], "", "")
3. Check count and average score
4. If count < 5 or score < 50: proceed with caution
5. If count >= 10 and score >= 75: trusted
```

### Reputation Thresholds (Suggested)

| Threshold | Action |
|-----------|--------|
| score >= 75, count >= 10 | Fully trusted |
| score >= 50, count >= 5 | Moderate trust |
| score < 50 OR count < 5 | Verify manually |
| score < 0 | Avoid interaction |

---

## Events to Monitor

Listen for reputation events affecting our agent:

```solidity
event NewFeedback(
    uint256 indexed agentId,
    address indexed clientAddress,
    uint64 feedbackIndex,
    int128 value,
    uint8 valueDecimals,
    string indexed indexedTag1,
    string tag1,
    string tag2,
    string endpoint,
    string feedbackURI,
    bytes32 feedbackHash
)

event FeedbackRevoked(
    uint256 indexed agentId,
    address indexed clientAddress,
    uint64 indexed feedbackIndex
)

event ResponseAppended(
    uint256 indexed agentId,
    address indexed clientAddress,
    uint64 feedbackIndex,
    address indexed responder,
    string responseURI,
    bytes32 responseHash
)
```

---

## Security Notes

1. **Sybil Resistance**: Feedback weighted by reviewer's own reputation
2. **Payment Proofs**: Verify tx_hash on-chain before trusting
3. **Revocation**: Feedback can be revoked but history preserved
4. **Timing**: Submit feedback promptly after interaction

## Related Skills

- `eip8004-register` - Register agent identity
- `eip8004-discover` - Find other agents
- `swap` - Token swaps (x402 payment source)
