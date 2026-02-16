# Discord Hooks Bot - Modular Integration Plan

## Overview

A **self-contained submodule** for Discord command handling that plugs into StarkBot with minimal code changes to the existing codebase.

### Goals
1. `@starkbot tip @jimmy 1000 coins` - Admin executes agentic commands via Discord
2. `@starkbot register 0x...` - Regular users register their public address
3. **Minimal coupling** - New module handles all Discord-specific logic independently

---

## Architecture: Isolated Submodule

```
┌─────────────────────────────────────────────────────────────────────────┐
│                         EXISTING STARKBOT CODE                          │
│                                                                          │
│  ┌─────────────────────┐      ┌─────────────────────────────────────┐  │
│  │  discord.rs         │      │  dispatcher.rs                      │  │
│  │  (existing listener)│      │  (existing agent dispatcher)        │  │
│  │                     │      │                                     │  │
│  │  + 1 line:          │      │  (NO CHANGES)                       │  │
│  │  discord_hooks::    │      │                                     │  │
│  │    process()        │      │                                     │  │
│  └──────────┬──────────┘      └──────────────────────────────────────┘  │
│             │                                                            │
│             │ delegates @mentions                                        │
│             ▼                                                            │
│  ┌──────────────────────────────────────────────────────────────────┐   │
│  │                    NEW: discord_hooks/ SUBMODULE                  │   │
│  │                                                                   │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │   │
│  │  │   mod.rs    │  │  config.rs  │  │   db.rs     │              │   │
│  │  │  (router)   │  │  (settings) │  │  (profiles) │              │   │
│  │  └──────┬──────┘  └─────────────┘  └─────────────┘              │   │
│  │         │                                                        │   │
│  │         ├─────────────────┬─────────────────┐                   │   │
│  │         ▼                 ▼                 ▼                   │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐              │   │
│  │  │ commands/   │  │permissions. │  │  tools/     │              │   │
│  │  │ register.rs │  │    rs       │  │ resolve.rs  │              │   │
│  │  │ status.rs   │  │             │  │             │              │   │
│  │  │ help.rs     │  │             │  │             │              │   │
│  │  └─────────────┘  └─────────────┘  └─────────────┘              │   │
│  │                                                                   │   │
│  └──────────────────────────────────────────────────────────────────┘   │
│                                                                          │
└─────────────────────────────────────────────────────────────────────────┘
```

---

## Minimal Changes to Existing Code

### Total Touchpoints: 3 files, ~15 lines

| File | Change | Lines |
|------|--------|-------|
| `src/channels/discord.rs` | Add early-return delegation to `discord_hooks::process()` | ~5 lines |
| `src/db/sqlite.rs` | Add `discord_hooks::db::init_tables(&conn)` call | ~1 line |
| `src/tools/builtin/mod.rs` | Register `discord_hooks::tools::resolve_user_tool()` | ~1 line |

That's it. Everything else lives in the new submodule.

---

## Submodule Structure

```
src/
└── discord_hooks/
    ├── mod.rs              # Public API: process(), init()
    ├── config.rs           # DiscordHooksConfig (admin IDs, settings)
    ├── db.rs               # discord_user_profiles table + CRUD
    ├── permissions.rs      # is_admin(), can_execute()
    ├── router.rs           # Route messages to handlers or dispatcher
    ├── commands/
    │   ├── mod.rs          # Command enum + parser
    │   ├── register.rs     # Register address handler
    │   ├── status.rs       # Status check handler
    │   ├── help.rs         # Help text handler
    │   └── unregister.rs   # Unregister handler
    └── tools/
        ├── mod.rs          # Tool exports
        └── resolve_user.rs # discord_resolve_user tool for agent
```

---

## Public API (Entry Points)

### `discord_hooks::process()`

Called from existing `discord.rs` message handler. Returns `Option<String>` - if `Some`, the module handled the message and returns a response. If `None`, fall through to existing behavior.

