//! Keystore API client with SIWE authentication and x402 payment support
//!
//! Handles authenticated access to the keystore.defirelay.com API for
//! storing and retrieving encrypted backups. Supports x402 payments when
//! the keystore server requires them.

use ethers::signers::{LocalWallet, Signer};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

use crate::backup::BackupData;
use crate::wallet::WalletProvider;

/// Default keystore API URL
pub const DEFAULT_KEYSTORE_URL: &str = "https://keystore.defirelay.com";

/// HTTP request timeout
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum payment amount in wei (1000 STARKBOT with 18 decimals)
/// This is a safety limit to prevent the keystore server from overcharging
const MAX_PAYMENT_WEI: &str = "1000000000000000000000";

/// Cached session for keystore API
#[derive(Debug, Clone)]
struct KeystoreSession {
    token: String,
    expires_at: chrono::DateTime<chrono::Utc>,
}

/// Thread-safe keystore client with session caching
pub struct KeystoreClient {
    session: Arc<RwLock<Option<KeystoreSession>>>,
    http_client: reqwest::Client,
    /// Configurable keystore URL (can be changed at runtime)
    base_url: Arc<RwLock<String>>,
}

// Request/Response types for keystore API

#[derive(Serialize)]
struct AuthorizeRequest {
    address: String,
}

#[derive(Deserialize)]
struct AuthorizeResponse {
    success: bool,
    message: Option<String>,
    nonce: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
struct VerifyRequest {
    address: String,
    signature: String,
}

#[derive(Deserialize)]
struct VerifyResponse {
    success: bool,
    token: Option<String>,
    expires_at: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
struct StoreKeysRequest {
    encrypted_data: String,
    key_count: usize,
}

#[derive(Deserialize)]
pub struct StoreKeysResponse {
    pub success: bool,
    pub message: Option<String>,
    pub key_count: Option<usize>,
    pub updated_at: Option<String>,
    pub error: Option<String>,
}

#[derive(Deserialize)]
pub struct GetKeysResponse {
    pub success: bool,
    pub encrypted_data: Option<String>,
    pub key_count: Option<usize>,
    pub updated_at: Option<String>,
    pub error: Option<String>,
}

/// x402 Payment Required response (returned on 402)
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PaymentRequiredResponse {
    pub x402_version: u32,
    pub accepts: Vec<PaymentRequirements>,
}

/// Payment requirements from 402 response
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PaymentRequirements {
    pub scheme: String,
    pub network: String,
    pub max_amount_required: String,
    pub resource: String,
    pub pay_to: String,
    pub max_timeout_seconds: u64,
    pub asset: String,
    #[serde(default)]
    pub extra: Option<serde_json::Value>,
}

impl KeystoreClient {
    /// Create a new keystore client with default URL
    pub fn new() -> Self {
        Self::with_url(DEFAULT_KEYSTORE_URL)
    }

    /// Create a new keystore client with custom URL
    pub fn with_url(url: &str) -> Self {
        Self {
            session: Arc::new(RwLock::new(None)),
            http_client: reqwest::Client::builder()
                .timeout(REQUEST_TIMEOUT)
                .build()
                .expect("Failed to build HTTP client"),
            base_url: Arc::new(RwLock::new(url.trim_end_matches('/').to_string())),
        }
    }

    /// Get the current base URL
    pub async fn get_base_url(&self) -> String {
        self.base_url.read().await.clone()
    }

    /// Set a new base URL (clears session since it may be for different server)
    pub async fn set_base_url(&self, url: &str) {
        let mut base_url = self.base_url.write().await;
        let new_url = url.trim_end_matches('/').to_string();
        if *base_url != new_url {
            *base_url = new_url;
            // Clear session since we're pointing to a different server
            let mut session = self.session.write().await;
            *session = None;
            log::info!("[Keystore] Base URL changed, session cleared");
        }
    }

