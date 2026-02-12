//! Database operations for Discord user profiles

use crate::db::Database;
use rusqlite::params;

/// Discord user profile with optional public address registration
#[derive(Debug, Clone)]
pub struct DiscordUserProfile {
    pub id: i64,
    pub discord_user_id: String,
    pub discord_username: Option<String>,
    pub public_address: Option<String>,
    pub registration_status: String,
    pub registered_at: Option<String>,
    pub last_interaction_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

/// Initialize the discord_user_profiles table
pub fn init_tables(conn: &rusqlite::Connection) -> Result<(), rusqlite::Error> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS discord_user_profiles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            discord_user_id TEXT NOT NULL UNIQUE,
            discord_username TEXT,
            public_address TEXT,
            registration_status TEXT NOT NULL DEFAULT 'unregistered',
            registered_at TEXT,
            last_interaction_at TEXT,
            created_at TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at TEXT NOT NULL DEFAULT (datetime('now'))
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_discord_profiles_address
         ON discord_user_profiles(public_address)",
        [],
    )?;

    log::info!("Discord hooks: Initialized discord_user_profiles table");
    Ok(())
}

/// Get or create a Discord user profile
pub fn get_or_create_profile(
    db: &Database,
    discord_user_id: &str,
    username: &str,
) -> Result<DiscordUserProfile, String> {
    let conn = db.conn();

    // Try to insert (ignore if exists)
    conn.execute(
        "INSERT OR IGNORE INTO discord_user_profiles (discord_user_id, discord_username)
         VALUES (?1, ?2)",
        params![discord_user_id, username],
    )
    .map_err(|e| format!("Failed to insert profile: {}", e))?;

    // Update last interaction and username
    conn.execute(
        "UPDATE discord_user_profiles
         SET last_interaction_at = datetime('now'),
             discord_username = ?2,
             updated_at = datetime('now')
         WHERE discord_user_id = ?1",
        params![discord_user_id, username],
    )
    .map_err(|e| format!("Failed to update profile: {}", e))?;

    // Fetch the profile
    get_profile_impl(&conn, discord_user_id)
}

/// Get a Discord user profile by user ID
pub fn get_profile(db: &Database, discord_user_id: &str) -> Result<Option<DiscordUserProfile>, String> {
    let conn = db.conn();
    match get_profile_impl(&conn, discord_user_id) {
        Ok(p) => Ok(Some(p)),
        Err(e) if e.contains("not found") => Ok(None),
        Err(e) => Err(e),
    }
}

