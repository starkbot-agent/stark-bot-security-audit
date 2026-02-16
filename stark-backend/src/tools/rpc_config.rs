//! RPC Provider Configuration
//!
//! Loads RPC provider configurations from config/rpc_providers.ron
//! Supports x402-enabled (paid) and regular (free) RPC endpoints.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;
use strum::{Display, EnumString, AsRefStr};

/// Supported blockchain networks
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Display, EnumString, AsRefStr)]
#[strum(serialize_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum Network {
    Base,
    Mainnet,
    Polygon,
}

impl Network {
    /// Get the chain ID for this network
    pub fn chain_id(&self) -> u64 {
        match self {
            Network::Base => 8453,
            Network::Mainnet => 1,
            Network::Polygon => 137,
        }
    }

    /// Get the native currency symbol
    pub fn native_currency(&self) -> &'static str {
        match self {
            Network::Base => "ETH",
            Network::Mainnet => "ETH",
            Network::Polygon => "MATIC",
        }
    }

    /// Get the block explorer URL
    pub fn explorer_url(&self) -> &'static str {
        match self {
            Network::Base => "https://basescan.org",
            Network::Mainnet => "https://etherscan.io",
            Network::Polygon => "https://polygonscan.com",
        }
    }

    /// Get the USDC contract address for this network
    pub fn usdc_address(&self) -> &'static str {
        match self {
            Network::Base => "0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913",
            Network::Mainnet => "0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48",
            Network::Polygon => "0x3c499c542cEF5E3811e1192ce70d8cC03d5c3359",
        }
    }

    /// All supported networks
    pub fn all() -> &'static [Network] {
        &[Network::Base, Network::Mainnet, Network::Polygon]
    }

    /// Try to detect network from a known contract address
    pub fn from_contract_address(address: &str) -> Option<Network> {
        let addr_lower = address.to_lowercase();
        match addr_lower.as_str() {
            // USDC addresses
            "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913" => Some(Network::Base),
            "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48" => Some(Network::Mainnet),
            "0x3c499c542cef5e3811e1192ce70d8cc03d5c3359" => Some(Network::Polygon),
            _ => None,
        }
    }
}

impl Default for Network {
    fn default() -> Self {
        Network::Base
    }
}

/// Global storage for RPC providers
static RPC_PROVIDERS: OnceLock<HashMap<String, RpcProvider>> = OnceLock::new();

/// RPC Provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RpcProvider {
    /// Display name for UI (e.g., "DeFi Relay (x402)")
    pub display_name: String,
    /// Description of the provider
    pub description: String,
    /// Whether this provider uses x402 payment protocol
    pub x402: bool,
    /// Network -> URL mapping (e.g., "base" -> "https://...")
    pub endpoints: HashMap<String, String>,
}

impl RpcProvider {
    /// Get the endpoint URL for a specific network
    pub fn get_endpoint(&self, network: &str) -> Option<&String> {
        self.endpoints.get(network)
    }

    /// Get list of supported networks
    pub fn supported_networks(&self) -> Vec<&String> {
        self.endpoints.keys().collect()
    }
}

/// Load RPC providers from config directory
pub fn load_rpc_providers(config_dir: &Path) {
    let config_path = config_dir.join("rpc_providers.ron");

    let providers = if config_path.exists() {
        match std::fs::read_to_string(&config_path) {
            Ok(content) => match ron::from_str::<HashMap<String, RpcProvider>>(&content) {
                Ok(providers) => {
                    log::info!(
                        "Loaded {} RPC providers from config: {:?}",
                        providers.len(),
                        providers.keys().collect::<Vec<_>>()
                    );
                    providers
                }
                Err(e) => {
                    log::error!("Failed to parse rpc_providers.ron: {}", e);
                    default_providers()
                }
            },
            Err(e) => {
                log::error!("Failed to read rpc_providers.ron: {}", e);
                default_providers()
            }
        }
    } else {
        log::info!("No rpc_providers.ron found, using defaults");
        default_providers()
    };

    if RPC_PROVIDERS.set(providers).is_err() {
        log::warn!("RPC providers already initialized");
    }
}

