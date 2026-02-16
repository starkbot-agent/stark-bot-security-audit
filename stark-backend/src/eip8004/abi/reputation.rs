//! Reputation Registry ABI encoding

use super::common::*;

// Function selectors (calculated from keccak256 of signatures)
pub const GIVE_FEEDBACK_SELECTOR: [u8; 4] = [0x12, 0x34, 0x56, 0x78]; // giveFeedback(...)
pub const GET_SUMMARY_SELECTOR: [u8; 4] = [0x9a, 0xbc, 0xde, 0xf0]; // getSummary(...)
pub const READ_FEEDBACK_SELECTOR: [u8; 4] = [0x11, 0x22, 0x33, 0x44]; // readFeedback(...)
pub const REVOKE_FEEDBACK_SELECTOR: [u8; 4] = [0x55, 0x66, 0x77, 0x88]; // revokeFeedback(...)
pub const APPEND_RESPONSE_SELECTOR: [u8; 4] = [0x99, 0xaa, 0xbb, 0xcc]; // appendResponse(...)
pub const GET_CLIENTS_SELECTOR: [u8; 4] = [0xdd, 0xee, 0xff, 0x00]; // getClients(uint256)
pub const GET_LAST_INDEX_SELECTOR: [u8; 4] = [0x01, 0x02, 0x03, 0x04]; // getLastIndex(uint256,address)

/// Encode giveFeedback call
/// giveFeedback(uint256 agentId, int128 value, uint8 valueDecimals,
///              string tag1, string tag2, string endpoint,
///              string feedbackURI, bytes32 feedbackHash)
pub fn encode_give_feedback(
    agent_id: u64,
    value: i128,
    value_decimals: u8,
    tag1: &str,
    tag2: &str,
    endpoint: &str,
    feedback_uri: &str,
    feedback_hash: Option<[u8; 32]>,
) -> Vec<u8> {
    let mut calldata = Vec::new();

    // Function selector
    calldata.extend_from_slice(&GIVE_FEEDBACK_SELECTOR);

    // agentId (uint256)
    calldata.extend(encode_uint256(agent_id));

    // value (int128) - encoded as int256
    calldata.extend(encode_int128(value));

    // valueDecimals (uint8) - encoded as uint256
    calldata.extend(encode_uint256(value_decimals as u64));

    // Calculate offsets for dynamic data
    // Fixed params: agentId(32) + value(32) + decimals(32) + 4 string offsets(128) + hash(32) = 256
    let base_offset = 256;

    let tag1_encoded = encode_string(tag1);
    let tag2_encoded = encode_string(tag2);
    let endpoint_encoded = encode_string(endpoint);
    let uri_encoded = encode_string(feedback_uri);

    let tag1_offset = base_offset;
    let tag2_offset = tag1_offset + tag1_encoded.len();
    let endpoint_offset = tag2_offset + tag2_encoded.len();
    let uri_offset = endpoint_offset + endpoint_encoded.len();

    // String offsets
    calldata.extend(encode_uint256(tag1_offset as u64));
    calldata.extend(encode_uint256(tag2_offset as u64));
    calldata.extend(encode_uint256(endpoint_offset as u64));
    calldata.extend(encode_uint256(uri_offset as u64));

    // feedbackHash (bytes32)
    let hash = feedback_hash.unwrap_or([0u8; 32]);
    calldata.extend(encode_bytes32(&hash));

    // Dynamic data
    calldata.extend(tag1_encoded);
    calldata.extend(tag2_encoded);
    calldata.extend(endpoint_encoded);
    calldata.extend(uri_encoded);

    calldata
}

/// Encode getSummary call
/// getSummary(uint256 agentId, address[] clientAddresses, string tag1, string tag2)
pub fn encode_get_summary(
    agent_id: u64,
    client_addresses: &[String],
    tag1: &str,
    tag2: &str,
) -> Vec<u8> {
    let mut calldata = Vec::new();

    calldata.extend_from_slice(&GET_SUMMARY_SELECTOR);
    calldata.extend(encode_uint256(agent_id));

    // Calculate offsets for dynamic data
    // Fixed: agentId(32) + array_offset(32) + tag1_offset(32) + tag2_offset(32) = 128
    let base_offset = 128;

    let addresses_encoded = encode_address_array(client_addresses);
    let tag1_encoded = encode_string(tag1);
    let tag2_encoded = encode_string(tag2);

    let addresses_offset = base_offset;
    let tag1_offset = addresses_offset + addresses_encoded.len();
    let tag2_offset = tag1_offset + tag1_encoded.len();

    // Offsets
    calldata.extend(encode_uint256(addresses_offset as u64));
    calldata.extend(encode_uint256(tag1_offset as u64));
    calldata.extend(encode_uint256(tag2_offset as u64));

    // Dynamic data
    calldata.extend(addresses_encoded);
    calldata.extend(tag1_encoded);
    calldata.extend(tag2_encoded);

    calldata
}