```rust
// In src/discord_hooks/mod.rs

pub struct ProcessResult {
    pub handled: bool,
    pub response: Option<String>,
    pub forward_to_agent: Option<ForwardRequest>,
}

pub struct ForwardRequest {
    pub text: String,
    pub user_id: String,
    pub user_name: String,
    pub is_admin: bool,
}

/// Process a Discord message. Returns how to handle it.
///
/// - If bot not mentioned: ProcessResult { handled: false, .. }
/// - If limited command (register/status/help): handles internally, returns response
/// - If admin command: returns ForwardRequest to send to agent dispatcher
pub async fn process(
    msg: &Message,
    ctx: &Context,
    db: &Database,
    config: &DiscordHooksConfig,
) -> Result<ProcessResult, Error> {
    // 1. Check if bot is mentioned
    let bot_id = ctx.cache.current_user().id;
    if !is_bot_mentioned(msg, bot_id) {
        return Ok(ProcessResult::not_handled());
    }

    // 2. Extract command text
    let command_text = extract_command_text(&msg.content, bot_id);

    // 3. Get/create user profile
    let user_id = msg.author.id.to_string();
    db::get_or_create_profile(db, &user_id, &msg.author.name).await?;

    // 4. Check permissions
    let is_admin = config.is_admin(&user_id);

    // 5. Route based on permissions
    if is_admin {
        // Admin: forward to agent
        Ok(ProcessResult::forward_to_agent(ForwardRequest {
            text: command_text,
            user_id,
            user_name: msg.author.name.clone(),
            is_admin: true,
        }))
    } else {
        // Regular user: try limited commands
        match commands::parse(&command_text) {
            Some(cmd) => {
                let response = commands::execute(cmd, &user_id, db).await?;
                Ok(ProcessResult::handled(response))
            }
            None => {
                Ok(ProcessResult::handled(commands::permission_denied_message()))
            }
        }
    }
}
```

### `discord_hooks::db::init_tables()`

Called once at startup to create the module's table.

```rust
// In src/discord_hooks/db.rs

pub fn init_tables(conn: &Connection) -> Result<()> {
    conn.execute(
        "CREATE TABLE IF NOT EXISTS discord_user_profiles (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            discord_user_id TEXT NOT NULL UNIQUE,
            discord_username TEXT,
            public_address TEXT,
            registration_status TEXT NOT NULL DEFAULT 'unregistered',
            registered_at TEXT,
            last_interaction_at TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        )",
        [],
    )?;

    conn.execute(
        "CREATE INDEX IF NOT EXISTS idx_discord_profiles_address
         ON discord_user_profiles(public_address)",
        [],
    )?;

    Ok(())
}
```

### `discord_hooks::tools::resolve_user_tool()`

Returns a Tool instance that can be registered with the tool registry.

```rust
// In src/discord_hooks/tools/mod.rs

pub fn resolve_user_tool(db: Arc<Database>) -> impl Tool {
    ResolveUserTool { db }
}
```

---

## Integration Code (Changes to Existing Files)

### 1. `src/channels/discord.rs` (~5 lines)

```rust
// At the top of message() handler, add early delegation:

async fn message(&self, ctx: Context, msg: Message) {
    if msg.author.bot {
        return;
    }

    // ===== NEW: Delegate to discord_hooks module =====
    match discord_hooks::process(&msg, &ctx, &self.db, &self.discord_hooks_config).await {
        Ok(result) => {
            if let Some(response) = result.response {
                // Module handled it with a direct response
                let _ = msg.reply(&ctx.http, &response).await;
                return;
            }
            if let Some(forward) = result.forward_to_agent {
                // Module says forward to agent with admin context
                let normalized = NormalizedMessage {
                    text: forward.text,
                    user_id: forward.user_id,
                    is_admin: forward.is_admin,
                    // ... rest of fields from msg
                };
                // Continue to dispatch below
            }
            if !result.handled {
                // Module didn't handle it, fall through to existing behavior
                // (existing code continues below)
            }
        }
        Err(e) => {
            log::error!("discord_hooks error: {}", e);
            // Fall through to existing behavior
        }
    }
    // ===== END NEW =====

    // ... existing message handling code unchanged ...
}
```

