# Future Upgrades

Roadmap to evolve StarkBot into a full-featured personal AI assistant like Clawdbot.

## Phase 1: Core Infrastructure

### WebSocket Support
Add real-time bidirectional communication for live updates and control.

- [ ] Add `actix-web-actors` for WebSocket handling
- [ ] Create `/ws` endpoint for control plane connections
- [ ] Implement message protocol (JSON-RPC or custom)
- [ ] Add connection authentication via session token
- [ ] Broadcast events to connected clients

```
stark-backend/src/
├── ws/
│   ├── mod.rs
│   ├── server.rs      # WebSocket server actor
│   ├── session.rs     # Client session actor
│   └── messages.rs    # Message types
```

### Background Job System
Process long-running tasks without blocking requests.

- [ ] Add `tokio` task spawning for background jobs
- [ ] Create job queue table in SQLite
- [ ] Implement job status tracking (pending, running, completed, failed)
- [ ] Add `/api/jobs` endpoints for status polling
- [ ] Consider `sqlx` for async database operations

### Configuration Management
Move beyond environment variables for complex settings.

- [ ] Create `config.toml` for static configuration
- [ ] Add database-backed settings for runtime config
- [ ] Implement `/api/settings` CRUD endpoints
- [ ] Add config hot-reloading support

---

## Phase 2: AI Integration

### Claude API Integration
Connect to Anthropic's Claude for AI capabilities.

- [ ] Add `reqwest` for HTTP client
- [ ] Create `claude.rs` service module
- [ ] Implement streaming responses via SSE
- [ ] Add conversation context management
- [ ] Store API key securely (encrypted in DB or env)

```rust
// Example service structure
pub struct ClaudeService {
    client: reqwest::Client,
    api_key: String,
    model: String,  // claude-3-opus, claude-3-sonnet, etc.
}

impl ClaudeService {
    pub async fn chat(&self, messages: Vec<Message>) -> Result<Response>;
    pub async fn chat_stream(&self, messages: Vec<Message>) -> impl Stream<Item = Chunk>;
}
```

### Conversation Memory
Persist and manage conversation history.

- [ ] Create `conversations` table
- [ ] Create `messages` table with foreign key to conversations
- [ ] Implement context window management (token counting)
- [ ] Add conversation summarization for long histories
- [ ] Support multiple concurrent conversations

### System Prompts & Personas
Allow customizable AI behavior.

- [ ] Create `personas` table for system prompts
- [ ] Add persona selection per conversation
- [ ] Support persona templates with variables
- [ ] Add `/api/personas` CRUD endpoints

---

## Phase 3: Communication Platforms

### Discord Integration
Connect StarkBot to Discord servers.

- [ ] Add `serenity` or `twilight` Discord library
- [ ] Implement Discord OAuth2 for bot token management
- [ ] Create event handlers (message, reaction, slash commands)
- [ ] Map Discord channels to StarkBot conversations
- [ ] Support Discord-specific formatting (embeds, attachments)

```
stark-backend/src/
├── platforms/
│   ├── mod.rs
│   ├── discord/
│   │   ├── mod.rs
│   │   ├── bot.rs
│   │   ├── commands.rs
│   │   └── handlers.rs
```

### Telegram Integration
Connect to Telegram via Bot API.

- [ ] Add `teloxide` Telegram library
- [ ] Implement bot token configuration
- [ ] Create message handlers
- [ ] Support inline keyboards and callbacks
- [ ] Handle media messages (photos, documents, voice)

### Slack Integration
Connect to Slack workspaces.

- [ ] Add Slack Bolt SDK or raw API client
- [ ] Implement OAuth2 flow for workspace installation
- [ ] Create event subscriptions (messages, reactions, app mentions)
- [ ] Support Slack Block Kit for rich messages
- [ ] Handle slash commands

### WhatsApp Integration
Connect via WhatsApp Business API or bridge.

- [ ] Research WhatsApp Business API requirements
- [ ] Alternative: integrate with Matrix bridge (mautrix-whatsapp)
- [ ] Implement webhook handlers
- [ ] Support media messages

### Unified Message Router
Abstract platform differences behind common interface.

- [ ] Define `Platform` trait for all integrations
- [ ] Create `IncomingMessage` and `OutgoingMessage` types
- [ ] Implement message routing based on source platform
- [ ] Add platform-specific adapters for formatting

```rust
#[async_trait]
pub trait Platform: Send + Sync {
    async fn send_message(&self, channel: &str, message: OutgoingMessage) -> Result<()>;
    async fn start(&self) -> Result<()>;
    fn name(&self) -> &'static str;
}
```

---

## Phase 4: Automation & Agents

### Scheduled Tasks (Cron)
Run automated tasks on schedules.

- [ ] Add `tokio-cron-scheduler` for cron expressions
- [ ] Create `scheduled_tasks` table
- [ ] Implement task types (send message, run script, API call)
- [ ] Add `/api/schedules` CRUD endpoints
- [ ] Support timezone-aware scheduling

### Agent Runtime
Execute multi-step automated workflows.

- [ ] Define agent action types (message, API call, wait, condition)
- [ ] Create workflow DSL or JSON schema
- [ ] Implement step-by-step execution with state
- [ ] Add error handling and retry logic
- [ ] Support human-in-the-loop approvals

```rust
pub struct Agent {
    pub id: String,
    pub name: String,
    pub trigger: Trigger,      // cron, webhook, message pattern
    pub steps: Vec<AgentStep>,
    pub state: AgentState,
}

pub enum AgentStep {
    SendMessage { platform: String, channel: String, content: String },
    CallApi { url: String, method: String, body: Option<Value> },
    WaitForReply { timeout: Duration },
    Condition { expression: String, then: Vec<AgentStep>, else_: Vec<AgentStep> },
    RunCode { language: String, code: String },
}
```

