# EIP-8004 Full Implementation Plan

## Executive Summary

This document outlines the complete implementation of EIP-8004 (Trustless Agents) integration for StarkBot, enabling:
- On-chain discoverable identity
- Reputation-based trust
- Payment proof linking
- Agent-to-agent interactions

---

## Part 1: Infrastructure Setup

### 1.1 Contract Deployment

**Option A: Deploy Custom Registries**

Deploy the three EIP-8004 registries on Base:

```solidity
// Identity Registry - ERC-721 with URI storage
// Reputation Registry - Feedback storage
// Validation Registry - Validator hooks
```

**Deployment Steps**:
1. Compile contracts from EIP-8004 reference implementation
2. Deploy to Base testnet first (Base Sepolia: 84532)
3. Verify contracts on BaseScan
4. Deploy to Base mainnet (8453)

**Option B: Use Existing/Shared Registries**

Check for existing EIP-8004 deployments and use shared infrastructure.

**Config File** (`stark-backend/src/eip8004/config.rs`):
```rust
pub struct Eip8004Config {
    pub identity_registry: Address,
    pub reputation_registry: Address,
    pub validation_registry: Address,
    pub chain_id: u64,
}

impl Eip8004Config {
    pub fn base_mainnet() -> Self {
        Self {
            identity_registry: "0x...".parse().unwrap(),
            reputation_registry: "0x...".parse().unwrap(),
            validation_registry: "0x...".parse().unwrap(),
            chain_id: 8453,
        }
    }
}
```

### 1.2 Database Schema

**File**: `stark-backend/src/db/migrations/007_eip8004.sql`

```sql
-- Agent identity (our registration)
CREATE TABLE IF NOT EXISTS agent_identity (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id INTEGER NOT NULL,
    agent_registry TEXT NOT NULL,
    chain_id INTEGER NOT NULL DEFAULT 8453,
    registration_uri TEXT,
    registration_hash TEXT,
    wallet_address TEXT NOT NULL,
    is_active INTEGER DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Payment history with proof tracking
CREATE TABLE IF NOT EXISTS x402_payments (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    channel_id INTEGER,
    session_id INTEGER,
    execution_id TEXT,
    tool_name TEXT,
    resource TEXT,
    amount TEXT NOT NULL,
    amount_formatted TEXT,
    asset TEXT DEFAULT 'USDC',
    pay_to TEXT NOT NULL,
    tx_hash TEXT,
    block_number INTEGER,
    feedback_submitted INTEGER DEFAULT 0,
    feedback_id INTEGER,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (session_id) REFERENCES chat_sessions(id),
    FOREIGN KEY (feedback_id) REFERENCES reputation_feedback(id)
);

CREATE INDEX idx_x402_payments_channel ON x402_payments(channel_id);
CREATE INDEX idx_x402_payments_tx_hash ON x402_payments(tx_hash);

-- Reputation feedback
CREATE TABLE IF NOT EXISTS reputation_feedback (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    direction TEXT NOT NULL CHECK (direction IN ('given', 'received')),
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
    feedback_hash TEXT,
    proof_of_payment_tx TEXT,
    response_uri TEXT,
    response_hash TEXT,
    is_revoked INTEGER DEFAULT 0,
    tx_hash TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_reputation_direction ON reputation_feedback(direction);
CREATE INDEX idx_reputation_agent ON reputation_feedback(agent_id, agent_registry);

-- Known agents (discovered)
CREATE TABLE IF NOT EXISTS known_agents (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    agent_id INTEGER NOT NULL,
    agent_registry TEXT NOT NULL,
    name TEXT,
    description TEXT,
    registration_uri TEXT,
    x402_support INTEGER DEFAULT 0,
    is_active INTEGER DEFAULT 1,
    reputation_score INTEGER,
    reputation_count INTEGER DEFAULT 0,
    last_interaction_at TEXT,
    discovered_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(agent_id, agent_registry)
);

-- Validation records
CREATE TABLE IF NOT EXISTS validations (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    direction TEXT NOT NULL CHECK (direction IN ('requested', 'responded')),
    request_hash TEXT NOT NULL,
    agent_id INTEGER NOT NULL,
    agent_registry TEXT,
    validator_address TEXT,
    request_uri TEXT,
    response INTEGER CHECK (response >= 0 AND response <= 100),
    response_uri TEXT,
    response_hash TEXT,
    tag TEXT,
    tx_hash TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_validations_request_hash ON validations(request_hash);
```

