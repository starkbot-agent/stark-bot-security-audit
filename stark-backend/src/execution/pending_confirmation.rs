//! Pending confirmation tracking for tool executions
//!
//! Tracks tool calls that require user confirmation before execution.
//! Used for high-risk operations like token transfers and swaps.

use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{Duration, Instant};

/// Tools that require user confirmation before execution
pub const CONFIRMATION_REQUIRED_TOOLS: &[&str] = &[
    "web3_tx",  // All blockchain transactions
];

/// Tool name patterns that require confirmation (checked via contains)
pub const CONFIRMATION_REQUIRED_PATTERNS: &[&str] = &[
    // Add patterns here if needed, e.g., "swap", "transfer"
];

/// Timeout for pending confirmations (5 minutes)
pub const CONFIRMATION_TIMEOUT: Duration = Duration::from_secs(300);

/// A pending tool execution awaiting user confirmation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingConfirmation {
    /// Unique ID for this pending confirmation
    pub id: String,
    /// Channel ID where confirmation is needed
    pub channel_id: i64,
    /// Session ID for context
    pub session_id: i64,
    /// Tool name to execute
    pub tool_name: String,
    /// Tool call ID (for AI response)
    pub tool_call_id: String,
    /// Tool arguments
    pub arguments: Value,
    /// Human-readable description of the action
    pub description: String,
    /// When this confirmation was requested
    #[serde(skip)]
    pub requested_at: Option<Instant>,
    /// User who initiated the action
    pub user_id: String,
}

impl PendingConfirmation {
    pub fn new(
        channel_id: i64,
        session_id: i64,
        tool_name: String,
        tool_call_id: String,
        arguments: Value,
        user_id: String,
    ) -> Self {
        let description = Self::build_description(&tool_name, &arguments);
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            channel_id,
            session_id,
            tool_name,
            tool_call_id,
            arguments,
            description,
            requested_at: Some(Instant::now()),
            user_id,
        }
    }

    /// Build a human-readable description of the pending action
    fn build_description(tool_name: &str, arguments: &Value) -> String {
        match tool_name {
            "web3_tx" => {
                let to = arguments.get("to").and_then(|v| v.as_str()).unwrap_or("unknown");
                let value = arguments.get("value").and_then(|v| v.as_str()).unwrap_or("0");
                let data = arguments.get("data").and_then(|v| v.as_str()).unwrap_or("0x");
                let network = arguments.get("network").and_then(|v| v.as_str()).unwrap_or("base");

                // Try to detect what kind of transaction this is
                if data == "0x" || data.is_empty() {
                    // Simple ETH transfer
                    let eth_value = Self::wei_to_eth(value);
                    format!(
                        "Transfer {} ETH to {} on {}",
                        eth_value, Self::short_address(to), network
                    )
                } else if data.starts_with("0xa9059cbb") {
                    // ERC20 transfer
                    format!(
                        "ERC20 transfer to contract {} on {}",
                        Self::short_address(to), network
                    )
                } else if data.starts_with("0x095ea7b3") {
                    // ERC20 approve
                    format!(
                        "Approve token spending on contract {} ({})",
                        Self::short_address(to), network
                    )
                } else {
                    // Generic contract call
                    let eth_value = Self::wei_to_eth(value);
                    if eth_value != "0" {
                        format!(
                            "Contract call to {} with {} ETH on {}",
                            Self::short_address(to), eth_value, network
                        )
                    } else {
                        format!(
                            "Contract call to {} on {}",
                            Self::short_address(to), network
                        )
                    }
                }
            }
            _ => format!("Execute {} tool", tool_name),
        }
    }

    /// Convert wei string to ETH string
    fn wei_to_eth(wei: &str) -> String {
        if let Ok(wei_num) = wei.parse::<u128>() {
            if wei_num == 0 {
                return "0".to_string();
            }
            let eth = wei_num as f64 / 1e18;
            if eth < 0.0001 {
                format!("{:.8}", eth)
            } else {
                format!("{:.6}", eth)
            }
        } else {
            wei.to_string()
        }
    }

    /// Shorten an address for display
    fn short_address(addr: &str) -> String {
        if addr.len() > 12 {
            format!("{}...{}", &addr[..6], &addr[addr.len()-4..])
        } else {
            addr.to_string()
        }
    }

    /// Check if this confirmation has expired
    pub fn is_expired(&self) -> bool {
        self.requested_at
            .map(|t| t.elapsed() > CONFIRMATION_TIMEOUT)
            .unwrap_or(true)
    }
}