/// Get default providers (DeFi Relay)
fn default_providers() -> HashMap<String, RpcProvider> {
    let mut providers = HashMap::new();
    let mut endpoints = HashMap::new();
    endpoints.insert(
        "base".to_string(),
        "https://rpc.defirelay.com/rpc/light/base".to_string(),
    );
    endpoints.insert(
        "mainnet".to_string(),
        "https://rpc.defirelay.com/rpc/light/mainnet".to_string(),
    );
    endpoints.insert(
        "polygon".to_string(),
        "https://rpc.defirelay.com/rpc/light/polygon".to_string(),
    );

    providers.insert(
        "defirelay".to_string(),
        RpcProvider {
            display_name: "DeFi Relay (x402)".to_string(),
            description: "Paid RPC via x402 payment protocol".to_string(),
            x402: true,
            endpoints,
        },
    );

    providers
}

/// Get a specific RPC provider by name
pub fn get_rpc_provider(name: &str) -> Option<RpcProvider> {
    RPC_PROVIDERS
        .get()
        .and_then(|providers| providers.get(name).cloned())
}

/// List all available RPC providers
pub fn list_rpc_providers() -> Vec<(String, RpcProvider)> {
    RPC_PROVIDERS
        .get()
        .map(|providers| {
            providers
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default()
}

/// Get the endpoint URL for a provider and network
/// Returns (url, use_x402) tuple
pub fn get_rpc_endpoint(provider_name: &str, network: &str) -> Option<(String, bool)> {
    get_rpc_provider(provider_name).and_then(|provider| {
        provider
            .get_endpoint(network)
            .map(|url| (url.clone(), provider.x402))
    })
}

/// Resolve RPC configuration from settings
/// If custom endpoints are provided, uses those (no x402)
/// Otherwise falls back to the configured provider
pub fn resolve_rpc_config(
    provider_name: &str,
    custom_endpoints: Option<&HashMap<String, String>>,
    network: &str,
) -> Option<(String, bool)> {
    // Custom endpoints take precedence (no x402)
    if let Some(endpoints) = custom_endpoints {
        if let Some(url) = endpoints.get(network) {
            if !url.is_empty() {
                return Some((url.clone(), false));
            }
        }
    }

    // Fall back to provider
    get_rpc_endpoint(provider_name, network)
}

/// Resolved RPC configuration ready for use
#[derive(Debug, Clone)]
pub struct ResolvedRpcConfig {
    pub url: String,
    pub use_x402: bool,
}

/// Resolve RPC configuration using default provider
/// Used when tool context is not available (e.g., gateway RPC methods)
pub fn resolve_rpc_from_network(network: &str) -> ResolvedRpcConfig {
    match resolve_rpc_config("defirelay", None, network) {
        Some((url, use_x402)) => {
            log::info!(
                "[rpc_config] Resolved RPC for {} (default provider): {} (x402={})",
                network,
                url,
                use_x402
            );
            ResolvedRpcConfig { url, use_x402 }
        }
        None => {
            let url = format!("https://rpc.defirelay.com/rpc/light/{}", network);
            log::info!(
                "[rpc_config] Using fallback RPC for {}: {} (x402=true)",
                network,
                url
            );
            ResolvedRpcConfig { url, use_x402: true }
        }
    }
}

/// Extract and resolve RPC configuration from ToolContext.extra
/// This is the canonical way to get RPC config in any tool.
pub fn resolve_rpc_from_context(
    extra: &HashMap<String, serde_json::Value>,
    network: &str,
) -> ResolvedRpcConfig {
    let rpc_provider = extra
        .get("rpc_provider")
        .and_then(|v| v.as_str())
        .unwrap_or("defirelay");

    let custom_endpoints: Option<HashMap<String, String>> = extra
        .get("custom_rpc_endpoints")
        .and_then(|v| serde_json::from_value(v.clone()).ok());

    match resolve_rpc_config(rpc_provider, custom_endpoints.as_ref(), network) {
        Some((url, use_x402)) => {
            log::info!(
                "[rpc_config] Resolved RPC for {}: {} (x402={})",
                network,
                url,
                use_x402
            );
            ResolvedRpcConfig { url, use_x402 }
        }
        None => {
            // Fallback to default defirelay URL
            let url = format!("https://rpc.defirelay.com/rpc/light/{}", network);
            log::info!(
                "[rpc_config] Using fallback RPC for {}: {} (x402=true)",
                network,
                url
            );
            ResolvedRpcConfig { url, use_x402: true }
        }
    }
}