    /// Check if current session is valid (exists and not expired)
    #[allow(dead_code)]
    async fn is_session_valid(&self) -> bool {
        let session = self.session.read().await;
        if let Some(ref s) = *session {
            // Add 60 second buffer before expiry
            s.expires_at > chrono::Utc::now() + chrono::Duration::seconds(60)
        } else {
            false
        }
    }

    /// Get the current session token if valid
    async fn get_token(&self) -> Option<String> {
        let session = self.session.read().await;
        if let Some(ref s) = *session {
            if s.expires_at > chrono::Utc::now() + chrono::Duration::seconds(60) {
                return Some(s.token.clone());
            }
        }
        None
    }

    /// Authenticate with the keystore server using SIWE
    async fn authenticate(&self, private_key: &str) -> Result<String, String> {
        // Parse wallet from private key
        let pk_clean = private_key.trim_start_matches("0x");
        let wallet: LocalWallet = pk_clean
            .parse()
            .map_err(|e| format!("Invalid private key: {:?}", e))?;
        let address = format!("{:?}", wallet.address());

        let base_url = self.get_base_url().await;
        log::info!("[Keystore] Authenticating wallet: {} (server: {})", address, base_url);

        // Step 1: Request challenge
        let auth_resp = self
            .http_client
            .post(format!("{}/api/authorize", base_url))
            .json(&AuthorizeRequest {
                address: address.clone(),
            })
            .send()
            .await
            .map_err(|e| format!("Failed to connect to keystore: {}", e))?;

        if !auth_resp.status().is_success() {
            return Err(format!(
                "Keystore authorize failed with status: {}",
                auth_resp.status()
            ));
        }

        let auth_data: AuthorizeResponse = auth_resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse authorize response: {}", e))?;

        if !auth_data.success {
            return Err(auth_data.error.unwrap_or_else(|| "Authorization failed".to_string()));
        }

        let message = auth_data
            .message
            .ok_or_else(|| "No challenge message in response".to_string())?;

        log::debug!("[Keystore] Got challenge, signing...");

        // Step 2: Sign the SIWE message
        let signature = wallet
            .sign_message(&message)
            .await
            .map_err(|e| format!("Failed to sign message: {:?}", e))?;
        let signature_hex = format!("0x{}", hex::encode(signature.to_vec()));

        // Step 3: Verify signature and get token
        let verify_resp = self
            .http_client
            .post(format!("{}/api/authorize/verify", base_url))
            .json(&VerifyRequest {
                address: address.clone(),
                signature: signature_hex,
            })
            .send()
            .await
            .map_err(|e| format!("Failed to verify signature: {}", e))?;

        if !verify_resp.status().is_success() {
            return Err(format!(
                "Keystore verify failed with status: {}",
                verify_resp.status()
            ));
        }

        let verify_data: VerifyResponse = verify_resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse verify response: {}", e))?;

        if !verify_data.success {
            return Err(verify_data.error.unwrap_or_else(|| "Verification failed".to_string()));
        }

        let token = verify_data
            .token
            .ok_or_else(|| "No token in response".to_string())?;

        let expires_at = if let Some(exp) = verify_data.expires_at {
            chrono::DateTime::parse_from_rfc3339(&exp)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now() + chrono::Duration::hours(1))
        } else {
            chrono::Utc::now() + chrono::Duration::hours(1)
        };

        // Cache the session
        let mut session = self.session.write().await;
        *session = Some(KeystoreSession {
            token: token.clone(),
            expires_at,
        });

        log::info!("[Keystore] Authentication successful, token expires at {}", expires_at);

        Ok(token)
    }

    /// Ensure we have a valid session, authenticating if needed
    async fn ensure_authenticated(&self, private_key: &str) -> Result<String, String> {
        if let Some(token) = self.get_token().await {
            return Ok(token);
        }
        self.authenticate(private_key).await
    }