### 2. `src/db/sqlite.rs` (~1 line)

```rust
// In the init function, after other table creations:

pub fn init_database(conn: &Connection) -> Result<()> {
    // ... existing table creations ...

    // NEW: Initialize discord_hooks tables
    discord_hooks::db::init_tables(conn)?;

    Ok(())
}
```

### 3. `src/tools/builtin/mod.rs` (~1 line)

```rust
// In the tool registration:

pub fn register_builtin_tools(registry: &mut ToolRegistry, db: Arc<Database>) {
    // ... existing tool registrations ...

    // NEW: Discord user resolution tool
    registry.register(discord_hooks::tools::resolve_user_tool(db.clone()));
}
```

---

## Submodule Implementation Details

### Config (`discord_hooks/config.rs`)

```rust
use std::collections::HashSet;

pub struct DiscordHooksConfig {
    admin_user_ids: HashSet<String>,
    require_mention_in_servers: bool,
    allow_dm_without_mention: bool,
}

impl DiscordHooksConfig {
    pub fn from_env() -> Self {
        let admin_ids = std::env::var("DISCORD_ADMIN_USER_IDS")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();

        Self {
            admin_user_ids: admin_ids,
            require_mention_in_servers: true,
            allow_dm_without_mention: true,
        }
    }

    pub fn is_admin(&self, user_id: &str) -> bool {
        self.admin_user_ids.contains(user_id)
    }
}
```

### Database Operations (`discord_hooks/db.rs`)

```rust
use rusqlite::{Connection, params};

#[derive(Debug, Clone)]
pub struct DiscordUserProfile {
    pub id: i64,
    pub discord_user_id: String,
    pub discord_username: Option<String>,
    pub public_address: Option<String>,
    pub registration_status: String,
    pub registered_at: Option<String>,
}

pub async fn get_or_create_profile(
    db: &Database,
    discord_user_id: &str,
    username: &str,
) -> Result<DiscordUserProfile> {
    // Insert or ignore, then select
    db.execute(
        "INSERT OR IGNORE INTO discord_user_profiles (discord_user_id, discord_username)
         VALUES (?1, ?2)",
        params![discord_user_id, username],
    )?;

    get_profile(db, discord_user_id)
}

pub async fn get_profile(db: &Database, discord_user_id: &str) -> Result<Option<DiscordUserProfile>> {
    // Query and map to struct
}

pub async fn get_profile_by_address(db: &Database, address: &str) -> Result<Option<DiscordUserProfile>> {
    // Query by public_address
}

pub async fn register_address(
    db: &Database,
    discord_user_id: &str,
    address: &str,
) -> Result<()> {
    db.execute(
        "UPDATE discord_user_profiles
         SET public_address = ?1,
             registration_status = 'registered',
             registered_at = datetime('now'),
             updated_at = datetime('now')
         WHERE discord_user_id = ?2",
        params![address, discord_user_id],
    )
}

pub async fn unregister_address(db: &Database, discord_user_id: &str) -> Result<()> {
    db.execute(
        "UPDATE discord_user_profiles
         SET public_address = NULL,
             registration_status = 'unregistered',
             registered_at = NULL,
             updated_at = datetime('now')
         WHERE discord_user_id = ?1",
        params![discord_user_id],
    )
}
```

### Command Router (`discord_hooks/commands/mod.rs`)

