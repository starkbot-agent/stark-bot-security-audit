//! Common ABI encoding utilities

use ethers::utils::keccak256 as ethers_keccak256;

/// Compute function selector (first 4 bytes of keccak256 hash)
pub fn function_selector(signature: &str) -> [u8; 4] {
    let hash = ethers_keccak256(signature.as_bytes());
    let mut selector = [0u8; 4];
    selector.copy_from_slice(&hash[..4]);
    selector
}

/// Encode a uint256 value as 32 bytes
pub fn encode_uint256(value: u64) -> Vec<u8> {
    let mut encoded = vec![0u8; 32];
    let bytes = value.to_be_bytes();
    encoded[24..32].copy_from_slice(&bytes);
    encoded
}

/// Encode a signed int128 value as 32 bytes
pub fn encode_int128(value: i128) -> Vec<u8> {
    let mut encoded = vec![0u8; 32];
    let bytes = value.to_be_bytes();
    // For negative numbers, fill with 0xFF (sign extension)
    if value < 0 {
        encoded[..16].fill(0xFF);
    }
    encoded[16..32].copy_from_slice(&bytes);
    encoded
}

/// Encode an address (20 bytes) as 32 bytes (left-padded)
pub fn encode_address(address: &str) -> Vec<u8> {
    let addr = address.trim_start_matches("0x");
    let addr_bytes = hex::decode(addr).unwrap_or_else(|_| vec![0u8; 20]);

    let mut encoded = vec![0u8; 32];
    let start = 32 - addr_bytes.len().min(20);
    encoded[start..].copy_from_slice(&addr_bytes[..addr_bytes.len().min(20)]);
    encoded
}

/// Encode a bytes32 value
pub fn encode_bytes32(data: &[u8; 32]) -> Vec<u8> {
    data.to_vec()
}

/// Encode a dynamic string
pub fn encode_string(s: &str) -> Vec<u8> {
    let bytes = s.as_bytes();
    let len = bytes.len();

    // Calculate padding to 32 bytes
    let padded_len = ((len + 31) / 32) * 32;

    let mut encoded = Vec::new();

    // Length as uint256
    encoded.extend(encode_uint256(len as u64));

    // String data, padded to 32-byte boundary
    encoded.extend_from_slice(bytes);
    encoded.resize(encoded.len() + (padded_len - len), 0);

    encoded
}

/// Encode a dynamic array of addresses
pub fn encode_address_array(addresses: &[String]) -> Vec<u8> {
    let mut encoded = Vec::new();

    // Array length
    encoded.extend(encode_uint256(addresses.len() as u64));

    // Each address
    for addr in addresses {
        encoded.extend(encode_address(addr));
    }

    encoded
}

/// Decode a uint256 from 32 bytes
pub fn decode_uint256(data: &[u8]) -> u64 {
    if data.len() < 32 {
        return 0;
    }
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&data[24..32]);
    u64::from_be_bytes(bytes)
}

/// Decode an int128 from 32 bytes
pub fn decode_int128(data: &[u8]) -> i128 {
    if data.len() < 32 {
        return 0;
    }
    let mut bytes = [0u8; 16];
    bytes.copy_from_slice(&data[16..32]);
    i128::from_be_bytes(bytes)
}

/// Decode an address from 32 bytes
pub fn decode_address(data: &[u8]) -> String {
    if data.len() < 32 {
        return "0x0000000000000000000000000000000000000000".to_string();
    }
    format!("0x{}", hex::encode(&data[12..32]))
}

/// Decode a string from ABI encoded data
pub fn decode_string(data: &[u8], offset: usize) -> Option<String> {
    if data.len() < offset + 32 {
        return None;
    }

    // Get string length from offset position
    let len = decode_uint256(&data[offset..]) as usize;

    if data.len() < offset + 32 + len {
        return None;
    }

    // Extract string bytes
    let string_bytes = &data[offset + 32..offset + 32 + len];
    String::from_utf8(string_bytes.to_vec()).ok()
}

/// Decode a bool from 32 bytes
pub fn decode_bool(data: &[u8]) -> bool {
    if data.len() < 32 {
        return false;
    }
    data[31] != 0
}

/// Compute keccak256 hash
pub fn keccak256(data: &[u8]) -> [u8; 32] {
    ethers_keccak256(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_function_selector() {
        // "register(string)" should produce the correct selector
        let selector = function_selector("register(string)");
        assert_eq!(selector.len(), 4);
    }

    #[test]
    fn test_encode_uint256() {
        let encoded = encode_uint256(42);
        assert_eq!(encoded.len(), 32);
        assert_eq!(encoded[31], 42);
        assert_eq!(decode_uint256(&encoded), 42);
    }

    #[test]
    fn test_encode_address() {
        let addr = "0x1234567890AbCdEf1234567890aBcDeF12345678";
        let encoded = encode_address(addr);
        assert_eq!(encoded.len(), 32);
        let decoded = decode_address(&encoded);
        assert_eq!(decoded.to_lowercase(), addr.to_lowercase());
    }

    #[test]
    fn test_encode_string() {
        let s = "hello";
        let encoded = encode_string(s);
        // Length (32) + padded data (32) = 64 bytes
        assert!(encoded.len() >= 64);
        assert_eq!(decode_uint256(&encoded[..32]), 5);
    }
}
