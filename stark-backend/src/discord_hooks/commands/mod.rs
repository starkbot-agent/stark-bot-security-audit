//! Discord command handling for limited user commands

pub mod force_register;
mod help;
mod register;
mod status;
mod unregister;

use crate::db::Database;

/// Available commands for non-admin users
#[derive(Debug)]
pub enum Command {
    /// Register a public address: `register 0x...`
    Register(String),
    /// Check registration status: `status`
    Status,
    /// Show help: `help`
    Help,
    /// Unregister address: `unregister`
    Unregister,
}

/// Parse a command from text
pub fn parse(text: &str) -> Option<Command> {
    let text = text.trim();
    let parts: Vec<&str> = text.split_whitespace().collect();

    log::debug!(
        "Discord commands: Parsing '{}' -> {} parts: {:?}",
        text,
        parts.len(),
        parts
    );

    let command = parts.first()?.to_lowercase();

    match command.as_str() {
        "register" => {
            // Need an address argument
            let result = parts.get(1).map(|addr| Command::Register(addr.to_string()));
            if result.is_none() {
                log::warn!(
                    "Discord commands: 'register' parsed but no address found. Raw bytes: {:?}",
                    text.as_bytes()
                );
            }
            result
        }
        "status" | "whoami" | "me" => Some(Command::Status),
        "help" | "?" => Some(Command::Help),
        "unregister" | "deregister" | "remove" => Some(Command::Unregister),
        _ => {
            log::debug!(
                "Discord commands: Unknown command '{}' (bytes: {:?})",
                command,
                command.as_bytes()
            );
            None
        }
    }
}

/// Execute a command and return the response
pub async fn execute(cmd: Command, user_id: &str, db: &Database) -> Result<String, String> {
    // Guard: tipping commands require the discord_tipping module to be installed
    if !matches!(cmd, Command::Help) {
        if !db.is_module_installed("discord_tipping").unwrap_or(false) {
            return Ok(
                "The **discord_tipping** module is not installed.\n\n\
                Ask an admin to install it from the Modules page or via:\n\
                `manage_modules(action=\"install\", name=\"discord_tipping\")`"
                    .to_string(),
            );
        }
    }

    match cmd {
        Command::Register(addr) => register::execute(user_id, &addr, db).await,
        Command::Status => status::execute(user_id, db).await,
        Command::Help => Ok(help::execute()),
        Command::Unregister => unregister::execute(user_id, db).await,
    }
}

/// Message shown when a user tries to run an unauthorized command
pub fn permission_denied_message() -> String {
    "You don't have permission to run that command.\n\n\
    **Available commands:**\n\
    - `@starkbot register <address>` - Register your public address for tipping\n\
    - `@starkbot status` - Check your registration status\n\
    - `@starkbot help` - Show available commands\n\
    - `@starkbot unregister` - Remove your registered address"
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_register() {
        match parse("register 0x123abc") {
            Some(Command::Register(addr)) => assert_eq!(addr, "0x123abc"),
            _ => panic!("Expected Register command"),
        }

        // Missing address
        assert!(parse("register").is_none());
    }

    #[test]
    fn test_parse_status() {
        assert!(matches!(parse("status"), Some(Command::Status)));
        assert!(matches!(parse("whoami"), Some(Command::Status)));
        assert!(matches!(parse("me"), Some(Command::Status)));
    }

    #[test]
    fn test_parse_help() {
        assert!(matches!(parse("help"), Some(Command::Help)));
        assert!(matches!(parse("?"), Some(Command::Help)));
    }

    #[test]
    fn test_parse_unregister() {
        assert!(matches!(parse("unregister"), Some(Command::Unregister)));
        assert!(matches!(parse("deregister"), Some(Command::Unregister)));
        assert!(matches!(parse("remove"), Some(Command::Unregister)));
    }

    #[test]
    fn test_parse_unknown() {
        assert!(parse("unknown command").is_none());
        assert!(parse("tip @someone 100").is_none());
    }

    #[test]
    fn test_case_insensitive() {
        assert!(matches!(parse("REGISTER 0x123"), Some(Command::Register(_))));
        assert!(matches!(parse("Status"), Some(Command::Status)));
        assert!(matches!(parse("HELP"), Some(Command::Help)));
    }
}
