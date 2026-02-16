# How StarkBot Works

This document explains the unified message processing architecture used by StarkBot.

## Unified Message Pipeline

All message sources (web frontend, Telegram, Slack) flow through the same `MessageDispatcher`, ensuring consistent behavior across all platforms.

```
┌──────────────────────────────────────────────────────────────────────────┐
│                        UNIFIED MESSAGE PIPELINE                          │
│                                                                          │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐                │
│  │   Browser   │     │  Telegram   │     │    Slack    │                │
│  │ (Agent Chat)│     │    Bot      │     │    Bot      │                │
│  └──────┬──────┘     └──────┬──────┘     └──────┬──────┘                │
│         │                   │                   │                        │
│         ▼                   ▼                   ▼                        │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐                │
│  │ POST /chat  │     │  Telegram   │     │   Slack     │                │
│  │ Controller  │     │  Handler    │     │  Handler    │                │
│  └──────┬──────┘     └──────┬──────┘     └──────┬──────┘                │
│         │                   │                   │                        │
│         └───────────────────┼───────────────────┘                        │
│                             │                                            │
│                             ▼                                            │
│              ┌──────────────────────────────┐                            │
│              │    NormalizedMessage         │                            │
│              │  - channel_id (0 for web)    │                            │
│              │  - channel_type              │                            │
│              │  - user_id                   │                            │
│              │  - text                      │                            │
│              └──────────────┬───────────────┘                            │
│                             │                                            │
│                             ▼                                            │
│              ┌──────────────────────────────┐                            │
│              │    MessageDispatcher         │  ◄── SHARED INSTANCE       │
│              │    .dispatch(message)        │                            │
│              └──────────────┬───────────────┘                            │
│                             │                                            │
│         ┌───────────────────┼───────────────────┐                        │
│         ▼                   ▼                   ▼                        │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐                │
│  │  Identity   │     │   Session   │     │   Memory    │                │
│  │  Tracking   │     │  Management │     │   Context   │                │
│  └─────────────┘     └─────────────┘     └─────────────┘                │
│                             │                                            │
│                             ▼                                            │
│              ┌──────────────────────────────┐                            │
│              │    AI Generation with        │                            │
│              │    Tool Execution Loop       │                            │
│              │  (web_search, web_fetch,     │                            │
│              │   read_file, exec, etc.)     │                            │
│              └──────────────┬───────────────┘                            │
│                             │                                            │
│         ┌───────────────────┼───────────────────┐                        │
│         ▼                   ▼                   ▼                        │
│  ┌─────────────┐     ┌─────────────┐     ┌─────────────┐                │
│  │   Memory    │     │   Session   │     │  Gateway    │                │
│  │   Markers   │     │   Storage   │     │   Events    │                │
│  │ [REMEMBER:] │     │   (DB)      │     │ (WebSocket) │                │
│  └─────────────┘     └─────────────┘     └─────────────┘                │
│                             │                                            │
│                             ▼                                            │
│              ┌──────────────────────────────┐                            │
│              │       Response               │                            │
│              └──────────────────────────────┘                            │
└──────────────────────────────────────────────────────────────────────────┘
```

## Key Components

### NormalizedMessage

All incoming messages are converted to a `NormalizedMessage` struct:

```rust
struct NormalizedMessage {
    channel_id: i64,      // 0 for web, DB ID for Telegram/Slack
    channel_type: String, // "web", "telegram", "slack"
    chat_id: String,      // Platform-specific chat ID
    user_id: String,      // Platform-specific user ID
    user_name: String,    // Display name
    text: String,         // Message content
    message_id: Option<String>,
}
```

### MessageDispatcher

The `MessageDispatcher` is the core processing engine. It handles:

1. **Identity Management** - Creates/retrieves user identities across platforms
2. **Session Management** - Maintains conversation sessions in the database
3. **Memory Context** - Loads relevant memories for personalized responses
4. **AI Generation** - Calls the configured AI provider (Claude, OpenAI, Llama)
5. **Tool Execution** - Runs tools like web_search in a loop (up to 10 iterations)
6. **Memory Extraction** - Parses [REMEMBER:] markers from AI responses
7. **Event Broadcasting** - Emits events to WebSocket clients

### Tool Execution Loop

When the AI needs to use tools (e.g., web search), the dispatcher runs a loop:

```
1. Send message + available tools to AI
2. AI returns either:
   - Final text response → Done
   - Tool call request → Continue
3. Execute requested tool (web_search, web_fetch, etc.)
4. Send tool results back to AI
5. Repeat (max 10 iterations)
```