---

## Part 2: Core Module Implementation

### 2.1 Module Structure

```
stark-backend/src/eip8004/
├── mod.rs              # Module exports
├── config.rs           # Contract addresses, chain config
├── types.rs            # Shared types (AgentId, RegistrationFile, etc.)
├── identity.rs         # Identity Registry interactions
├── reputation.rs       # Reputation Registry interactions
├── validation.rs       # Validation Registry interactions
├── discovery.rs        # Agent discovery logic
├── abi/
│   ├── mod.rs
│   ├── identity.rs     # Identity Registry ABI
│   ├── reputation.rs   # Reputation Registry ABI
│   └── validation.rs   # Validation Registry ABI
└── ipfs.rs             # IPFS upload/fetch helpers
```

### 2.2 Core Types

**File**: `stark-backend/src/eip8004/types.rs`

```rust
use serde::{Deserialize, Serialize};

/// Full agent identifier
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentifier {
    pub agent_id: u64,
    pub agent_registry: String,  // "eip155:8453:0x..."
}

impl AgentIdentifier {
    pub fn new(agent_id: u64, chain_id: u64, registry_address: &str) -> Self {
        Self {
            agent_id,
            agent_registry: format!("eip155:{}:{}", chain_id, registry_address),
        }
    }

    pub fn parse_registry(&self) -> Option<(u64, String)> {
        let parts: Vec<&str> = self.agent_registry.split(':').collect();
        if parts.len() == 3 && parts[0] == "eip155" {
            let chain_id = parts[1].parse().ok()?;
            let address = parts[2].to_string();
            Some((chain_id, address))
        } else {
            None
        }
    }
}

/// Agent registration file (JSON hosted on IPFS)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationFile {
    #[serde(rename = "type")]
    pub type_url: String,  // "https://eips.ethereum.org/EIPS/eip-8004#registration-v1"
    pub name: String,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    pub services: Vec<ServiceEntry>,
    #[serde(rename = "x402Support")]
    pub x402_support: bool,
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub registrations: Option<Vec<AgentIdentifier>>,
    #[serde(rename = "supportedTrust")]
    pub supported_trust: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEntry {
    pub name: String,      // "mcp", "a2a", "chat", "x402"
    pub endpoint: String,
    pub version: String,
}

/// Feedback with proof of payment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackFile {
    #[serde(rename = "agentRegistry")]
    pub agent_registry: String,
    #[serde(rename = "agentId")]
    pub agent_id: u64,
    #[serde(rename = "clientAddress")]
    pub client_address: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    pub value: i128,
    #[serde(rename = "valueDecimals")]
    pub value_decimals: u8,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag1: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tag2: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(rename = "proofOfPayment", skip_serializing_if = "Option::is_none")]
    pub proof_of_payment: Option<ProofOfPayment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProofOfPayment {
    #[serde(rename = "fromAddress")]
    pub from_address: String,
    #[serde(rename = "toAddress")]
    pub to_address: String,
    #[serde(rename = "chainId")]
    pub chain_id: String,
    #[serde(rename = "txHash")]
    pub tx_hash: String,
}

/// Reputation summary
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationSummary {
    pub agent_id: u64,
    pub agent_registry: String,
    pub count: u64,
    pub average_value: f64,
    pub total_payments: Option<String>,  // Total USDC with proofs
}
```

### 2.3 Identity Module

**File**: `stark-backend/src/eip8004/identity.rs`

