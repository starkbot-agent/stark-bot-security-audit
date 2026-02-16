//! x402 Payment Limits — per-call maximum amounts
//!
//! Loaded from `config/x402_payment_limits.ron` at startup, then overridden
//! by any user-configured values from the database.  The global is updated
//! at runtime when the user changes limits via the API.

use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::RwLock;

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Single limit entry as stored in the RON config file.
#[derive(Debug, Clone, Deserialize)]
pub struct PaymentLimitEntry {
    /// Maximum raw-unit amount per x402 call (string to avoid u128 precision issues)
    pub max_amount: String,
    /// Token decimals (6 for USDC, 18 for most ERC-20s)
    pub decimals: u8,
    /// Human-friendly token name
    pub display_name: String,
    /// Optional contract address (e.g. USDC on Base)
    pub address: Option<String>,
}

/// Runtime representation kept in the global.
#[derive(Debug, Clone)]
pub struct PaymentLimit {
    pub max_amount: String,
    pub decimals: u8,
    pub display_name: String,
    pub address: Option<String>,
}

// ---------------------------------------------------------------------------
// Global state
// ---------------------------------------------------------------------------

static LIMITS: RwLock<Option<HashMap<String, PaymentLimit>>> = RwLock::new(None);

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Load default limits from the RON config file.
/// Called once at startup; DB overrides are applied afterwards via `set_limit`.
pub fn load_defaults(config_dir: &Path) {
    let path = config_dir.join("x402_payment_limits.ron");
    let map = if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                match ron::from_str::<HashMap<String, PaymentLimitEntry>>(&content) {
                    Ok(parsed) => {
                        log::info!(
                            "[x402_limits] Loaded {} default payment limits from RON",
                            parsed.len()
                        );
                        parsed
                            .into_iter()
                            .map(|(k, v)| {
                                (
                                    k.to_uppercase(),
                                    PaymentLimit {
                                        max_amount: v.max_amount,
                                        decimals: v.decimals,
                                        display_name: v.display_name,
                                        address: v.address,
                                    },
                                )
                            })
                            .collect()
                    }
                    Err(e) => {
                        log::error!("[x402_limits] Failed to parse RON config: {}", e);
                        builtin_defaults()
                    }
                }
            }
            Err(e) => {
                log::error!("[x402_limits] Failed to read config file: {}", e);
                builtin_defaults()
            }
        }
    } else {
        log::warn!("[x402_limits] Config file not found, using built-in defaults");
        builtin_defaults()
    };

    let mut guard = LIMITS.write().unwrap();
    *guard = Some(map);
}

/// Return all current limits (asset → PaymentLimit).
pub fn get_all_limits() -> HashMap<String, PaymentLimit> {
    let guard = LIMITS.read().unwrap();
    guard.clone().unwrap_or_default()
}

/// Return the limit for a specific asset (case-insensitive).
/// If `asset` starts with "0x" and no symbol match is found, falls back to
/// scanning all limits for a matching contract address.
pub fn get_limit(asset: &str) -> Option<PaymentLimit> {
    let guard = LIMITS.read().unwrap();
    let map = guard.as_ref()?;

    // Try direct symbol lookup first
    if let Some(limit) = map.get(&asset.to_uppercase()) {
        return Some(limit.clone());
    }

    // Fallback: if asset looks like a contract address, scan for matching address
    if asset.starts_with("0x") || asset.starts_with("0X") {
        let asset_lower = asset.to_lowercase();
        for limit in map.values() {
            if let Some(ref addr) = limit.address {
                if addr.to_lowercase() == asset_lower {
                    return Some(limit.clone());
                }
            }
        }
    }

    None
}

/// Update (or insert) a single limit at runtime.
/// Called from the API controller and from the DB-restore path.
pub fn set_limit(asset: &str, max_amount: &str, decimals: u8, display_name: &str, address: Option<&str>) {
    let mut guard = LIMITS.write().unwrap();
    let map = guard.get_or_insert_with(HashMap::new);
    map.insert(
        asset.to_uppercase(),
        PaymentLimit {
            max_amount: max_amount.to_string(),
            decimals,
            display_name: display_name.to_string(),
            address: address.map(|s| s.to_string()),
        },
    );
}

/// Remove a limit at runtime.
pub fn remove_limit(asset: &str) {
    let mut guard = LIMITS.write().unwrap();
    if let Some(map) = guard.as_mut() {
        map.remove(&asset.to_uppercase());
    }
}

/// Check whether a payment of `amount_raw` (in smallest units) for `asset`
/// would exceed the configured per-call limit.
///
/// Returns `Ok(())` if allowed, or `Err(message)` if blocked.
pub fn check_payment_limit(asset: &str, amount_raw: &str) -> Result<(), String> {
    let limit = match get_limit(asset) {
        Some(l) => l,
        None => return Err(format!(
            "x402 payment rejected: no payment limit configured for asset {}. \
             Add a limit on the Crypto Transactions page to enable payments with this token.",
            asset
        )),
    };

    let requested: u128 = amount_raw
        .parse()
        .map_err(|_| format!("Cannot parse payment amount '{}' as integer", amount_raw))?;
    let max: u128 = limit
        .max_amount
        .parse()
        .map_err(|_| format!("Cannot parse limit '{}' as integer", limit.max_amount))?;

    if requested > max {
        let req_fmt = format_amount(requested, limit.decimals);
        let max_fmt = format_amount(max, limit.decimals);
        return Err(format!(
            "x402 payment of {} {} exceeds the configured per-call limit of {} {}. \
             Adjust the limit on the Crypto Transactions page if this is intentional.",
            req_fmt, asset, max_fmt, asset
        ));
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn format_amount(raw: u128, decimals: u8) -> String {
    let divisor = 10u128.pow(decimals as u32);
    let whole = raw / divisor;
    let frac = raw % divisor;
    if frac == 0 {
        format!("{}", whole)
    } else {
        let frac_str = format!("{:0>width$}", frac, width = decimals as usize)
            .trim_end_matches('0')
            .to_string();
        format!("{}.{}", whole, frac_str)
    }
}

fn builtin_defaults() -> HashMap<String, PaymentLimit> {
    let mut map = HashMap::new();
    map.insert(
        "USDC".to_string(),
        PaymentLimit {
            max_amount: "1000000".to_string(),
            decimals: 6,
            display_name: "USDC".to_string(),
            address: Some("0x833589fCD6eDb6E08f4c7C32D4f71b54bdA02913".to_string()),
        },
    );
    map.insert(
        "STARKBOT".to_string(),
        PaymentLimit {
            max_amount: "100000000000000000000000".to_string(),
            decimals: 18,
            display_name: "STARKBOT".to_string(),
            address: Some("0x587Cd533F418825521f3A1daa7CCd1E7339a1B07".to_string()),
        },
    );
    map
}
