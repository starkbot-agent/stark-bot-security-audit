# StarkBot Module Distribution System — Full Plan

## Context

StarkBot already has a solid module architecture: a `Module` trait, `ModuleRegistry`, microservice-based modules (wallet_monitor, discord_tipping) communicating via HTTP RPC, tool registration, and a database tracking installed/enabled modules. StarkHub (hub.starkbot.ai) already serves as a skills marketplace with auth, payments, and install tracking.

**The gap:** Modules are currently compiled into the starkbot binary. There's no way to publish, discover, download, and install a module without modifying source code and recompiling. This plan bridges that gap with a dynamic module system backed by pre-compiled binaries and a StarkHub module registry.

---

## Architecture Overview

```
┌─────────────────┐         ┌──────────────────┐
│   Module Author  │         │    StarkHub       │
│                  │ publish  │  hub.starkbot.ai  │
│ module.toml      ├────────►│                   │
│ bin/service      │         │  /api/modules/... │
│ skill.md         │         └────────┬──────────┘
└─────────────────┘                   │ download
                                      ▼
                            ┌──────────────────┐
                            │    StarkBot       │
                            │                   │
                            │ ~/.starkbot/modules/
                            │   wallet_monitor/ │
                            │     module.toml   │
                            │     bin/service   │
                            │     skill.md      │
                            │                   │
                            │ DynamicModule     │
                            │ DynamicModuleTool │
                            │   ──► HTTP RPC ──►│ service:9100
                            └──────────────────┘
```

---

## Phase 1: Module Manifest + Dynamic Bridge (starkbot-monorepo)

### 1.1 Define `module.toml` Manifest Format

The manifest is the single source of truth — everything starkbot needs to load a module without compiling module-specific code.

```toml
[module]
name = "wallet_monitor"
version = "1.1.0"
author = "@ethereumdegen"
description = "Monitor ETH wallets for on-chain activity and whale trades"
license = "MIT"

[service]
default_port = 9100
port_env_var = "WALLET_MONITOR_PORT"
url_env_var = "WALLET_MONITOR_URL"
has_dashboard = true
health_endpoint = "/rpc/status"

[service.env_vars]
ALCHEMY_API_KEY = { required = true, description = "Alchemy API key for blockchain queries" }
ALERT_CALLBACK_URL = { required = false, description = "Webhook URL for alerts" }

[skill]
content_file = "skill.md"

[platforms]
supported = ["linux-x86_64", "linux-aarch64", "darwin-x86_64", "darwin-aarch64"]

[[tools]]
name = "wallet_watchlist"
description = "Manage the wallet watchlist for monitoring on-chain activity"
group = "finance"
rpc_method = "POST"
rpc_endpoint = "/rpc/watchlist"

[tools.parameters.action]
type = "string"
description = "Action to perform"
required = true
enum = ["add", "remove", "list", "update"]

[tools.parameters.address]
type = "string"
description = "Ethereum wallet address"
required = false

# ... more params per tool
```

### 1.2 New Files in `stark-backend/src/modules/`

| File | Purpose |
|------|---------|
| `manifest.rs` | Parse `module.toml` into `ModuleManifest` struct using `toml` crate |
| `dynamic_tool.rs` | `DynamicModuleTool` — implements `Tool` trait, proxies calls via HTTP to service RPC endpoints |
| `dynamic_module.rs` | `DynamicModule` — implements `Module` trait using manifest data |
| `loader.rs` | Scans `~/.starkbot/modules/` dirs, loads manifests, returns `Vec<DynamicModule>` |

### 1.3 Key Design: `DynamicModuleTool`

```
AI calls tool "wallet_watchlist" with params { "action": "add", "address": "0x..." }
    → DynamicModuleTool.call()
    → POST http://127.0.0.1:9100/rpc/watchlist  body: { "action": "add", "address": "0x..." }
    → service responds with RpcResponse<Value>
    → DynamicModuleTool returns ToolResult
```

This is exactly what the current typed tools (WalletWatchlistTool) already do, just generalized — `serde_json::Value` instead of typed request/response structs.

### 1.4 Module Trait Change

Change `Module` trait methods from `&'static str` to `&str`:
- `fn name(&self) -> &str`
- `fn description(&self) -> &str`
- `fn version(&self) -> &str`

Trivial change to existing WalletMonitorModule and DiscordTippingModule impls.

### 1.5 ModuleRegistry Extension

`ModuleRegistry::new()` calls `loader::load_dynamic_modules()` and registers them alongside built-in modules. Dynamic modules get the same treatment: tools registered if enabled, services started as child processes.

### 1.6 Service Process Management

Extend `start_module_services()` to find and spawn binaries at `~/.starkbot/modules/<name>/bin/<name>-service`, checking health endpoint before spawning.

### 1.7 Installed Modules DB Extension

Add columns to `installed_modules`:
- `source TEXT DEFAULT 'builtin'` — `'builtin'` or `'starkhub'`
- `manifest_path TEXT` — path to module.toml
- `binary_path TEXT` — path to service binary
- `author TEXT` — @username from StarkHub
- `sha256_checksum TEXT`

---

## Phase 2: StarkHub Module Registry (starkhub-monorepo)

### 2.1 Database Migration (`migrations/007_modules.sql`)