```rust
use crate::x402::X402EvmRpc;
use super::types::*;
use super::abi::identity::*;
use ethers::types::{Address, U256};

pub struct IdentityRegistry {
    rpc: X402EvmRpc,
    registry_address: Address,
    chain_id: u64,
}

impl IdentityRegistry {
    pub fn new(rpc: X402EvmRpc, registry_address: Address, chain_id: u64) -> Self {
        Self { rpc, registry_address, chain_id }
    }

    /// Register a new agent
    pub async fn register(&self, agent_uri: &str) -> Result<u64, String> {
        // Encode register(string) call
        let calldata = encode_register(agent_uri);

        // This would need to go through web3_tx for signing
        // Return the agentId from event logs
        todo!("Implement via web3_tx tool")
    }

    /// Get agent URI
    pub async fn get_agent_uri(&self, agent_id: u64) -> Result<String, String> {
        let calldata = encode_token_uri(agent_id);
        let result = self.rpc.eth_call(
            &self.registry_address.to_string(),
            &hex::encode(&calldata),
        ).await?;
        decode_string_result(&result)
    }

    /// Get agent owner
    pub async fn get_owner(&self, agent_id: u64) -> Result<Address, String> {
        let calldata = encode_owner_of(agent_id);
        let result = self.rpc.eth_call(
            &self.registry_address.to_string(),
            &hex::encode(&calldata),
        ).await?;
        decode_address_result(&result)
    }

    /// Get agent wallet (payment address)
    pub async fn get_agent_wallet(&self, agent_id: u64) -> Result<Address, String> {
        let calldata = encode_get_agent_wallet(agent_id);
        let result = self.rpc.eth_call(
            &self.registry_address.to_string(),
            &hex::encode(&calldata),
        ).await?;
        decode_address_result(&result)
    }

    /// Check if an agent exists
    pub async fn agent_exists(&self, agent_id: u64) -> Result<bool, String> {
        match self.get_owner(agent_id).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Get total registered agents
    pub async fn total_supply(&self) -> Result<u64, String> {
        let calldata = encode_total_supply();
        let result = self.rpc.eth_call(
            &self.registry_address.to_string(),
            &hex::encode(&calldata),
        ).await?;
        decode_u256_result(&result).map(|n| n.as_u64())
    }
}
```

### 2.4 Reputation Module

**File**: `stark-backend/src/eip8004/reputation.rs`

```rust
use super::types::*;

pub struct ReputationRegistry {
    rpc: X402EvmRpc,
    registry_address: Address,
}

impl ReputationRegistry {
    /// Submit feedback for an agent
    pub async fn give_feedback(
        &self,
        agent_id: u64,
        value: i128,
        value_decimals: u8,
        tag1: Option<&str>,
        tag2: Option<&str>,
        endpoint: Option<&str>,
        feedback_uri: Option<&str>,
        feedback_hash: Option<[u8; 32]>,
    ) -> Result<u64, String> {
        // Encode giveFeedback call
        // Execute via web3_tx
        todo!()
    }

    /// Get reputation summary
    pub async fn get_summary(
        &self,
        agent_id: u64,
        client_addresses: &[Address],
        tag1: Option<&str>,
        tag2: Option<&str>,
    ) -> Result<ReputationSummary, String> {
        let calldata = encode_get_summary(agent_id, client_addresses, tag1, tag2);
        let result = self.rpc.eth_call(
            &self.registry_address.to_string(),
            &hex::encode(&calldata),
        ).await?;
        decode_summary_result(&result)
    }

    /// Read specific feedback
    pub async fn read_feedback(
        &self,
        agent_id: u64,
        client_address: Address,
        feedback_index: u64,
    ) -> Result<FeedbackEntry, String> {
        todo!()
    }

    /// Append response to feedback
    pub async fn append_response(
        &self,
        agent_id: u64,
        client_address: Address,
        feedback_index: u64,
        response_uri: &str,
        response_hash: [u8; 32],
    ) -> Result<(), String> {
        todo!()
    }

    /// Revoke previously given feedback
    pub async fn revoke_feedback(
        &self,
        agent_id: u64,
        feedback_index: u64,
    ) -> Result<(), String> {
        todo!()
    }
}
```

