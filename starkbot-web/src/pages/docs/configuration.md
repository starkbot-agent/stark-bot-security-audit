---
name: Configuration
---

Configure StarkBot through environment variables and the dashboard.

## Environment Variables

Set in `.env` or container environment. All environment variables are now optional - the system will boot with sensible defaults.

### Authentication (Optional)

| Variable | Description |
|----------|-------------|
| `LOGIN_ADMIN_PUBLIC_ADDRESS` | Ethereum address for admin login (0x...). If not set, uses a default address. |

### Server

| Variable | Default | Description |
|----------|---------|-------------|
| `PORT` | 8080 | HTTP server port |
| `GATEWAY_PORT` | 8081 | WebSocket port |
| `DATABASE_URL` | ./.db/stark.db | SQLite path |
| `RUST_LOG` | info | Log level |
| `FRONTEND_DIST_DIR` | ./stark-frontend/dist | Frontend build directory |

### Memory Features

| Variable | Default | Description |
|----------|---------|-------------|
| `STARK_MEMORY_ENABLE_AUTO_CONSOLIDATION` | false | Auto-merge similar memories |
| `STARK_MEMORY_ENABLE_CROSS_SESSION` | false | Share memories across channels |
| `STARK_MEMORY_CROSS_SESSION_LIMIT` | 5 | Max cross-session memories |
| `STARK_MEMORY_ENABLE_ENTITY_EXTRACTION` | false | Auto-extract named entities |

### Directories

| Variable | Default | Description |
|----------|---------|-------------|
| `STARK_WORKSPACE_DIR` | ./workspace | File operations directory |
| `STARK_SKILLS_DIR` | ./skills | Skills directory |

### Web3 / Finance (Optional)

| Variable | Description |
|----------|-------------|
| `BURNER_WALLET_BOT_PRIVATE_KEY` | Private key for x402 payments |

Network configuration is stored in `config/networks.ron`:

```ron
networks: {
    "ethereum": (
        chain_id: 1,
        rpc_url: "https://eth.llamarpc.com",
        explorer_url: "https://etherscan.io",
        native_symbol: "ETH",
    ),
    "base": (
        chain_id: 8453,
        rpc_url: "https://mainnet.base.org",
        explorer_url: "https://basescan.org",
        native_symbol: "ETH",
    ),
}
```

### Example .env

```bash
# Authentication (required)
LOGIN_ADMIN_PUBLIC_ADDRESS=0x1234567890abcdef...

# Server
PORT=8080
GATEWAY_PORT=8081
DATABASE_URL=./.db/stark.db
RUST_LOG=info

# Memory
STARK_MEMORY_ENABLE_AUTO_CONSOLIDATION=true
STARK_MEMORY_ENABLE_CROSS_SESSION=true
```

---

## Dashboard Configuration

### API Keys

Configure in **API Keys**:

| Service | Purpose |
|---------|---------|
| `ANTHROPIC_API_KEY` | Claude models |
| `OPENAI_API_KEY` | GPT models |
| `BRAVE_SEARCH_API_KEY` | Web search |
| `SERPAPI_API_KEY` | Web search (alternative) |
| `GITHUB_TOKEN` | GitHub operations |
| `MOLTX_API_KEY` | MoltX social integrations |
| `DISCORD_BOT_TOKEN` | Discord channel |
| `TELEGRAM_BOT_TOKEN` | Telegram channel |
| `SLACK_BOT_TOKEN` | Slack channel |
| `SLACK_APP_TOKEN` | Slack socket mode |

### Agent Settings

Configure in **Agent Settings**:

| Setting | Options |
|---------|---------|
| Provider | Claude, OpenAI, Llama |
| Model | claude-sonnet-4-20250514, gpt-4, etc. |
| Temperature | 0.0 - 1.0 |
| Max Tokens | 1024 - 8192 |

---

## Docker

### Production

```yaml
# docker-compose.yml
version: '3.8'
services:
  starkbot:
    build: .
    ports:
      - "8080:8080"
      - "8081:8081"
    volumes:
      - ./data:/app/.db
    environment:
      - LOGIN_ADMIN_PUBLIC_ADDRESS=${LOGIN_ADMIN_PUBLIC_ADDRESS}
      - PORT=8080
      - GATEWAY_PORT=8081
      - RUST_LOG=info
```

### Development

```yaml
# docker-compose.dev.yml
version: '3.8'
services:
  backend:
    build:
      context: .
      dockerfile: Dockerfile.dev
    ports:
      - "8082:8082"
      - "8081:8081"
    volumes:
      - ./stark-backend:/app/stark-backend
      - ./data:/app/.db
    environment:
      - RUST_LOG=debug

  frontend:
    build:
      context: ./stark-frontend
      dockerfile: Dockerfile.dev
    ports:
      - "8080:8080"
    depends_on:
      - backend
```

---

## Database

### Tables

| Table | Purpose |
|-------|---------|
| `auth_sessions` | Login tokens |
| `identity_links` | Cross-platform users |
| `chat_sessions` | Conversations |
| `session_messages` | Message history |
| `memories` | Long-term storage |
| `external_channels` | Platform configs |
| `external_api_keys` | Encrypted API keys |
| `agent_settings` | AI configuration |
| `skills` | Custom skills |
| `cron_jobs` | Scheduled tasks |

### Backup

```bash
cp .db/stark.db .db/stark.db.backup
```

---

## Logging

### Levels

| Level | Use |
|-------|-----|
| `error` | Errors only |
| `warn` | + Warnings |
| `info` | + General info (recommended) |
| `debug` | + Detailed debugging |
| `trace` | Very verbose |

### Format

```
2024-01-15T10:30:00Z INFO stark_backend::dispatcher - Processing message from telegram
2024-01-15T10:30:01Z DEBUG stark_backend::ai::claude - Sending to Claude API
```

---

## Reverse Proxy

### Nginx

```nginx
server {
    listen 80;
    server_name starkbot.example.com;

    location / {
        proxy_pass http://localhost:8080;
        proxy_http_version 1.1;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
    }

    location /ws {
        proxy_pass http://localhost:8081;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
    }
}
```

---

## Troubleshooting

| Issue | Solution |
|-------|----------|
| Can't connect to dashboard | Check PORT, firewall |
| WebSocket not connecting | Check GATEWAY_PORT, browser console |
| Wallet won't connect | Verify LOGIN_ADMIN_PUBLIC_ADDRESS matches |
| API key not working | Check key is correct, review logs |
| Database errors | Ensure DATABASE_URL path is writable |
