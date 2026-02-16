//! DomainUint256 - Wrapper type for ethers U256 with proper serde support
//!
//! Handles deserialization from:
//! - Decimal strings: "331157" -> U256(331157)
//! - Hex strings with 0x prefix: "0x50dc5" -> U256(331157)
//! - Integers: 331157 -> U256(331157)
//!
//! This is critical because ethers U256::parse() treats all strings as hex,
//! causing "331157" to be parsed as 0x331157 = 3346775 instead of 331157.

use ethers::types::U256;
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct DomainUint256(pub U256);

impl Serialize for DomainUint256 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize U256 as a decimal string
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for DomainUint256 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct DomainUint256Visitor;

        impl<'de> Visitor<'de> for DomainUint256Visitor {
            type Value = DomainUint256;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter
                    .write_str("a string representing a U256 value in decimal or hexadecimal format")
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                let cleaned_value = value.trim().trim_matches('"');

                if cleaned_value.starts_with("0x") || cleaned_value.starts_with("0X") {
                    // For hex values, use from_str which handles 0x prefix
                    // Normalize to lowercase as U256::from_str only handles lowercase 0x
                    let normalized = if cleaned_value.starts_with("0X") {
                        format!("0x{}", &cleaned_value[2..])
                    } else {
                        cleaned_value.to_string()
                    };
                    match U256::from_str(&normalized) {
                        Ok(u) => Ok(DomainUint256(u)),
                        Err(e) => Err(de::Error::custom(format!(
                            "Failed to parse hex: {} for value {}",
                            e, cleaned_value
                        ))),
                    }
                } else {
                    // Try to parse as decimal first (THIS IS THE KEY FIX!)
                    // U256::from_dec_str() correctly treats the string as decimal
                    match U256::from_dec_str(cleaned_value) {
                        Ok(u) => Ok(DomainUint256(u)),
                        Err(_) => {
                            // Last attempt: maybe it's hex without 0x prefix
                            let with_prefix = format!("0x{}", cleaned_value);
                            U256::from_str(&with_prefix).map(DomainUint256).map_err(|e| {
                                de::Error::custom(format!(
                                    "Failed to parse as decimal or hex: {} for value {}",
                                    e, cleaned_value
                                ))
                            })
                        }
                    }
                }
            }

            fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                self.visit_str(&value)
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(DomainUint256(U256::from(value)))
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value < 0 {
                    Err(de::Error::custom(
                        "negative value cannot be converted to U256",
                    ))
                } else {
                    Ok(DomainUint256(U256::from(value as u64)))
                }
            }
        }

        deserializer.deserialize_any(DomainUint256Visitor)
    }
}

impl From<U256> for DomainUint256 {
    fn from(input: U256) -> Self {
        Self(input)
    }
}

impl From<DomainUint256> for U256 {
    fn from(input: DomainUint256) -> Self {
        input.0
    }
}

impl std::ops::Deref for DomainUint256 {
    type Target = U256;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deserialize_decimal_string() {
        let json = r#""331157""#;
        let result: DomainUint256 = serde_json::from_str(json).unwrap();
        assert_eq!(result.0, U256::from(331157u64));
    }

    #[test]
    fn test_deserialize_hex_string() {
        // 0x50d95 = 331157 decimal
        let json = r#""0x50d95""#;
        let result: DomainUint256 = serde_json::from_str(json).unwrap();
        assert_eq!(result.0, U256::from(331157u64));
    }

    #[test]
    fn test_deserialize_hex_uppercase() {
        // 0X50D95 = 331157 decimal (uppercase X should be normalized)
        let json = r#""0X50D95""#;
        let result: DomainUint256 = serde_json::from_str(json).unwrap();
        assert_eq!(result.0, U256::from(331157u64));
    }

    #[test]
    fn test_deserialize_integer() {
        let json = r#"331157"#;
        let result: DomainUint256 = serde_json::from_str(json).unwrap();
        assert_eq!(result.0, U256::from(331157u64));
    }

    #[test]
    fn test_deserialize_large_decimal() {
        // 1 gwei in wei
        let json = r#""1000000000""#;
        let result: DomainUint256 = serde_json::from_str(json).unwrap();
        assert_eq!(result.0, U256::from(1_000_000_000u64));
    }

    #[test]
    fn test_deserialize_gas_price_hex() {
        // Typical Base gas price: 0xf4240 = 1000000 wei
        let json = r#""0xf4240""#;
        let result: DomainUint256 = serde_json::from_str(json).unwrap();
        assert_eq!(result.0, U256::from(1_000_000u64));
    }

    #[test]
    fn test_serialize_to_decimal() {
        let value = DomainUint256(U256::from(331157u64));
        let json = serde_json::to_string(&value).unwrap();
        assert_eq!(json, r#""331157""#);
    }

    #[test]
    fn test_from_u256() {
        let u256 = U256::from(12345u64);
        let domain: DomainUint256 = u256.into();
        assert_eq!(domain.0, u256);
    }

    #[test]
    fn test_into_u256() {
        let domain = DomainUint256(U256::from(12345u64));
        let u256: U256 = domain.into();
        assert_eq!(u256, U256::from(12345u64));
    }

    #[test]
    fn test_deref() {
        let domain = DomainUint256(U256::from(12345u64));
        // Can use U256 methods directly via Deref
        assert!(!domain.is_zero());
    }
}
