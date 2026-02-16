# Wallet Monitor Plugin: ETH Wallet Activity Tracking & Whale Alerts

## Context

StarkBot needs the ability to monitor a list of ETH wallets, log their on-chain activity (transfers, swaps, contract interactions), and flag large trades. This sets the foundation for future copy-trading capabilities. The feature uses Alchemy Enhanced APIs (`alchemy_getAssetTransfers`) as the data source, monitoring wallets on **Ethereum Mainnet + Base**. Phase 1 is alert-only; copy trading comes later.

**This is implemented as a plugin/module** — not installed by default. Users opt-in via `manage_modules(action="install", name="wallet_monitor")`, which creates the necessary DB tables, registers tools, installs the skill, and starts the background worker.

---

## Part 1: Plugin System (Core Infrastructure)

Before building the wallet monitor itself, StarkBot needs a lightweight module/plugin system so features like this can be installed on demand rather than baked in.

### 1.1 `installed_modules` Table

Added to `sqlite.rs init()` (this IS part of core, always present):

```sql
CREATE TABLE IF NOT EXISTS installed_modules (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    module_name TEXT UNIQUE NOT NULL,
    enabled INTEGER NOT NULL DEFAULT 1,
    version TEXT NOT NULL DEFAULT '1.0.0',
    description TEXT NOT NULL,
    has_db_tables INTEGER NOT NULL DEFAULT 0,
    has_tools INTEGER NOT NULL DEFAULT 0,
    has_worker INTEGER NOT NULL DEFAULT 0,
    required_api_keys TEXT NOT NULL DEFAULT '[]',
    installed_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
)
```

### 1.2 Module Registry (Rust)

**New file:** `stark-backend/src/modules/mod.rs`
**New file:** `stark-backend/src/modules/registry.rs`

A static registry of available modules (compiled into the binary). Each module declares:
- Name, description, version
- What it provides (tables, tools, worker)
- Required API keys
- `install()` function — creates tables, returns tool constructors
- `worker()` function — returns a future to spawn

```rust
pub trait Module: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn version(&self) -> &'static str;
    fn required_api_keys(&self) -> Vec<&'static str>;
    fn has_db_tables(&self) -> bool;
    fn has_tools(&self) -> bool;
    fn has_worker(&self) -> bool;

    /// Create DB tables (idempotent, uses CREATE IF NOT EXISTS)
    fn init_tables(&self, conn: &rusqlite::Connection) -> rusqlite::Result<()>;

    /// Return tool instances to register
    fn create_tools(&self) -> Vec<Arc<dyn Tool>>;

    /// Spawn background worker (if has_worker). Returns a JoinHandle.
    fn spawn_worker(
        &self,
        db: Arc<Database>,
        broadcaster: Arc<EventBroadcaster>,
        dispatcher: Arc<MessageDispatcher>,
    ) -> Option<tokio::task::JoinHandle<()>>;
}

pub struct ModuleRegistry {
    modules: HashMap<String, Box<dyn Module>>,
}
```

Available modules are registered at compile time:
```rust
impl ModuleRegistry {
    pub fn new() -> Self {
        let mut reg = Self { modules: HashMap::new() };
        reg.register(Box::new(WalletMonitorModule));
        // Future: reg.register(Box::new(CopyTradeModule));
        reg
    }
}
```

### 1.3 `manage_modules` Tool

**New file:** `stark-backend/src/tools/builtin/core/manage_modules.rs`

Actions:
- `list` — Show all available modules + install status + whether dependencies (API keys) are met
- `install <name>` — Insert into `installed_modules`, call `init_tables()`, register tools, install skill, spawn worker
- `uninstall <name>` — Mark as uninstalled (keeps tables/data, just disables)
- `enable <name>` / `disable <name>` — Toggle without uninstalling
- `status <name>` — Worker health, last tick, error count

### 1.4 Conditional Tool Registration

In `tools/mod.rs`, modify `register_all_tools()`:

```rust
fn register_all_tools(registry: &mut ToolRegistry, module_registry: &ModuleRegistry, db: Option<&Database>) {
    // Core tools — always registered
    registry.register(Arc::new(builtin::SubagentTool::new()));
    registry.register(Arc::new(builtin::ManageModulesTool::new()));  // NEW
    // ... all existing core tools ...

    // Module tools — only if installed & enabled
    if let Some(db) = db {
        let installed = db.list_installed_modules().unwrap_or_default();
        for module_entry in &installed {
            if module_entry.enabled {
                if let Some(module) = module_registry.get(&module_entry.module_name) {
                    for tool in module.create_tools() {
                        registry.register(tool);
                    }
                }
            }
        }
    }
}
```

