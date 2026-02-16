# Safe Mode Chat Feature for Non-Admin Discord Users

## Overview

When a non-admin Discord user @mentions starkbot with the word "chat", create a safe mode chat session (one per user). Safe mode disables all tools except whitelisted read-only ones (memory_read, memory_search). Admin sessions are never in safe mode.

## Key Files to Modify

| File | Changes |
|------|---------|
| `stark-backend/src/db/sqlite.rs` | Add `safe_mode` column migration |
| `stark-backend/src/models/chat_session.rs` | Add `safe_mode: bool` field |
| `stark-backend/src/db/tables/chat_sessions.rs` | Add safe mode session methods |
| `stark-backend/src/discord_hooks/mod.rs` | Handle "chat" keyword, track safe mode sessions |
| `stark-backend/src/channels/types.rs` | Add safe mode fields to `NormalizedMessage` |
| `stark-backend/src/channels/discord.rs` | Pass safe mode fields through |
| `stark-backend/src/channels/dispatcher.rs` | Create safe mode sessions, filter tools |
| `stark-backend/src/tools/types.rs` | Add `SAFE_MODE_ALLOWED_TOOLS` whitelist |

## Implementation Steps

### 1. Database: Add `safe_mode` column

**File:** `stark-backend/src/db/sqlite.rs`

Add migration in `apply_migrations()`:
```rust
let _ = conn.execute(
    "ALTER TABLE chat_sessions ADD COLUMN safe_mode INTEGER NOT NULL DEFAULT 0",
    [],
);
```

### 2. Model: Add `safe_mode` field

**File:** `stark-backend/src/models/chat_session.rs`

Add to `ChatSession` struct:
```rust
pub struct ChatSession {
    // ... existing fields ...
    /// Whether this session is in safe mode (restricted tools for non-admins)
    pub safe_mode: bool,
}
```

Also add to `ChatSessionResponse`.

### 3. Database Methods: Safe mode session handling

**File:** `stark-backend/src/db/tables/chat_sessions.rs`

Update `row_to_chat_session()` to read `safe_mode` field.

Add new methods:
```rust
/// Get active safe mode session for a Discord user (returns most recent active one)
pub fn get_safe_mode_session_for_discord_user(
    &self,
    channel_id: i64,
    discord_user_id: &str,
) -> SqliteResult<Option<ChatSession>>

/// Create a new safe mode session, deactivating any existing one for this user
pub fn create_safe_mode_session(
    &self,
    channel_id: i64,
    discord_user_id: &str,
    scope: SessionScope,
) -> SqliteResult<ChatSession>
```

Session key format: `discord:{channel_id}:safemode:{discord_user_id}:{timestamp}`

### 4. Discord Hooks: Handle "chat" keyword

**File:** `stark-backend/src/discord_hooks/mod.rs`

Add in-memory session tracker:
```rust
lazy_static::lazy_static! {
    /// Track active safe mode sessions: discord_user_id -> session_id
    static ref SAFE_MODE_SESSIONS: Mutex<HashMap<String, i64>> = Mutex::new(HashMap::new());
}

pub fn get_safe_mode_session(user_id: &str) -> Option<i64>
pub fn set_safe_mode_session(user_id: &str, session_id: i64)
pub fn clear_safe_mode_session(user_id: &str)
```

Extend `ForwardRequest`:
```rust
pub struct ForwardRequest {
    pub text: String,
    pub user_id: String,
    pub user_name: String,
    pub is_admin: bool,
    /// Continue existing safe mode session
    pub safe_mode_session_id: Option<i64>,
    /// Create new safe mode session
    pub start_safe_mode: bool,
}
```

In `process()` for non-admin users (after line 301):
```rust
// Check for "chat" keyword
if command_text.to_lowercase().contains("chat") {
    // Check for existing active session in memory + DB
    if let Some(session_id) = get_safe_mode_session(&user_id) {
        // Validate session still active in DB
        // If valid: forward with safe_mode_session_id
        // If invalid: clear and create new
    }
    // Create new safe mode session
    return Ok(ProcessResult::forward_to_agent(ForwardRequest {
        text: clean_text,  // remove "chat" word
        start_safe_mode: true,
        ...
    }));
}

// Check if user has existing safe mode session (continuation without "chat")
if let Some(session_id) = get_safe_mode_session(&user_id) {
    // Forward to continue existing session
    return Ok(ProcessResult::forward_to_agent(ForwardRequest {
        safe_mode_session_id: Some(session_id),
        ...
    }));
}

// Otherwise: existing limited commands (register, status, help)
```

