//! EIP-8004 Trustless Agents implementation
//!
//! This module provides integration with EIP-8004 registries:
//! - Identity Registry: ERC-721 agent handles for discovery
//! - Reputation Registry: On-chain feedback with payment proofs
//! - Validation Registry: Independent work verification
//!
//! Combined with x402 payments, this enables trustless agent economies.

pub mod types;
pub mod abi;
pub mod identity;
pub mod reputation;
pub mod discovery;
pub mod config;

pub use types::*;
pub use config::Eip8004Config;
pub use identity::IdentityRegistry;
pub use reputation::ReputationRegistry;
pub use discovery::AgentDiscovery;