### 1.5 Conditional Worker Spawning

In `main.rs`, after scheduler startup:

```rust
// Spawn workers for installed & enabled modules
let installed = db.list_installed_modules().unwrap_or_default();
for entry in &installed {
    if entry.enabled {
        if let Some(module) = module_registry.get(&entry.module_name) {
            if let Some(handle) = module.spawn_worker(db.clone(), broadcaster.clone(), dispatcher.clone()) {
                log::info!("[MODULE] Started worker for: {}", entry.module_name);
            }
        }
    }
}
```

### 1.6 Module DB Operations

**New file:** `stark-backend/src/db/tables/modules.rs`

Methods on `impl Database`:
- `list_installed_modules()` → Vec<InstalledModule>
- `is_module_installed(name)` → bool
- `is_module_enabled(name)` → bool
- `install_module(name, description, version, flags)` → InstalledModule
- `uninstall_module(name)` → ()
- `set_module_enabled(name, enabled)` → ()

---

## Part 2: Wallet Monitor Plugin

Everything below is encapsulated in the `wallet_monitor` module and only activated when installed.

### Architecture

```
┌─────────────────┐     ┌──────────────────┐     ┌─────────────────┐
│  AI Agent Tools  │     │  Background      │     │  Alchemy        │
│  (CRUD/Query)    │     │  Monitor Worker  │     │  Enhanced API   │
│                  │     │  (tokio task)    │────▶│                 │
│ wallet_watchlist │     │  polls every 60s │     │ getAssetTransfers│
│ wallet_activity  │     │                  │     └─────────────────┘
│ wallet_monitor   │     │ Classifies txs,  │
│   _control       │     │ detects swaps,   │           │
└────────┬─────────┘     │ computes USD val │◀──────────┘
         │               │                  │
         ▼               │ Alerts on large  │     ┌─────────────────┐
┌─────────────────┐     │ trades via       │────▶│  DexScreener    │
│  SQLite DB      │◀────│ MessageDispatcher│     │  (USD pricing)  │
│                 │     └──────────────────┘     └─────────────────┘
│ wallet_watchlist│
│ wallet_activity │
└─────────────────┘
```

**Key design decisions:**
- **Hybrid approach**: Mechanical polling runs in pure Rust (no AI token cost per tick). Only large-trade alerts dispatch through the AI for natural-language formatting + delivery to channels.
- **Block-number cursor**: `last_checked_block` per wallet for gap-free incremental polling. If the bot is down for hours, it catches up from exactly where it left off.
- **Per-wallet thresholds**: Each watched wallet has its own `large_trade_threshold_usd`.

### 2.1 Add `ALCHEMY_API_KEY` to Core API Keys

**File:** `stark-backend/src/controllers/api_keys.rs`

This is a **core** change (not plugin-specific) since Alchemy is a fundamental blockchain provider.

```rust
#[strum(serialize = "ALCHEMY_API_KEY")]
AlchemyApiKey,
```

Add corresponding `as_str()` match arm. This makes it a first-class API key alongside Railway, GitHub, etc. — stored in DB, backed up to keystore.

### 2.2 Module Implementation

**New file:** `stark-backend/src/modules/wallet_monitor.rs`

Implements `Module` trait:
```rust
pub struct WalletMonitorModule;

impl Module for WalletMonitorModule {
    fn name(&self) -> &'static str { "wallet_monitor" }
    fn description(&self) -> &'static str { "Monitor ETH wallets for activity and whale trades (Mainnet + Base)" }
    fn version(&self) -> &'static str { "1.0.0" }
    fn required_api_keys(&self) -> Vec<&'static str> { vec!["ALCHEMY_API_KEY"] }
    fn has_db_tables(&self) -> bool { true }
    fn has_tools(&self) -> bool { true }
    fn has_worker(&self) -> bool { true }

    fn init_tables(&self, conn: &Connection) -> Result<()> { /* create wallet_watchlist + wallet_activity */ }
    fn create_tools(&self) -> Vec<Arc<dyn Tool>> { /* 3 tools */ }
    fn spawn_worker(&self, db, broadcaster, dispatcher) -> Option<JoinHandle<()>> { /* tokio::spawn monitor loop */ }
}
```

### 2.3 Database Tables (Created on Install)

**`wallet_watchlist` table:**
```sql
CREATE TABLE IF NOT EXISTS wallet_watchlist (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    address TEXT NOT NULL,
    label TEXT,
    chain TEXT NOT NULL DEFAULT 'mainnet',
    monitor_enabled INTEGER NOT NULL DEFAULT 1,
    large_trade_threshold_usd REAL NOT NULL DEFAULT 10000.0,
    copy_trade_enabled INTEGER NOT NULL DEFAULT 0,
    copy_trade_max_usd REAL,
    last_checked_block INTEGER,
    last_checked_at TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now')),
    UNIQUE(address, chain)
)
```

