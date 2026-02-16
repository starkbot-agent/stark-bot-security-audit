//! EIP-8004 Type definitions

use serde::{Deserialize, Serialize};

/// Full agent identifier (agentRegistry + agentId)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AgentIdentifier {
    pub agent_id: u64,
    pub agent_registry: String, // "eip155:8453:0x..."
}

impl AgentIdentifier {
    pub fn new(agent_id: u64, chain_id: u64, registry_address: &str) -> Self {
        let addr = registry_address.to_lowercase();
        Self {
            agent_id,
            agent_registry: format!("eip155:{}:{}", chain_id, addr),
        }
    }

    /// Parse the registry string to extract chain_id and address
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

    /// Get the chain ID
    pub fn chain_id(&self) -> Option<u64> {
        self.parse_registry().map(|(chain_id, _)| chain_id)
    }

    /// Get the registry contract address
    pub fn registry_address(&self) -> Option<String> {
        self.parse_registry().map(|(_, addr)| addr)
    }
}

impl std::fmt::Display for AgentIdentifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}", self.agent_registry, self.agent_id)
    }
}

/// Agent registration file (JSON hosted on IPFS/HTTPS)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationFile {
    #[serde(rename = "type")]
    pub type_url: String, // "https://eips.ethereum.org/EIPS/eip-8004#registration-v1"

    pub name: String,
    pub description: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,

    #[serde(default)]
    pub services: Vec<ServiceEntry>,

    #[serde(rename = "x402Support", default)]
    pub x402_support: bool,

    #[serde(default = "default_true")]
    pub active: bool,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub registrations: Option<Vec<RegistrationEntry>>,

    #[serde(rename = "supportedTrust", default)]
    pub supported_trust: Vec<String>,
}

fn default_true() -> bool {
    true
}

impl RegistrationFile {
    pub fn new(name: &str, description: &str) -> Self {
        Self {
            type_url: "https://eips.ethereum.org/EIPS/eip-8004#registration-v1".to_string(),
            name: name.to_string(),
            description: description.to_string(),
            image: None,
            services: Vec::new(),
            x402_support: true,
            active: true,
            registrations: None,
            supported_trust: vec!["reputation".to_string(), "x402-payments".to_string()],
        }
    }

    pub fn with_service(mut self, name: &str, endpoint: &str, version: &str) -> Self {
        self.services.push(ServiceEntry {
            name: name.to_string(),
            endpoint: endpoint.to_string(),
            version: version.to_string(),
        });
        self
    }

    pub fn with_image(mut self, url: &str) -> Self {
        self.image = Some(url.to_string());
        self
    }
}

/// Service entry in registration file
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceEntry {
    pub name: String,    // "mcp", "a2a", "chat", "x402", "swap", etc.
    pub endpoint: String,
    pub version: String,
}

/// Registration entry for cross-chain identity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrationEntry {
    #[serde(rename = "agentId")]
    pub agent_id: u64,
    #[serde(rename = "agentRegistry")]
    pub agent_registry: String,
}

/// Feedback file (JSON for detailed feedback with proof of payment)
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

    pub value: i64,

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

/// Proof of payment for linking x402 payments to feedback
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

    #[serde(skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset: Option<String>,
}

/// Reputation summary from registry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReputationSummary {
    pub agent_id: u64,
    pub agent_registry: String,
    pub count: u64,
    pub total_value: i128,
    pub value_decimals: u8,
    pub average_score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_payments_usdc: Option<String>,
}

impl ReputationSummary {
    pub fn trust_level(&self) -> TrustLevel {
        if self.count >= 10 && self.average_score >= 75.0 {
            TrustLevel::High
        } else if self.count >= 5 && self.average_score >= 50.0 {
            TrustLevel::Medium
        } else if self.count >= 3 && self.average_score >= 25.0 {
            TrustLevel::Low
        } else {
            TrustLevel::Unverified
        }
    }
}

/// Trust level derived from reputation
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustLevel {
    High,       // score >= 75, count >= 10
    Medium,     // score >= 50, count >= 5
    Low,        // score >= 25, count >= 3
    Unverified, // insufficient data
    Negative,   // score < 0
}

impl std::fmt::Display for TrustLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrustLevel::High => write!(f, "high"),
            TrustLevel::Medium => write!(f, "medium"),
            TrustLevel::Low => write!(f, "low"),
            TrustLevel::Unverified => write!(f, "unverified"),
            TrustLevel::Negative => write!(f, "negative"),
        }
    }
}

