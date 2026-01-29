//! x402-aware HTTP client

use reqwest::{header, Client, Response};
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;

use super::signer::X402Signer;
use super::types::PaymentRequired;

/// HTTP client that automatically handles x402 payment flow
pub struct X402Client {
    client: Client,
    signer: Arc<X402Signer>,
}

impl X402Client {
    /// Create a new x402 client with a burner wallet private key
    pub fn new(private_key: &str) -> Result<Self, String> {
        let client = Client::builder()
            .timeout(Duration::from_secs(120))
            .build()
            .map_err(|e| format!("Failed to create HTTP client: {}", e))?;

        let signer = X402Signer::new(private_key)?;

        log::info!("[X402] Initialized with wallet address: {}", signer.address());

        Ok(Self {
            client,
            signer: Arc::new(signer),
        })
    }

    /// Get the wallet address
    pub fn wallet_address(&self) -> String {
        self.signer.address()
    }

    /// Make a POST request with automatic x402 payment handling
    pub async fn post_with_payment<T: Serialize>(
        &self,
        url: &str,
        body: &T,
    ) -> Result<Response, String> {
        log::info!("[X402] Making request to {}", url);

        // First request without payment
        let initial_response = self.client
            .post(url)
            .header(header::CONTENT_TYPE, "application/json")
            .json(body)
            .send()
            .await
            .map_err(|e| format!("Request failed: {}", e))?;

        // Check if payment is required
        if initial_response.status().as_u16() != 402 {
            log::info!("[X402] No payment required, status: {}", initial_response.status());
            return Ok(initial_response);
        }

        log::info!("[X402] Received 402 Payment Required");

        // Get payment requirements from header
        let payment_header = initial_response
            .headers()
            .get("payment-required")
            .or_else(|| initial_response.headers().get("PAYMENT-REQUIRED"))
            .ok_or_else(|| "402 response missing payment-required header".to_string())?
            .to_str()
            .map_err(|e| format!("Invalid payment-required header: {}", e))?;

        let payment_required = PaymentRequired::from_base64(payment_header)?;

        log::info!(
            "[X402] Payment requirements: {} {} to {}",
            payment_required.accepts.first().map(|a| a.max_amount_required.as_str()).unwrap_or("?"),
            payment_required.accepts.first().map(|a| a.asset.as_str()).unwrap_or("?"),
            payment_required.accepts.first().map(|a| a.pay_to_address.as_str()).unwrap_or("?")
        );

        // Get the first (and typically only) payment option
        let requirements = payment_required.accepts.first()
            .ok_or_else(|| "No payment options in 402 response".to_string())?;

        // Sign the payment
        let payment_payload = self.signer.sign_payment(requirements).await?;
        let payment_header_value = payment_payload.to_base64()?;

        log::info!("[X402] Signed payment, retrying request with X-PAYMENT header");

        // Retry with payment
        let paid_response = self.client
            .post(url)
            .header(header::CONTENT_TYPE, "application/json")
            .header("X-PAYMENT", payment_header_value)
            .json(body)
            .send()
            .await
            .map_err(|e| format!("Paid request failed: {}", e))?;

        log::info!("[X402] Payment sent, response status: {}", paid_response.status());

        Ok(paid_response)
    }
}

/// Check if a URL is a defirelay endpoint that uses x402
pub fn is_x402_endpoint(url: &str) -> bool {
    url.contains("defirelay.com") || url.contains("defirelay.io")
}