    /// Store encrypted keys to the keystore
    /// Handles x402 payment automatically if required by the server
    pub async fn store_keys(
        &self,
        private_key: &str,
        encrypted_data: &str,
        key_count: usize,
    ) -> Result<StoreKeysResponse, String> {
        let token = self.ensure_authenticated(private_key).await?;
        let base_url = self.get_base_url().await;

        let resp = self
            .http_client
            .post(format!("{}/api/store_keys", base_url))
            .header("Authorization", format!("Bearer {}", token))
            .json(&StoreKeysRequest {
                encrypted_data: encrypted_data.to_string(),
                key_count,
            })
            .send()
            .await
            .map_err(|e| format!("Failed to connect to keystore: {}", e))?;

        // If unauthorized, try re-authenticating once
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            log::warn!("[Keystore] Token expired, re-authenticating...");
            let new_token = self.authenticate(private_key).await?;

            let retry_resp = self
                .http_client
                .post(format!("{}/api/store_keys", base_url))
                .header("Authorization", format!("Bearer {}", new_token))
                .json(&StoreKeysRequest {
                    encrypted_data: encrypted_data.to_string(),
                    key_count,
                })
                .send()
                .await
                .map_err(|e| format!("Failed to connect to keystore: {}", e))?;

            // Check for 402 on retry
            if retry_resp.status().as_u16() == 402 {
                return self.handle_x402_store_keys(
                    private_key,
                    &new_token,
                    encrypted_data,
                    key_count,
                    retry_resp,
                ).await;
            }

            return retry_resp
                .json()
                .await
                .map_err(|e| format!("Failed to parse response: {}", e));
        }

        // Check for 402 Payment Required
        if resp.status().as_u16() == 402 {
            return self.handle_x402_store_keys(
                private_key,
                &token,
                encrypted_data,
                key_count,
                resp,
            ).await;
        }

        resp.json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Handle x402 payment for store_keys
    async fn handle_x402_store_keys(
        &self,
        private_key: &str,
        token: &str,
        encrypted_data: &str,
        key_count: usize,
        response: reqwest::Response,
    ) -> Result<StoreKeysResponse, String> {
        let base_url = self.get_base_url().await;
        log::info!("[Keystore] Server requires x402 payment for storage");

        // Parse 402 response to get payment requirements
        let payment_required: PaymentRequiredResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse 402 response: {}", e))?;

        let requirements = payment_required.accepts.first()
            .ok_or_else(|| "No payment options in 402 response".to_string())?;

        // Check if amount is within our safety limit
        let required_amount = &requirements.max_amount_required;

        // Parse amounts as decimal strings for comparison
        let required_num: u128 = required_amount.parse()
            .map_err(|_| format!("Invalid payment amount: {}", required_amount))?;
        let max_num: u128 = MAX_PAYMENT_WEI.parse()
            .map_err(|_| "Invalid max payment constant".to_string())?;

        if required_num > max_num {
            return Err(format!(
                "Payment amount {} exceeds safety limit of {} (1000 STARKBOT)",
                required_amount, MAX_PAYMENT_WEI
            ));
        }

        log::info!(
            "[Keystore] Payment required: {} to {} on {}",
            requirements.max_amount_required,
            requirements.pay_to,
            requirements.network
        );

        // Use our x402 signer to create the payment
        let signer = crate::x402::X402Signer::from_private_key(private_key)
            .map_err(|e| format!("Failed to create x402 signer: {}", e))?;

        // Parse extra field to get token metadata
        let extra = requirements.extra.as_ref().and_then(|e| {
            serde_json::from_value::<crate::x402::PaymentExtra>(e.clone()).ok()
        });

        // Build x402 payment requirements in the format expected by the signer
        let x402_requirements = crate::x402::PaymentRequirements {
            scheme: requirements.scheme.clone(),
            network: requirements.network.clone(),
            max_amount_required: requirements.max_amount_required.clone(),
            resource: Some(requirements.resource.clone()),
            description: Some("Store encrypted backup".to_string()),
            pay_to_address: requirements.pay_to.clone(),
            max_timeout_seconds: requirements.max_timeout_seconds,
            asset: requirements.asset.clone(),
            extra,
        };

        // Sign the payment
        let payment_payload = signer.sign_payment(&x402_requirements).await
            .map_err(|e| format!("Failed to sign x402 payment: {}", e))?;

        let payment_header = payment_payload.to_base64()
            .map_err(|e| format!("Failed to encode payment: {}", e))?;

        log::info!("[Keystore] Retrying request with x402 payment...");

        // Retry the request with both auth and payment headers
        let retry_resp = self
            .http_client
            .post(format!("{}/api/store_keys", base_url))
            .header("Authorization", format!("Bearer {}", token))
            .header("X-PAYMENT", payment_header)
            .json(&StoreKeysRequest {
                encrypted_data: encrypted_data.to_string(),
                key_count,
            })
            .send()
            .await
            .map_err(|e| format!("Failed to send paid request: {}", e))?;

        if retry_resp.status().is_success() {
            log::info!("[Keystore] Payment accepted, backup stored successfully");
        } else {
            log::warn!("[Keystore] Paid request returned status: {}", retry_resp.status());
        }

        retry_resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse response after payment: {}", e))
    }

