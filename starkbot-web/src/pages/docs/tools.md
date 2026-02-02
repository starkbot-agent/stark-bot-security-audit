---
name: Tools
---

Tools let the AI agent take actions beyond conversation. StarkBot includes 50+ built-in tools across nine groups.

## How Tools Work

```
User Message → Task Planner → Subtype Selection → Tool Calls → Results → AI Continues
```

The AI can chain up to 100 tool calls per message to complete complex tasks. Tools are organized by group and controlled by agent subtypes.

## Tool Groups

| Group | Description |
|-------|-------------|
| **System** | Core agent tools, always available |
| **Web** | HTTP requests and web fetching |
| **Filesystem** | File reading and directory listing |
| **Finance** | Crypto transactions, DeFi, token lookups |
| **Development** | Code editing, git, search utilities |
| **Exec** | Shell command execution |
| **Messaging** | Inter-agent and external messaging |
| **Social** | Social media and marketing |
| **Memory** | Long-term memory operations |

## Agent Subtypes

Before using domain-specific tools, the agent must select a subtype:

| Subtype | Groups Enabled |
|---------|---------------|
| **Finance** | System, Web, Filesystem, Finance |
| **CodeEngineer** | System, Web, Filesystem, Development, Exec |
| **Secretary** | System, Web, Filesystem, Messaging, Social |

---

## Web Tools

### web_search

Search the web using Brave Search or SerpAPI.

```json
{
  "name": "web_search",
  "parameters": {
    "query": "Rust async runtime comparison 2024"
  }
}
```

Requires: Brave Search or SerpAPI key in API Keys.

### web_fetch

Fetch and parse content from a URL.

```json
{
  "name": "web_fetch",
  "parameters": {
    "url": "https://example.com/api/data",
    "selector": ".main-content"
  }
}
```

---

## Filesystem Tools

### read_file

Read file contents.

```json
{ "name": "read_file", "parameters": { "path": "./src/main.rs" } }
```

### write_file

Create or overwrite a file.

```json
{
  "name": "write_file",
  "parameters": {
    "path": "./output.txt",
    "content": "Hello, World!"
  }
}
```

### list_files

List directory contents.

```json
{ "name": "list_files", "parameters": { "path": "./src", "recursive": true } }
```

### glob

Find files matching a pattern.

```json
{ "name": "glob", "parameters": { "pattern": "**/*.rs" } }
```

### grep

Search file contents.

```json
{ "name": "grep", "parameters": { "pattern": "TODO", "path": "./src" } }
```

### apply_patch

Apply a unified diff patch to a file.

```json
{
  "name": "apply_patch",
  "parameters": {
    "path": "./src/lib.rs",
    "patch": "@@ -1,3 +1,4 @@\n+// New comment\n fn main() {"
  }
}
```

---

## Exec Tools

### exec

Execute shell commands (requires `code_engineer` subtype).

```json
{
  "name": "exec",
  "parameters": {
    "command": "cargo build --release",
    "cwd": "./project",
    "timeout": 60000
  }
}
```

**Safety:** Dangerous commands are blocked. Shell metacharacters are restricted.

---

## Development Tools

Development tools require the `code_engineer` subtype.

### git

Git operations with built-in safety.

```json
{ "name": "git", "parameters": { "command": "status" } }
{ "name": "git", "parameters": { "command": "diff HEAD~1" } }
```

### github_user

Get the authenticated GitHub username.

```json
{ "name": "github_user", "parameters": {} }
```

### edit_file

Edit a file using search and replace.

```json
{
  "name": "edit_file",
  "parameters": {
    "path": "./src/main.rs",
    "old_string": "fn old_name",
    "new_string": "fn new_name"
  }
}
```

### committer

Create scoped git commits with proper messages.

```json
{
  "name": "committer",
  "parameters": {
    "message": "Fix authentication bug",
    "files": ["src/auth.rs", "src/lib.rs"]
  }
}
```

### deploy

Deploy to configured environments.

```json
{ "name": "deploy", "parameters": { "environment": "staging" } }
```

### pr_quality

Analyze pull request quality and suggest improvements.

```json
{ "name": "pr_quality", "parameters": { "pr_number": 123 } }
```

---

## Messaging Tools

### agent_send

Send a message to any configured channel.

```json
{
  "name": "agent_send",
  "parameters": {
    "channel_id": "discord-channel-uuid",
    "message": "Build completed successfully!"
  }
}
```

### say_to_user

Reply in the current conversation.

```json
{ "name": "say_to_user", "parameters": { "message": "Working on it..." } }
```

### ask_user

Request user confirmation before proceeding.

```json
{
  "name": "ask_user",
  "parameters": {
    "question": "Deploy to production?",
    "options": ["Yes", "No"]
  }
}
```

---

## Finance Tools

Finance tools require the `finance` subtype to be selected first.

### web3_tx

Create a blockchain transaction. Transactions are queued for user approval before broadcast.

```json
{
  "name": "web3_tx",
  "parameters": {
    "to": "0x1234...",
    "value": "0.1",
    "data": "0x...",
    "network": "base"
  }
}
```