**`wallet_activity` table:**
```sql
CREATE TABLE IF NOT EXISTS wallet_activity (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    watchlist_id INTEGER NOT NULL,
    chain TEXT NOT NULL,
    tx_hash TEXT NOT NULL,
    block_number INTEGER NOT NULL,
    block_timestamp TEXT,
    from_address TEXT NOT NULL,
    to_address TEXT NOT NULL,
    activity_type TEXT NOT NULL,         -- 'eth_transfer', 'erc20_transfer', 'swap', 'internal'
    asset_symbol TEXT,
    asset_address TEXT,
    amount_raw TEXT,
    amount_formatted TEXT,
    usd_value REAL,
    is_large_trade INTEGER NOT NULL DEFAULT 0,
    swap_from_token TEXT,
    swap_from_amount TEXT,
    swap_to_token TEXT,
    swap_to_amount TEXT,
    raw_data TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    FOREIGN KEY (watchlist_id) REFERENCES wallet_watchlist(id) ON DELETE CASCADE,
    UNIQUE(tx_hash, watchlist_id)
)
```

**Indexes:**
```sql
CREATE INDEX IF NOT EXISTS idx_wallet_activity_watchlist ON wallet_activity(watchlist_id, block_number DESC);
CREATE INDEX IF NOT EXISTS idx_wallet_activity_large ON wallet_activity(is_large_trade, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_wallet_activity_chain ON wallet_activity(chain, block_number DESC);
```

### 2.4 DB CRUD Operations

**New file:** `stark-backend/src/db/tables/wallet_monitor.rs`

Methods on `impl Database`:
- `add_to_watchlist(address, label, chain, threshold_usd)` → WatchlistEntry
- `remove_from_watchlist(id)` → ()
- `get_watchlist_entry(id)` → WatchlistEntry
- `list_watchlist()` → Vec<WatchlistEntry>
- `list_active_watchlist()` → Vec<WatchlistEntry> (monitor_enabled=1 only)
- `update_watchlist_entry(id, fields...)` → ()
- `update_watchlist_cursor(id, block_number)` → ()
- `insert_activity(entry)` → i64 (ON CONFLICT ignore for dedup)
- `query_activity(filters)` → Vec<ActivityEntry>
- `get_recent_large_trades(limit)` → Vec<ActivityEntry>
- `get_activity_stats()` → ActivityStats

### 2.5 Alchemy Client

**New file:** `stark-backend/src/integrations/alchemy.rs`

- `AlchemyClient` struct with `api_key` and `reqwest::Client`
- `alchemy_base_url(chain, api_key)` — maps "mainnet"/"base" to Alchemy endpoints
- `get_asset_transfers(chain, address, from_block, categories)` — calls `alchemy_getAssetTransfers` with pagination
- `get_block_number(chain)` — gets latest block via `eth_blockNumber`
- Categories: `["external", "internal", "erc20"]`

### 2.6 Background Worker

**New file:** `stark-backend/src/integrations/wallet_monitor_worker.rs`

Core function: `wallet_monitor_tick(db, api_key, broadcaster, dispatcher)`

**Each tick (every 60s):**
1. Load all `monitor_enabled=1` watchlist entries
2. For each entry, call Alchemy `getAssetTransfers` from `last_checked_block + 1` to `latest`
3. Fetch both outgoing and incoming transfers
4. **Swap detection**: Group transfers by `tx_hash`. If tx has both outgoing + incoming ERC-20 transfers, classify as "swap"
5. **USD estimation**: DexScreener public API for price data (cached 60s)
6. Insert into `wallet_activity` (UNIQUE deduplicates)
7. Mark `is_large_trade = 1` if `usd_value >= threshold_usd`
8. Update `last_checked_block` cursor
9. If large trades found → dispatch alert to AI via MessageDispatcher
10. Broadcast `wallet_monitor_tick` event via EventBroadcaster

### 2.7 Tools (3 tools)

**New file:** `stark-backend/src/tools/builtin/cryptocurrency/wallet_monitor.rs`

#### `wallet_watchlist`
- **Group**: Finance | **Safety**: Standard
- **Actions**: `add`, `remove`, `list`, `update`
- **Params**: `action`, `address`, `label`, `chain` (default "mainnet"), `threshold_usd`, `id`, `notes`
- Validates address format (0x + 40 hex)

