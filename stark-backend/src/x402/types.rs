//! x402 Protocol data types

use serde::{Deserialize, Serialize};

/// USDC contract address on Base mainnet
pub const USDC_ADDRESS: &str = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";

/// Base mainnet chain ID
pub const BASE_CHAIN_ID: u64 = 8453;

/// x402 protocol version
pub const X402_VERSION: u8 = 2;

/// Network identifier for Base
pub const NETWORK_ID: &str = "eip155:8453";

/// Payment requirements returned by server in 402 response
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequired {
    pub x402_version: u8,
    pub accepts: Vec<PaymentRequirements>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequirements {
    pub scheme: String,
    pub network: String,
    pub max_amount_required: String,
    pub pay_to_address: String,
    pub asset: String,
    #[serde(default)]
    pub max_timeout_seconds: u64,
    #[serde(default)]
    pub resource: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
}

/// Payment payload sent to server with X-PAYMENT header
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentPayload {
    pub x402_version: u8,
    pub accepted: AcceptedPayment,
    pub payload: ExactEvmPayload,
}

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
    pub authorization: Eip3009Authorization,
}

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
