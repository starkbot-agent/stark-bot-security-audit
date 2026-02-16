---
name: eip8004-register
description: "Register this agent on-chain via EIP-8004 Identity Registry for discovery"
version: 1.0.0
author: starkbot
homepage: https://eips.ethereum.org/EIPS/eip-8004
metadata: {"requires_auth": true, "clawdbot":{"emoji":"🆔"}}
tags: [crypto, identity, eip8004, registration, discovery, agent]
---

# EIP-8004 Agent Registration

Register this agent on-chain using the EIP-8004 Identity Registry. This mints an ERC-721 token that makes the agent discoverable by other agents and services.

## Overview

EIP-8004 defines three registries for trustless agent ecosystems:
1. **Identity Registry** - ERC-721 agent handles (this skill)
2. **Reputation Registry** - Feedback/ratings
3. **Validation Registry** - Work verification

## Contract Addresses (Base Mainnet)

| Contract | Address |
|----------|---------|
| Identity Registry | `0x...` (TBD - deploy or use existing) |

## Registration Process

### Step 1: Create Registration File

The agent registration file is a JSON document describing the agent's capabilities:

```json
{
  "type": "https://eips.ethereum.org/EIPS/eip-8004#registration-v1",
  "name": "StarkBot",
  "description": "AI agent with crypto capabilities on Base. Supports swaps, transfers, and x402 payments.",
  "image": "https://example.com/starkbot-logo.png",
  "services": [
    {
      "name": "chat",
      "endpoint": "https://api.starkbot.xyz/chat",
      "version": "1.0"
    },
    {
      "name": "x402",
      "endpoint": "https://api.starkbot.xyz/x402",
      "version": "1.0"
    }
  ],
  "x402Support": true,
  "active": true,
  "supportedTrust": ["reputation", "x402-payments"]
}
```

### Step 2: Upload to IPFS

Use the `exec` tool to upload the registration file to IPFS:

```bash
# Using Pinata, web3.storage, or local IPFS node
curl -X POST "https://api.pinata.cloud/pinning/pinJSONToIPFS" \
  -H "Authorization: Bearer $PINATA_JWT" \
  -H "Content-Type: application/json" \
  -d '{"pinataContent": <registration_json>}'
```

The result will be an IPFS CID like `QmXxx...` which becomes the `agentURI`:
```
ipfs://QmXxx.../registration.json
```

### Step 3: Register On-Chain

Use the `web3_tx` tool to call the Identity Registry's `register` function:

**Function Signature**:
```solidity
function register(string calldata agentURI) external returns (uint256 agentId)
```

**ABI Encoding**:
- Function selector: `0x82fbdc9c` (keccak256("register(string)")[:4])
- Parameter: ABI-encoded string (agentURI)

**Example using web3_tx**:
```json
{
  "to": "<IDENTITY_REGISTRY_ADDRESS>",
  "data": "<encoded_register_call>",
  "network": "base"
}
```

### Step 4: Store Agent Identity

After successful registration, store the returned `agentId` and construct the full agent identifier:

```
agentRegistry: eip155:8453:<IDENTITY_REGISTRY_ADDRESS>
agentId: <returned_token_id>
```

## Helper: Encode Register Call

To encode the `register(string)` call, use this pattern:

```
0x82fbdc9c                                                         // function selector
0000000000000000000000000000000000000000000000000000000000000020     // offset to string
<length_of_uri_hex_padded_to_32_bytes>                              // string length
<uri_bytes_padded_to_32_byte_boundary>                              // string data
```

## Verification

After registration, verify the agent is discoverable:

1. Query `tokenURI(agentId)` to get the registration file
2. Check `ownerOf(agentId)` matches your wallet
3. Optionally set `agentWallet` if using a different payment address

## Setting Agent Wallet

To link a different wallet for payments (e.g., the burner wallet):

**Function Signature**:
```solidity
function setAgentWallet(
    uint256 agentId,
    address newWallet,
    uint256 deadline,
    bytes calldata signature
) external
```

This requires an EIP-712 signature from the current wallet owner.

## Example: Full Registration Flow

### 1. Get wallet address
```json
{"action": "address"}
```
→ Use `local_burner_wallet` tool

### 2. Prepare registration JSON
Create the registration file with your agent's details.

### 3. Upload to IPFS
```bash
# Store registration.json content and upload
echo '<registration_json>' > /tmp/registration.json
# Upload via preferred IPFS service
```

### 4. Register on-chain
```json
{
  "to": "0x<IDENTITY_REGISTRY>",
  "data": "0x82fbdc9c...<encoded_uri>",
  "network": "base"
}
```
→ Use `web3_tx` tool

### 5. Capture agentId
The transaction receipt contains the minted token ID in the `Transfer` event logs.

## Updating Registration

To update the registration file (e.g., add new services):

1. Upload new registration JSON to IPFS
2. Call `setAgentURI(agentId, newURI)`:

```solidity
function setAgentURI(uint256 agentId, string calldata newURI) external
```

Only the token owner can update the URI.

## On-Chain Metadata

Store additional metadata on-chain:

```solidity
function setMetadata(uint256 agentId, string memory key, bytes memory value) external
```

Common metadata keys:
- `agentWallet` - Payment receiving address
- `publicKey` - For encrypted communication
- `version` - Agent software version

## Security Notes

1. **Ownership**: Only the NFT owner can modify registration
2. **Wallet Signatures**: Changing `agentWallet` requires EIP-712 signature
3. **Immutability**: On-chain pointers are permanent (can update URI content)
4. **Gas Costs**: Registration costs ~100-200k gas on Base (~$0.01-0.05)

## Related Skills

- `eip8004-reputation` - Give/receive feedback
- `eip8004-discover` - Find other agents
- `swap` - Token swaps (uses x402)
- `transfer` - Token transfers
