//! EIP-2612 and EIP-3009 signing for x402 payments
//!
//! Supports two payment schemes:
//! - "permit" (EIP-2612): Permit signature allowing facilitator to transfer tokens
//! - "exact" (EIP-3009): TransferWithAuthorization for direct transfers

use ethers::core::k256::ecdsa::SigningKey;
use ethers::signers::{LocalWallet, Signer};
use ethers::types::{H256, U256};
use ethers::utils::keccak256;
use std::time::{SystemTime, UNIX_EPOCH};

use super::erc20;
use super::types::*;

/// x402 payment signer using a local wallet
pub struct X402Signer {
    wallet: LocalWallet,
    private_key: String,
}

impl X402Signer {
    /// Create a new signer from a private key (hex string with or without 0x prefix)
    pub fn new(private_key: &str) -> Result<Self, String> {
        let key_hex = private_key.strip_prefix("0x").unwrap_or(private_key);
        let key_bytes = hex::decode(key_hex)
            .map_err(|e| format!("Invalid private key hex: {}", e))?;

        let signing_key = SigningKey::from_bytes(key_bytes.as_slice().into())
            .map_err(|e| format!("Invalid private key: {}", e))?;

        // Default chain ID, will be overridden per-signature based on network
        let wallet = LocalWallet::from(signing_key).with_chain_id(BASE_CHAIN_ID);

        Ok(Self {
            wallet,
            private_key: private_key.to_string(),
        })
    }

    /// Get the wallet address
    pub fn address(&self) -> String {
        format!("{:?}", self.wallet.address()).to_lowercase()
    }

    /// Generate a cryptographically secure nonce (for EIP-3009)
    fn generate_nonce() -> H256 {
        let mut bytes = [0u8; 32];
        getrandom::getrandom(&mut bytes).expect("Failed to generate random bytes");
        H256::from(keccak256(bytes))
    }

