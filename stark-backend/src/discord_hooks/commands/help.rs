//! Help command - shows available commands

/// Execute the help command
pub fn execute() -> String {
    "**StarkBot Discord Commands**\n\n\
    **For all users:**\n\
    - `@starkbot register <address>` - Register your public address to receive tips\n\
    - `@starkbot status` - Check your registration status\n\
    - `@starkbot unregister` - Remove your registered address\n\
    - `@starkbot help` - Show this help message\n\n\
    **Admin only:**\n\
    - `@starkbot force_register @user <address>` - Register an address for another user\n\n\
    **Example:**\n\
    ```\n\
    @starkbot register 0x1234567890123456789012345678901234567890\n\
    ```\n\n\
    Once registered, other users can tip you using your Discord mention!"
        .to_string()
}