/// Feedback entry from on-chain query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackEntry {
    pub agent_id: u64,
    pub client_address: String,
    pub feedback_index: u64,
    pub value: i64,
    pub value_decimals: u8,
    pub tag1: Option<String>,
    pub tag2: Option<String>,
    pub endpoint: Option<String>,
    pub feedback_uri: Option<String>,
    pub is_revoked: bool,
    pub response_uri: Option<String>,
}

/// Discovered agent with full details
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredAgent {
    pub identifier: AgentIdentifier,
    pub registration: Option<RegistrationFile>,
    pub owner_address: String,
    pub wallet_address: Option<String>,
    pub reputation: Option<ReputationSummary>,
    pub discovered_at: String,
    pub last_updated: String,
}

impl DiscoveredAgent {
    pub fn is_x402_enabled(&self) -> bool {
        self.registration
            .as_ref()
            .map(|r| r.x402_support)
            .unwrap_or(false)
    }

    pub fn is_active(&self) -> bool {
        self.registration
            .as_ref()
            .map(|r| r.active)
            .unwrap_or(false)
    }

    pub fn has_service(&self, name: &str) -> bool {
        self.registration
            .as_ref()
            .map(|r| r.services.iter().any(|s| s.name == name))
            .unwrap_or(false)
    }

    pub fn get_service_endpoint(&self, name: &str) -> Option<String> {
        self.registration.as_ref().and_then(|r| {
            r.services
                .iter()
                .find(|s| s.name == name)
                .map(|s| s.endpoint.clone())
        })
    }

    pub fn trust_level(&self) -> TrustLevel {
        self.reputation
            .as_ref()
            .map(|r| r.trust_level())
            .unwrap_or(TrustLevel::Unverified)
    }
}

/// Validation request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationRequest {
    pub request_hash: String,
    pub agent_id: u64,
    pub validator_address: String,
    pub request_uri: String,
}

/// Validation response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationResponse {
    pub request_hash: String,
    pub response: u8, // 0-100
    pub response_uri: Option<String>,
    pub tag: Option<String>,
}

/// x402 Payment record for database storage
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X402PaymentRecord {
    pub id: Option<i64>,
    pub channel_id: Option<i64>,
    pub session_id: Option<i64>,
    pub execution_id: Option<String>,
    pub tool_name: Option<String>,
    pub resource: Option<String>,
    pub amount: String,
    pub amount_formatted: String,
    pub asset: String,
    pub pay_to: String,
    pub from_address: Option<String>,
    pub tx_hash: Option<String>,
    pub block_number: Option<i64>,
    pub feedback_submitted: bool,
    pub feedback_id: Option<i64>,
    pub created_at: String,
}

impl X402PaymentRecord {
    /// Create a proof of payment from this record
    pub fn to_proof(&self) -> Option<ProofOfPayment> {
        let tx_hash = self.tx_hash.as_ref()?;
        let from_address = self.from_address.as_ref()?;

        Some(ProofOfPayment {
            from_address: from_address.clone(),
            to_address: self.pay_to.clone(),
            chain_id: "8453".to_string(), // Base
            tx_hash: tx_hash.clone(),
            amount: Some(self.amount_formatted.clone()),
            asset: Some(self.asset.clone()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_identifier() {
        let id = AgentIdentifier::new(42, 8453, "0x1234567890abcdef");
        assert_eq!(id.agent_id, 42);
        assert_eq!(id.agent_registry, "eip155:8453:0x1234567890abcdef");

        let (chain_id, addr) = id.parse_registry().unwrap();
        assert_eq!(chain_id, 8453);
        assert_eq!(addr, "0x1234567890abcdef");
    }

    #[test]
    fn test_registration_file() {
        let reg = RegistrationFile::new("TestAgent", "A test agent")
            .with_service("x402", "https://api.example.com", "1.0")
            .with_image("https://example.com/image.png");

        assert!(reg.x402_support);
        assert!(reg.active);
        assert_eq!(reg.services.len(), 1);
        assert_eq!(reg.services[0].name, "x402");
    }

    #[test]
    fn test_trust_levels() {
        let high = ReputationSummary {
            agent_id: 1,
            agent_registry: "test".to_string(),
            count: 15,
            total_value: 1200,
            value_decimals: 0,
            average_score: 80.0,
            total_payments_usdc: None,
        };
        assert_eq!(high.trust_level(), TrustLevel::High);

        let low = ReputationSummary {
            agent_id: 1,
            agent_registry: "test".to_string(),
            count: 2,
            total_value: 100,
            value_decimals: 0,
            average_score: 50.0,
            total_payments_usdc: None,
        };
        assert_eq!(low.trust_level(), TrustLevel::Unverified);
    }
}
