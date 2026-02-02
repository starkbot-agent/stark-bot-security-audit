//! Context Bank - extracts and stores key terms from user input
//!
//! Scans user messages for:
//! - Ethereum wallet addresses (0x...)
//! - Token symbols from config/tokens.ron
//! - Network names from config/networks.ron
//!
//! These extracted terms are stored in the context bank and made available
//! to the agent in the system context.

use crate::tools::builtin::network_lookup::get_all_network_identifiers;
use crate::tools::builtin::token_lookup::get_all_token_symbols;
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

/// Scan input text for key terms and return detected items
pub fn scan_input(text: &str) -> Vec<ContextBankItem> {
    let mut items = Vec::new();

    // Scan for Ethereum addresses (0x followed by 40 hex chars)
    let eth_addr_regex = Regex::new(r"0x[a-fA-F0-9]{40}").unwrap();
    for cap in eth_addr_regex.find_iter(text) {
        let addr = cap.as_str().to_string();
        // Normalize to checksummed format (lowercase for now)
        items.push(ContextBankItem {
            value: addr.to_lowercase(),
            item_type: "eth_address".to_string(),
            label: None,
        });
    }

    // Scan for token symbols from config
    let token_symbols = get_all_token_symbols();

    for (symbol, name) in token_symbols {
        // Match as whole word (surrounded by non-alphanumeric or at start/end)
        let pattern = format!(r"(?i)\b{}\b", regex::escape(&symbol));
        if let Ok(re) = Regex::new(&pattern) {
            if re.is_match(text) {
                items.push(ContextBankItem {
                    value: symbol.to_uppercase(),
                    item_type: "token_symbol".to_string(),
                    label: Some(name),
                });
            }
        }
    }

    // Scan for network names from config
    let network_identifiers = get_all_network_identifiers();

    for (identifier, name) in network_identifiers {
        // Match as whole word (case-insensitive)
        let pattern = format!(r"(?i)\b{}\b", regex::escape(&identifier));
        if let Ok(re) = Regex::new(&pattern) {
            if re.is_match(text) {
                items.push(ContextBankItem {
                    value: identifier.to_lowercase(),
                    item_type: "network".to_string(),
                    label: Some(name),
                });
            }
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