### broadcast_web3_tx

Broadcast a queued transaction after user approval.

```json
{ "name": "broadcast_web3_tx", "parameters": { "tx_id": "tx_123" } }
```

### list_queued_web3_tx

List all transactions waiting for approval.

```json
{ "name": "list_queued_web3_tx", "parameters": {} }
```

### web3_function_call

Call a smart contract function (read-only or with transaction).

```json
{
  "name": "web3_function_call",
  "parameters": {
    "preset": "erc20_balance",
    "network": "base",
    "call_only": true
  }
}
```

### token_lookup

Get token information by symbol or address.

```json
{ "name": "token_lookup", "parameters": { "symbol": "USDC", "network": "base" } }
```

### register_set

Store values in registers for passing data between tools safely.

```json
{ "name": "register_set", "parameters": { "key": "token_address", "value": "0x..." } }
```

### x402_rpc

Make RPC calls to x402-enabled endpoints with automatic micropayments.

```json
{
  "name": "x402_rpc",
  "parameters": {
    "endpoint": "https://api.example.com/rpc",
    "method": "eth_getBalance",
    "params": ["0x..."]
  }
}
```

### x402_fetch

Fetch from a pay-per-use API with automatic USDC payment.

```json
{
  "name": "x402_fetch",
  "parameters": {
    "url": "https://api.example.com/premium",
    "method": "GET"
  }
}
```

### x402_post

POST to an x402-enabled endpoint.

```json
{
  "name": "x402_post",
  "parameters": {
    "url": "https://api.example.com/data",
    "body": { "key": "value" }
  }
}
```

---

## System Tools

System tools are always available regardless of subtype.

### set_agent_subtype

Select the agent's toolbox. **Must be called first** before using domain-specific tools.

```json
{ "name": "set_agent_subtype", "parameters": { "subtype": "finance" } }
```

Options: `finance`, `code_engineer`, `secretary`

### subagent

Spawn a background agent for parallel tasks.

```json
{
  "name": "subagent",
  "parameters": {
    "label": "Research Agent",
    "task": "Research competitor pricing",
    "timeout_secs": 300
  }
}
```

### subagent_status

Check the status of a running subagent.

```json
{ "name": "subagent_status", "parameters": { "subagent_id": "sub_123" } }
```

### ask_user

Request user input or confirmation.

```json
{
  "name": "ask_user",
  "parameters": {
    "question": "Which network should I use?",
    "options": ["Ethereum", "Base", "Arbitrum"]
  }
}
```

### say_to_user

Send an intermediate message to the user.

```json
{ "name": "say_to_user", "parameters": { "message": "Working on it..." } }
```

### task_fully_completed

Signal that the current task is complete.

```json
{ "name": "task_fully_completed", "parameters": { "summary": "Transferred 10 USDC successfully" } }
```

### manage_skills

List, enable, or disable skills.

```json
{ "name": "manage_skills", "parameters": { "action": "list" } }
```

### memory_store

Explicitly store a memory.

```json
{
  "name": "memory_store",
  "parameters": {
    "content": "User prefers TypeScript over JavaScript",
    "memory_type": "preference",
    "importance": 7
  }
}
```

### multi_memory_search

Search memories efficiently with multiple queries.

```json
{
  "name": "multi_memory_search",
  "parameters": {
    "queries": ["user preferences", "timezone", "wallet"]
  }
}
```

### modify_soul

Update the agent's personality or instructions.

```json
{
  "name": "modify_soul",
  "parameters": {
    "instruction": "Always respond in bullet points"
  }
}
```

### api_keys_check

Check which API keys are configured.

```json
{ "name": "api_keys_check", "parameters": {} }
```

---

## Tool Profiles

Tool profiles provide preset configurations:

| Profile | Groups Enabled |
|---------|----------------|
| **Minimal** | Web only |
| **Standard** | Web, Filesystem, Exec |
| **Messaging** | Web, Filesystem, Exec, Messaging |
| **Finance** | Web, Filesystem, Finance, System |
| **Developer** | Web, Filesystem, Exec, Development, System |
| **Secretary** | Web, Filesystem, Exec, Messaging, Social, System |
| **Full** | All tool groups |

## Transaction Queue

Web3 transactions go through a queue system:

1. Agent creates transaction with `web3_tx`
2. Transaction added to queue (`tx.queued` event)
3. User reviews in **Crypto Transactions** page
4. User approves or rejects
5. On approval, agent calls `broadcast_web3_tx`
6. Transaction broadcast to network (`tx.pending` event)
7. Confirmation received (`tx.confirmed` event)

This prevents unauthorized transactions and gives users full control.

---

## Real-Time Events

Tool execution broadcasts WebSocket events:

```json
// Started
{ "type": "agent.tool_call", "tool": "web_search", "parameters": { "query": "..." } }

// Completed
{ "type": "tool.result", "tool": "web_search", "success": true, "result": "..." }
```

The dashboard shows these in real-time as the agent works.

---

## Custom Tools

Extend capabilities through the [Skills](/docs/skills) system. Skills can combine multiple tools into reusable workflows.
