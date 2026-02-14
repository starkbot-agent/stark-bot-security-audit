//! Status command - shows user's registration status

use crate::db::Database;
use crate::discord_hooks::db;

/// Execute the status command
pub async fn execute(user_id: &str, database: &Database) -> Result<String, String> {
    let profile = match db::get_profile(database, user_id).await? {
        Some(p) => p,
        None => {
            return Ok(
                "**Your StarkBot Profile**\n\n\
                Status: Not registered\n\n\
                Use `@starkbot register <your-address>` to register your public address for tipping."
                    .to_string(),
            );
        }
    };

    if let Some(addr) = profile.public_address {
        let registered_at = profile
            .registered_at
            .as_deref()
            .unwrap_or("Unknown");

        Ok(format!(
            "**Your StarkBot Profile**\n\n\
            **Status:** Registered\n\
            **Address:** `{}`\n\
            **Registered:** {}\n\n\
            You can receive tips from other users!",
            addr, registered_at
        ))
    } else {
        Ok(
            "**Your StarkBot Profile**\n\n\
            **Status:** Not registered\n\n\
            Use `@starkbot register <your-address>` to register your public address for tipping."
                .to_string(),
        )
    }
}
