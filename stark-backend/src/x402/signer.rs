//! EIP-3009 signing for x402 payments

use ethers::core::k256::ecdsa::SigningKey;
use ethers::signers::{LocalWallet, Signer};
use ethers::types::{H256, U256};
use ethers::utils::keccak256;
use std::time::{SystemTime, UNIX_EPOCH};

use super::types::*;

/// x402 payment signer using a local wallet
pub struct X402Signer {
    wallet: LocalWallet,
}

impl X402Signer {
    /// Create a new signer from a private key (hex string with or without 0x prefix)
    pub fn new(private_key: &str) -> Result<Self, String> {
        let key_hex = private_key.strip_prefix("0x").unwrap_or(private_key);
        let key_bytes = hex::decode(key_hex)
            .map_err(|e| format!("Invalid private key hex: {}", e))?;

        let signing_key = SigningKey::from_bytes(key_bytes.as_slice().into())
            .map_err(|e| format!("Invalid private key: {}", e))?;

        let wallet = LocalWallet::from(signing_key).with_chain_id(BASE_CHAIN_ID);

        Ok(Self { wallet })
    }

    /// Get the wallet address
    pub fn address(&self) -> String {
        format!("{:?}", self.wallet.address()).to_lowercase()
    }

    /// Generate a cryptographically secure nonce
    fn generate_nonce() -> H256 {
        let mut bytes = [0u8; 32];
        getrandom::getrandom(&mut bytes).expect("Failed to generate random bytes");
        H256::from(keccak256(bytes))
    }

    /// Sign an EIP-3009 TransferWithAuthorization for x402 payment
    pub async fn sign_payment(
        &self,
        requirements: &PaymentRequirements,
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

        // Build EIP-712 typed data for USDC TransferWithAuthorization
        let domain = eip712_domain();
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
        let signature = self.sign_typed_data(&domain, &message).await?;

        let authorization = Eip3009Authorization {
            from,
            to,
            value,
            valid_after,
            valid_before,
            nonce: nonce_hex,
        };

        let payload = PaymentPayload {
            x402_version: X402_VERSION,
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
                authorization,
            },
        };

        Ok(payload)
    }

    /// Sign EIP-712 typed data
    async fn sign_typed_data(
        &self,
        domain: &Eip712Domain,
        message: &TransferWithAuthorizationMessage,
    ) -> Result<String, String> {
        // Calculate domain separator
        let domain_separator = domain.separator();

        // Calculate struct hash
        let struct_hash = message.struct_hash();

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

/// EIP-712 domain for USDC on Base
struct Eip712Domain {
    name: String,
    version: String,
    chain_id: u64,
    verifying_contract: ethers::types::Address,
}

fn eip712_domain() -> Eip712Domain {
    Eip712Domain {
        name: "USD Coin".to_string(),
        version: "2".to_string(),
        chain_id: BASE_CHAIN_ID,
        verifying_contract: USDC_ADDRESS.parse().expect("Invalid USDC address"),
    }
}

impl Eip712Domain {
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
