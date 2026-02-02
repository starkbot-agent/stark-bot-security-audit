---
name: Getting Started
---

Get StarkBot running with Docker in under 5 minutes.

## Prerequisites

- Docker and Docker Compose
- An Ethereum wallet (MetaMask or similar)
- Node.js 18+ *(optional, for local development)*
- Rust toolchain *(optional, for local development)*

## Quick Start

### 1. Clone and Configure

```bash
git clone https://github.com/ethereumdegen/starkbot-monorepo.git
cd starkbot-monorepo
```

Create `.env` in the project root:

```bash
# Your wallet address for admin login
LOGIN_ADMIN_PUBLIC_ADDRESS=0xYourEthereumAddress

# Server ports
PORT=8080
GATEWAY_PORT=8081

# Database
DATABASE_URL=./.db/stark.db

# Logging
RUST_LOG=info
```

> **Important:** Only the wallet address in `LOGIN_ADMIN_PUBLIC_ADDRESS` can access the dashboard.

### 2. Run with Docker

```bash
docker-compose up --build
```

For development with hot-reload:

```bash
docker-compose -f docker-compose.dev.yml up --build
```

### 3. Sign In

1. Open `http://localhost:8080`
2. Click **Connect Wallet**
3. Sign the challenge message in MetaMask
4. You're in

---

## First-Time Setup

### Add API Keys

Go to **API Keys** and configure:

| Service | Purpose |
|---------|---------|
| Anthropic | Claude models |
| OpenAI | GPT models |
| Brave Search | Web search tool |

### Configure the Agent

In **Agent Settings**, select:

- **Provider** — Claude, OpenAI, Llama, or Kimi
- **Model** — claude-sonnet-4-20250514, gpt-4, etc.
- **Archetype** — Claude, OpenAI, Llama, or Kimi (determines API format)
- **Temperature** — 0.0 (precise) to 1.0 (creative)
- **Max Tokens** — Response length limit

### Connect a Channel *(Optional)*

In **Channels**, add messaging platforms:

| Platform | What You Need |
|----------|--------------|
| Telegram | Bot token from @BotFather |
| Slack | Bot token + App token |
| Discord | Bot token from Developer Portal |

### Test It

Go to **Agent Chat** and send a message:

> "Search the web for the latest Rust news and summarize it"

Watch the agent:
1. **Plan tasks** — Breaks down your request into steps
2. **Select subtype** — Chooses the right toolbox (CodeEngineer for this)
3. **Execute tools** — Calls web_fetch and other tools
4. **Report results** — Summarizes findings

The debug panel shows real-time tool execution and task progress.

---

## Local Development

### Backend

```bash
cd stark-backend
cargo run
```

Runs on port 8080 (HTTP) and 8081 (WebSocket).

### Frontend

```bash
cd stark-frontend
npm install
npm run dev
```

Vite dev server with hot-reload and API proxy.

---

## Project Structure

```
starkbot-monorepo/
├── stark-backend/           # Rust backend
│   └── src/
│       ├── main.rs          # Entry point
│       ├── channels/        # Telegram, Slack, Discord
│       ├── ai/              # AI provider clients
│       ├── tools/           # 40+ built-in tools
│       └── db/              # SQLite persistence
├── stark-frontend/          # React dashboard
│   └── src/
│       ├── pages/           # Route components
│       ├── components/      # UI components
│       └── lib/             # API client, gateway
├── starkbot-web/            # Marketing site & docs
├── docker-compose.yml       # Production
└── docker-compose.dev.yml   # Development
```

---

## Next Steps

- [Architecture](/docs/architecture) — Understand the system
- [Tools](/docs/tools) — See what the agent can do
- [Telegram](/docs/telegram) — Set up your first bot