### Webhook Receiver
Accept incoming webhooks to trigger actions.

- [ ] Add `/api/webhooks/:id` dynamic endpoints
- [ ] Create `webhooks` table with secret validation
- [ ] Route webhook payloads to appropriate handlers
- [ ] Log webhook invocations for debugging

---

## Phase 5: User Experience



### Dashboard Improvements
Enhance the web UI with more functionality.

- [ ] Add conversation list view
- [ ] Implement chat interface for direct AI interaction
- [ ] Create platform connection status panel
- [ ] Add scheduled task management UI
- [ ] Show agent execution logs
- [ ] Add settings configuration page

### Mobile-Responsive Design
Ensure dashboard works on mobile devices.

- [ ] Audit and fix CSS for mobile breakpoints
- [ ] Add touch-friendly controls
- [ ] Consider PWA support for installability

---

## Phase 6: Operations & Reliability

### Logging & Observability
Add structured logging and metrics.

- [ ] Replace `env_logger` with `tracing` for structured logs
- [ ] Add request ID tracking across async operations
- [ ] Implement `/metrics` endpoint (Prometheus format)
- [ ] Add health check details (DB status, platform connections)

### Database Migrations
Manage schema changes properly.

- [ ] Add `refinery` or `sqlx` migrations
- [ ] Create migration files for each schema change
- [ ] Run migrations on startup
- [ ] Support rollback for failed deployments

### Rate Limiting
Protect against abuse.

- [ ] Add `actix-governor` for rate limiting
- [ ] Configure limits per endpoint
- [ ] Add rate limit headers to responses

### Graceful Shutdown
Handle shutdown signals properly.

- [ ] Catch SIGTERM/SIGINT signals
- [ ] Drain active connections
- [ ] Complete in-flight background jobs
- [ ] Disconnect platforms cleanly

---

## Phase 7: Advanced Features

### Plugin System
Allow extensibility without core changes.

- [ ] Define plugin interface (Rust trait or WASM)
- [ ] Create plugin loader and lifecycle management
- [ ] Add plugin configuration storage
- [ ] Support hot-reloading plugins

### Command System
Parse and execute user commands.

- [ ] Define command syntax (`/command arg1 arg2`)
- [ ] Create command registry
- [ ] Implement built-in commands (help, status, config)
- [ ] Allow platforms to register custom commands

### File Storage
Handle file uploads and attachments.

- [ ] Add local file storage with configurable path
- [ ] Optional: S3-compatible storage backend
- [ ] Implement file upload endpoints
- [ ] Add file reference tracking in messages

### Encryption at Rest
Secure sensitive data in the database.

- [ ] Encrypt API keys and tokens before storage
- [ ] Use SQLCipher for full database encryption
- [ ] Implement key derivation from master secret

---

## Suggested Implementation Order

1. **WebSocket Support** - Foundation for real-time features
2. **Claude API Integration** - Core AI functionality
3. **Conversation Memory** - Required for useful AI interactions
4. **Discord Integration** - Most common platform, good starting point
5. **Scheduled Tasks** - Automation basics
6. **Multi-User Support** - Scale beyond single user
7. **Remaining Platforms** - Telegram, Slack, WhatsApp
8. **Agent Runtime** - Advanced automation
9. **Plugin System** - Extensibility

---

## Dependencies to Add

```toml
# Phase 1
actix-web-actors = "4"      # WebSocket support
sqlx = { version = "0.7", features = ["runtime-tokio", "sqlite"] }  # Async DB

# Phase 2
reqwest = { version = "0.11", features = ["json", "stream"] }  # HTTP client
tiktoken-rs = "0.5"         # Token counting
async-stream = "0.3"        # Streaming helpers

# Phase 3
serenity = "0.12"           # Discord
teloxide = "0.12"           # Telegram
# Slack - use reqwest with Slack API directly

# Phase 4
tokio-cron-scheduler = "0.10"  # Cron scheduling

# Phase 6
tracing = "0.1"             # Structured logging
tracing-subscriber = "0.3"
tracing-actix-web = "0.7"
refinery = "0.8"            # DB migrations
actix-governor = "0.4"      # Rate limiting
```

---

## Reference Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        StarkBot                              │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │   Web UI    │  │  REST API   │  │     WebSocket       │  │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘  │
│         │                │                     │             │
│  ┌──────┴─────────────────┴─────────────────────┴──────────┐ │
│  │                    Router / Auth                        │ │
│  └──────┬─────────────────┬─────────────────────┬──────────┘ │
│         │                 │                     │            │
│  ┌──────┴──────┐  ┌───────┴───────┐  ┌─────────┴─────────┐  │
│  │   Claude    │  │   Platforms   │  │     Scheduler     │  │
│  │   Service   │  │    Router     │  │                   │  │
│  └─────────────┘  └───────┬───────┘  └───────────────────┘  │
│                           │                                  │
│         ┌─────────────────┼─────────────────┐               │
│         │                 │                 │               │
│  ┌──────┴──────┐  ┌───────┴───────┐  ┌──────┴──────┐       │
│  │   Discord   │  │   Telegram    │  │    Slack    │       │
│  └─────────────┘  └───────────────┘  └─────────────┘       │
│                                                              │
│  ┌──────────────────────────────────────────────────────┐   │
│  │                    SQLite + Jobs                      │   │
│  └──────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```




#### Automated Droplet Spin Up


Charge users $20/mo  and a node will be auto-hosted for them via digitalocean - they dont need to mess with DO themselves at all .  Just pay and they get a droplet for themselves ... secret is provided to them or whatever. 