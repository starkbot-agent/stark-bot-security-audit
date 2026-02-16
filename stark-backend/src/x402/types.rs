//! x402 Protocol data types

use serde::{Deserialize, Serialize};

/// USDC contract address on Base mainnet (default fallback)
pub const USDC_ADDRESS: &str = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";

/// Base mainnet chain ID
pub const BASE_CHAIN_ID: u64 = 8453;

/// Base Sepolia testnet chain ID
pub const BASE_SEPOLIA_CHAIN_ID: u64 = 84532;

/// x402 protocol version V1 (for keystore relay)
pub const X402_VERSION_V1: u8 = 1;

/// x402 protocol version V2 (for Kimi/AI relay - requires "accepted" field)
pub const X402_VERSION_V2: u8 = 2;

/// Network identifier for Base
pub const NETWORK_ID: &str = "eip155:8453";

/// Get chain ID from network name
pub fn chain_id_for_network(network: &str) -> u64 {
    match network {
        "base" => BASE_CHAIN_ID,
        "base-sepolia" => BASE_SEPOLIA_CHAIN_ID,
        "ethereum" => 1,
        "sepolia" => 11155111,
        _ => BASE_CHAIN_ID, // default
    }
}

/// Payment requirements returned by server in 402 response
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequired {
    pub x402_version: u8,
    pub accepts: Vec<PaymentRequirements>,
}

/// Extra metadata about the token (provided in 402 response)
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentExtra {
    /// Token symbol (e.g., "USDC")
    pub token: Option<String>,
    /// Token contract address
    pub address: Option<String>,
    /// Token decimals (e.g., 6 for USDC)
    pub decimals: Option<u8>,
    /// Token name for EIP-712 domain (e.g., "USD Coin")
    pub name: Option<String>,
    /// Token version for EIP-712 domain (e.g., "2")
    pub version: Option<String>,
    /// Facilitator signer address (spender for EIP-2612 permits)
    pub facilitator_signer: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequirements {
    pub scheme: String,
    pub network: String,
    pub max_amount_required: String,
    #[serde(alias = "payTo")]
    pub pay_to_address: String,
    pub asset: String,
    #[serde(default)]
    pub max_timeout_seconds: u64,
    #[serde(default)]
    pub resource: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    /// Extra token metadata for signing
    #[serde(default)]
    pub extra: Option<PaymentExtra>,
}

/// Token metadata needed for EIP-712 signing
#[derive(Debug, Clone)]
pub struct TokenMetadata {
    pub name: String,
    pub version: String,
    pub address: String,
    pub chain_id: u64,
    pub decimals: u8,
}

impl TokenMetadata {
    /// Build from payment requirements, using extra field or defaults
    pub fn from_requirements(req: &PaymentRequirements) -> Self {
        let chain_id = chain_id_for_network(&req.network);

        if let Some(extra) = &req.extra {
            Self {
                name: extra.name.clone().unwrap_or_else(|| "USD Coin".to_string()),
                version: extra.version.clone().unwrap_or_else(|| "2".to_string()),
                address: extra.address.clone().unwrap_or_else(|| req.asset.clone()),
                chain_id,
                decimals: extra.decimals.unwrap_or(6),
            }
        } else {
            // Fallback to defaults (Base mainnet USDC)
            Self {
                name: "USD Coin".to_string(),
                version: "2".to_string(),
                address: req.asset.clone(),
                chain_id,
                decimals: 6,
            }
        }
    }
}

/// Payment payload V1 format (for keystore relay)
/// scheme/network at top level, no "accepted" field
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentPayload {
    pub x402_version: u8,
    /// Scheme at top level (V1 format)
    pub scheme: String,
    /// Network at top level (V1 format)
    pub network: String,
    pub payload: ExactEvmPayload,
}

/// Payment payload V2 format (for Kimi/AI relay)
/// Contains "accepted" field as expected by Kimi relay
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentPayloadV2 {
    pub x402_version: u8,
    pub accepted: AcceptedPayment,
    pub payload: ExactEvmPayload,
}

/// AcceptedPayment - used in V2 format for Kimi relay
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AcceptedPayment {
    pub scheme: String,
    pub network: String,
    pub amount: String,
    pub pay_to: String,
    pub max_timeout_seconds: u64,
    pub asset: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExactEvmPayload {
    pub signature: String,
    pub authorization: EvmAuthorization,
}

/// Authorization types for different EIP standards
#[derive(Debug, Clone, Serialize)]
#[serde(untagged)]
pub enum EvmAuthorization {
    /// EIP-2612 Permit authorization (for "permit" scheme)
    Eip2612(Eip2612Authorization),
    /// EIP-3009 TransferWithAuthorization (for "exact" scheme)
    Eip3009(Eip3009Authorization),
}

/// EIP-2612 Permit authorization fields
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Eip2612Authorization {
    pub owner: String,
    pub spender: String,
    pub value: String,
    pub nonce: String,
    pub deadline: String,
}

/// EIP-3009 TransferWithAuthorization fields
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Eip3009Authorization {
    pub from: String,
    pub to: String,
    pub value: String,
    pub valid_after: String,
    pub valid_before: String,
    pub nonce: String,
}

impl PaymentPayload {
    /// Encode payment payload to base64 for X-PAYMENT header
    pub fn to_base64(&self) -> Result<String, String> {
        let json = serde_json::to_string(self)
            .map_err(|e| format!("Failed to serialize payment payload: {}", e))?;
        Ok(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, json))
    }
}

impl PaymentPayloadV2 {
    /// Encode V2 payment payload to base64 for X-PAYMENT header
    pub fn to_base64(&self) -> Result<String, String> {
        let json = serde_json::to_string(self)
            .map_err(|e| format!("Failed to serialize payment payload: {}", e))?;
        Ok(base64::Engine::encode(&base64::engine::general_purpose::STANDARD, json))
    }
}

impl PaymentRequired {
    /// Decode payment requirements from base64 PAYMENT-REQUIRED header
    pub fn from_base64(encoded: &str) -> Result<Self, String> {
        let decoded = base64::Engine::decode(&base64::engine::general_purpose::STANDARD, encoded)
            .map_err(|e| format!("Failed to decode payment required header: {}", e))?;
        let json = String::from_utf8(decoded)
            .map_err(|e| format!("Invalid UTF-8 in payment required header: {}", e))?;
        serde_json::from_str(&json)
            .map_err(|e| format!("Failed to parse payment required: {}", e))
    }
}

/// Payment status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum PaymentStatus {
    Pending,
    Confirmed,
    Failed,
}

