//! Register command - allows users to register their public address

use crate::db::Database;
use crate::discord_hooks::db;

/// Validate an Ethereum/Starknet address format
fn is_valid_address(addr: &str) -> bool {
    // Must start with 0x
    if !addr.starts_with("0x") {
        return false;
    }

    // Must be at least 42 chars (Ethereum) and at most 66 chars (Starknet)
    if addr.len() < 42 || addr.len() > 66 {
        return false;
    }

    // All characters after 0x must be hex digits
    addr[2..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Execute the register command
pub async fn execute(user_id: &str, address: &str, database: &Database) -> Result<String, String> {
    // Validate address format
    if !is_valid_address(address) {
        return Ok(
            "Invalid address format. Please provide a valid Ethereum or Starknet address \
            starting with `0x`.\n\n\
            Example: `@starkbot register 0x1234...abcd`"
                .to_string(),
        );
    }

    // Check if address is already registered to someone else
    if let Some(existing) = db::get_profile_by_address(database, address)? {
        if existing.discord_user_id != user_id {
            return Ok(
                "This address is already registered to another Discord user. \
                Each address can only be registered once."
                    .to_string(),
            );
        }
        // Already registered to this user
        return Ok(format!(
            "You already have this address registered: `{}`",
            address
        ));
    }

    // Register the address
    db::register_address(database, user_id, address)?;

    Ok(format!(
        "Successfully registered your address: `{}`\n\n\
        You can receive tips. 🚀",
        address
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_eth_address() {
        // Standard Ethereum address (40 hex chars + 0x = 42 total)
        assert!(is_valid_address(
            "0x1234567890123456789012345678901234567890"
        ));
    }

    #[test]
    fn test_valid_starknet_address() {
        // Starknet address (64 hex chars + 0x = 66 total)
        assert!(is_valid_address(
            "0x0123456789012345678901234567890123456789012345678901234567890123"
        ));
    }

    #[test]
    fn test_invalid_no_prefix() {
        assert!(!is_valid_address(
            "1234567890123456789012345678901234567890"
        ));
    }

    #[test]
    fn test_invalid_too_short() {
        assert!(!is_valid_address("0x123"));
    }

    #[test]
    fn test_invalid_too_long() {
        // 68 chars total (too long)
        assert!(!is_valid_address(
            "0x01234567890123456789012345678901234567890123456789012345678901234567"
        ));
    }

    #[test]
    fn test_invalid_non_hex() {
        assert!(!is_valid_address(
            "0xGGGG567890123456789012345678901234567890"
        ));
    }

    #[test]
    fn test_case_insensitive_hex() {
        assert!(is_valid_address(
            "0xAbCdEf7890123456789012345678901234567890"
        ));
    }
}
