//! x402 Protocol implementation for pay-per-use AI endpoints
//!
//! This module handles the x402 payment protocol flow:
//! 1. Make initial request
//! 2. If 402 returned, parse payment requirements
//! 3. Sign EIP-3009 authorization with burner wallet
//! 4. Retry with X-PAYMENT header

mod types;
mod client;
mod signer;

pub use types::*;
pub use client::{X402Client, is_x402_endpoint};
pub use signer::X402Signer;
