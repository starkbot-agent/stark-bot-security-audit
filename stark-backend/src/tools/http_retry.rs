//! HTTP retry helper with exponential backoff
//!
//! Provides a centralized mechanism for handling transient HTTP errors with
//! exponential backoff. When an HTTP tool encounters a retryable error (timeout,
//! 502, 503, 504, connection errors), it should use this helper to determine
//! the appropriate backoff delay.

use std::collections::HashMap;
use std::sync::RwLock;
use std::time::{Duration, Instant};

/// Minimum backoff delay in seconds
const MIN_BACKOFF_SECS: u64 = 5;
/// Maximum backoff delay in seconds
const MAX_BACKOFF_SECS: u64 = 60;
/// Time after which to reset backoff if no errors occur
const RESET_AFTER_SUCCESS_SECS: u64 = 120;

/// Backoff state for a single endpoint/tool
#[derive(Debug, Clone)]
struct BackoffState {
    /// Current backoff delay in seconds
    current_delay: u64,
    /// When the last error occurred
    last_error_at: Instant,
    /// Number of consecutive errors
    error_count: u32,
}

impl Default for BackoffState {
    fn default() -> Self {
        BackoffState {
            current_delay: MIN_BACKOFF_SECS,
            last_error_at: Instant::now(),
            error_count: 0,
        }
    }
}

/// Global HTTP retry manager with per-endpoint backoff tracking
pub struct HttpRetryManager {
    /// Backoff state per endpoint key (e.g., hostname or tool name)
    states: RwLock<HashMap<String, BackoffState>>,
}

impl HttpRetryManager {
    pub fn new() -> Self {
        HttpRetryManager {
            states: RwLock::new(HashMap::new()),
        }
    }

    /// Get the global instance of the retry manager
    pub fn global() -> &'static HttpRetryManager {
        use std::sync::OnceLock;
        static INSTANCE: OnceLock<HttpRetryManager> = OnceLock::new();
        INSTANCE.get_or_init(HttpRetryManager::new)
    }

    /// Record a successful request, potentially resetting backoff
    pub fn record_success(&self, key: &str) {
        if let Ok(mut states) = self.states.write() {
            // Reset backoff on success
            states.remove(key);
            log::debug!("[HTTP_RETRY] Success for '{}', backoff reset", key);
        }
    }

    /// Record a failed request and get the backoff delay
    /// Returns the number of seconds to wait before retrying
    pub fn record_error(&self, key: &str) -> u64 {
        let mut states = match self.states.write() {
            Ok(s) => s,
            Err(_) => return MIN_BACKOFF_SECS,
        };

        let state = states.entry(key.to_string()).or_default();
        let now = Instant::now();

        // Check if we should reset due to time since last error
        let elapsed = now.duration_since(state.last_error_at);
        if elapsed > Duration::from_secs(RESET_AFTER_SUCCESS_SECS) {
            // Reset backoff if enough time has passed
            state.current_delay = MIN_BACKOFF_SECS;
            state.error_count = 1;
        } else {
            // Exponential backoff: double the delay, capped at max
            state.error_count += 1;
            if state.error_count > 1 {
                state.current_delay = (state.current_delay * 2).min(MAX_BACKOFF_SECS);
            }
        }

        state.last_error_at = now;
        let delay = state.current_delay;

        log::warn!(
            "[HTTP_RETRY] Error #{} for '{}', backoff: {}s",
            state.error_count,
            key,
            delay
        );

        delay
    }

    /// Get the current backoff delay for an endpoint without recording an error
    pub fn get_current_delay(&self, key: &str) -> Option<u64> {
        if let Ok(states) = self.states.read() {
            states.get(key).map(|s| s.current_delay)
        } else {
            None
        }
    }

    /// Check if an error is retryable based on HTTP status or error type
    pub fn is_retryable_error(error: &str) -> bool {
        let error_lower = error.to_lowercase();

        // Network/connection errors
        if error_lower.contains("timeout")
            || error_lower.contains("timed out")
            || error_lower.contains("connection")
            || error_lower.contains("network")
            || error_lower.contains("dns")
            || error_lower.contains("resolve")
        {
            return true;
        }

        // Gateway errors (5xx that are typically transient)
        if error_lower.contains("502")
            || error_lower.contains("bad gateway")
            || error_lower.contains("503")
            || error_lower.contains("service unavailable")
            || error_lower.contains("504")
            || error_lower.contains("gateway timeout")
            || error_lower.contains("520")
            || error_lower.contains("521")
            || error_lower.contains("522")
            || error_lower.contains("523")
            || error_lower.contains("524")
        {
            return true;
        }

        // Rate limiting
        if error_lower.contains("429")
            || error_lower.contains("too many requests")
            || error_lower.contains("rate limit")
        {
            return true;
        }

        false
    }

    /// Check if an HTTP status code indicates a retryable error
    pub fn is_retryable_status(status: u16) -> bool {
        matches!(
            status,
            408 | // Request Timeout
            429 | // Too Many Requests
            500 | // Internal Server Error (sometimes transient)
            502 | // Bad Gateway
            503 | // Service Unavailable
            504 | // Gateway Timeout
            520 | // Cloudflare - Web Server Returned an Unknown Error
            521 | // Cloudflare - Web Server Is Down
            522 | // Cloudflare - Connection Timed Out
            523 | // Cloudflare - Origin Is Unreachable
            524   // Cloudflare - A Timeout Occurred
        )
    }
}