    /// Get encrypted keys from the keystore
    pub async fn get_keys(&self, private_key: &str) -> Result<GetKeysResponse, String> {
        let token = self.ensure_authenticated(private_key).await?;
        let base_url = self.get_base_url().await;

        let resp = self
            .http_client
            .post(format!("{}/api/get_keys", base_url))
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| format!("Failed to connect to keystore: {}", e))?;

        // If unauthorized, try re-authenticating once
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            log::warn!("[Keystore] Token expired, re-authenticating...");
            let new_token = self.authenticate(private_key).await?;

            let retry_resp = self
                .http_client
                .post(format!("{}/api/get_keys", base_url))
                .header("Authorization", format!("Bearer {}", new_token))
                .send()
                .await
                .map_err(|e| format!("Failed to connect to keystore: {}", e))?;

            return retry_resp
                .json()
                .await
                .map_err(|e| format!("Failed to parse response: {}", e));
        }

        // Handle 404 specifically
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(GetKeysResponse {
                success: false,
                encrypted_data: None,
                key_count: None,
                updated_at: None,
                error: Some("No backup found for this wallet".to_string()),
            });
        }

        resp.json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Clear the cached session (for testing or logout)
    pub async fn clear_session(&self) {
        let mut session = self.session.write().await;
        *session = None;
    }

    // =====================================================
    // WalletProvider-aware methods (for Flash/Privy mode)
    // =====================================================

    /// Authenticate with the keystore server using SIWE via WalletProvider
    async fn authenticate_with_provider(&self, provider: &Arc<dyn WalletProvider>) -> Result<String, String> {
        let address = provider.get_address();

        let base_url = self.get_base_url().await;
        log::info!("[Keystore] Authenticating wallet via provider: {} (server: {})", address, base_url);

        // Step 1: Request challenge
        let auth_resp = self
            .http_client
            .post(format!("{}/api/authorize", base_url))
            .json(&AuthorizeRequest {
                address: address.clone(),
            })
            .send()
            .await
            .map_err(|e| format!("Failed to connect to keystore: {}", e))?;

        if !auth_resp.status().is_success() {
            return Err(format!(
                "Keystore authorize failed with status: {}",
                auth_resp.status()
            ));
        }

        let auth_data: AuthorizeResponse = auth_resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse authorize response: {}", e))?;

        if !auth_data.success {
            return Err(auth_data.error.unwrap_or_else(|| "Authorization failed".to_string()));
        }

        let message = auth_data
            .message
            .ok_or_else(|| "No challenge message in response".to_string())?;

        log::debug!("[Keystore] Got challenge, signing via provider...");

        // Step 2: Sign the SIWE message via WalletProvider
        let signature = provider
            .sign_message(message.as_bytes())
            .await
            .map_err(|e| format!("Failed to sign message: {}", e))?;
        let signature_hex = format!("0x{}", hex::encode(signature.to_vec()));

        // Step 3: Verify signature and get token
        let verify_resp = self
            .http_client
            .post(format!("{}/api/authorize/verify", base_url))
            .json(&VerifyRequest {
                address: address.clone(),
                signature: signature_hex,
            })
            .send()
            .await
            .map_err(|e| format!("Failed to verify signature: {}", e))?;

        if !verify_resp.status().is_success() {
            return Err(format!(
                "Keystore verify failed with status: {}",
                verify_resp.status()
            ));
        }

        let verify_data: VerifyResponse = verify_resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse verify response: {}", e))?;

        if !verify_data.success {
            return Err(verify_data.error.unwrap_or_else(|| "Verification failed".to_string()));
        }

        let token = verify_data
            .token
            .ok_or_else(|| "No token in response".to_string())?;

        let expires_at = if let Some(exp) = verify_data.expires_at {
            chrono::DateTime::parse_from_rfc3339(&exp)
                .map(|dt| dt.with_timezone(&chrono::Utc))
                .unwrap_or_else(|_| chrono::Utc::now() + chrono::Duration::hours(1))
        } else {
            chrono::Utc::now() + chrono::Duration::hours(1)
        };

        // Cache the session
        let mut session = self.session.write().await;
        *session = Some(KeystoreSession {
            token: token.clone(),
            expires_at,
        });

        log::info!("[Keystore] Authentication via provider successful, token expires at {}", expires_at);

        Ok(token)
    }

    /// Ensure we have a valid session, authenticating via WalletProvider if needed
    async fn ensure_authenticated_with_provider(&self, provider: &Arc<dyn WalletProvider>) -> Result<String, String> {
        if let Some(token) = self.get_token().await {
            return Ok(token);
        }
        self.authenticate_with_provider(provider).await
    }

    /// Store encrypted keys using WalletProvider for auth and x402 signing
    pub async fn store_keys_with_provider(
        &self,
        provider: &Arc<dyn WalletProvider>,
        encrypted_data: &str,
        key_count: usize,
    ) -> Result<StoreKeysResponse, String> {
        let token = self.ensure_authenticated_with_provider(provider).await?;
        let base_url = self.get_base_url().await;

        let resp = self
            .http_client
            .post(format!("{}/api/store_keys", base_url))
            .header("Authorization", format!("Bearer {}", token))
            .json(&StoreKeysRequest {
                encrypted_data: encrypted_data.to_string(),
                key_count,
            })
            .send()
            .await
            .map_err(|e| format!("Failed to connect to keystore: {}", e))?;

        // If unauthorized, try re-authenticating once
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            log::warn!("[Keystore] Token expired, re-authenticating via provider...");
            let new_token = self.authenticate_with_provider(provider).await?;

            let retry_resp = self
                .http_client
                .post(format!("{}/api/store_keys", base_url))
                .header("Authorization", format!("Bearer {}", new_token))
                .json(&StoreKeysRequest {
                    encrypted_data: encrypted_data.to_string(),
                    key_count,
                })
                .send()
                .await
                .map_err(|e| format!("Failed to connect to keystore: {}", e))?;

            // Check for 402 on retry
            if retry_resp.status().as_u16() == 402 {
                return self.handle_x402_store_keys_with_provider(
                    provider,
                    &new_token,
                    encrypted_data,
                    key_count,
                    retry_resp,
                ).await;
            }

            return retry_resp
                .json()
                .await
                .map_err(|e| format!("Failed to parse response: {}", e));
        }

        // Check for 402 Payment Required
        if resp.status().as_u16() == 402 {
            return self.handle_x402_store_keys_with_provider(
                provider,
                &token,
                encrypted_data,
                key_count,
                resp,
            ).await;
        }

        resp.json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }

    /// Handle x402 payment for store_keys using WalletProvider
    async fn handle_x402_store_keys_with_provider(
        &self,
        provider: &Arc<dyn WalletProvider>,
        token: &str,
        encrypted_data: &str,
        key_count: usize,
        response: reqwest::Response,
    ) -> Result<StoreKeysResponse, String> {
        let base_url = self.get_base_url().await;
        log::info!("[Keystore] Server requires x402 payment for storage (via provider)");

        // Parse 402 response to get payment requirements
        let payment_required: PaymentRequiredResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse 402 response: {}", e))?;

        let requirements = payment_required.accepts.first()
            .ok_or_else(|| "No payment options in 402 response".to_string())?;

        // Check if amount is within our safety limit
        let required_amount = &requirements.max_amount_required;

        let required_num: u128 = required_amount.parse()
            .map_err(|_| format!("Invalid payment amount: {}", required_amount))?;
        let max_num: u128 = MAX_PAYMENT_WEI.parse()
            .map_err(|_| "Invalid max payment constant".to_string())?;

        if required_num > max_num {
            return Err(format!(
                "Payment amount {} exceeds safety limit of {} (1000 STARKBOT)",
                required_amount, MAX_PAYMENT_WEI
            ));
        }

        log::info!(
            "[Keystore] Payment required: {} to {} on {}",
            requirements.max_amount_required,
            requirements.pay_to,
            requirements.network
        );

        // Use X402Signer with the WalletProvider (supports Flash/Privy)
        let signer = crate::x402::X402Signer::new(provider.clone());

        // Parse extra field to get token metadata
        let extra = requirements.extra.as_ref().and_then(|e| {
            serde_json::from_value::<crate::x402::PaymentExtra>(e.clone()).ok()
        });

        // Build x402 payment requirements in the format expected by the signer
        let x402_requirements = crate::x402::PaymentRequirements {
            scheme: requirements.scheme.clone(),
            network: requirements.network.clone(),
            max_amount_required: requirements.max_amount_required.clone(),
            resource: Some(requirements.resource.clone()),
            description: Some("Store encrypted backup".to_string()),
            pay_to_address: requirements.pay_to.clone(),
            max_timeout_seconds: requirements.max_timeout_seconds,
            asset: requirements.asset.clone(),
            extra,
        };

        // Sign the payment
        let payment_payload = signer.sign_payment(&x402_requirements).await
            .map_err(|e| format!("Failed to sign x402 payment: {}", e))?;

        let payment_header = payment_payload.to_base64()
            .map_err(|e| format!("Failed to encode payment: {}", e))?;

        log::info!("[Keystore] Retrying request with x402 payment (via provider)...");

        // Retry the request with both auth and payment headers
        let retry_resp = self
            .http_client
            .post(format!("{}/api/store_keys", base_url))
            .header("Authorization", format!("Bearer {}", token))
            .header("X-PAYMENT", payment_header)
            .json(&StoreKeysRequest {
                encrypted_data: encrypted_data.to_string(),
                key_count,
            })
            .send()
            .await
            .map_err(|e| format!("Failed to send paid request: {}", e))?;

        if retry_resp.status().is_success() {
            log::info!("[Keystore] Payment accepted, backup stored successfully");
        } else {
            log::warn!("[Keystore] Paid request returned status: {}", retry_resp.status());
        }

        retry_resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse response after payment: {}", e))
    }

    /// Get encrypted keys using WalletProvider for auth
    pub async fn get_keys_with_provider(&self, provider: &Arc<dyn WalletProvider>) -> Result<GetKeysResponse, String> {
        let token = self.ensure_authenticated_with_provider(provider).await?;
        let base_url = self.get_base_url().await;

        let resp = self
            .http_client
            .post(format!("{}/api/get_keys", base_url))
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| format!("Failed to connect to keystore: {}", e))?;

        // If unauthorized, try re-authenticating once
        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            log::warn!("[Keystore] Token expired, re-authenticating via provider...");
            let new_token = self.authenticate_with_provider(provider).await?;

            let retry_resp = self
                .http_client
                .post(format!("{}/api/get_keys", base_url))
                .header("Authorization", format!("Bearer {}", new_token))
                .send()
                .await
                .map_err(|e| format!("Failed to connect to keystore: {}", e))?;

            return retry_resp
                .json()
                .await
                .map_err(|e| format!("Failed to parse response: {}", e));
        }

        // Handle 404 specifically
        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(GetKeysResponse {
                success: false,
                encrypted_data: None,
                key_count: None,
                updated_at: None,
                error: Some("No backup found for this wallet".to_string()),
            });
        }

        resp.json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))
    }
}