```rust
pub mod register;
pub mod status;
pub mod help;
pub mod unregister;

#[derive(Debug)]
pub enum Command {
    Register(String),  // address
    Status,
    Help,
    Unregister,
}

pub fn parse(text: &str) -> Option<Command> {
    let text = text.trim().to_lowercase();
    let parts: Vec<&str> = text.split_whitespace().collect();

    match parts.first().map(|s| *s) {
        Some("register") => {
            parts.get(1).map(|addr| Command::Register(addr.to_string()))
        }
        Some("status") => Some(Command::Status),
        Some("help") => Some(Command::Help),
        Some("unregister") => Some(Command::Unregister),
        _ => None,
    }
}

pub async fn execute(cmd: Command, user_id: &str, db: &Database) -> Result<String> {
    match cmd {
        Command::Register(addr) => register::execute(user_id, &addr, db).await,
        Command::Status => status::execute(user_id, db).await,
        Command::Help => Ok(help::execute()),
        Command::Unregister => unregister::execute(user_id, db).await,
    }
}

pub fn permission_denied_message() -> String {
    "You don't have permission to run that command.\n\n\
     **Available commands:**\n\
     • `@starkbot register <address>` - Register your public address\n\
     • `@starkbot status` - Check your registration\n\
     • `@starkbot help` - Show available commands".to_string()
}
```

### Register Command (`discord_hooks/commands/register.rs`)

```rust
pub async fn execute(user_id: &str, address: &str, db: &Database) -> Result<String> {
    // Validate address format
    if !is_valid_address(address) {
        return Ok("Invalid address format. Please provide a valid address starting with `0x`.".into());
    }

    // Check if address registered to someone else
    if let Some(existing) = db::get_profile_by_address(db, address).await? {
        if existing.discord_user_id != user_id {
            return Ok("This address is already registered to another Discord user.".into());
        }
    }

    // Register
    db::register_address(db, user_id, address).await?;

    Ok(format!(
        "Successfully registered your address: `{}`\n\
         You can now receive tips from other users!",
        address
    ))
}

fn is_valid_address(addr: &str) -> bool {
    addr.starts_with("0x") &&
    addr.len() >= 42 &&
    addr.len() <= 66 &&
    addr[2..].chars().all(|c| c.is_ascii_hexdigit())
}
```

### Resolve User Tool (`discord_hooks/tools/resolve_user.rs`)

```rust
use regex::Regex;
use serde_json::{json, Value};

pub struct ResolveUserTool {
    db: Arc<Database>,
}

impl Tool for ResolveUserTool {
    fn name(&self) -> &str { "discord_resolve_user" }

    fn description(&self) -> &str {
        "Resolve a Discord user mention to their registered public address. \
         Use this when you need to tip or send tokens to a Discord user mentioned \
         in a message. Returns the user's public address if they have registered one."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "user_mention": {
                    "type": "string",
                    "description": "Discord mention like '<@123456789>' or '<@!123456789>'"
                }
            },
            "required": ["user_mention"]
        })
    }

    async fn execute(&self, params: Value, _ctx: &ToolContext) -> Result<String, String> {
        let mention = params["user_mention"].as_str()
            .ok_or("Missing user_mention")?;

        // Parse <@123> or <@!123>
        let re = Regex::new(r"<@!?(\d+)>").unwrap();
        let user_id = re.captures(mention)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str())
            .ok_or("Invalid mention format")?;

        match db::get_profile(&self.db, user_id).await {
            Ok(Some(p)) if p.public_address.is_some() => {
                Ok(json!({
                    "discord_user_id": p.discord_user_id,
                    "username": p.discord_username,
                    "public_address": p.public_address,
                    "registered": true
                }).to_string())
            }
            Ok(Some(p)) => {
                Ok(json!({
                    "discord_user_id": p.discord_user_id,
                    "username": p.discord_username,
                    "registered": false,
                    "error": "User has not registered a public address"
                }).to_string())
            }
            Ok(None) => {
                Ok(json!({
                    "discord_user_id": user_id,
                    "registered": false,
                    "error": "User has never interacted with StarkBot"
                }).to_string())
            }
            Err(e) => Err(format!("Database error: {}", e)),
        }
    }
}
```

---

## Configuration

### Environment Variables