### 5. NormalizedMessage: Add safe mode fields

**File:** `stark-backend/src/channels/types.rs`

```rust
pub struct NormalizedMessage {
    // ... existing fields ...
    #[serde(default)]
    pub safe_mode_session_id: Option<i64>,
    #[serde(default)]
    pub start_safe_mode: bool,
}
```

### 6. Discord Handler: Pass safe mode fields

**File:** `stark-backend/src/channels/discord.rs`

When creating `NormalizedMessage` from `ForwardRequest`, copy the safe mode fields.

### 7. Tool Whitelist: Define allowed safe mode tools

**File:** `stark-backend/src/tools/types.rs`

```rust
/// Tools allowed in safe mode - easily extensible whitelist
pub const SAFE_MODE_ALLOWED_TOOLS: &[&str] = &[
    "memory_read",
    "memory_search",
    // Future: add more read-only tools here
    // "journal_read",
];

impl ToolConfig {
    /// Create config for safe mode sessions
    pub fn safe_mode() -> Self {
        ToolConfig {
            profile: ToolProfile::Custom,
            allow_list: SAFE_MODE_ALLOWED_TOOLS.iter().map(|s| s.to_string()).collect(),
            deny_list: vec![],
            allowed_groups: vec![],
            denied_groups: ToolGroup::all().iter().map(|g| g.as_str().to_string()).collect(),
            ..Default::default()
        }
    }
}
```

### 8. Dispatcher: Handle safe mode sessions

**File:** `stark-backend/src/channels/dispatcher.rs`

In the dispatch logic:

```rust
// Determine if this is a safe mode session
let (session, is_safe_mode) = if message.start_safe_mode {
    // Create new safe mode session
    let session = self.db.create_safe_mode_session(
        message.channel_id,
        &message.user_id,
        SessionScope::Dm,
    )?;
    discord_hooks::set_safe_mode_session(&message.user_id, session.id);
    (session, true)
} else if let Some(session_id) = message.safe_mode_session_id {
    // Continue existing safe mode session
    let session = self.db.get_chat_session(session_id)?
        .ok_or("Session not found")?;
    (session, session.safe_mode)
} else {
    // Normal session handling (existing code)
    (normal_session, false)
};

// Get tools based on safe mode
let tools = if is_safe_mode {
    let safe_config = ToolConfig::safe_mode();
    self.tool_registry.get_tools_filtered_by_config(&safe_config)
} else {
    // Existing tool selection logic
};
```

Add safe mode notice to system prompt when `is_safe_mode`:
```
## Safe Mode Active
You are in SAFE MODE with limited capabilities:
- memory_read: Read memories and daily logs
- memory_search: Search stored knowledge

You cannot perform actions like transactions, file operations, or messaging.
Focus on providing helpful information from memories and friendly conversation.
```

## User Flow

1. **User:** `@starkbot chat hello!`
   - Bot detects "chat" keyword + non-admin
   - Creates safe mode session in DB with `safe_mode=1`
   - Stores session ID in `SAFE_MODE_SESSIONS` map
   - Runs agentic loop with only memory tools
   - Bot responds conversationally

2. **User:** `@starkbot what do you remember about X?`
   - Bot finds existing session in `SAFE_MODE_SESSIONS`
   - Validates session still active in DB
   - Continues same session with same tool restrictions
   - Bot uses `memory_search` to find relevant info

3. **Admin:** `@starkbot query check my portfolio`
   - Admin flow unchanged - full tools available
   - `safe_mode=0` on their sessions

## Verification

1. Test non-admin "chat" flow creates safe mode session
2. Test subsequent @mentions continue same session
3. Test only whitelisted tools are available (memory_read, memory_search)
4. Test admin sessions have full tools (not safe mode)
5. Test session persistence across bot restarts (DB lookup)
6. Test adding new tools to `SAFE_MODE_ALLOWED_TOOLS` whitelist