impl Default for HttpRetryManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to check if a reqwest error is retryable
pub fn is_reqwest_error_retryable(err: &reqwest::Error) -> bool {
    err.is_timeout()
        || err.is_connect()
        || err.is_request()
        || err.status().map(|s| HttpRetryManager::is_retryable_status(s.as_u16())).unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_exponential_backoff() {
        let manager = HttpRetryManager::new();

        // First error: 5s
        assert_eq!(manager.record_error("test"), 5);

        // Second error: 10s
        assert_eq!(manager.record_error("test"), 10);

        // Third error: 20s
        assert_eq!(manager.record_error("test"), 20);

        // Fourth error: 40s
        assert_eq!(manager.record_error("test"), 40);

        // Fifth error: 60s (capped)
        assert_eq!(manager.record_error("test"), 60);

        // Sixth error: still 60s (capped)
        assert_eq!(manager.record_error("test"), 60);
    }

    #[test]
    fn test_success_resets_backoff() {
        let manager = HttpRetryManager::new();

        // Build up some backoff
        manager.record_error("test");
        manager.record_error("test");
        assert_eq!(manager.get_current_delay("test"), Some(10));

        // Success resets
        manager.record_success("test");
        assert_eq!(manager.get_current_delay("test"), None);

        // Next error starts from minimum
        assert_eq!(manager.record_error("test"), 5);
    }

    #[test]
    fn test_is_retryable_error() {
        assert!(HttpRetryManager::is_retryable_error("504 Gateway Timeout"));
        assert!(HttpRetryManager::is_retryable_error("Connection timed out"));
        assert!(HttpRetryManager::is_retryable_error("502 Bad Gateway"));
        assert!(HttpRetryManager::is_retryable_error("429 Too Many Requests"));
        assert!(!HttpRetryManager::is_retryable_error("404 Not Found"));
        assert!(!HttpRetryManager::is_retryable_error("401 Unauthorized"));
    }

    #[test]
    fn test_is_retryable_status() {
        assert!(HttpRetryManager::is_retryable_status(502));
        assert!(HttpRetryManager::is_retryable_status(503));
        assert!(HttpRetryManager::is_retryable_status(504));
        assert!(HttpRetryManager::is_retryable_status(429));
        assert!(!HttpRetryManager::is_retryable_status(404));
        assert!(!HttpRetryManager::is_retryable_status(401));
        assert!(!HttpRetryManager::is_retryable_status(200));
    }
}