    /// Fetch EIP-2612 permit nonce from token contract
    async fn fetch_permit_nonce(
        &self,
        network: &str,
        token_address: ethers::types::Address,
    ) -> Result<U256, String> {
        // Get RPC URL based on network
        let rpc_url = match network {
            "base-sepolia" => "https://sepolia.base.org",
            _ => "https://mainnet.base.org", // Default to Base mainnet
        };

        // Encode the nonces(address) call
        let call_data = erc20::encode_nonces(self.wallet.address());

        // Build JSON-RPC request
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "eth_call",
            "params": [{
                "to": format!("{:?}", token_address),
                "data": format!("0x{}", hex::encode(&call_data))
            }, "latest"],
            "id": 1
        });

        // Make the RPC call
        let client = reqwest::Client::new();
        let response = client
            .post(rpc_url)
            .json(&request)
            .send()
            .await
            .map_err(|e| format!("RPC request failed: {}", e))?;

        let body: serde_json::Value = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse RPC response: {}", e))?;

        // Extract result
        let result = body.get("result")
            .and_then(|r| r.as_str())
            .ok_or_else(|| {
                let error = body.get("error").map(|e| e.to_string()).unwrap_or_default();
                format!("RPC error: {}", error)
            })?;

        // Decode the nonce
        let bytes = hex::decode(result.trim_start_matches("0x"))
            .map_err(|e| format!("Failed to decode nonce hex: {}", e))?;

        erc20::decode_nonces(&bytes)
    }

    /// Sign a payment in V1 format (for keystore relay)
    /// Automatically chooses EIP-2612 (permit) or EIP-3009 (exact) based on scheme
    pub async fn sign_payment(
        &self,
        requirements: &PaymentRequirements,
    ) -> Result<PaymentPayload, String> {
        let token_metadata = TokenMetadata::from_requirements(requirements);

        match requirements.scheme.as_str() {
            "permit" => self.sign_permit_v1(requirements, &token_metadata).await,
            "exact" | "eip3009" => self.sign_transfer_with_auth_v1(requirements, &token_metadata).await,
            other => Err(format!("Unsupported payment scheme: {}", other)),
        }
    }

    /// Sign a payment in V2 format (for Kimi/AI relay - has "accepted" field)
    /// Automatically chooses EIP-2612 (permit) or EIP-3009 (exact) based on scheme
    pub async fn sign_payment_v2(
        &self,
        requirements: &PaymentRequirements,
    ) -> Result<PaymentPayloadV2, String> {
        let token_metadata = TokenMetadata::from_requirements(requirements);

        match requirements.scheme.as_str() {
            "permit" => self.sign_permit_v2(requirements, &token_metadata).await,
            "exact" | "eip3009" => self.sign_transfer_with_auth_v2(requirements, &token_metadata).await,
            other => Err(format!("Unsupported payment scheme: {}", other)),
        }
    }

    /// Sign an EIP-2612 Permit for x402 payment (V1 format for keystore)
    /// The facilitator becomes the spender, allowing them to transfer tokens on our behalf
    async fn sign_permit_v1(
        &self,
        requirements: &PaymentRequirements,
        token_metadata: &TokenMetadata,
    ) -> Result<PaymentPayload, String> {
        let from = self.address();
        let value = requirements.max_amount_required.clone();

        // Get facilitator address (spender) from extra field
        let spender = requirements.extra.as_ref()
            .and_then(|e| e.facilitator_signer.clone())
            .ok_or("EIP-2612 permit requires facilitatorSigner in extra field")?;

        // Deadline: valid for 1 hour
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| format!("Time error: {}", e))?;
        let deadline = now.as_secs() + 3600;

        // For EIP-2612, nonce must be fetched from the token contract
        let token_address: ethers::types::Address = token_metadata.address.parse()
            .map_err(|e| format!("Invalid token address: {}", e))?;

        // Fetch nonce from token contract using direct RPC call
        let nonce_u256 = self.fetch_permit_nonce(&requirements.network, token_address).await?;

        log::info!(
            "[X402Signer] EIP-2612 permit nonce for {} on {}: {}",
            from, requirements.network, nonce_u256
        );

        // Build EIP-712 domain from token metadata
        let domain = Eip712Domain::from_token_metadata(token_metadata)?;

        // Build permit message
        let message = PermitMessage {
            owner: self.wallet.address(),
            spender: spender.parse()
                .map_err(|e| format!("Invalid facilitatorSigner address: {}", e))?,
            value: U256::from_dec_str(&requirements.max_amount_required)
                .map_err(|e| format!("Invalid amount: {}", e))?,
            nonce: nonce_u256,
            deadline: U256::from(deadline),
        };

        // Sign the typed data
        let signature = self.sign_eip712(&domain, message.struct_hash()).await?;

        // Build EIP-2612 authorization format
        let authorization = Eip2612Authorization {
            owner: from,
            spender,
            value,
            nonce: nonce_u256.to_string(),
            deadline: deadline.to_string(),
        };

        let payload = PaymentPayload {
            x402_version: X402_VERSION_V1,
            scheme: requirements.scheme.clone(),
            network: requirements.network.clone(),
            payload: ExactEvmPayload {
                signature,
                authorization: EvmAuthorization::Eip2612(authorization),
            },
        };

        Ok(payload)
    }

    /// Sign an EIP-2612 Permit for x402 payment (V2 format for Kimi/AI relay)
    async fn sign_permit_v2(
        &self,
        requirements: &PaymentRequirements,
        token_metadata: &TokenMetadata,
    ) -> Result<PaymentPayloadV2, String> {
        let from = self.address();
        let value = requirements.max_amount_required.clone();

        // Get facilitator address (spender) from extra field
        let spender = requirements.extra.as_ref()
            .and_then(|e| e.facilitator_signer.clone())
            .ok_or("EIP-2612 permit requires facilitatorSigner in extra field")?;

        // Deadline: valid for 1 hour
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| format!("Time error: {}", e))?;
        let deadline = now.as_secs() + 3600;

        // For EIP-2612, nonce must be fetched from the token contract
        let token_address: ethers::types::Address = token_metadata.address.parse()
            .map_err(|e| format!("Invalid token address: {}", e))?;

        // Fetch nonce from token contract using direct RPC call
        let nonce_u256 = self.fetch_permit_nonce(&requirements.network, token_address).await?;

        log::info!(
            "[X402Signer] EIP-2612 permit nonce for {} on {}: {}",
            from, requirements.network, nonce_u256
        );

        // Build EIP-712 domain from token metadata
        let domain = Eip712Domain::from_token_metadata(token_metadata)?;

        // Build permit message
        let message = PermitMessage {
            owner: self.wallet.address(),
            spender: spender.parse()
                .map_err(|e| format!("Invalid facilitatorSigner address: {}", e))?,
            value: U256::from_dec_str(&requirements.max_amount_required)
                .map_err(|e| format!("Invalid amount: {}", e))?,
            nonce: nonce_u256,
            deadline: U256::from(deadline),
        };

        // Sign the typed data
        let signature = self.sign_eip712(&domain, message.struct_hash()).await?;

        // Build EIP-2612 authorization format
        let authorization = Eip2612Authorization {
            owner: from,
            spender,
            value,
            nonce: nonce_u256.to_string(),
            deadline: deadline.to_string(),
        };

        // Build V2 payload with "accepted" field
        let payload = PaymentPayloadV2 {
            x402_version: X402_VERSION_V2,
            accepted: AcceptedPayment {
                scheme: requirements.scheme.clone(),
                network: requirements.network.clone(),
                amount: requirements.max_amount_required.clone(),
                pay_to: requirements.pay_to_address.clone(),
                max_timeout_seconds: requirements.max_timeout_seconds.max(60),
                asset: requirements.asset.clone(),
            },
            payload: ExactEvmPayload {
                signature,
                authorization: EvmAuthorization::Eip2612(authorization),
            },
        };

        Ok(payload)
    }

    /// Sign an EIP-3009 TransferWithAuthorization for x402 payment (V1 format)
    async fn sign_transfer_with_auth_v1(
        &self,
        requirements: &PaymentRequirements,
        token_metadata: &TokenMetadata,
    ) -> Result<PaymentPayload, String> {
        let from = self.address();
        let to = requirements.pay_to_address.to_lowercase();
        let value = requirements.max_amount_required.clone();
        let valid_after = "0".to_string();

        // Valid for 1 hour
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| format!("Time error: {}", e))?;
        let valid_before = (now.as_secs() + 3600).to_string();

        let nonce = Self::generate_nonce();
        let nonce_hex = format!("{:?}", nonce);

        // Build EIP-712 domain from token metadata
        let domain = Eip712Domain::from_token_metadata(token_metadata)?;

        let message = TransferWithAuthorizationMessage {
            from: self.wallet.address(),
            to: requirements.pay_to_address.parse()
                .map_err(|e| format!("Invalid pay_to_address: {}", e))?,
            value: U256::from_dec_str(&requirements.max_amount_required)
                .map_err(|e| format!("Invalid amount: {}", e))?,
            valid_after: U256::zero(),
            valid_before: U256::from_dec_str(&valid_before)
                .map_err(|e| format!("Invalid valid_before: {}", e))?,
            nonce,
        };

        // Sign the typed data
        let signature = self.sign_eip712(&domain, message.struct_hash()).await?;

        // Build EIP-3009 authorization format
        let authorization = Eip3009Authorization {
            from,
            to,
            value,
            valid_after,
            valid_before,
            nonce: nonce_hex,
        };

        let payload = PaymentPayload {
            x402_version: X402_VERSION_V1,
            scheme: requirements.scheme.clone(),
            network: requirements.network.clone(),
            payload: ExactEvmPayload {
                signature,
                authorization: EvmAuthorization::Eip3009(authorization),
            },
        };

        Ok(payload)
    }

    /// Sign an EIP-3009 TransferWithAuthorization for x402 payment (V2 format for Kimi/AI relay)
    async fn sign_transfer_with_auth_v2(
        &self,
        requirements: &PaymentRequirements,
        token_metadata: &TokenMetadata,
    ) -> Result<PaymentPayloadV2, String> {
        let from = self.address();
        let to = requirements.pay_to_address.to_lowercase();
        let value = requirements.max_amount_required.clone();
        let valid_after = "0".to_string();

        // Valid for 1 hour
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| format!("Time error: {}", e))?;
        let valid_before = (now.as_secs() + 3600).to_string();

        let nonce = Self::generate_nonce();
        let nonce_hex = format!("{:?}", nonce);

        // Build EIP-712 domain from token metadata
        let domain = Eip712Domain::from_token_metadata(token_metadata)?;

        let message = TransferWithAuthorizationMessage {
            from: self.wallet.address(),
            to: requirements.pay_to_address.parse()
                .map_err(|e| format!("Invalid pay_to_address: {}", e))?,
            value: U256::from_dec_str(&requirements.max_amount_required)
                .map_err(|e| format!("Invalid amount: {}", e))?,
            valid_after: U256::zero(),
            valid_before: U256::from_dec_str(&valid_before)
                .map_err(|e| format!("Invalid valid_before: {}", e))?,
            nonce,
        };

        // Sign the typed data
        let signature = self.sign_eip712(&domain, message.struct_hash()).await?;

        // Build EIP-3009 authorization format
        let authorization = Eip3009Authorization {
            from,
            to,
            value: value.clone(),
            valid_after,
            valid_before,
            nonce: nonce_hex,
        };

        // Build V2 payload with "accepted" field
        let payload = PaymentPayloadV2 {
            x402_version: X402_VERSION_V2,
            accepted: AcceptedPayment {
                scheme: requirements.scheme.clone(),
                network: requirements.network.clone(),
                amount: requirements.max_amount_required.clone(),
                pay_to: requirements.pay_to_address.clone(),
                max_timeout_seconds: requirements.max_timeout_seconds.max(60),
                asset: requirements.asset.clone(),
            },
            payload: ExactEvmPayload {
                signature,
                authorization: EvmAuthorization::Eip3009(authorization),
            },
        };

        Ok(payload)
    }

    /// Sign EIP-712 typed data with a pre-computed struct hash
    async fn sign_eip712(
        &self,
        domain: &Eip712Domain,
        struct_hash: H256,
    ) -> Result<String, String> {
        // Calculate domain separator
        let domain_separator = domain.separator();

        // Calculate final hash: keccak256("\x19\x01" ++ domainSeparator ++ structHash)
        let mut to_sign = Vec::with_capacity(66);
        to_sign.push(0x19);
        to_sign.push(0x01);
        to_sign.extend_from_slice(domain_separator.as_bytes());
        to_sign.extend_from_slice(struct_hash.as_bytes());
        let digest = H256::from(keccak256(&to_sign));

        // Sign the digest
        let signature = self.wallet
            .sign_hash(digest)
            .map_err(|e| format!("Failed to sign: {}", e))?;

        Ok(format!("0x{}", hex::encode(signature.to_vec())))
    }
}

