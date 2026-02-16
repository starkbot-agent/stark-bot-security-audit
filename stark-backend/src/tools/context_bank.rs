//! Context Bank - extracts and stores key terms from user input
//!
//! Scans user messages for:
//! - Ethereum wallet addresses (0x...)
//! - Token symbols from config/tokens.ron
//! - Network names from config/networks.ron
//! - Numeric values (amounts, quantities, etc.)
//! - URLs (especially GitHub URLs for repo references)
//!
//! These extracted terms are stored in the context bank and made available
//! to the agent in the system context.

use crate::tools::builtin::cryptocurrency::network_lookup::get_all_network_identifiers;
use crate::tools::builtin::cryptocurrency::token_lookup::get_all_token_symbols;
use once_cell::sync::Lazy;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::{Arc, RwLock};

/// A detected item in the context bank
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct ContextBankItem {
    /// The detected value (address, symbol, etc.)
    pub value: String,
    /// Type of the item: "eth_address", "token_symbol"
    pub item_type: String,
    /// Optional additional info (e.g., token name for symbols)
    pub label: Option<String>,
}

/// Context bank storage - thread-safe collection of detected terms
#[derive(Debug, Clone)]
pub struct ContextBank {
    inner: Arc<RwLock<HashSet<ContextBankItem>>>,
}

impl Default for ContextBank {
    fn default() -> Self {
        Self::new()
    }
}

impl ContextBank {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// Add an item to the context bank
    pub fn add(&self, item: ContextBankItem) {
        if let Ok(mut bank) = self.inner.write() {
            bank.insert(item);
        }
    }

    /// Add multiple items at once
    pub fn add_all(&self, items: Vec<ContextBankItem>) {
        if let Ok(mut bank) = self.inner.write() {
            for item in items {
                bank.insert(item);
            }
        }
    }

    /// Get all items in the context bank
    pub fn items(&self) -> Vec<ContextBankItem> {
        self.inner
            .read()
            .map(|bank| bank.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Get items formatted for display/agent context
    pub fn format_for_agent(&self) -> Option<String> {
        let items = self.items();
        if items.is_empty() {
            return None;
        }

        let mut parts = Vec::new();

        // Group by type
        let addresses: Vec<_> = items.iter().filter(|i| i.item_type == "eth_address").collect();
        let tokens: Vec<_> = items.iter().filter(|i| i.item_type == "token_symbol").collect();
        let networks: Vec<_> = items.iter().filter(|i| i.item_type == "network").collect();
        let urls: Vec<_> = items.iter().filter(|i| i.item_type == "url" || i.item_type == "github_url").collect();

        // URLs first - they're often the primary focus of the request
        if !urls.is_empty() {
            let url_list: Vec<_> = urls
                .iter()
                .map(|u| {
                    if let Some(ref label) = u.label {
                        format!("{} ({})", u.value, label)
                    } else {
                        u.value.clone()
                    }
                })
                .collect();
            parts.push(format!("URLs: {}", url_list.join(", ")));
        }

        if !addresses.is_empty() {
            let addr_list: Vec<_> = addresses.iter().map(|a| a.value.as_str()).collect();
            parts.push(format!("Addresses: {}", addr_list.join(", ")));
        }

        if !tokens.is_empty() {
            let token_list: Vec<_> = tokens
                .iter()
                .map(|t| {
                    if let Some(ref label) = t.label {
                        format!("{} ({})", t.value, label)
                    } else {
                        t.value.clone()
                    }
                })
                .collect();
            parts.push(format!("Tokens: {}", token_list.join(", ")));
        }

        if !networks.is_empty() {
            let network_list: Vec<_> = networks
                .iter()
                .map(|n| {
                    if let Some(ref label) = n.label {
                        format!("{} ({})", n.value, label)
                    } else {
                        n.value.clone()
                    }
                })
                .collect();
            parts.push(format!("Networks: {}", network_list.join(", ")));
        }

        let numbers: Vec<_> = items.iter().filter(|i| i.item_type == "number").collect();
        if !numbers.is_empty() {
            let number_list: Vec<_> = numbers.iter().map(|n| n.value.as_str()).collect();
            parts.push(format!("Numbers: {}", number_list.join(", ")));
        }

        if parts.is_empty() {
            None
        } else {
            Some(parts.join("; "))
        }
    }

    /// Clear the context bank
    pub fn clear(&self) {
        if let Ok(mut bank) = self.inner.write() {
            bank.clear();
        }
    }

    /// Check if the context bank is empty
    pub fn is_empty(&self) -> bool {
        self.inner
            .read()
            .map(|bank| bank.is_empty())
            .unwrap_or(true)
    }

    /// Get the count of items
    pub fn len(&self) -> usize {
        self.inner
            .read()
            .map(|bank| bank.len())
            .unwrap_or(0)
    }

    /// Convert to JSON for frontend
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "items": self.items(),
            "count": self.len(),
            "formatted": self.format_for_agent()
        })
    }
}

