# EIP-8004 Integration Plan for StarkBot

## Overview

EIP-8004 defines three on-chain registries for trustless agent ecosystems:
1. **Identity Registry** - ERC-721 agent handles for discovery
2. **Reputation Registry** - Feedback/ratings system
3. **Validation Registry** - Independent work verification

Combined with x402 (which we already support), this enables:
- Discoverable agents with on-chain identity
- Reputation-based trust without prior relationships
- Payment proofs linked to feedback
- Cross-organizational agent interactions

---

## Phase 1: Agent Identity (Foundation)

### 1.1 Register StarkBot as an Agent

**Goal**: Mint an ERC-721 identity for the bot on Base

**Tasks**:
- [ ] Deploy or use existing Identity Registry on Base
- [ ] Create agent registration file (JSON) hosted on IPFS/Arweave
- [ ] Call `register(agentURI)` to mint agent NFT
- [ ] Store `agentId` and `agentRegistry` in bot config

**Registration File Structure**:
```json
{
  "type": "https://eips.ethereum.org/EIPS/eip-8004#registration-v1",
  "name": "StarkBot",
  "description": "AI agent with crypto capabilities on Base",
  "image": "ipfs://...",
  "services": [
    {
      "name": "mcp",
      "endpoint": "https://api.starkbot.xyz/mcp",
      "version": "1.0"
    },
    {
      "name": "chat",
      "endpoint": "https://api.starkbot.xyz/chat",
      "version": "1.0"
    }
  ],
  "x402Support": true,
  "active": true,
  "registrations": [
    {
      "agentId": 1,
      "agentRegistry": "eip155:8453:0x..."
    }
  ],
  "supportedTrust": ["reputation", "x402-payments"]
}
```

### 1.2 Agent Wallet Management

**Goal**: Link burner wallet to agent identity

**Tasks**:
- [ ] Implement `setAgentWallet()` call with EIP-712 signature
- [ ] Display agent identity in UI (agentId, registry)
- [ ] Allow wallet rotation via settings

**Code Location**: `stark-backend/src/identity/` (new module)

---

## Phase 2: Reputation System

### 2.1 Give Feedback to Other Agents

**Goal**: Rate agents/services the bot interacts with

**Tasks**:
- [ ] After x402 payments, optionally submit feedback
- [ ] Include `proofOfPayment` with tx details
- [ ] Track feedback given in local DB

**Feedback Payload**:
```json
{
  "agentRegistry": "eip155:8453:0x...",
  "agentId": 42,
  "clientAddress": "eip155:8453:0x...(our wallet)",
  "value": 100,
  "valueDecimals": 0,
  "tag1": "api",
  "tag2": "swap-quote",
  "endpoint": "https://quoter.defirelay.com",
  "proofOfPayment": {
    "fromAddress": "0x...",
    "toAddress": "0x...",
    "chainId": "8453",
    "txHash": "0x..."
  }
}
```

### 2.2 Accept Feedback

**Goal**: Allow users/agents to rate StarkBot

**Tasks**:
- [ ] Monitor `NewFeedback` events for our agentId
- [ ] Display reputation score in UI
- [ ] Respond to feedback via `appendResponse()`

### 2.3 Reputation Queries

**Goal**: Check reputation before interacting with new agents

**Tasks**:
- [ ] Implement `getSummary()` calls
- [ ] Add reputation threshold settings
- [ ] Display reputation in agent discovery UI

**Code Location**: `stark-backend/src/reputation/` (new module)

---

## Phase 3: Agent Discovery

### 3.1 Browse Agent Registry

**Goal**: Discover other EIP-8004 agents

**Tasks**:
- [ ] Query Identity Registry for agents
- [ ] Fetch and parse registration files
- [ ] Filter by `x402Support`, services, reputation
- [ ] UI for browsing available agents

### 3.2 Agent-to-Agent Communication

**Goal**: Enable StarkBot to call other agents

**Tasks**:
- [ ] Parse `services` from registration files
- [ ] Support MCP/A2A protocol connections
- [ ] x402 payments to other agents
- [ ] Track interactions and give feedback

**Code Location**: `stark-backend/src/discovery/` (new module)

---

## Phase 4: Validation Integration

### 4.1 Request Validation

**Goal**: Have work validated by independent validators

**Tasks**:
- [ ] After significant operations, submit validation requests
- [ ] Include operation details in `requestURI`
- [ ] Monitor for validation responses

### 4.2 Become a Validator (Optional)

**Goal**: Validate other agents' work

**Tasks**:
- [ ] Monitor `ValidationRequest` events
- [ ] Implement validation logic for known operation types
- [ ] Submit `validationResponse()` with evidence

**Code Location**: `stark-backend/src/validation/` (new module)

---

## Phase 5: Enhanced Payment Integration

### 5.1 Payment Proof Linking

**Goal**: Connect x402 payments to reputation feedback

**Current Flow**:
```
Request → 402 → Sign EIP-3009 → X-PAYMENT header → Response
```

**Enhanced Flow**:
```
Request → 402 → Sign EIP-3009 → X-PAYMENT header → Response
                                                      ↓
                                              Track tx hash
                                                      ↓
                                              Submit feedback with proofOfPayment
```