New tables:
- **`modules`** — slug, name, description, version, author_id, manifest (JSONB), tools_provided, install_count, featured, status
- **`module_binaries`** — module_id, version, platform, file_path (S3 key), file_size, sha256_checksum
- **`module_versions`** — module_id, version, changelog, manifest
- **`module_install_events`** — module_id, installer_address, platform

### 2.2 Backend Files

| File | Purpose |
|------|---------|
| `models/module.rs` | Module, ModuleSummary, ModuleBinary structs |
| `services/module.rs` | CRUD, binary upload/download, search |
| `controllers/modules.rs` | REST endpoints (see below) |

### 2.3 API Endpoints

| Method | Path | Description |
|--------|------|-------------|
| `GET` | `/api/modules` | List/browse modules (paginated, filterable) |
| `GET` | `/api/modules/@{user}/{slug}` | Module detail |
| `GET` | `/api/modules/search?q=...` | Search modules |
| `POST` | `/api/modules` | Publish new module (auth required) |
| `PUT` | `/api/modules/@{user}/{slug}` | Update module metadata |
| `POST` | `/api/modules/@{user}/{slug}/binaries` | Upload platform binary (multipart) |
| `GET` | `/api/modules/@{user}/{slug}/download/{platform}` | Download binary archive |
| `GET` | `/api/modules/trending` | Top modules by install count |
| `GET` | `/api/modules/featured` | Featured modules |

### 2.4 Binary Storage

S3-compatible storage (R2, MinIO, or AWS S3):
- Key format: `modules/{author}/{slug}/{version}/{platform}.tar.gz`
- Archive contains: `module.toml`, `bin/<service-binary>`, `skill.md` (optional)
- SHA256 checksum stored in DB, verified on download

### 2.5 Frontend Pages

| Page | Description |
|------|-------------|
| `ModuleBrowse.tsx` | Grid of modules, filter by tag, sort by installs/date |
| `ModuleDetail.tsx` | Module info, tools list, install command, download links |
| `ModuleSubmit.tsx` | Publish form: upload module.toml + binaries |

Add "Modules" navigation tab alongside "Skills".

---

## Phase 3: StarkBot ↔ StarkHub Integration

### 3.1 StarkHub Client (`stark-backend/src/integrations/starkhub_client.rs`)

HTTP client for StarkHub API: `search_modules()`, `get_module()`, `download_module()`, `publish_module()`.

### 3.2 Extended `manage_modules` Tool

New actions for the existing manage modules tool:
- `"search_hub"` — search StarkHub for modules
- `"install_remote"` — download from StarkHub, extract to `~/.starkbot/modules/`, register dynamically
- `"publish"` — package local module and upload to StarkHub
- `"update"` — check for newer version, download and replace

### 3.3 Install Flow

1. `GET /api/modules/@author/slug/download/{platform}` — download `.tar.gz`
2. Verify SHA256 checksum
3. Extract to `~/.starkbot/modules/<name>/`
4. Parse `module.toml`, create `DynamicModule`
5. Register in `ModuleRegistry` + insert into `installed_modules` DB
6. Register tools in `ToolRegistry`
7. Start service binary
8. Install skill if present

---

## Phase 4: Build & Packaging for Module Authors

### Package Format

Archive: `{slug}-{version}-{platform}.tar.gz`
```
module.toml
bin/{slug}-service
skill.md          (optional)
README.md         (optional)
```

Platforms: `linux-x86_64`, `linux-aarch64`, `darwin-x86_64`, `darwin-aarch64`

### Security

- SHA256 checksums generated at upload, verified at download
- Future: Ed25519 signatures, sandboxed execution

---

## Implementation Order

1. **Phase 1** first — gets dynamic loading working locally (can test by manually placing modules)
2. **Phase 2** in parallel — StarkHub backend + frontend for module registry
3. **Phase 3** last — wires starkbot to StarkHub for the full install/publish flow

---

## Key Files to Modify

**starkbot-monorepo:**
- `stark-backend/src/modules/mod.rs` — trait change + new submodules
- `stark-backend/src/modules/registry.rs` — load dynamic modules
- `stark-backend/src/modules/wallet_monitor.rs` — adapt to new trait signatures
- `stark-backend/src/modules/discord_tipping.rs` — adapt to new trait signatures
- `stark-backend/src/main.rs` — start dynamic module services
- `stark-backend/src/db/tables/modules.rs` — new columns
- `stark-backend/src/tools/builtin/core/manage_modules.rs` — new remote actions
- NEW: `stark-backend/src/modules/{manifest,dynamic_tool,dynamic_module,loader}.rs`
- NEW: `stark-backend/src/integrations/starkhub_client.rs`

**starkhub-monorepo:**
- NEW: `migrations/007_modules.sql`
- NEW: `hub-backend/src/{models,services,controllers}/module.rs`
- NEW: `hub-frontend/src/pages/{ModuleBrowse,ModuleDetail,ModuleSubmit}.tsx`
- `hub-frontend/src/App.tsx` — add module routes
- `hub-backend/src/main.rs` — add module routes

## Verification

1. Create a `module.toml` for wallet_monitor, place it with the binary in `~/.starkbot/modules/wallet_monitor/`
2. Start starkbot — verify wallet_monitor loads dynamically, tools register, service starts
3. Publish wallet_monitor to StarkHub via API
4. On a fresh starkbot, run `install_remote @ethereumdegen/wallet-monitor`
5. Verify binary downloaded, service started, tools available to AI