// Pre-compiled regexes for scan_input — compiled once, used on every dispatch
static ETH_ADDR_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"0x[a-fA-F0-9]{40}").unwrap());
static URL_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"https?://[^\s<>\[\]()]+[^\s<>\[\]().,;:!?]").unwrap());
static GITHUB_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"github\.com/([^/\s]+)/([^/\s?#]+)").unwrap());
static NUMBER_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?i)\b(\d{1,3}(?:,\d{3})*|\d+)(?:\.(\d+))?(k|m|b|mil|million|billion|bil|thousand)?\b").unwrap());

/// Pre-compiled token/network matchers — built once from config data
static TOKEN_MATCHERS: Lazy<Vec<(Regex, String, String)>> = Lazy::new(|| {
    get_all_token_symbols()
        .into_iter()
        .filter_map(|(symbol, name)| {
            let pattern = format!(r"(?i)\b{}\b", regex::escape(&symbol));
            Regex::new(&pattern).ok().map(|re| (re, symbol, name))
        })
        .collect()
});

static NETWORK_MATCHERS: Lazy<Vec<(Regex, String, String)>> = Lazy::new(|| {
    get_all_network_identifiers()
        .into_iter()
        .filter_map(|(identifier, name)| {
            let pattern = format!(r"(?i)\b{}\b", regex::escape(&identifier));
            Regex::new(&pattern).ok().map(|re| (re, identifier, name))
        })
        .collect()
});

