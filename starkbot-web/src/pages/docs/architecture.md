---
name: Architecture
---

StarkBot is a modular system with a Rust backend, React frontend, and real-time WebSocket communication. The agent uses a task planner and specialized subtypes for different workloads.

## System Overview

```
┌──────────────────────────────────────────────────────────────┐
│                    External Platforms                         │
│       Telegram        Slack        Discord        Web         │
└───────────┬─────────────┬────────────┬─────────────┬─────────┘
            │             │            │             │
            ▼             ▼            ▼             ▼
┌──────────────────────────────────────────────────────────────┐
│                     Channel Handlers                          │
│     telegram.rs    slack.rs    discord.rs    REST API         │
└───────────────────────────┬──────────────────────────────────┘
                            ▼
┌──────────────────────────────────────────────────────────────┐
│                   Message Dispatcher                          │
│                                                               │
│   1. Normalize message    4. Task planner (define_tasks)      │
│   2. Load identity        5. Agent subtype selection          │
│   3. Build AI context     6. Execute tool loop (max 100)      │
│                           7. Store history & respond          │
└───────────────────────────┬──────────────────────────────────┘
                            │
         ┌──────────────────┼──────────────────┐
         ▼                  ▼                  ▼
    ┌─────────┐       ┌──────────┐       ┌──────────┐
    │   AI    │       │  Tools   │       │  SQLite  │
    │ Client  │       │ Registry │       │ Database │
    │         │       │          │       │          │
    │ Claude  │       │ System   │       │ Sessions │
    │ OpenAI  │       │ Finance  │       │ Memories │
    │ Llama   │       │ Dev      │       │ Tx Queue │
    │ Kimi    │       │ Exec     │       │ Skills   │
    └─────────┘       └──────────┘       └──────────┘
```

---

## Backend (Rust + Actix-web)

### Entry Point

The backend initializes services in order:

1. Load environment configuration
2. Initialize SQLite database with migrations
3. Create tool registry (40+ built-in tools)
4. Create skill registry (custom extensions)
5. Start WebSocket gateway (port 8081)
6. Start HTTP server (port 8080)
7. Auto-start enabled channels

### Message Dispatcher

The core message processing engine:

```
Message → Normalize → Identity → Context → Task Planner → Subtype → AI → Tools → Memory → Response
```

| Step | Action |
|------|--------|
| **Normalize** | Convert platform message to standard format |
| **Identity** | Get or create user identity across platforms |
| **Context** | Load session history + relevant memories + context bank |
| **Task Planner** | Break down request into discrete tasks (`define_tasks`) |
| **Subtype** | Select toolbox: Finance, CodeEngineer, or Secretary |
| **AI** | Send to configured provider with tool definitions |
| **Tools** | Execute tool calls in loop (up to 100 iterations) |
| **Memory** | Extract `[REMEMBER:]` markers and store |
| **Response** | Send back to originating platform |

### Task Planner

The first iteration uses Task Planner mode to break down complex requests:

```
User: "Check my wallet balance and transfer 10 USDC to 0x123..."
        ↓
Task Planner calls define_tasks:
  1. "Check wallet balance for all tokens"
  2. "Transfer 10 USDC to address 0x123..."
        ↓
Agent executes tasks sequentially
```

### Agent Subtypes

After planning, the agent selects a specialized toolbox:

| Subtype | Tools Enabled | Use Cases |
|---------|--------------|-----------|
| **Finance** | x402_rpc, web3_tx, token_lookup, register_set | DeFi, swaps, transfers, balances |
| **CodeEngineer** | grep, glob, edit_file, git, exec | Code editing, git operations, testing |
| **Secretary** | agent_send, moltx tools, scheduling | Social media, messaging, marketing |

The agent MUST call `set_agent_subtype` before using domain-specific tools.

### AI Providers

Unified interface supporting multiple providers:

| Provider | Features |
|----------|----------|
| **Claude** | Tool calling, extended thinking, streaming |
| **OpenAI** | Tool calling, streaming, x402 payment support |
| **Llama** | Local/Ollama, custom endpoints |

### Tool Registry

Tools organized by 9 groups with access control:

