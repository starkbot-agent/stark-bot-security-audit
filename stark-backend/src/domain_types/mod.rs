//! Domain types for stark-backend
//!
//! These types provide proper serialization/deserialization for blockchain values.
//! Adapted from teller-pools-bot-rs, without postgres dependencies.

pub mod eth_address;
pub mod uint256;

pub use eth_address::DomainEthAddress;
pub use uint256::DomainUint256;