/// Scan input text for key terms and return detected items
pub fn scan_input(text: &str) -> Vec<ContextBankItem> {
    let mut items = Vec::new();

    // Scan for Ethereum addresses (0x followed by 40 hex chars)
    for cap in ETH_ADDR_RE.find_iter(text) {
        let addr = cap.as_str().to_string();
        items.push(ContextBankItem {
            value: addr.to_lowercase(),
            item_type: "eth_address".to_string(),
            label: None,
        });
    }

    // Scan for token symbols from config (pre-compiled matchers)
    for (re, symbol, name) in TOKEN_MATCHERS.iter() {
        if re.is_match(text) {
            items.push(ContextBankItem {
                value: symbol.to_uppercase(),
                item_type: "token_symbol".to_string(),
                label: Some(name.clone()),
            });
        }
    }

    // Scan for network names from config (pre-compiled matchers)
    for (re, identifier, name) in NETWORK_MATCHERS.iter() {
        if re.is_match(text) {
            items.push(ContextBankItem {
                value: identifier.to_lowercase(),
                item_type: "network".to_string(),
                label: Some(name.clone()),
            });
        }
    }

    // Scan for URLs (especially GitHub URLs)
    for cap in URL_RE.find_iter(text) {
        let url = cap.as_str().to_string();

        if url.contains("github.com") {
            if let Some(caps) = GITHUB_RE.captures(&url) {
                let owner = caps.get(1).map(|m| m.as_str()).unwrap_or("");
                let repo = caps.get(2).map(|m| m.as_str()).unwrap_or("");
                items.push(ContextBankItem {
                    value: url.clone(),
                    item_type: "github_url".to_string(),
                    label: Some(format!("{}/{}", owner, repo)),
                });
            } else {
                items.push(ContextBankItem {
                    value: url,
                    item_type: "github_url".to_string(),
                    label: None,
                });
            }
        } else {
            items.push(ContextBankItem {
                value: url,
                item_type: "url".to_string(),
                label: None,
            });
        }
    }

    // Scan for numeric values (integers, decimals, with optional commas and suffixes like k/m/b)
    for cap in NUMBER_RE.captures_iter(text) {
        let whole_part = cap[1].replace(',', "");
        let decimal_part = cap.get(2).map(|m| m.as_str());
        let suffix = cap.get(3).map(|m| m.as_str().to_lowercase());

        let base_num: f64 = if let Some(dec) = decimal_part {
            format!("{}.{}", whole_part, dec).parse().unwrap_or(0.0)
        } else {
            whole_part.parse().unwrap_or(0.0)
        };

        let multiplier: f64 = match suffix.as_deref() {
            Some("k" | "thousand") => 1_000.0,
            Some("m" | "mil" | "million") => 1_000_000.0,
            Some("b" | "bil" | "billion") => 1_000_000_000.0,
            _ => 1.0,
        };

        let expanded = base_num * multiplier;

        // Only capture numbers >= 1 to avoid noise from small fragments
        if expanded >= 1.0 {
            let value = if expanded.fract() == 0.0 {
                format!("{}", expanded as u64)
            } else {
                format!("{}", expanded)
            };
            items.push(ContextBankItem {
                value,
                item_type: "number".to_string(),
                label: None,
            });
        }
    }

    // Deduplicate
    let mut seen = HashSet::new();
    items.retain(|item| {
        let key = format!("{}:{}", item.item_type, item.value.to_lowercase());
        seen.insert(key)
    });

    items
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scan_eth_address() {
        let text = "Send to 0x742d35Cc6634C0532925a3b844Bc9e7595f8FdF0 please";
        let items = scan_input(text);

        assert_eq!(items.len(), 1);
        assert_eq!(items[0].item_type, "eth_address");
        assert!(items[0].value.starts_with("0x"));
    }

    #[test]
    fn test_scan_multiple_addresses() {
        let text = "From 0x742d35Cc6634C0532925a3b844Bc9e7595f8FdF0 to 0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48";
        let items = scan_input(text);

        let addresses: Vec<_> = items.iter().filter(|i| i.item_type == "eth_address").collect();
        assert_eq!(addresses.len(), 2);
    }

    #[test]
    fn test_scan_numbers() {
        let text = "Send 100000 tokens, or maybe 2500000 or even 1 token";
        let items = scan_input(text);

        let numbers: Vec<_> = items.iter().filter(|i| i.item_type == "number").collect();
        assert!(numbers.len() >= 3);
        assert!(numbers.iter().any(|n| n.value == "100000"));
        assert!(numbers.iter().any(|n| n.value == "2500000"));
        assert!(numbers.iter().any(|n| n.value == "1"));
    }

    #[test]
    fn test_scan_numbers_with_commas() {
        let text = "Transfer 1,000,000 USDC";
        let items = scan_input(text);

        let numbers: Vec<_> = items.iter().filter(|i| i.item_type == "number").collect();
        assert!(numbers.iter().any(|n| n.value == "1000000"));
    }

    #[test]
    fn test_scan_github_url() {
        let text = "Check out https://github.com/ethereumdegen/stark-bot for the source";
        let items = scan_input(text);

        let urls: Vec<_> = items.iter().filter(|i| i.item_type == "github_url").collect();
        assert_eq!(urls.len(), 1);
        assert!(urls[0].value.contains("github.com/ethereumdegen/stark-bot"));
        assert_eq!(urls[0].label, Some("ethereumdegen/stark-bot".to_string()));
    }

    #[test]
    fn test_scan_user_exact_input() {
        // Exact user input that wasn't being detected
        let text = "the latest update commit to https://github.com/ethereumdegen/stark-bot -- does that use duality and diversity? how?";
        let items = scan_input(text);

        println!("Items found: {:?}", items);

        let urls: Vec<_> = items.iter().filter(|i| i.item_type == "github_url").collect();
        assert_eq!(urls.len(), 1, "Expected 1 GitHub URL, found: {:?}", urls);
        assert!(urls[0].value.contains("github.com/ethereumdegen/stark-bot"));
        assert_eq!(urls[0].label, Some("ethereumdegen/stark-bot".to_string()));
    }

    #[test]
    fn test_scan_with_discord_prefix() {
        // Test with Discord message prefix
        let text = "[DISCORD MESSAGE - Use discord skill for tipping/messaging. Use discord_resolve_user to resolve @mentions to addresses.]\n\nthe latest update commit to https://github.com/ethereumdegen/stark-bot -- does that use duality and diversity? how?";
        let items = scan_input(text);

        println!("Items found with Discord prefix: {:?}", items);

        let urls: Vec<_> = items.iter().filter(|i| i.item_type == "github_url").collect();
        assert_eq!(urls.len(), 1, "Expected 1 GitHub URL, found: {:?}", urls);
    }

    #[test]
    fn test_scan_generic_url() {
        let text = "Visit https://example.com/page for more info";
        let items = scan_input(text);

        let urls: Vec<_> = items.iter().filter(|i| i.item_type == "url").collect();
        assert_eq!(urls.len(), 1);
        assert!(urls[0].value.contains("example.com"));
    }

    #[test]
    fn test_scan_number_suffix_k() {
        let text = "send 10k starkbot";
        let items = scan_input(text);
        let numbers: Vec<_> = items.iter().filter(|i| i.item_type == "number").collect();
        assert!(numbers.iter().any(|n| n.value == "10000"), "Expected 10000, got: {:?}", numbers);
    }

    #[test]
    fn test_scan_number_suffix_m() {
        let text = "send 10m starkbot";
        let items = scan_input(text);
        let numbers: Vec<_> = items.iter().filter(|i| i.item_type == "number").collect();
        assert!(numbers.iter().any(|n| n.value == "10000000"), "Expected 10000000, got: {:?}", numbers);
    }

    #[test]
    fn test_scan_number_no_suffix() {
        let text = "send 10 starkbot";
        let items = scan_input(text);
        let numbers: Vec<_> = items.iter().filter(|i| i.item_type == "number").collect();
        assert!(numbers.iter().any(|n| n.value == "10"), "Expected 10, got: {:?}", numbers);
    }

    #[test]
    fn test_scan_number_suffix_decimal_k() {
        let text = "send 1.5k tokens";
        let items = scan_input(text);
        let numbers: Vec<_> = items.iter().filter(|i| i.item_type == "number").collect();
        assert!(numbers.iter().any(|n| n.value == "1500"), "Expected 1500, got: {:?}", numbers);
    }

    #[test]
    fn test_scan_number_suffix_b() {
        let text = "send 10b tokens";
        let items = scan_input(text);
        let numbers: Vec<_> = items.iter().filter(|i| i.item_type == "number").collect();
        assert!(numbers.iter().any(|n| n.value == "10000000000"), "Expected 10000000000, got: {:?}", numbers);
    }

    #[test]
    fn test_context_bank() {
        let bank = ContextBank::new();

        bank.add(ContextBankItem {
            value: "0x123".to_string(),
            item_type: "eth_address".to_string(),
            label: None,
        });

        assert_eq!(bank.len(), 1);
        assert!(!bank.is_empty());

        let formatted = bank.format_for_agent();
        assert!(formatted.is_some());
        assert!(formatted.unwrap().contains("0x123"));
    }
}