/// EIP-712 domain for token signatures
struct Eip712Domain {
    name: String,
    version: String,
    chain_id: u64,
    verifying_contract: ethers::types::Address,
}

impl Eip712Domain {
    /// Create domain from token metadata (dynamic, not hardcoded)
    fn from_token_metadata(metadata: &TokenMetadata) -> Result<Self, String> {
        Ok(Eip712Domain {
            name: metadata.name.clone(),
            version: metadata.version.clone(),
            chain_id: metadata.chain_id,
            verifying_contract: metadata.address.parse()
                .map_err(|e| format!("Invalid token address: {}", e))?,
        })
    }

    fn separator(&self) -> H256 {
        let type_hash = keccak256(
            b"EIP712Domain(string name,string version,uint256 chainId,address verifyingContract)"
        );

        let name_hash = keccak256(self.name.as_bytes());
        let version_hash = keccak256(self.version.as_bytes());

        let mut encoded = Vec::new();
        encoded.extend_from_slice(&type_hash);
        encoded.extend_from_slice(&name_hash);
        encoded.extend_from_slice(&version_hash);
        encoded.extend_from_slice(&ethers::abi::encode(&[
            ethers::abi::Token::Uint(U256::from(self.chain_id)),
        ]));
        encoded.extend_from_slice(&ethers::abi::encode(&[
            ethers::abi::Token::Address(self.verifying_contract),
        ]));

        H256::from(keccak256(&encoded))
    }
}