impl Default for KeystoreClient {
    fn default() -> Self {
        Self::new()
    }
}

// Global singleton for the keystore client
lazy_static::lazy_static! {
    pub static ref KEYSTORE_CLIENT: KeystoreClient = KeystoreClient::new();
}

// =====================================================
// Extended Backup Helpers
// =====================================================

/// Encrypt BackupData using ECIES with the wallet's public key
pub fn encrypt_backup_data(private_key: &str, backup: &BackupData) -> Result<String, String> {
    use ecies::{encrypt, PublicKey, SecretKey};

    // Serialize backup to JSON
    let backup_json = serde_json::to_string(backup)
        .map_err(|e| format!("Failed to serialize backup: {}", e))?;

    // Parse private key
    let pk_hex = private_key.trim_start_matches("0x");
    let pk_bytes = hex::decode(pk_hex)
        .map_err(|e| format!("Invalid private key hex: {}", e))?;

    // Derive public key
    let secret_key = SecretKey::parse_slice(&pk_bytes)
        .map_err(|e| format!("Invalid private key: {:?}", e))?;
    let public_key = PublicKey::from_secret_key(&secret_key);

    // Encrypt
    let encrypted = encrypt(&public_key.serialize(), backup_json.as_bytes())
        .map_err(|e| format!("Encryption failed: {:?}", e))?;

    Ok(hex::encode(encrypted))
}