#### `wallet_activity`
- **Group**: Finance | **Safety**: ReadOnly
- **Actions**: `recent`, `large_trades`, `search`, `stats`
- **Params**: `action`, `address`, `activity_type`, `chain`, `large_only`, `limit`

#### `wallet_monitor_control`
- **Group**: Finance | **Safety**: Standard
- **Actions**: `status`, `trigger`

### 2.8 Skill

**New file:** `skills/inactive/wallet_monitor.md` (in inactive/ until installed)

Installed to DB and activated by `manage_modules install wallet_monitor`.

Tags: `[crypto, defi, monitoring, wallets, whale, alerts]`
Required tools: `[wallet_watchlist, wallet_activity, wallet_monitor_control, dexscreener, token_lookup]`

---

## User Journey

```
1. User: "install the wallet monitor module"
   → AI calls manage_modules(action="install", name="wallet_monitor")
   → Checks ALCHEMY_API_KEY exists (prompts to install if missing)
   → Creates wallet_watchlist + wallet_activity tables
   → Registers 3 tools
   → Installs skill
   → Spawns background worker
   → "Wallet Monitor installed! Add wallets with wallet_watchlist."

2. User: "watch 0xABC...123 — label it 'Whale Alpha'"
   → AI calls wallet_watchlist(action="add", address="0xABC...", label="Whale Alpha", chain="mainnet")

3. Background worker polls every 60s:
   → Fetches new transfers from Alchemy
   → Logs to wallet_activity
   → Detects $50k swap → dispatches alert through AI

4. User: "show me recent large trades"
   → AI calls wallet_activity(action="large_trades")

5. User: "disable wallet monitor"
   → AI calls manage_modules(action="disable", name="wallet_monitor")
   → Worker stops, tools hidden, data preserved
```

---

## Files to Create/Modify

### Core Plugin System (Part 1)
| Action | File | What |
|--------|------|------|
| **Modify** | `stark-backend/src/db/sqlite.rs` | Add `installed_modules` table |
| **Create** | `stark-backend/src/db/tables/modules.rs` | Module CRUD operations |
| **Modify** | `stark-backend/src/db/tables/mod.rs` | Register modules table |
| **Create** | `stark-backend/src/modules/mod.rs` | Module trait + registry |
| **Create** | `stark-backend/src/modules/registry.rs` | ModuleRegistry impl |
| **Create** | `stark-backend/src/tools/builtin/core/manage_modules.rs` | manage_modules tool |
| **Modify** | `stark-backend/src/tools/builtin/core/mod.rs` | Export ManageModulesTool |
| **Modify** | `stark-backend/src/tools/mod.rs` | Conditional tool registration |
| **Modify** | `stark-backend/src/main.rs` | Module registry init + conditional worker spawn |

### Wallet Monitor Plugin (Part 2)
| Action | File | What |
|--------|------|------|
| **Modify** | `stark-backend/src/controllers/api_keys.rs` | Add `AlchemyApiKey` to enum |
| **Create** | `stark-backend/src/modules/wallet_monitor.rs` | Module impl |
| **Create** | `stark-backend/src/db/tables/wallet_monitor.rs` | CRUD operations |
| **Modify** | `stark-backend/src/db/tables/mod.rs` | Register wallet_monitor |
| **Create** | `stark-backend/src/integrations/alchemy.rs` | Alchemy API client |
| **Create** | `stark-backend/src/integrations/wallet_monitor_worker.rs` | Background worker |
| **Modify** | `stark-backend/src/integrations/mod.rs` | Register new modules |
| **Create** | `stark-backend/src/tools/builtin/cryptocurrency/wallet_monitor.rs` | 3 tools |
| **Modify** | `stark-backend/src/tools/builtin/cryptocurrency/mod.rs` | Export tools |
| **Create** | `skills/inactive/wallet_monitor.md` | Skill definition |

## Verification

1. `cargo build` — everything compiles
2. `manage_modules list` — shows wallet_monitor as available but not installed
3. Install Alchemy API key → `manage_modules install wallet_monitor`
4. `manage_modules status wallet_monitor` — shows running
5. Add a known active wallet to watchlist
6. Wait for or trigger a monitor tick
7. Query `wallet_activity` for logged transactions
8. Verify large trade detection and alert dispatch
9. `manage_modules disable wallet_monitor` — worker stops, tools hidden
10. `manage_modules enable wallet_monitor` — worker restarts

## Phase 2 (Future): Copy Trading

When a monitored wallet makes an interesting swap:
1. Detect the swap (token A → token B)
2. Calculate proportional amount based on bot's wallet size
3. Apply safeguards (max amount, slippage, token whitelist)
4. Execute via existing swap infrastructure
5. Implemented as a separate module that depends on `wallet_monitor`