| Group | Tools |
|-------|-------|
| **System** | `set_agent_subtype`, `subagent`, `ask_user`, `say_to_user`, `memory_store`, `multi_memory_search`, `modify_soul`, `task_fully_completed`, `manage_skills` |
| **Web** | `web_fetch` |
| **Filesystem** | `read_file`, `list_files` |
| **Finance** | `x402_rpc`, `x402_fetch`, `x402_post`, `web3_tx`, `broadcast_web3_tx`, `list_queued_web3_tx`, `web3_function_call`, `token_lookup`, `register_set` |
| **Development** | `write_file`, `edit_file`, `apply_patch`, `delete_file`, `rename_file`, `grep`, `glob`, `git`, `github_user`, `committer`, `deploy`, `pr_quality` |
| **Exec** | `exec` |
| **Messaging** | `agent_send`, `discord_lookup`, `twitter_post` |
| **Social** | MoltX integrations, scheduling tools |
| **Memory** | Long-term memory storage and retrieval |

### WebSocket Gateway

Real-time event broadcasting (port 8081):

| Event | Description |
|-------|-------------|
| `agent.tool_call` | Tool execution started |
| `tool.result` | Tool completed with result |
| `agent.thinking` | AI processing indicator |
| `agent.subtype_changed` | Agent switched toolbox |
| `task.defined` | Task planner created tasks |
| `task.completed` | Individual task finished |
| `tx.pending` | Blockchain transaction pending |
| `tx.confirmed` | Transaction confirmed |
| `tx.queued` | Transaction added to queue for approval |
| `x402.payment` | x402 micropayment made |
| `confirmation.required` | User approval needed |
| `context_bank.update` | Context bank extracted new terms |
| `register.update` | Register value changed |

---

## Frontend (React + TypeScript)

### Tech Stack

- **React 18** with TypeScript
- **Vite** for builds and hot-reload
- **Tailwind CSS** for styling
- **React Router** for navigation
- **Ethers.js** for wallet integration

### Page Structure

| Page | Purpose |
|------|---------|
| **Dashboard** | Stats and quick actions |
| **Agent Chat** | Real-time conversation interface |
| **Agent Settings** | AI model configuration |
| **Bot Settings** | Bot identity and personality |
| **Crypto Transactions** | Transaction queue and history |
| **Channels** | Platform connections |
| **Tools** | Browse available tools by group |
| **Skills** | Upload and manage skills |
| **Scheduling** | Cron jobs and automation |
| **API Keys** | Service credentials management |
| **Sessions** | Conversation history |
| **Memories** | Long-term storage browser |
| **Identities** | Cross-platform user tracking |
| **Files** | Workspace file browser |
| **System Files** | System configuration files |
| **Journal** | Activity logs |
| **Logs** | Live server logs |
| **Debug** | Developer debugging tools |
| **Payments** | x402 payment history |
| **EIP-8004** | On-chain identity and reputation |

### Real-Time Updates

The frontend maintains a WebSocket connection:

```typescript
useGateway({
  onToolCall: (e) => showProgress(e),
  onToolResult: (e) => updateResults(e),
  onConfirmation: (e) => promptUser(e)
});
```

---

## Data Flow

### Chat Message

```
1. User types "Search for Rust news"
        ↓
2. POST /api/chat with message + session
        ↓
3. Dispatcher normalizes and builds context
        ↓
4. AI decides to call web_search tool
        ↓
5. WebSocket broadcasts agent.tool_call
        ↓
6. Tool executes, returns results
        ↓
7. WebSocket broadcasts tool.result
        ↓
8. AI generates final response
        ↓
9. Response stored in session
        ↓
10. Response sent to user
```

### Scheduled Job

```
1. Scheduler checks every 10 seconds
        ↓
2. Job with cron "0 9 * * MON" is due
        ↓
3. Dispatcher processes like regular message
        ↓
4. Response logged to job history
        ↓
5. Next run time calculated
```

---

## Security Model

| Layer | Mechanism |
|-------|-----------|
| **Authentication** | SIWE (Sign In With Ethereum) |
| **Authorization** | Single admin wallet address |
| **API Keys** | Encrypted at rest in SQLite |
| **Tool Safety** | Dangerous commands blocklisted |
| **Session Tokens** | JWT with expiration |

---

## Database Schema

Key tables in SQLite:

| Table | Purpose |
|-------|---------|
| `auth_sessions` | JWT tokens and expiration |
| `identity_links` | Cross-platform user mapping |
| `chat_sessions` | Conversation contexts |
| `session_messages` | Message history |
| `agent_contexts` | Agent state, task queue, subtype |
| `memories` | Long-term facts and preferences |
| `external_channels` | Platform configurations |
| `external_api_keys` | Encrypted service credentials |
| `skills` | Custom skill definitions |
| `cron_jobs` | Scheduled tasks |
| `agent_settings` | AI model configuration |
| `bot_settings` | Bot identity and personality |
| `tx_queue` | Pending Web3 transactions |
| `x402_payments` | Payment history |
