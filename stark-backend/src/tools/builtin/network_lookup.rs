//! Network Lookup for context bank scanning
//!
//! Loads network configuration from config/networks.ron at startup.
//! Used by context bank to detect network names in user input.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

/// Global network storage (loaded once at startup)
static NETWORKS: OnceLock<HashMap<String, NetworkInfo>> = OnceLock::new();

/// Network info loaded from config
#[derive(Debug, Clone, Deserialize)]
pub struct NetworkInfo {
    pub name: String,
    pub chain_id: u64,
    #[serde(default)]
    pub aliases: Vec<String>,
}

/// Load networks from config directory. Logs warning if config file is missing.
pub fn load_networks(config_dir: &Path) {
    let networks_path = config_dir.join("networks.ron");

    if !networks_path.exists() {
        log::warn!("[networks] Config file not found: {:?}, using defaults", networks_path);
        let mut defaults = HashMap::new();
        defaults.insert("base".to_string(), NetworkInfo {
            name: "Base".to_string(),
            chain_id: 8453,
            aliases: vec!["base mainnet".to_string()],
        });
        defaults.insert("mainnet".to_string(), NetworkInfo {
            name: "Ethereum Mainnet".to_string(),
            chain_id: 1,
            aliases: vec!["ethereum".to_string(), "eth".to_string()],
        });
        let _ = NETWORKS.set(defaults);
        return;
    }

    let content = match std::fs::read_to_string(&networks_path) {
        Ok(c) => c,
        Err(e) => {
            log::error!("[networks] Failed to read {:?}: {}", networks_path, e);
            return;
        }
    };

    let networks: HashMap<String, NetworkInfo> = match ron::from_str(&content) {
        Ok(n) => n,
        Err(e) => {
            log::error!("[networks] Failed to parse {:?}: {}", networks_path, e);
            return;
        }
    };

    log::info!(
        "[networks] Loaded {} networks from {:?}",
        networks.len(),
        networks_path
    );

    let _ = NETWORKS.set(networks);
}

/// Get all network identifiers with their names (for context bank scanning)
/// Returns a list of (identifier, display_name) pairs including aliases
pub fn get_all_network_identifiers() -> Vec<(String, String)> {
    let networks = match NETWORKS.get() {
        Some(n) => n,
        None => return Vec::new(),
    };

    let mut result = Vec::new();

    for (id, info) in networks {
        // Add the primary identifier
        result.push((id.clone(), info.name.clone()));
        
        // Add all aliases
        for alias in &info.aliases {
            result.push((alias.clone(), info.name.clone()));
        }
    }

    result
}