**Tasks**:
- [ ] Capture transaction hash from x402 payments
- [ ] Store in `x402_payments` table
- [ ] Auto-generate feedback after successful operations
- [ ] Link `proofOfPayment` in feedback file

### 5.2 Payment Verification

**Goal**: Verify payment proofs from other agents

**Tasks**:
- [ ] Parse `proofOfPayment` from feedback
- [ ] Verify tx on-chain (Base RPC)
- [ ] Weight reputation by payment value

---

## Implementation Architecture

```
stark-backend/
├── src/
│   ├── eip8004/                    # NEW MODULE
│   │   ├── mod.rs
│   │   ├── identity.rs             # Identity Registry interactions
│   │   ├── reputation.rs           # Reputation Registry interactions
│   │   ├── validation.rs           # Validation Registry interactions
│   │   ├── discovery.rs            # Agent discovery logic
│   │   ├── contracts/              # Contract ABIs
│   │   │   ├── identity_registry.rs
│   │   │   ├── reputation_registry.rs
│   │   │   └── validation_registry.rs
│   │   └── types.rs                # Shared types
│   │
│   ├── x402/                       # EXISTING - Enhanced
│   │   ├── ...
│   │   └── payment_proof.rs        # NEW: Track proofs for feedback
│   │
│   └── db/
│       └── tables/
│           ├── x402_payments.rs    # NEW: Payment history
│           ├── agent_identity.rs   # NEW: Our identity
│           ├── reputation.rs       # NEW: Given/received feedback
│           └── validations.rs      # NEW: Validation records

stark-frontend/
├── src/
│   └── pages/
│       ├── AgentIdentity.tsx       # NEW: View/manage identity
│       ├── Reputation.tsx          # NEW: View reputation
│       └── Discovery.tsx           # NEW: Browse agents
```

---

## Database Schema Additions

```sql
-- Our agent identity
CREATE TABLE agent_identity (
    id INTEGER PRIMARY KEY,
    agent_id INTEGER NOT NULL,
    agent_registry TEXT NOT NULL,
    registration_uri TEXT,
    wallet_address TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Payment history with proof tracking
CREATE TABLE x402_payments (
    id INTEGER PRIMARY KEY,
    channel_id INTEGER,
    session_id INTEGER,
    tool_name TEXT,
    amount TEXT NOT NULL,
    amount_formatted TEXT,
    asset TEXT DEFAULT 'USDC',
    pay_to TEXT NOT NULL,
    resource TEXT,
    tx_hash TEXT,                    -- For proofOfPayment
    feedback_submitted INTEGER DEFAULT 0,
    created_at TEXT NOT NULL
);

-- Reputation feedback (given and received)
CREATE TABLE reputation_feedback (
    id INTEGER PRIMARY KEY,
    direction TEXT NOT NULL,         -- 'given' or 'received'
    agent_id INTEGER NOT NULL,
    agent_registry TEXT NOT NULL,
    client_address TEXT NOT NULL,
    feedback_index INTEGER,
    value INTEGER NOT NULL,
    value_decimals INTEGER DEFAULT 0,
    tag1 TEXT,
    tag2 TEXT,
    endpoint TEXT,
    feedback_uri TEXT,
    proof_of_payment_tx TEXT,
    response_uri TEXT,
    is_revoked INTEGER DEFAULT 0,
    created_at TEXT NOT NULL
);

-- Validation requests/responses
CREATE TABLE validations (
    id INTEGER PRIMARY KEY,
    direction TEXT NOT NULL,         -- 'requested' or 'responded'
    request_hash TEXT NOT NULL,
    agent_id INTEGER NOT NULL,
    validator_address TEXT,
    response INTEGER,                -- 0-100
    request_uri TEXT,
    response_uri TEXT,
    tag TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT
);
```

---

## Contract Deployment

### Option A: Use Existing Registries
- Check if EIP-8004 registries exist on Base
- Use official deployments if available

### Option B: Deploy Our Own
- Deploy Identity, Reputation, Validation registries
- Open for other agents to use
- Become a registry operator

### Contract Addresses (TBD)
```
Base Mainnet (8453):
- Identity Registry: 0x...
- Reputation Registry: 0x...
- Validation Registry: 0x...
```

---

## Priority Order

1. **Phase 1.1** - Register agent identity (enables discovery)
2. **Phase 5.1** - Payment proof linking (enhances existing x402)
3. **Phase 2.1** - Give feedback (build reputation data)
4. **Phase 2.3** - Query reputation (use reputation data)
5. **Phase 3.1** - Agent discovery (find other agents)
6. **Phase 2.2** - Accept feedback (receive ratings)
7. **Phase 3.2** - Agent-to-agent calls (interact with others)
8. **Phase 4** - Validation (advanced trust)

---

## Quick Start: Minimum Viable Integration

For immediate value, implement:

1. **Payment History Table** - Store all x402 payments with tx hashes
2. **Agent Registration File** - Host on IPFS, advertise x402 support
3. **Basic Reputation Query** - Check agent reputation before interactions

This gives you:
- Discoverable agent identity
- Payment audit trail
- Trust signals for new interactions