impl std::fmt::Display for PaymentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PaymentStatus::Pending => write!(f, "pending"),
            PaymentStatus::Confirmed => write!(f, "confirmed"),
            PaymentStatus::Failed => write!(f, "failed"),
        }
    }
}

/// Information about a completed x402 payment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct X402PaymentInfo {
    /// Amount paid in smallest unit (e.g., micro-USDC for 6 decimals)
    pub amount: String,
    /// Human-readable amount (e.g., "0.001234")
    pub amount_formatted: String,
    /// Asset symbol (e.g., "USDC")
    pub asset: String,
    /// Address that received the payment
    pub pay_to: String,
    /// Optional resource identifier
    pub resource: Option<String>,
    /// Transaction hash if available
    pub tx_hash: Option<String>,
    /// Payment status
    pub status: PaymentStatus,
    /// Timestamp of payment
    pub timestamp: chrono::DateTime<chrono::Utc>,
}

impl X402PaymentInfo {
    /// Create from payment requirements (starts as pending with no tx_hash)
    pub fn from_requirements(req: &PaymentRequirements) -> Self {
        let decimals = req.extra.as_ref().and_then(|e| e.decimals).unwrap_or(6);
        let amount_formatted = format_token_amount(&req.max_amount_required, decimals);

        Self {
            amount: req.max_amount_required.clone(),
            amount_formatted,
            asset: req.asset.clone(),
            pay_to: req.pay_to_address.clone(),
            resource: req.resource.clone(),
            tx_hash: None,
            status: PaymentStatus::Pending,
            timestamp: chrono::Utc::now(),
        }
    }

    /// Set transaction hash and mark as confirmed
    pub fn with_tx_hash(mut self, tx_hash: String) -> Self {
        self.tx_hash = Some(tx_hash);
        self.status = PaymentStatus::Confirmed;
        self
    }

    /// Mark payment as confirmed (even without tx_hash)
    pub fn mark_confirmed(mut self) -> Self {
        self.status = PaymentStatus::Confirmed;
        self
    }

    /// Mark payment as failed
    pub fn mark_failed(mut self) -> Self {
        self.status = PaymentStatus::Failed;
        self
    }
}

/// Format token amount from raw value to human-readable string using the given decimals
fn format_token_amount(raw: &str, decimals: u8) -> String {
    if let Ok(value) = raw.parse::<u128>() {
        let divisor = 10u128.pow(decimals as u32);
        let whole = value / divisor;
        let frac = value % divisor;
        if frac == 0 {
            format!("{}", whole)
        } else {
            let frac_str = format!("{:0>width$}", frac, width = decimals as usize)
                .trim_end_matches('0')
                .to_string();
            format!("{}.{}", whole, frac_str)
        }
    } else {
        raw.to_string()
    }
}
