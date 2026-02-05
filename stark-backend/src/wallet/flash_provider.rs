//! Flash Mode Wallet Provider
//!
//! Fetches wallet credentials from the Flash control plane.
//! Used when Starkbot is deployed via Flash (Starkbot Lite) where each
//! tenant has a Privy-managed wallet.
//!
//! Required environment variables:
//! - FLASH_KEYSTORE_URL: URL of the Flash control plane (e.g., https://flash.starkbot.io)
//! - FLASH_TENANT_ID: Tenant identifier
//! - FLASH_INSTANCE_TOKEN: Authentication token for this instance
//! - FLASH_WALLET_ADDRESS: Public wallet address (for caching/display)

use async_trait::async_trait;
use ethers::core::k256::ecdsa::SigningKey;
use ethers::signers::{LocalWallet, Signer};
use serde::Deserialize;
use tokio::sync::RwLock;

use super::WalletProvider;

/// Environment variables for Flash mode
pub mod env_vars {
    pub const FLASH_KEYSTORE_URL: &str = "FLASH_KEYSTORE_URL";
    pub const FLASH_TENANT_ID: &str = "FLASH_TENANT_ID";
    pub const FLASH_INSTANCE_TOKEN: &str = "FLASH_INSTANCE_TOKEN";
    pub const FLASH_WALLET_ADDRESS: &str = "FLASH_WALLET_ADDRESS";
}

/// Response from the Flash keystore API
#[derive(Debug, Deserialize)]
struct KeystoreResponse {
    private_key: String,
    admin_address: String,
}

/// Wallet provider that fetches credentials from Flash control plane
pub struct FlashWalletProvider {
    keystore_url: String,
    tenant_id: String,
    instance_token: String,
    address: String,
    http_client: reqwest::Client,
    /// Cached wallet - fetched on first use, can be refreshed
    cached_wallet: RwLock<Option<LocalWallet>>,
}

impl FlashWalletProvider {
    /// Create a new Flash wallet provider from environment variables
    pub async fn new() -> Result<Self, String> {
        let keystore_url = std::env::var(env_vars::FLASH_KEYSTORE_URL)
            .map_err(|_| format!("{} not set", env_vars::FLASH_KEYSTORE_URL))?;

        let tenant_id = std::env::var(env_vars::FLASH_TENANT_ID)
            .map_err(|_| format!("{} not set", env_vars::FLASH_TENANT_ID))?;

        let instance_token = std::env::var(env_vars::FLASH_INSTANCE_TOKEN)
            .map_err(|_| format!("{} not set", env_vars::FLASH_INSTANCE_TOKEN))?;

        let address = std::env::var(env_vars::FLASH_WALLET_ADDRESS)
            .map_err(|_| format!("{} not set", env_vars::FLASH_WALLET_ADDRESS))?;

        let provider = Self {
            keystore_url,
            tenant_id,
            instance_token,
            address,
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .map_err(|e| format!("Failed to create HTTP client: {}", e))?,
            cached_wallet: RwLock::new(None),
        };

        // Pre-fetch wallet on initialization to fail fast if credentials are invalid
        provider.fetch_wallet().await?;

        Ok(provider)
    }

    /// Fetch wallet from the Flash control plane
    async fn fetch_wallet(&self) -> Result<LocalWallet, String> {
        log::debug!("Fetching wallet from Flash control plane: {}", self.keystore_url);

        let url = format!("{}/api/keystore/wallet", self.keystore_url);

        let response = self
            .http_client
            .get(&url)
            .header("X-Tenant-ID", &self.tenant_id)
            .header("X-Instance-Token", &self.instance_token)
            .send()
            .await
            .map_err(|e| format!("Flash keystore request failed: {}", e))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(format!(
                "Flash keystore error ({}): {}",
                status, body
            ));
        }

        let data: KeystoreResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse keystore response: {}", e))?;

        // Parse private key into wallet
        let key_hex = data.private_key.strip_prefix("0x").unwrap_or(&data.private_key);

        let key_bytes = hex::decode(key_hex)
            .map_err(|e| format!("Invalid private key from keystore: {}", e))?;

        let signing_key = SigningKey::from_bytes(key_bytes.as_slice().into())
            .map_err(|e| format!("Invalid signing key from keystore: {}", e))?;

        let wallet = LocalWallet::from(signing_key);

        // Verify address matches what we expect
        let derived_address = format!("{:?}", wallet.address()).to_lowercase();
        if derived_address != self.address.to_lowercase() {
            log::warn!(
                "Flash wallet address mismatch: expected {}, got {}",
                self.address,
                derived_address
            );
        }

        log::info!("Successfully fetched wallet from Flash control plane");
        Ok(wallet)
    }
}

#[async_trait]
impl WalletProvider for FlashWalletProvider {
    async fn get_wallet(&self) -> Result<LocalWallet, String> {
        // Check cache first
        {
            let cache = self.cached_wallet.read().await;
            if let Some(wallet) = cache.as_ref() {
                return Ok(wallet.clone());
            }
        }

        // Fetch and cache
        let wallet = self.fetch_wallet().await?;
        {
            let mut cache = self.cached_wallet.write().await;
            *cache = Some(wallet.clone());
        }

        Ok(wallet)
    }

    fn get_address(&self) -> String {
        self.address.clone()
    }

    async fn refresh(&self) -> Result<(), String> {
        log::info!("Refreshing wallet from Flash control plane");
        let wallet = self.fetch_wallet().await?;
        let mut cache = self.cached_wallet.write().await;
        *cache = Some(wallet);
        Ok(())
    }

    fn mode_name(&self) -> &'static str {
        "flash"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_env_vars_defined() {
        // Just ensure the env var names are consistent
        assert_eq!(env_vars::FLASH_KEYSTORE_URL, "FLASH_KEYSTORE_URL");
        assert_eq!(env_vars::FLASH_TENANT_ID, "FLASH_TENANT_ID");
        assert_eq!(env_vars::FLASH_INSTANCE_TOKEN, "FLASH_INSTANCE_TOKEN");
        assert_eq!(env_vars::FLASH_WALLET_ADDRESS, "FLASH_WALLET_ADDRESS");
    }
}
