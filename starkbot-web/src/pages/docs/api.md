---
name: API Reference
---

REST API on port 8080, WebSocket gateway on port 8081. All endpoints except auth require `Authorization: Bearer <token>`.

## Authentication

StarkBot uses Sign In With Ethereum (SIWE).

### Get Challenge

```http
POST /api/auth/generate_challenge
Content-Type: application/json

{ "public_address": "0x1234..." }
```

**Response:**
```json
{ "challenge": "Sign in to StarkBot as 0x1234... at 1704067200" }
```

### Validate Signature

```http
POST /api/auth/validate_auth
Content-Type: application/json

{
  "public_address": "0x1234...",
  "challenge": "Sign in to StarkBot...",
  "signature": "0xabc..."
}
```

**Response:**
```json
{ "token": "eyJhbGciOiJIUzI1NiIs..." }
```

### Validate Token

```http
POST /api/auth/validate
Authorization: Bearer <token>
```

---

## Chat

### Send Message

```http
POST /api/chat
Authorization: Bearer <token>
Content-Type: application/json

{
  "messages": [
    { "role": "user", "content": "Hello!" }
  ],
  "session_id": "optional-uuid"
}
```

**Response:** Streamed or complete AI response.

### Stop Execution

```http
POST /api/chat/stop
Authorization: Bearer <token>
```

---

## Channels

### List Channels

```http
GET /api/channels
```

**Response:**
```json
{
  "channels": [
    {
      "id": "uuid",
      "channel_type": "telegram",
      "name": "My Bot",
      "enabled": true
    }
  ]
}
```

### Create Channel

```http
POST /api/channels
Content-Type: application/json

{
  "channel_type": "telegram",
  "name": "My Bot",
  "bot_token": "123456:ABC..."
}
```

For Slack, also include `app_token`.

### Start / Stop

```http
POST /api/channels/:id/start
POST /api/channels/:id/stop
```

### Update / Delete

```http
PUT /api/channels/:id
DELETE /api/channels/:id
```

---

## Sessions

### List Sessions

```http
GET /api/sessions
```

### Get Transcript

```http
GET /api/sessions/:id/transcript
```

### Reset Session

```http
POST /api/sessions/:id/reset
```

---

## Memories

### List Memories

```http
GET /api/memories
GET /api/memories?identity_id=xxx&memory_type=preference
```

### Search Memories

```http
POST /api/memories/search
Content-Type: application/json

{
  "identity_id": "user-uuid",
  "query": "timezone",
  "memory_types": ["preference", "fact"],
  "limit": 10
}
```

### Create / Update / Delete

```http
POST /api/memories
PUT /api/memories/:id
DELETE /api/memories/:id
```

### Merge Duplicates

```http
POST /api/memories/merge
Content-Type: application/json

{ "memory_ids": ["id1", "id2"] }
```

---

## Scheduling

### List Jobs

```http
GET /api/cron/jobs
```

### Create Job

```http
POST /api/cron/jobs
Content-Type: application/json

{
  "name": "Daily Summary",
  "schedule": "0 9 * * *",
  "message": "Generate today's summary"
}
```

### Job Actions

```http
POST /api/cron/jobs/:id/run      # Execute now
POST /api/cron/jobs/:id/pause    # Pause
POST /api/cron/jobs/:id/resume   # Resume
GET  /api/cron/jobs/:id/runs     # History
DELETE /api/cron/jobs/:id        # Delete
```

---

## Skills

### List Skills

```http
GET /api/skills
```

### Upload Skill

```http
POST /api/skills/upload
Content-Type: multipart/form-data

file: skill.md or skill.zip
```

### Enable / Disable

```http
PUT /api/skills/:name
Content-Type: application/json

{ "enabled": true }
```

### Delete

```http
DELETE /api/skills/:name
```

---

## Tools

### List Tools

```http
GET /api/tools
```

**Response:**
```json
{
  "tools": [
    {
      "name": "web_fetch",
      "description": "Fetch web content",
      "group": "web",
      "enabled": true
    }
  ]
}
```

### List Tool Groups

```http
GET /api/tool_groups
```

**Response:**
```json
{
  "groups": [
    { "key": "system", "label": "System Tools" },
    { "key": "finance", "label": "Finance/DeFi Tools" },
    { "key": "development", "label": "Development Tools" }
  ]
}
```

---

## Transaction Queue

### List Queued Transactions

```http
GET /api/tx_queue
```

### Approve Transaction

```http
POST /api/tx_queue/:id/approve
```

### Reject Transaction

```http
POST /api/tx_queue/:id/reject
```

---

## EIP-8004 (Agent Identity)

### Get Config

```http
GET /api/eip8004/config
```

### Get Identity

```http
GET /api/eip8004/identity
```

### Discover Agents

```http
GET /api/eip8004/agents
```

---

## Agent Settings

### Get / Update

```http
GET /api/agent_settings
PUT /api/agent_settings
Content-Type: application/json

{
  "endpoint": "https://api.anthropic.com/v1/messages",
  "model_archetype": "claude",
  "max_tokens": 4096,
  "secret_key": "sk-ant-..."
}
```

---

## API Keys

### List Keys

```http
GET /api/api_keys
```

**Response:**
```json
{
  "keys": [
    { "service": "anthropic", "configured": true },
    { "service": "brave_search", "configured": false }
  ]
}
```

### Add / Delete

```http
POST /api/api_keys
Content-Type: application/json

{ "service": "anthropic", "api_key": "sk-ant-..." }

DELETE /api/api_keys/:service
```

---

## WebSocket Gateway

Connect to `ws://localhost:8081` (or `wss://` in production).

### Authentication

```json
{ "jsonrpc": "2.0", "method": "auth", "params": { "token": "..." }, "id": 1 }
```

### Subscribe to Events

```json
{ "jsonrpc": "2.0", "method": "subscribe", "params": { "channel": "all" }, "id": 2 }
```

### Event Types

| Event | Payload |
|-------|---------|
| `agent.tool_call` | `{ tool, parameters }` |
| `tool.result` | `{ tool, success, result }` |
| `agent.thinking` | `{ message }` |
| `agent.subtype_changed` | `{ subtype, label, emoji }` |
| `task.defined` | `{ tasks: [...] }` |
| `task.started` | `{ task_id, description }` |
| `task.completed` | `{ task_id, summary }` |
| `tx.queued` | `{ id, to, value, network }` |
| `tx.pending` | `{ hash, to, value }` |
| `tx.confirmed` | `{ hash, status, gas_used }` |
| `x402.payment` | `{ amount, asset, pay_to }` |
| `context_bank.update` | `{ items: [...] }` |
| `register.update` | `{ key, value, source }` |
| `confirmation.required` | `{ id, action, params }` |

---

## Errors

All errors return:

```json
{ "error": "Description of what went wrong" }
```

| Status | Meaning |
|--------|---------|
| 400 | Bad request |
| 401 | Invalid or missing token |
| 404 | Resource not found |
| 500 | Server error |