---

## Part 3: x402 Payment Tracking Enhancement

### 3.1 Payment Storage

**File**: `stark-backend/src/db/tables/x402_payments.rs`

```rust
use rusqlite::{params, Connection, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X402Payment {
    pub id: i64,
    pub channel_id: Option<i64>,
    pub session_id: Option<i64>,
    pub execution_id: Option<String>,
    pub tool_name: Option<String>,
    pub resource: Option<String>,
    pub amount: String,
    pub amount_formatted: String,
    pub asset: String,
    pub pay_to: String,
    pub tx_hash: Option<String>,
    pub block_number: Option<i64>,
    pub feedback_submitted: bool,
    pub feedback_id: Option<i64>,
    pub created_at: String,
}

impl X402Payment {
    pub fn insert(conn: &Connection, payment: &NewX402Payment) -> Result<i64> {
        conn.execute(
            "INSERT INTO x402_payments (
                channel_id, session_id, execution_id, tool_name, resource,
                amount, amount_formatted, asset, pay_to, tx_hash, block_number
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            params![
                payment.channel_id,
                payment.session_id,
                payment.execution_id,
                payment.tool_name,
                payment.resource,
                payment.amount,
                payment.amount_formatted,
                payment.asset,
                payment.pay_to,
                payment.tx_hash,
                payment.block_number,
            ],
        )?;
        Ok(conn.last_insert_rowid())
    }

    pub fn get_by_channel(conn: &Connection, channel_id: i64, limit: i64) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM x402_payments WHERE channel_id = ?1
             ORDER BY created_at DESC LIMIT ?2"
        )?;
        // ... mapping
        todo!()
    }

    pub fn get_without_feedback(conn: &Connection, limit: i64) -> Result<Vec<Self>> {
        let mut stmt = conn.prepare(
            "SELECT * FROM x402_payments
             WHERE feedback_submitted = 0 AND tx_hash IS NOT NULL
             ORDER BY created_at ASC LIMIT ?1"
        )?;
        todo!()
    }

    pub fn mark_feedback_submitted(conn: &Connection, id: i64, feedback_id: i64) -> Result<()> {
        conn.execute(
            "UPDATE x402_payments SET feedback_submitted = 1, feedback_id = ?1 WHERE id = ?2",
            params![feedback_id, id],
        )?;
        Ok(())
    }

    pub fn get_total_spent(conn: &Connection, channel_id: Option<i64>) -> Result<String> {
        // Sum all payments, optionally filtered by channel
        todo!()
    }
}

#[derive(Debug)]
pub struct NewX402Payment {
    pub channel_id: Option<i64>,
    pub session_id: Option<i64>,
    pub execution_id: Option<String>,
    pub tool_name: Option<String>,
    pub resource: Option<String>,
    pub amount: String,
    pub amount_formatted: String,
    pub asset: String,
    pub pay_to: String,
    pub tx_hash: Option<String>,
    pub block_number: Option<i64>,
}
```

### 3.2 Integration with x402 Client

**Modify**: `stark-backend/src/x402/client.rs`

Add payment event emission with DB storage:

```rust
impl X402Client {
    pub async fn post_with_payment_tracked<T: Serialize>(
        &self,
        url: &str,
        body: &T,
        context: &PaymentContext,  // channel_id, session_id, tool_name
    ) -> Result<X402Response, String> {
        let response = self.post_with_payment(url, body).await?;

        // Store payment in database if one was made
        if let Some(ref payment) = response.payment {
            if let Some(db) = context.db.as_ref() {
                let new_payment = NewX402Payment {
                    channel_id: context.channel_id,
                    session_id: context.session_id,
                    execution_id: context.execution_id.clone(),
                    tool_name: context.tool_name.clone(),
                    resource: Some(url.to_string()),
                    amount: payment.amount.clone(),
                    amount_formatted: payment.amount_formatted.clone(),
                    asset: payment.asset.clone(),
                    pay_to: payment.pay_to.clone(),
                    tx_hash: None,  // Would need to track from blockchain
                    block_number: None,
                };
                X402Payment::insert(&db.conn, &new_payment)?;
            }
        }

        Ok(response)
    }
}
```

