//! DomainEthAddress - Wrapper type for ethers Address with serde support
//!
//! Provides consistent serialization/deserialization for Ethereum addresses.

use ethers::types::Address;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct DomainEthAddress(pub Address);

impl DomainEthAddress {
    /// Returns the full hex string representation with 0x prefix
    pub fn to_string_full(&self) -> String {
        format!("{:?}", self.0)
    }
}

impl From<Address> for DomainEthAddress {
    fn from(input: Address) -> Self {
        Self(input)
    }
}

impl From<DomainEthAddress> for Address {
    fn from(input: DomainEthAddress) -> Self {
        input.0
    }
}

impl std::ops::Deref for DomainEthAddress {
    type Target = Address;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Display for DomainEthAddress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl Serialize for DomainEthAddress {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize Address as a checksummed hex string
        serializer.serialize_str(&format!("{:?}", self.0))
    }
}

impl<'de> Deserialize<'de> for DomainEthAddress {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Address::from_str(&s)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_ADDRESS: &str = "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913";

    #[test]
    fn test_deserialize_from_string() {
        let json = format!(r#""{}""#, TEST_ADDRESS);
        let result: DomainEthAddress = serde_json::from_str(&json).unwrap();
        assert_eq!(result.to_string_full().to_lowercase(), TEST_ADDRESS.to_lowercase());
    }

    #[test]
    fn test_deserialize_lowercase() {
        let json = format!(r#""{}""#, TEST_ADDRESS.to_lowercase());
        let result: DomainEthAddress = serde_json::from_str(&json).unwrap();
        assert_eq!(result.0, Address::from_str(TEST_ADDRESS).unwrap());
    }

    #[test]
    fn test_serialize() {
        let addr = DomainEthAddress(Address::from_str(TEST_ADDRESS).unwrap());
        let json = serde_json::to_string(&addr).unwrap();
        // Should serialize with checksum
        assert!(json.contains("0x"));
    }

    #[test]
    fn test_from_address() {
        let addr = Address::from_str(TEST_ADDRESS).unwrap();
        let domain: DomainEthAddress = addr.into();
        assert_eq!(domain.0, addr);
    }

    #[test]
    fn test_into_address() {
        let domain = DomainEthAddress(Address::from_str(TEST_ADDRESS).unwrap());
        let addr: Address = domain.into();
        assert_eq!(addr, Address::from_str(TEST_ADDRESS).unwrap());
    }

    #[test]
    fn test_deref() {
        let domain = DomainEthAddress(Address::from_str(TEST_ADDRESS).unwrap());
        // Can use Address methods directly via Deref
        assert!(!domain.is_zero());
    }

    #[test]
    fn test_display() {
        let domain = DomainEthAddress(Address::from_str(TEST_ADDRESS).unwrap());
        let display = format!("{}", domain);
        assert!(display.starts_with("0x"));
    }

    #[test]
    fn test_invalid_address() {
        let json = r#""0xinvalid""#;
        let result: Result<DomainEthAddress, _> = serde_json::from_str(json);
        assert!(result.is_err());
    }

    #[test]
    fn test_zero_address() {
        let json = r#""0x0000000000000000000000000000000000000000""#;
        let result: DomainEthAddress = serde_json::from_str(json).unwrap();
        assert!(result.is_zero());
    }
}