## Features by Platform

| Feature | Web Chat | Telegram | Slack |
|---------|----------|----------|-------|
| Tool Execution | ✓ | ✓ | ✓ |
| Session Storage (DB) | ✓ | ✓ | ✓ |
| Identity Tracking | ✓ | ✓ | ✓ |
| Memory Markers | ✓ | ✓ | ✓ |
| Gateway Events | ✓ | ✓ | ✓ |
| Conversation History | ✓ | ✓ | ✓ |

## Memory Markers

The AI can save information using special markers in its responses:

- `[DAILY_LOG: note]` - Temporary daily notes (reset each day)
- `[REMEMBER: fact]` - Long-term memory (importance 7)
- `[REMEMBER_IMPORTANT: fact]` - Critical memory (importance 9)

These markers are automatically extracted and stored in the database, then removed from the response shown to the user.

## Available Tools

Tools are registered in the `ToolRegistry` and made available based on the configured `ToolProfile`:

| Tool | Description | Group |
|------|-------------|-------|
| `web_search` | Search the web (Brave/SerpAPI) | Web |
| `web_fetch` | Fetch and parse URL content | Web |
| `read_file` | Read file contents | Filesystem |
| `write_file` | Write to files | Filesystem |
| `list_files` | List directory contents | Filesystem |
| `exec` | Execute shell commands | Exec |

### Tool Profiles

- **None** - No tools available
- **Minimal** - Web tools only
- **Standard** - Web + Filesystem (default)
- **Messaging** - Web + Filesystem + Messaging
- **Full** - All tools
- **Custom** - User-defined allow/deny lists

## Configuration Requirements

### For Web Search to Work

1. **Claude as AI Provider** - Configure in Agent Settings (only Claude supports tools currently)
2. **Search API Key** - Set one of:
   - `BRAVE_SEARCH_API_KEY` environment variable
   - `SERPAPI_API_KEY` environment variable

### Environment Variables

```bash
# Required
DATABASE_URL=./starkbot.db

# AI Provider (configure in Agent Settings or via env)
ANTHROPIC_API_KEY=your-claude-key

# Search (configure in API Keys page or via env)
BRAVE_SEARCH_API_KEY=your-brave-key
# or
SERPAPI_API_KEY=your-serpapi-key

# Optional
PORT=8080
GATEWAY_PORT=8081
```

### API Keys Management

External service API keys can be configured in two ways:

1. **API Keys Page** (Recommended) - Keys are stored securely in the database
   - Navigate to `/api-keys.html` in the web UI
   - Add Brave Search or SerpAPI keys for web search functionality
   - Keys are loaded into the ToolContext for each tool execution

2. **Environment Variables** - Fallback for development/deployment
   - Set `BRAVE_SEARCH_API_KEY` or `SERPAPI_API_KEY` in your environment
   - Database-stored keys take precedence over environment variables

## Code Structure

```
src/
├── ai/                    # AI provider implementations
│   ├── claude.rs          # Claude with tool support
│   ├── openai.rs          # OpenAI (text-only)
│   └── llama.rs           # Llama/Ollama (text-only)
├── channels/
│   ├── dispatcher.rs      # MessageDispatcher (core pipeline)
│   ├── telegram.rs        # Telegram bot handler
│   └── slack.rs           # Slack bot handler
├── controllers/
│   ├── chat.rs            # POST /api/chat (web frontend)
│   └── ...
├── tools/
│   ├── registry.rs        # ToolRegistry
│   └── builtin/           # Built-in tools
│       ├── web_search.rs
│       ├── web_fetch.rs
│       └── ...
├── gateway/               # WebSocket server
└── db/                    # SQLite database layer
```

## Example Flow

User asks: "Search the web and tell me about the 1932 Olympics"

1. **Input** → Message arrives (web/Telegram/Slack)
2. **Normalize** → Convert to `NormalizedMessage`
3. **Identity** → Get/create user identity in DB
4. **Session** → Get/create chat session in DB
5. **Context** → Load memories for this user
6. **AI Call** → Send to Claude with tools available
7. **Tool Use** → Claude requests `web_search` tool
8. **Execute** → Run web search via Brave/SerpAPI
9. **Continue** → Send results back to Claude
10. **Response** → Claude synthesizes final answer
11. **Memory** → Extract any [REMEMBER:] markers
12. **Store** → Save response to session history
13. **Broadcast** → Emit gateway event
14. **Return** → Send response to user