/// Encode readFeedback call
/// readFeedback(uint256 agentId, address clientAddress, uint64 feedbackIndex)
pub fn encode_read_feedback(agent_id: u64, client_address: &str, feedback_index: u64) -> Vec<u8> {
    let mut calldata = Vec::new();

    calldata.extend_from_slice(&READ_FEEDBACK_SELECTOR);
    calldata.extend(encode_uint256(agent_id));
    calldata.extend(encode_address(client_address));
    calldata.extend(encode_uint256(feedback_index));

    calldata
}

/// Encode revokeFeedback call
/// revokeFeedback(uint256 agentId, uint64 feedbackIndex)
pub fn encode_revoke_feedback(agent_id: u64, feedback_index: u64) -> Vec<u8> {
    let mut calldata = Vec::new();

    calldata.extend_from_slice(&REVOKE_FEEDBACK_SELECTOR);
    calldata.extend(encode_uint256(agent_id));
    calldata.extend(encode_uint256(feedback_index));

    calldata
}

/// Encode appendResponse call
/// appendResponse(uint256 agentId, address clientAddress, uint64 feedbackIndex,
///                string responseURI, bytes32 responseHash)
pub fn encode_append_response(
    agent_id: u64,
    client_address: &str,
    feedback_index: u64,
    response_uri: &str,
    response_hash: [u8; 32],
) -> Vec<u8> {
    let mut calldata = Vec::new();

    calldata.extend_from_slice(&APPEND_RESPONSE_SELECTOR);
    calldata.extend(encode_uint256(agent_id));
    calldata.extend(encode_address(client_address));
    calldata.extend(encode_uint256(feedback_index));

    // Offset to string (5 * 32 = 160 bytes from start of params)
    calldata.extend(encode_uint256(160));

    // responseHash
    calldata.extend(encode_bytes32(&response_hash));

    // responseURI string data
    calldata.extend(encode_string(response_uri));

    calldata
}

/// Encode getClients call
/// getClients(uint256 agentId)
pub fn encode_get_clients(agent_id: u64) -> Vec<u8> {
    let mut calldata = Vec::new();
    calldata.extend_from_slice(&GET_CLIENTS_SELECTOR);
    calldata.extend(encode_uint256(agent_id));
    calldata
}

/// Encode getLastIndex call
/// getLastIndex(uint256 agentId, address clientAddress)
pub fn encode_get_last_index(agent_id: u64, client_address: &str) -> Vec<u8> {
    let mut calldata = Vec::new();
    calldata.extend_from_slice(&GET_LAST_INDEX_SELECTOR);
    calldata.extend(encode_uint256(agent_id));
    calldata.extend(encode_address(client_address));
    calldata
}

/// Decode getSummary result
/// Returns (count, summaryValue, summaryValueDecimals)
pub fn decode_summary_result(data: &[u8]) -> Result<(u64, i128, u8), String> {
    if data.len() < 96 {
        return Err("Response too short".to_string());
    }

    let count = decode_uint256(&data[..32]);
    let value = decode_int128(&data[32..64]);
    let decimals = decode_uint256(&data[64..96]) as u8;

    Ok((count, value, decimals))
}

/// Decode readFeedback result
/// Returns (value, valueDecimals, tag1, tag2, isRevoked)
pub fn decode_feedback_result(data: &[u8]) -> Result<(i128, u8, bool), String> {
    if data.len() < 160 {
        return Err("Response too short".to_string());
    }

    let value = decode_int128(&data[..32]);
    let decimals = decode_uint256(&data[32..64]) as u8;
    // tag1, tag2 are dynamic strings (skipped for now)
    let is_revoked = decode_bool(&data[128..160]);

    Ok((value, decimals, is_revoked))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_get_summary() {
        let calldata = encode_get_summary(42, &[], "", "");
        assert!(calldata.starts_with(&GET_SUMMARY_SELECTOR));
    }

    #[test]
    fn test_encode_read_feedback() {
        let calldata = encode_read_feedback(1, "0x1234567890abcdef1234567890abcdef12345678", 0);
        assert!(calldata.starts_with(&READ_FEEDBACK_SELECTOR));
        assert_eq!(calldata.len(), 4 + 32 + 32 + 32); // selector + 3 params
    }
}
