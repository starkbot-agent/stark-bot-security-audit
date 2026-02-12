//! Discord Hooks - Self-contained module for Discord command handling
//!
//! This module provides:
//! - Admin command detection and forwarding to the agent
//! - Limited command handling for regular users (register, status, help)
//! - Discord user profile management with public address registration
//! - Tool for resolving Discord mentions to registered public addresses
//!
//! ## Admin Flow
//!
//! Any `@bot <message>` from an admin is forwarded directly to the agent
//! (no safe mode), unless it matches a short-circuit keyword like "love"
//! or "register".

pub mod commands;
pub mod config;
pub mod db;
pub mod tools;

use rand::seq::SliceRandom;
use serenity::all::{Context, Message, UserId};

pub use config::DiscordHooksConfig;
pub use db::DiscordUserProfile;

/// Result of processing a Discord message
#[derive(Debug)]
pub struct ProcessResult {
    /// Whether the module handled the message
    pub handled: bool,
    /// Direct response to send (if handled internally)
    pub response: Option<String>,
    /// Request to forward to the agent (if admin command)
    pub forward_to_agent: Option<ForwardRequest>,
}

impl ProcessResult {
    /// Message was not handled (bot not mentioned, etc.)
    pub fn not_handled() -> Self {
        Self {
            handled: false,
            response: None,
            forward_to_agent: None,
        }
    }

    /// Message was handled with a direct response
    pub fn handled(response: String) -> Self {
        Self {
            handled: true,
            response: Some(response),
            forward_to_agent: None,
        }
    }

    /// Message should be forwarded to the agent
    pub fn forward_to_agent(request: ForwardRequest) -> Self {
        Self {
            handled: true,
            response: None,
            forward_to_agent: Some(request),
        }
    }
}

/// Request to forward a message to the agent dispatcher
#[derive(Debug, Clone)]
pub struct ForwardRequest {
    /// Command text (bot mention removed)
    pub text: String,
    /// Discord user ID
    pub user_id: String,
    /// Discord username
    pub user_name: String,
    /// Whether the user is an admin
    pub is_admin: bool,
    /// Force safe mode for this request (e.g., non-admin Discord queries)
    pub force_safe_mode: bool,
}

/// Check if text contains a "love" keyword (as a standalone word boundary)
fn has_love_keyword(text: &str) -> bool {
    text.contains(" love ") || text.starts_with("love ") || text.ends_with(" love") || text == "love"
}

/// Check if the bot is mentioned in a message
/// Checks both the mentions array AND the raw content as a fallback,
/// since Discord's mentions array can sometimes be incomplete when
/// multiple users are mentioned.
pub fn is_bot_mentioned(msg: &Message, bot_id: UserId) -> bool {
    // Primary check: mentions array
    let in_mentions = msg.mentions.iter().any(|u| u.id == bot_id);

    // Fallback check: look for bot ID pattern in raw content
    // Discord formats mentions as <@USER_ID> or <@!USER_ID> (with nickname)
    let bot_mention_pattern = format!("<@{}>", bot_id);
    let bot_mention_nick_pattern = format!("<@!{}>", bot_id);
    let in_content = msg.content.contains(&bot_mention_pattern)
        || msg.content.contains(&bot_mention_nick_pattern);

    if in_content && !in_mentions {
        log::warn!(
            "Discord hooks: Bot mention found in content but NOT in mentions array! content='{}', mentions={:?}",
            if msg.content.len() > 100 { format!("{}...", &msg.content[..100]) } else { msg.content.clone() },
            msg.mentions.iter().map(|u| u.id.to_string()).collect::<Vec<_>>()
        );
    }

    in_mentions || in_content
}

/// Extract command text from a message, removing bot mentions
pub fn extract_command_text(content: &str, bot_id: UserId) -> String {
    // Remove <@BOT_ID> and <@!BOT_ID> patterns
    let bot_mention = format!("<@{}>", bot_id);
    let bot_mention_nick = format!("<@!{}>", bot_id);

    content
        .replace(&bot_mention, "")
        .replace(&bot_mention_nick, "")
        .trim()
        .to_string()
}