---

## Part 4: API Endpoints

### 4.1 Identity Endpoints

**File**: `stark-backend/src/controllers/eip8004_identity.rs`

```rust
// GET /api/eip8004/identity
// Returns our agent's identity info

// POST /api/eip8004/identity/register
// Register agent (triggers skill or direct tx)

// PUT /api/eip8004/identity/uri
// Update registration URI

// GET /api/eip8004/identity/{agentId}
// Get any agent's registration info
```

### 4.2 Reputation Endpoints

**File**: `stark-backend/src/controllers/eip8004_reputation.rs`

```rust
// GET /api/eip8004/reputation/{agentId}
// Get reputation summary for an agent

// GET /api/eip8004/reputation/given
// List feedback we've given

// GET /api/eip8004/reputation/received
// List feedback we've received

// POST /api/eip8004/reputation/feedback
// Submit feedback (with optional proof of payment)
```

### 4.3 Payment Endpoints

**File**: `stark-backend/src/controllers/x402_payments.rs`

```rust
// GET /api/payments
// List all x402 payments with filters

// GET /api/payments/summary
// Get spending summary (total, by channel, by tool)

// GET /api/payments/{id}
// Get specific payment details

// POST /api/payments/{id}/feedback
// Submit feedback for a payment
```

### 4.4 Discovery Endpoints

**File**: `stark-backend/src/controllers/eip8004_discovery.rs`

```rust
// GET /api/eip8004/agents
// Search/browse registered agents

// GET /api/eip8004/agents/{agentId}
// Get agent details with reputation

// POST /api/eip8004/agents/refresh
// Refresh known agents from registry
```

---

## Part 5: Frontend Components

### 5.1 Agent Identity Page

**File**: `stark-frontend/src/pages/AgentIdentity.tsx`

Features:
- Display current agent registration status
- Show agentId, registry, wallet address
- Registration form if not registered
- Update URI functionality
- QR code for agent identifier

### 5.2 Reputation Dashboard

**File**: `stark-frontend/src/pages/Reputation.tsx`

Features:
- Reputation score display (given/received)
- Feedback history with filters
- Submit feedback form
- Response to feedback
- Link payments to feedback

### 5.3 Payment History

**File**: `stark-frontend/src/pages/Payments.tsx`

Features:
- Payment list with search/filter
- Cost breakdown charts
- Export functionality
- Link to feedback submission
- Spending limits configuration

### 5.4 Agent Discovery

**File**: `stark-frontend/src/pages/Discovery.tsx`

Features:
- Browse registered agents
- Filter by services, x402 support, reputation
- Agent detail modal
- "Interact" button to initiate contact

---

## Part 6: Skills

### 6.1 Registration Skill (Created)
`skills/eip8004-register.md` - Register agent identity

### 6.2 Reputation Skill

**File**: `skills/eip8004-reputation.md`

```markdown
# EIP-8004 Reputation

Submit and query reputation feedback for agents.

## Give Feedback
After successful x402 interaction, submit feedback with proof of payment.

## Check Reputation
Before interacting with new agent, query their reputation score.
```

### 6.3 Discovery Skill

**File**: `skills/eip8004-discover.md`

```markdown
# EIP-8004 Agent Discovery

Find and interact with other registered agents.

## Search Agents
Query the Identity Registry for agents matching criteria.

## Get Agent Info
Fetch registration file and reputation for an agent.
```

---

## Part 7: Implementation Phases

### Phase 1: Foundation (Week 1-2)
- [ ] Database schema migration
- [ ] Core types module (`eip8004/types.rs`)
- [ ] x402 payment storage
- [ ] Payment history API endpoints
- [ ] Payment history frontend page