/// Decrypt BackupData using ECIES with the wallet's private key
pub fn decrypt_backup_data(private_key: &str, encrypted_hex: &str) -> Result<BackupData, String> {
    use ecies::{decrypt, SecretKey};

    // Parse private key
    let pk_hex = private_key.trim_start_matches("0x");
    let pk_bytes = hex::decode(pk_hex)
        .map_err(|e| format!("Invalid private key hex: {}", e))?;

    // Parse encrypted data
    let encrypted = hex::decode(encrypted_hex)
        .map_err(|e| format!("Invalid encrypted data: {}", e))?;

    // Create secret key and decrypt
    let secret_key = SecretKey::parse_slice(&pk_bytes)
        .map_err(|e| format!("Invalid private key: {:?}", e))?;

    let decrypted = decrypt(&secret_key.serialize(), &encrypted)
        .map_err(|e| format!("Decryption failed: {:?}", e))?;

    // Parse JSON
    let backup: BackupData = serde_json::from_slice(&decrypted)
        .map_err(|e| format!("Invalid backup format: {}", e))?;

    Ok(backup)
}

/// Get wallet address from private key
pub fn get_wallet_address(private_key: &str) -> Result<String, String> {
    let pk_clean = private_key.trim_start_matches("0x");
    let wallet: LocalWallet = pk_clean
        .parse()
        .map_err(|e| format!("Invalid private key: {:?}", e))?;
    Ok(format!("{:?}", wallet.address()))
}