/// Process a Discord message through the hooks system
///
/// Returns a ProcessResult indicating how to handle the message:
/// - `handled: false` - Bot not mentioned, fall through to existing behavior
/// - `handled: true` with `response` - Send the response directly
/// - `handled: true` with `forward_to_agent` - Forward to agent dispatcher
///
/// Note: The config is reloaded from the database on each message to pick up
/// changes to admin user IDs without requiring a channel restart.
pub async fn process(
    msg: &Message,
    ctx: &Context,
    db: &std::sync::Arc<crate::db::Database>,
    channel_id: i64,
) -> Result<ProcessResult, String> {
    // Reload config from database to pick up any changes
    let config = DiscordHooksConfig::from_channel_settings(db, channel_id);
    // Get bot's user ID by fetching current user info
    let current_user = ctx
        .http
        .get_current_user()
        .await
        .map_err(|e| format!("Failed to get current user: {}", e))?;
    let bot_id = current_user.id;

    // Check if this is a reply to one of the bot's messages
    let is_reply_to_bot = msg.message_reference.is_some()
        && msg
            .referenced_message
            .as_ref()
            .map(|ref_msg| ref_msg.author.id == bot_id)
            .unwrap_or(false);

    if is_reply_to_bot {
        log::info!("Discord hooks: Processing reply-to-bot from {}", msg.author.name);
    }

    // Debug logging for mention analysis
    log::info!(
        "Discord hooks: Message from {} - mentions={:?}, content_preview='{}', reply_to_bot={}",
        msg.author.name,
        msg.mentions.iter().map(|u| format!("{}({})", u.name, u.id)).collect::<Vec<_>>(),
        if msg.content.len() > 100 { format!("{}...", &msg.content[..100]) } else { msg.content.clone() },
        is_reply_to_bot
    );

    // Check if bot is mentioned OR if user is replying to the bot
    if !is_reply_to_bot && !is_bot_mentioned(msg, bot_id) {
        // Check if they mentioned a role the bot has (common mistake)
        if !msg.mention_roles.is_empty() {
            if let Some(guild_id) = msg.guild_id {
                // Get bot's roles in this guild
                if let Ok(bot_member) = guild_id.member(&ctx.http, bot_id).await {
                    let bot_roles: std::collections::HashSet<_> = bot_member.roles.iter().collect();
                    let mentioned_bot_role = msg.mention_roles.iter().any(|r| bot_roles.contains(r));

                    if mentioned_bot_role {
                        return Ok(ProcessResult::handled(
                            "It looks like you mentioned my **role**, not me directly. \
                            Please @mention the bot user instead of the role.\n\n\
                            **Tip:** When typing `@stark`, look for the one with the bot icon 🤖, \
                            not the role icon 🏷️".to_string(),
                        ));
                    }
                }
            }
        }
        // In DMs, we might want to process without mention
        // For now, require mention in all contexts
        return Ok(ProcessResult::not_handled());
    }

    // Extract command text (remove bot mention)
    let command_text = extract_command_text(&msg.content, bot_id);

    if command_text.is_empty() {
        return Ok(ProcessResult::handled(
            "Hi! I'm StarkBot. Try `@starkbot help` to see available commands.".to_string(),
        ));
    }

    // Get user info
    let user_id = msg.author.id.to_string();
    let user_name = msg.author.name.clone();

    // Get or create user profile (only if discord_tipping module is installed)
    if db.is_module_installed("discord_tipping").unwrap_or(false) {
        if let Err(e) = db::get_or_create_profile(db, &user_id, &user_name) {
            log::error!("Discord hooks: Failed to get/create profile: {}", e);
            // Don't fail the whole request, just log it
        }
    }

    // Check if user is admin (explicit IDs or Discord Administrator permission)
    let is_admin = config.is_admin(&user_id, msg, ctx).await;

    log::info!(
        "Discord hooks: Processing message from {} ({}), admin={} (explicit_admins={}), text='{}'",
        user_name,
        user_id,
        is_admin,
        config.has_explicit_admins(),
        if command_text.len() > 50 {
            format!("{}...", &command_text[..50])
        } else {
            command_text.clone()
        }
    );

    if is_admin {
        // Admin flow: forward everything to agent unless it matches a short-circuit keyword
        let cmd_lower = command_text.to_lowercase();

        // Easter egg: "love"
        if has_love_keyword(&cmd_lower) {
            let responses = [
                "I love you too.",
                "I don't know, let me think about that.",
                "How much do you love me?",
            ];
            let response = responses.choose(&mut rand::thread_rng()).unwrap_or(&responses[0]);
            return Ok(ProcessResult::handled(response.to_string()));
        }

        // "force_register" command - admin registers an address for another user
        if cmd_lower.starts_with("force_register") {
            log::info!(
                "Discord hooks: Admin {} using force_register command",
                user_name
            );
            match commands::force_register::parse(&command_text) {
                Some((target_user_id, address)) => {
                    let response = commands::force_register::execute(
                        &target_user_id,
                        &address,
                        &user_id,
                        db,
                    )
                    .await?;
                    return Ok(ProcessResult::handled(response));
                }
                None => {
                    return Ok(ProcessResult::handled(
                        "Invalid force_register command.\n\n\
                        **Usage:** `@starkbot force_register @user 0x...`\n\
                        **Example:** `@starkbot force_register @alice 0x1234567890123456789012345678901234567890`"
                            .to_string(),
                    ));
                }
            }
        }

        // "register" command - handle directly like a regular user
        if cmd_lower.starts_with("register") {
            log::info!(
                "Discord hooks: Admin {} using register command",
                user_name
            );
            match commands::parse(&command_text) {
                Some(cmd) => {
                    let response = commands::execute(cmd, &user_id, db).await?;
                    return Ok(ProcessResult::handled(response));
                }
                None => {
                    return Ok(ProcessResult::handled(
                        "Invalid register command. Usage: `@starkbot register 0x...`".to_string(),
                    ));
                }
            }
        }

        // Default: forward to agent (no safe mode)
        log::info!(
            "Discord hooks: Admin {} forwarding to agent: '{}'",
            user_name,
            if command_text.len() > 50 {
                format!("{}...", &command_text[..50])
            } else {
                command_text.clone()
            }
        );
        Ok(ProcessResult::forward_to_agent(ForwardRequest {
            text: command_text,
            user_id,
            user_name,
            is_admin: true,
            force_safe_mode: false,
        }))
    } else {
        // Regular user: try limited commands
        let cmd_lower = command_text.to_lowercase();

        // Easter egg: respond to "love" messages
        if has_love_keyword(&cmd_lower) {
            let responses = [
                "I love you too.",
                "I don't know, let me think about that.",
                "How much do you love me?",
            ];
            let response = responses.choose(&mut rand::thread_rng()).unwrap_or(&responses[0]);
            return Ok(ProcessResult::handled(response.to_string()));
        }

        match commands::parse(&command_text) {
            Some(cmd) => {
                let response = commands::execute(cmd, &user_id, db).await?;
                Ok(ProcessResult::handled(response))
            }
            None => {
                // Forward to agent with safe mode restrictions
                log::info!(
                    "Discord hooks: Non-admin {} querying with safe mode: '{}'",
                    user_name,
                    command_text.chars().take(50).collect::<String>()
                );
                Ok(ProcessResult::forward_to_agent(ForwardRequest {
                    text: command_text,
                    user_id,
                    user_name,
                    is_admin: false,
                    force_safe_mode: true,
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_command_text() {
        let bot_id = UserId::new(123456789);

        // Normal mention
        assert_eq!(
            extract_command_text("<@123456789> help", bot_id),
            "help"
        );

        // Nickname mention
        assert_eq!(
            extract_command_text("<@!123456789> register 0x123", bot_id),
            "register 0x123"
        );

        // Multiple mentions
        assert_eq!(
            extract_command_text("<@123456789> <@123456789> test", bot_id),
            "test"
        );

        // No mention
        assert_eq!(
            extract_command_text("just some text", bot_id),
            "just some text"
        );
    }

    #[test]
    fn test_has_love_keyword() {
        // Should match
        assert!(has_love_keyword("love you"));
        assert!(has_love_keyword("i love you"));
        assert!(has_love_keyword("i really love"));
        assert!(has_love_keyword("love"));

        // Should not match
        assert!(!has_love_keyword("lovely day"));
        assert!(!has_love_keyword("gloves are warm"));
        assert!(!has_love_keyword("hello"));
        assert!(!has_love_keyword("tip @user 100"));
    }
}
