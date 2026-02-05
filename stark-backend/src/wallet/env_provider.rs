//! Environment-based Wallet Provider (Standard Mode)
//!
//! Loads wallet from BURNER_WALLET_BOT_PRIVATE_KEY environment variable.
//! This is the original Starkbot behavior - wallet is configured at deploy time.

use async_trait::async_trait;
use ethers::core::k256::ecdsa::SigningKey;
use ethers::signers::{LocalWallet, Signer};

use super::WalletProvider;
use crate::config::env_vars;

/// Wallet provider that loads from environment variable
pub struct EnvWalletProvider {
    wallet: LocalWallet,
    address: String,
}

impl EnvWalletProvider {
    /// Create provider from environment variable
    ///
    /// Requires: BURNER_WALLET_BOT_PRIVATE_KEY
    pub fn from_env() -> Result<Self, String> {
        let private_key = std::env::var(env_vars::BURNER_WALLET_PRIVATE_KEY)
            .map_err(|_| format!("{} not set", env_vars::BURNER_WALLET_PRIVATE_KEY))?;

        Self::from_private_key(&private_key)
    }

    /// Create provider from a private key string
    pub fn from_private_key(private_key: &str) -> Result<Self, String> {
        let key_hex = private_key.strip_prefix("0x").unwrap_or(private_key);

        let key_bytes = hex::decode(key_hex)
            .map_err(|e| format!("Invalid private key hex: {}", e))?;

        let signing_key = SigningKey::from_bytes(key_bytes.as_slice().into())
            .map_err(|e| format!("Invalid private key: {}", e))?;

        let wallet = LocalWallet::from(signing_key);
        let address = format!("{:?}", wallet.address()).to_lowercase();

        Ok(Self { wallet, address })
    }
}

#[async_trait]
impl WalletProvider for EnvWalletProvider {
    async fn get_wallet(&self) -> Result<LocalWallet, String> {
        Ok(self.wallet.clone())
    }

    fn get_address(&self) -> String {
        self.address.clone()
    }

    fn mode_name(&self) -> &'static str {
        "standard"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_private_key() {
        // Test with a known test private key (DO NOT USE IN PRODUCTION)
        let test_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let provider = EnvWalletProvider::from_private_key(test_key).unwrap();

        // This is the known address for this test key (Hardhat account #0)
        assert_eq!(
            provider.get_address(),
            "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
        );
    }

    #[test]
    fn test_from_private_key_no_prefix() {
        let test_key = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let provider = EnvWalletProvider::from_private_key(test_key).unwrap();

        assert_eq!(
            provider.get_address(),
            "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
        );
    }

    #[tokio::test]
    async fn test_get_wallet() {
        let test_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let provider = EnvWalletProvider::from_private_key(test_key).unwrap();

        let wallet = provider.get_wallet().await.unwrap();
        assert_eq!(
            format!("{:?}", wallet.address()).to_lowercase(),
            "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266"
        );
    }
}