### Phase 2: Identity (Week 3-4)
- [ ] Identity Registry ABI encoding
- [ ] Identity module implementation
- [ ] Registration skill enhancement
- [ ] Agent identity storage
- [ ] Identity API endpoints
- [ ] Identity frontend page

### Phase 3: Reputation (Week 5-6)
- [ ] Reputation Registry ABI encoding
- [ ] Reputation module implementation
- [ ] Feedback submission with payment proofs
- [ ] Reputation query functionality
- [ ] Reputation API endpoints
- [ ] Reputation frontend dashboard

### Phase 4: Discovery (Week 7-8)
- [ ] Discovery module implementation
- [ ] Agent indexing/caching
- [ ] Discovery API endpoints
- [ ] Discovery frontend page
- [ ] Agent-to-agent interaction basics

### Phase 5: Validation (Week 9-10)
- [ ] Validation Registry ABI
- [ ] Validation request/response
- [ ] Validation tracking
- [ ] API endpoints
- [ ] (Optional) Become a validator

### Phase 6: Polish (Week 11-12)
- [ ] Testing & bug fixes
- [ ] Performance optimization
- [ ] Documentation
- [ ] Security audit
- [ ] Production deployment

---

## Part 8: Testing Strategy

### Unit Tests
- ABI encoding/decoding
- Type serialization
- Database operations

### Integration Tests
- Registry interactions (testnet)
- Payment tracking flow
- Feedback submission

### E2E Tests
- Full registration flow
- Payment → Feedback flow
- Discovery → Interaction flow

---

## Part 9: Security Considerations

1. **Private Key Security**: Burner wallet key must be protected
2. **Signature Validation**: Verify all EIP-712 signatures
3. **IPFS Content**: Validate registration file schema
4. **Rate Limiting**: Prevent reputation spam
5. **Payment Verification**: Confirm on-chain before trusting proofs
6. **Registry Trust**: Verify registry contract addresses

---

## Appendix: Contract ABIs

### Identity Registry (Partial)

```json
[
  {
    "name": "register",
    "type": "function",
    "inputs": [{"name": "agentURI", "type": "string"}],
    "outputs": [{"name": "agentId", "type": "uint256"}]
  },
  {
    "name": "tokenURI",
    "type": "function",
    "inputs": [{"name": "tokenId", "type": "uint256"}],
    "outputs": [{"name": "", "type": "string"}]
  },
  {
    "name": "ownerOf",
    "type": "function",
    "inputs": [{"name": "tokenId", "type": "uint256"}],
    "outputs": [{"name": "", "type": "address"}]
  },
  {
    "name": "getAgentWallet",
    "type": "function",
    "inputs": [{"name": "agentId", "type": "uint256"}],
    "outputs": [{"name": "", "type": "address"}]
  },
  {
    "name": "setAgentURI",
    "type": "function",
    "inputs": [
      {"name": "agentId", "type": "uint256"},
      {"name": "newURI", "type": "string"}
    ],
    "outputs": []
  }
]
```

### Reputation Registry (Partial)

```json
[
  {
    "name": "giveFeedback",
    "type": "function",
    "inputs": [
      {"name": "agentId", "type": "uint256"},
      {"name": "value", "type": "int128"},
      {"name": "valueDecimals", "type": "uint8"},
      {"name": "tag1", "type": "string"},
      {"name": "tag2", "type": "string"},
      {"name": "endpoint", "type": "string"},
      {"name": "feedbackURI", "type": "string"},
      {"name": "feedbackHash", "type": "bytes32"}
    ],
    "outputs": []
  },
  {
    "name": "getSummary",
    "type": "function",
    "inputs": [
      {"name": "agentId", "type": "uint256"},
      {"name": "clientAddresses", "type": "address[]"},
      {"name": "tag1", "type": "string"},
      {"name": "tag2", "type": "string"}
    ],
    "outputs": [
      {"name": "count", "type": "uint64"},
      {"name": "summaryValue", "type": "int128"},
      {"name": "summaryValueDecimals", "type": "uint8"}
    ]
  }
]
```