fn get_profile_impl(
    conn: &rusqlite::Connection,
    discord_user_id: &str,
) -> Result<DiscordUserProfile, String> {
    conn.query_row(
        "SELECT id, discord_user_id, discord_username, public_address,
                registration_status, registered_at, last_interaction_at,
                created_at, updated_at
         FROM discord_user_profiles
         WHERE discord_user_id = ?1",
        params![discord_user_id],
        |row| {
            Ok(DiscordUserProfile {
                id: row.get(0)?,
                discord_user_id: row.get(1)?,
                discord_username: row.get(2)?,
                public_address: row.get(3)?,
                registration_status: row.get(4)?,
                registered_at: row.get(5)?,
                last_interaction_at: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        },
    )
    .map_err(|e| format!("Profile not found: {}", e))
}

/// Get a Discord user profile by public address
pub fn get_profile_by_address(
    db: &Database,
    address: &str,
) -> Result<Option<DiscordUserProfile>, String> {
    let conn = db.conn();

    let result = conn.query_row(
        "SELECT id, discord_user_id, discord_username, public_address,
                registration_status, registered_at, last_interaction_at,
                created_at, updated_at
         FROM discord_user_profiles
         WHERE LOWER(public_address) = LOWER(?1)",
        params![address],
        |row| {
            Ok(DiscordUserProfile {
                id: row.get(0)?,
                discord_user_id: row.get(1)?,
                discord_username: row.get(2)?,
                public_address: row.get(3)?,
                registration_status: row.get(4)?,
                registered_at: row.get(5)?,
                last_interaction_at: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        },
    );

    match result {
        Ok(profile) => Ok(Some(profile)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("Database error: {}", e)),
    }
}

/// Register a public address for a Discord user
pub fn register_address(
    db: &Database,
    discord_user_id: &str,
    address: &str,
) -> Result<(), String> {
    let conn = db.conn();

    conn.execute(
        "UPDATE discord_user_profiles
         SET public_address = ?1,
             registration_status = 'registered',
             registered_at = datetime('now'),
             updated_at = datetime('now')
         WHERE discord_user_id = ?2",
        params![address, discord_user_id],
    )
    .map_err(|e| format!("Failed to register address: {}", e))?;

    log::info!(
        "Discord hooks: Registered address {} for user {}",
        address,
        discord_user_id
    );

    Ok(())
}

/// List all registered profiles (those with a public address)
pub fn list_registered_profiles(db: &Database) -> Result<Vec<DiscordUserProfile>, String> {
    let conn = db.conn();
    let mut stmt = conn
        .prepare(
            "SELECT id, discord_user_id, discord_username, public_address,
                    registration_status, registered_at, last_interaction_at,
                    created_at, updated_at
             FROM discord_user_profiles
             WHERE registration_status = 'registered' AND public_address IS NOT NULL",
        )
        .map_err(|e| format!("Failed to prepare query: {}", e))?;

    let profiles = stmt
        .query_map([], |row| {
            Ok(DiscordUserProfile {
                id: row.get(0)?,
                discord_user_id: row.get(1)?,
                discord_username: row.get(2)?,
                public_address: row.get(3)?,
                registration_status: row.get(4)?,
                registered_at: row.get(5)?,
                last_interaction_at: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })
        .map_err(|e| format!("Failed to query profiles: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect profiles: {}", e))?;

    Ok(profiles)
}

/// List all profiles (registered and unregistered) — used for module dashboard
pub fn list_all_profiles(db: &Database) -> Result<Vec<DiscordUserProfile>, String> {
    let conn = db.conn();
    let mut stmt = conn
        .prepare(
            "SELECT id, discord_user_id, discord_username, public_address,
                    registration_status, registered_at, last_interaction_at,
                    created_at, updated_at
             FROM discord_user_profiles
             ORDER BY updated_at DESC",
        )
        .map_err(|e| format!("Failed to prepare query: {}", e))?;

    let profiles = stmt
        .query_map([], |row| {
            Ok(DiscordUserProfile {
                id: row.get(0)?,
                discord_user_id: row.get(1)?,
                discord_username: row.get(2)?,
                public_address: row.get(3)?,
                registration_status: row.get(4)?,
                registered_at: row.get(5)?,
                last_interaction_at: row.get(6)?,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })
        .map_err(|e| format!("Failed to query profiles: {}", e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| format!("Failed to collect profiles: {}", e))?;

    Ok(profiles)
}

/// Clear all discord user registrations (for restore)
pub fn clear_registrations_for_restore(db: &Database) -> Result<usize, String> {
    let conn = db.conn();
    let count = conn
        .execute("DELETE FROM discord_user_profiles", [])
        .map_err(|e| format!("Failed to clear registrations: {}", e))?;
    log::info!("Cleared {} discord registrations for restore", count);
    Ok(count)
}

/// Unregister a public address for a Discord user
pub fn unregister_address(db: &Database, discord_user_id: &str) -> Result<(), String> {
    let conn = db.conn();

    conn.execute(
        "UPDATE discord_user_profiles
         SET public_address = NULL,
             registration_status = 'unregistered',
             registered_at = NULL,
             updated_at = datetime('now')
         WHERE discord_user_id = ?1",
        params![discord_user_id],
    )
    .map_err(|e| format!("Failed to unregister address: {}", e))?;

    log::info!(
        "Discord hooks: Unregistered address for user {}",
        discord_user_id
    );

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn is_valid_address(addr: &str) -> bool {
        addr.starts_with("0x")
            && addr.len() >= 42
            && addr.len() <= 66
            && addr[2..].chars().all(|c| c.is_ascii_hexdigit())
    }

    #[test]
    fn test_address_validation() {
        // Valid Ethereum address (42 chars)
        assert!(is_valid_address("0x1234567890123456789012345678901234567890"));

        // Valid Starknet address (66 chars)
        assert!(is_valid_address(
            "0x0123456789012345678901234567890123456789012345678901234567890123"
        ));

        // Invalid: too short
        assert!(!is_valid_address("0x123"));

        // Invalid: no 0x prefix
        assert!(!is_valid_address("1234567890123456789012345678901234567890"));

        // Invalid: non-hex characters
        assert!(!is_valid_address("0xGGGG567890123456789012345678901234567890"));
    }
}
