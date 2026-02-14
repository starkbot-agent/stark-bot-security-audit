//! Unregister command - removes user's registered address

use crate::db::Database;
use crate::discord_hooks::db;

/// Execute the unregister command
pub async fn execute(user_id: &str, database: &Database) -> Result<String, String> {
    // Check if user has a registered address
    let profile = match db::get_profile(database, user_id).await? {
        Some(p) => p,
        None => {
            return Ok("You don't have a registered address to remove.".to_string());
        }
    };

    if profile.public_address.is_none() {
        return Ok("You don't have a registered address to remove.".to_string());
    }

    // Unregister the address
    db::unregister_address(database, user_id).await?;

    Ok("Your address has been unregistered. You will no longer receive tips.".to_string())
}
