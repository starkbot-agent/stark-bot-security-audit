//! Force register command - allows admins to register a wallet address for another Discord user

use crate::db::Database;
use crate::discord_hooks::db;

/// Extract a Discord user ID from a mention string like `<@123456>` or `<@!123456>`
fn extract_user_id(mention: &str) -> Option<String> {
    let trimmed = mention
        .strip_prefix("<@")
        .and_then(|s| s.strip_suffix('>'))?;
    // Handle nickname mentions: <@!ID>
    let id = trimmed.strip_prefix('!').unwrap_or(trimmed);
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(id.to_string())
}

/// Validate an Ethereum/Starknet address format
fn is_valid_address(addr: &str) -> bool {
    addr.starts_with("0x")
        && addr.len() >= 42
        && addr.len() <= 66
        && addr[2..].chars().all(|c| c.is_ascii_hexdigit())
}

/// Parse a force_register command, returning (mention, address) if valid
pub fn parse(text: &str) -> Option<(String, String)> {
    // Expected: "force_register <@USER_ID> 0xADDRESS"
    let parts: Vec<&str> = text.split_whitespace().collect();
    if parts.len() != 3 {
        return None;
    }
    let mention = parts[1];
    let address = parts[2];
    let user_id = extract_user_id(mention)?;
    if !is_valid_address(address) {
        return None;
    }
    Some((user_id, address.to_string()))
}

/// Execute the force_register command (admin only)
pub async fn execute(
    target_user_id: &str,
    address: &str,
    admin_user_id: &str,
    db: &Database,
) -> Result<String, String> {
    // Ensure target user profile exists
    // We don't know their username yet, so use a placeholder — it'll get
    // updated the next time they interact with the bot.
    if let Err(e) = db::get_or_create_profile(db, target_user_id, "unknown") {
        return Err(format!("Failed to create profile for user: {}", e));
    }

    // Check if address is already registered to a different user
    if let Some(existing) = db::get_profile_by_address(db, address)? {
        if existing.discord_user_id == target_user_id {
            return Ok(format!(
                "<@{}> already has this address registered: `{}`",
                target_user_id, address
            ));
        }
        return Ok(format!(
            "This address is already registered to <@{}>. \
            Each address can only be registered once.",
            existing.discord_user_id
        ));
    }

    // Register the address
    db::register_address(db, target_user_id, address)?;

    log::info!(
        "Discord hooks: Admin {} force-registered address {} for user {}",
        admin_user_id,
        address,
        target_user_id
    );

    Ok(format!(
        "Successfully registered <@{}> with address `{}`",
        target_user_id, address
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_user_id_standard() {
        assert_eq!(
            extract_user_id("<@123456789>"),
            Some("123456789".to_string())
        );
    }

    #[test]
    fn test_extract_user_id_nickname() {
        assert_eq!(
            extract_user_id("<@!123456789>"),
            Some("123456789".to_string())
        );
    }

    #[test]
    fn test_extract_user_id_invalid() {
        assert_eq!(extract_user_id("not_a_mention"), None);
        assert_eq!(extract_user_id("<@>"), None);
        assert_eq!(extract_user_id("<@!>"), None);
        assert_eq!(extract_user_id("<@abc>"), None);
    }

    #[test]
    fn test_parse_valid() {
        let result = parse("force_register <@123456789> 0x1234567890123456789012345678901234567890");
        assert!(result.is_some());
        let (user_id, address) = result.unwrap();
        assert_eq!(user_id, "123456789");
        assert_eq!(
            address,
            "0x1234567890123456789012345678901234567890"
        );
    }

    #[test]
    fn test_parse_nickname_mention() {
        let result =
            parse("force_register <@!987654321> 0xAbCdEf7890123456789012345678901234567890");
        assert!(result.is_some());
        let (user_id, _) = result.unwrap();
        assert_eq!(user_id, "987654321");
    }

    #[test]
    fn test_parse_missing_address() {
        assert!(parse("force_register <@123456789>").is_none());
    }

    #[test]
    fn test_parse_invalid_address() {
        assert!(parse("force_register <@123456789> not_an_address").is_none());
    }

    #[test]
    fn test_parse_too_many_args() {
        assert!(parse("force_register <@123> 0x1234567890123456789012345678901234567890 extra").is_none());
    }

    #[test]
    fn test_parse_no_mention() {
        assert!(parse("force_register someone 0x1234567890123456789012345678901234567890").is_none());
    }
}
