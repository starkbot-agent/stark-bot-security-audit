//! Discord user profile operations — delegates to the discord-tipping-service via RPC.
//!
//! All functions are async and call the standalone service directly.

use crate::integrations::discord_tipping_client::DiscordTippingClient;
pub use discord_tipping_types::DiscordUserProfile;

fn make_client() -> DiscordTippingClient {
    let url = std::env::var("DISCORD_TIPPING_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:9101".to_string());
    DiscordTippingClient::new(&url)
}

/// Initialize the discord_user_profiles table.
/// Now a no-op — the service manages its own schema.
pub fn init_tables(_conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    Ok(())
}

/// Get or create a Discord user profile
pub async fn get_or_create_profile(
    _db: &crate::db::Database,
    discord_user_id: &str,
    username: &str,
) -> Result<DiscordUserProfile, String> {
    let client = make_client();
    client.get_or_create_profile(discord_user_id, username).await
}

/// Get a Discord user profile by user ID
pub async fn get_profile(
    _db: &crate::db::Database,
    discord_user_id: &str,
) -> Result<Option<DiscordUserProfile>, String> {
    let client = make_client();
    client.get_profile(discord_user_id).await
}

/// Get a Discord user profile by public address
pub async fn get_profile_by_address(
    _db: &crate::db::Database,
    address: &str,
) -> Result<Option<DiscordUserProfile>, String> {
    let client = make_client();
    client.get_profile_by_address(address).await
}

/// Register a public address for a Discord user
pub async fn register_address(
    _db: &crate::db::Database,
    discord_user_id: &str,
    address: &str,
) -> Result<(), String> {
    let client = make_client();
    client.register_address(discord_user_id, address).await
}

/// Unregister a public address for a Discord user
pub async fn unregister_address(
    _db: &crate::db::Database,
    discord_user_id: &str,
) -> Result<(), String> {
    let client = make_client();
    client.unregister_address(discord_user_id).await
}

/// List all registered profiles (those with a public address)
pub async fn list_registered_profiles(
    _db: &crate::db::Database,
) -> Result<Vec<DiscordUserProfile>, String> {
    let client = make_client();
    client.list_registered_profiles().await
}

/// List all profiles (registered and unregistered) — used for module dashboard
pub async fn list_all_profiles(
    _db: &crate::db::Database,
) -> Result<Vec<DiscordUserProfile>, String> {
    let client = make_client();
    client.list_all_profiles().await
}

/// Clear all discord user registrations (for restore)
pub fn clear_registrations_for_restore(_db: &crate::db::Database) -> Result<usize, String> {
    // This is handled by the backup/restore endpoint now
    Ok(0)
}

#[cfg(test)]
mod tests {
    fn is_valid_address(addr: &str) -> bool {
        addr.starts_with("0x")
            && addr.len() >= 42
            && addr.len() <= 66
            && addr[2..].chars().all(|c| c.is_ascii_hexdigit())
    }

    #[test]
    fn test_address_validation() {
        assert!(is_valid_address("0x1234567890123456789012345678901234567890"));
        assert!(is_valid_address(
            "0x0123456789012345678901234567890123456789012345678901234567890123"
        ));
        assert!(!is_valid_address("0x123"));
        assert!(!is_valid_address("1234567890123456789012345678901234567890"));
        assert!(!is_valid_address("0xGGGG567890123456789012345678901234567890"));
    }
}
