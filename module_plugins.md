# Module Plugins Architecture

StarkBot uses a **microservice plugin architecture** where each module runs as its own standalone binary. Modules are decoupled from the main bot process — they can be started, stopped, updated, and scaled independently.

## How It Works

```
starkbot-monorepo/
  stark-backend/            # Main bot (connects to modules via RPC)
  stark-frontend/           # Web UI (shows module list + links to dashboards)

  modules/
    wallet-monitor-types/     # Shared types (lib crate)
    wallet-monitor-service/   # Standalone binary — port 9100

    discord-tipping-types/    # Shared types (lib crate)
    discord-tipping-service/  # Standalone binary — port 9101
```

Each module consists of **two crates**:

1. **`<module>-types`** — A tiny lib crate with shared Rust types (request/response structs, domain models). Used by both the service and the main bot's RPC client. Dependencies: `serde` only.

2. **`<module>-service`** — A standalone binary that runs an HTTP server (using `axum`). It manages its own SQLite database, background workers, and serves a self-contained web dashboard.

## What Each Service Provides

| Feature | Description |
|---------|-------------|
| **Own SQLite DB** | Each service creates and manages its own database file. No shared SQLite write lock with the main bot. |
| **JSON RPC API** | HTTP endpoints under `/rpc/...` for CRUD operations. The main bot calls these via `reqwest`. |
| **Web Dashboard** | A self-contained HTML page served at `/` with inline CSS and JavaScript. Open in a browser to see the module's data. |
| **Background Workers** | Long-running tasks (polling, monitoring) run inside the service process. |
| **Health Endpoint** | `GET /rpc/status` returns service health, uptime, and statistics. |

## Running

```bash
# Build everything (all binaries land in the same target directory)
cargo build

# Start the main bot — module services are auto-launched as child processes
cargo run -p stark-backend
```

When `stark-backend` starts, it automatically finds and spawns the sibling service binaries (`discord-tipping-service`, `wallet-monitor-service`) from the same directory. If a service is already running on its port, it's skipped.

You can also run services standalone (e.g., on a different machine):

```bash
cargo run -p discord-tipping-service
cargo run -p wallet-monitor-service
```

Set `DISABLE_MODULE_SERVICES=1` to prevent stark-backend from auto-starting services.

Services can run on any machine — configure the URL via environment variables:

| Env Var | Default | Description |
|---------|---------|-------------|
| `WALLET_MONITOR_URL` | `http://127.0.0.1:9100` | Wallet monitor service URL |
| `WALLET_MONITOR_PORT` | `9100` | Port (for the service binary) |
| `WALLET_MONITOR_DB_PATH` | `./wallet_monitor.db` | Database file path |
| `DISCORD_TIPPING_URL` | `http://127.0.0.1:9101` | Discord tipping service URL |
| `DISCORD_TIPPING_PORT` | `9101` | Port (for the service binary) |
| `DISCORD_TIPPING_DB_PATH` | `./discord_tipping.db` | Database file path |

## How the Main Bot Integrates

The main bot (`stark-backend`) uses a **Module trait** to register each module:

```rust
pub trait Module: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn default_port(&self) -> u16;
    fn service_url(&self) -> String;
    fn has_tools(&self) -> bool;
    fn has_dashboard(&self) -> bool;
    fn create_tools(&self) -> Vec<Arc<dyn Tool>>;
    fn skill_content(&self) -> Option<&'static str>;
    fn dashboard_data(&self, db: &Database) -> Option<Value>;
    fn backup_data(&self, db: &Database) -> Option<Value>;
    fn restore_data(&self, db: &Database, data: &Value) -> Result<(), String>;
}
```

Each module implementation creates an **RPC client** (e.g., `WalletMonitorClient`) that makes HTTP requests to the service. The module's `create_tools()` method injects this client into tool structs so AI tool calls go through RPC.

## Creating a New Module

1. Create `<name>-types/` with shared types and `RpcResponse<T>` wrapper
2. Create `<name>-service/` with:
   - `db.rs` — SQLite schema and CRUD
   - `routes.rs` — axum RPC handlers
   - `dashboard.rs` — self-contained HTML dashboard
   - `main.rs` — server setup, env config, worker spawning
3. Add both crates to workspace `Cargo.toml`
4. In `stark-backend`:
   - Add `<name>-types` dependency
   - Create `integrations/<name>_client.rs` (async HTTP client)
   - Create `modules/<name>.rs` implementing the `Module` trait
   - Register in `modules/registry.rs`

## Current Modules

| Module | Port | Description |
|--------|------|-------------|
| `wallet_monitor` | 9100 | Monitors ETH/Base wallets for transfers, swaps, and large trades using Alchemy APIs |
| `discord_tipping` | 9101 | Manages Discord user profiles and wallet registrations for token tipping |