```bash
# Required: Your Discord user ID(s) for admin access
DISCORD_ADMIN_USER_IDS=123456789012345678

# Optional: Multiple admins (comma-separated)
DISCORD_ADMIN_USER_IDS=123456789012345678,987654321098765432
```

### Finding Your Discord User ID

1. Enable Developer Mode in Discord (Settings → Advanced → Developer Mode)
2. Right-click your username → Copy User ID

---

## User Flows

### Admin: Full Agentic Commands

```
You: @starkbot tip @jimmy 100 STARK

discord_hooks::process():
  → Bot mentioned? Yes
  → Is admin? Yes (your ID in DISCORD_ADMIN_USER_IDS)
  → Return ForwardRequest { text: "tip @jimmy 100 STARK", is_admin: true }

Agent receives message, uses discord_resolve_user tool:
  → Resolves @jimmy to 0x04abc...
  → Executes transfer

Response: "Sent 100 STARK to @jimmy (0x04abc...)"
```

### Regular User: Limited Commands

```
Jimmy: @starkbot register 0x04abc123...def

discord_hooks::process():
  → Bot mentioned? Yes
  → Is admin? No
  → Parse command: Register("0x04abc123...def")
  → Execute register command
  → Return ProcessResult::handled("Successfully registered...")

Response: "Successfully registered your address: 0x04abc123...def"
```

### Regular User: Unauthorized Command

```
Jimmy: @starkbot post a tweet about cats

discord_hooks::process():
  → Bot mentioned? Yes
  → Is admin? No
  → Parse command: None (not a limited command)
  → Return ProcessResult::handled(permission_denied_message())

Response: "You don't have permission to run that command..."
```

---

## File Summary

### New Files (self-contained module)

| File | Lines | Purpose |
|------|-------|---------|
| `src/discord_hooks/mod.rs` | ~80 | Public API, process() function |
| `src/discord_hooks/config.rs` | ~40 | Configuration loading |
| `src/discord_hooks/db.rs` | ~100 | Database table + CRUD |
| `src/discord_hooks/router.rs` | ~50 | Message routing logic |
| `src/discord_hooks/permissions.rs` | ~20 | Permission checks |
| `src/discord_hooks/commands/mod.rs` | ~50 | Command parsing + dispatch |
| `src/discord_hooks/commands/register.rs` | ~40 | Register handler |
| `src/discord_hooks/commands/status.rs` | ~30 | Status handler |
| `src/discord_hooks/commands/help.rs` | ~20 | Help text |
| `src/discord_hooks/commands/unregister.rs` | ~20 | Unregister handler |
| `src/discord_hooks/tools/mod.rs` | ~10 | Tool exports |
| `src/discord_hooks/tools/resolve_user.rs` | ~70 | Resolve tool |
| **Total new code** | **~530** | |

### Modified Files (minimal changes)

| File | Lines Changed | Change |
|------|---------------|--------|
| `src/channels/discord.rs` | ~10 | Add delegation to discord_hooks |
| `src/db/sqlite.rs` | ~1 | Init discord_hooks tables |
| `src/tools/builtin/mod.rs` | ~1 | Register resolve tool |
| `src/lib.rs` or `src/main.rs` | ~1 | `mod discord_hooks;` |
| **Total modified** | **~13** | |

---

## Benefits of This Architecture

1. **Isolation**: All Discord-specific logic in one place
2. **Testability**: Module can be unit tested independently
3. **Maintainability**: Changes don't risk breaking existing channel code
4. **Extensibility**: Easy to add new commands without touching core
5. **Reversibility**: Can disable by removing 3 integration lines
6. **Reusability**: Pattern can be copied for Telegram hooks, etc.

---

## Future Enhancements

1. **Slash Commands**: Add Discord application commands (separate registration)
2. **Signature Verification**: Prove address ownership before registration
3. **Role-Based Permissions**: Check Discord roles for admin status
4. **Rate Limiting**: Add cooldowns to prevent spam
5. **Audit Logging**: Log all commands to a separate table
6. **Multi-Chain Support**: Register addresses per chain