/// Manager for pending confirmations
pub struct PendingConfirmationManager {
    /// Map of channel_id -> pending confirmation
    /// Only one pending confirmation per channel at a time
    pending: DashMap<i64, PendingConfirmation>,
}

impl PendingConfirmationManager {
    pub fn new() -> Self {
        Self {
            pending: DashMap::new(),
        }
    }

    /// Check if a tool requires confirmation
    pub fn requires_confirmation(tool_name: &str) -> bool {
        // Check exact matches
        if CONFIRMATION_REQUIRED_TOOLS.contains(&tool_name) {
            return true;
        }
        // Check patterns
        for pattern in CONFIRMATION_REQUIRED_PATTERNS {
            if tool_name.contains(pattern) {
                return true;
            }
        }
        false
    }

    /// Add a pending confirmation for a channel
    /// Returns the confirmation ID
    pub fn add_pending(
        &self,
        channel_id: i64,
        session_id: i64,
        tool_name: String,
        tool_call_id: String,
        arguments: Value,
        user_id: String,
    ) -> PendingConfirmation {
        let confirmation = PendingConfirmation::new(
            channel_id,
            session_id,
            tool_name,
            tool_call_id,
            arguments,
            user_id,
        );
        let result = confirmation.clone();
        self.pending.insert(channel_id, confirmation);
        result
    }

    /// Get pending confirmation for a channel (if not expired)
    pub fn get_pending(&self, channel_id: i64) -> Option<PendingConfirmation> {
        if let Some(entry) = self.pending.get(&channel_id) {
            if !entry.is_expired() {
                return Some(entry.clone());
            } else {
                // Remove expired entry
                drop(entry);
                self.pending.remove(&channel_id);
            }
        }
        None
    }

    /// Confirm and remove a pending confirmation
    /// Returns the confirmation if it exists and is not expired
    pub fn confirm(&self, channel_id: i64) -> Option<PendingConfirmation> {
        if let Some((_, confirmation)) = self.pending.remove(&channel_id) {
            if !confirmation.is_expired() {
                return Some(confirmation);
            }
        }
        None
    }

    /// Cancel and remove a pending confirmation
    pub fn cancel(&self, channel_id: i64) -> Option<PendingConfirmation> {
        self.pending.remove(&channel_id).map(|(_, c)| c)
    }

    /// Check if a channel has a pending confirmation
    pub fn has_pending(&self, channel_id: i64) -> bool {
        self.get_pending(channel_id).is_some()
    }

    /// Clean up expired confirmations
    pub fn cleanup_expired(&self) {
        self.pending.retain(|_, v| !v.is_expired());
    }
}

impl Default for PendingConfirmationManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_requires_confirmation() {
        assert!(PendingConfirmationManager::requires_confirmation("web3_tx"));
        assert!(!PendingConfirmationManager::requires_confirmation("read_file"));
        assert!(!PendingConfirmationManager::requires_confirmation("exec"));
    }

    #[test]
    fn test_wei_to_eth() {
        assert_eq!(PendingConfirmation::wei_to_eth("0"), "0");
        assert_eq!(PendingConfirmation::wei_to_eth("1000000000000000000"), "1.000000");
        assert_eq!(PendingConfirmation::wei_to_eth("10000000000000000"), "0.010000");
    }

    #[test]
    fn test_short_address() {
        assert_eq!(
            PendingConfirmation::short_address("0x1234567890abcdef1234567890abcdef12345678"),
            "0x1234...5678"
        );
    }
}