/// EIP-2612 Permit message
struct PermitMessage {
    owner: ethers::types::Address,
    spender: ethers::types::Address,
    value: U256,
    nonce: U256,
    deadline: U256,
}

impl PermitMessage {
    fn struct_hash(&self) -> H256 {
        let type_hash = keccak256(
            b"Permit(address owner,address spender,uint256 value,uint256 nonce,uint256 deadline)"
        );

        let encoded = ethers::abi::encode(&[
            ethers::abi::Token::FixedBytes(type_hash.to_vec()),
            ethers::abi::Token::Address(self.owner),
            ethers::abi::Token::Address(self.spender),
            ethers::abi::Token::Uint(self.value),
            ethers::abi::Token::Uint(self.nonce),
            ethers::abi::Token::Uint(self.deadline),
        ]);

        H256::from(keccak256(&encoded))
    }
}

/// TransferWithAuthorization message for EIP-3009
struct TransferWithAuthorizationMessage {
    from: ethers::types::Address,
    to: ethers::types::Address,
    value: U256,
    valid_after: U256,
    valid_before: U256,
    nonce: H256,
}

impl TransferWithAuthorizationMessage {
    fn struct_hash(&self) -> H256 {
        let type_hash = keccak256(
            b"TransferWithAuthorization(address from,address to,uint256 value,uint256 validAfter,uint256 validBefore,bytes32 nonce)"
        );

        let encoded = ethers::abi::encode(&[
            ethers::abi::Token::FixedBytes(type_hash.to_vec()),
            ethers::abi::Token::Address(self.from),
            ethers::abi::Token::Address(self.to),
            ethers::abi::Token::Uint(self.value),
            ethers::abi::Token::Uint(self.valid_after),
            ethers::abi::Token::Uint(self.valid_before),
            ethers::abi::Token::FixedBytes(self.nonce.as_bytes().to_vec()),
        ]);

        H256::from(keccak256(&encoded))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_address_derivation() {
        // Test with a known private key
        let private_key = "0xac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80";
        let signer = X402Signer::new(private_key).unwrap();
        // This is Hardhat's first default account
        assert_eq!(signer.address(), "0xf39fd6e51aad88f6f4ce6ab8827279cfffb92266");
    }
}
