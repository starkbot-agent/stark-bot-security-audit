//! Shared utilities for channel implementations.

/// Split a message into chunks respecting a platform's character limit.
/// Splits on line boundaries; lines exceeding `max_len` are hard-split.
pub fn split_message(text: &str, max_len: usize) -> Vec<String> {
    if text.len() <= max_len {
        return vec![text.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();

    for line in text.lines() {
        if current.len() + line.len() + 1 > max_len {
            if !current.is_empty() {
                chunks.push(current);
                current = String::new();
            }
            if line.len() > max_len {
                let mut remaining = line;
                while remaining.len() > max_len {
                    chunks.push(remaining[..max_len].to_string());
                    remaining = &remaining[max_len..];
                }
                if !remaining.is_empty() {
                    current = remaining.to_string();
                }
            } else {
                current = line.to_string();
            }
        } else {
            if !current.is_empty() {
                current.push('\n');
            }
            current.push_str(line);
        }
    }

    if !current.is_empty() {
        chunks.push(current);
    }

    chunks
}

/// Parse "Retry after Xs" from a platform API error string.
/// Returns the number of seconds to wait, or None if not a rate-limit error.
pub fn parse_retry_after(err: &str) -> Option<u64> {
    let lower = err.to_lowercase();
    if let Some(pos) = lower.find("retry after ") {
        let after = &err[pos + 12..];
        let digits: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        digits.parse::<u64>().ok()
    } else {
        None
    }
}

/// Tracks throttle and rate-limit state for status message updates.
///
/// Used by channel event handlers (Telegram, Discord) in `MinimalThrottled` mode
/// to avoid hitting platform rate limits during long tool loops.
pub struct StatusThrottler {
    /// Minimum interval between status message edits
    pub interval: std::time::Duration,
    /// When the last successful status edit was sent
    last_edit: tokio::time::Instant,
    /// If rate-limited, don't attempt status messages until this time
    rate_limited_until: Option<tokio::time::Instant>,
}

impl StatusThrottler {
    /// Create a new throttler with the given minimum interval between edits.
    pub fn new(interval: std::time::Duration) -> Self {
        Self {
            interval,
            // Allow the first edit immediately
            last_edit: tokio::time::Instant::now() - interval - std::time::Duration::from_secs(1),
            rate_limited_until: None,
        }
    }

    /// Default throttler: 3-second interval (good for Telegram/Discord)
    pub fn default_for_gateway() -> Self {
        Self::new(std::time::Duration::from_secs(3))
    }

    /// Returns true if a status message edit should be attempted now.
    /// Returns false if we're in a throttle/rate-limit cooldown period.
    /// `is_first` should be true when no status message exists yet (creating, not editing).
    pub fn should_send(&self, is_first: bool) -> bool {
        // Always allow the first status message (creation)
        if is_first {
            // But respect rate limits even for creation
            if let Some(until) = self.rate_limited_until {
                if tokio::time::Instant::now() < until {
                    return false;
                }
            }
            return true;
        }

        // Check rate limit
        if let Some(until) = self.rate_limited_until {
            if tokio::time::Instant::now() < until {
                return false;
            }
        }

        // Check throttle interval
        tokio::time::Instant::now().duration_since(self.last_edit) >= self.interval
    }

    /// Record a successful status message send/edit.
    pub fn record_success(&mut self) {
        self.last_edit = tokio::time::Instant::now();
        self.rate_limited_until = None;
    }

    /// Record a rate-limit error. Parses "Retry after Xs" from the error string.
    /// Returns true if a rate-limit was detected (and cooldown was set).
    pub fn record_error(&mut self, err: &str) -> bool {
        if let Some(secs) = parse_retry_after(err) {
            log::warn!(
                "Status message rate-limited, pausing for {}s",
                secs
            );
            self.rate_limited_until = Some(
                tokio::time::Instant::now() + std::time::Duration::from_secs(secs)
            );
            true
        } else {
            false
        }
    }
}

/// Check whether a broadcast event belongs to a specific channel + chat session.
///
/// Matches on `channel_id` and optionally `chat_id` inside `event.data`:
/// - Both present in event → both must match
/// - Only `channel_id` present (legacy event) → channel must match
/// - No `channel_id` → returns `false`
pub fn event_matches_session(
    data: &serde_json::Value,
    channel_id: i64,
    chat_id: &str,
) -> bool {
    let ev_channel_id = data.get("channel_id").and_then(|v| v.as_i64());
    let ev_chat_id = data.get("chat_id").and_then(|v| v.as_str());

    match (ev_channel_id, ev_chat_id) {
        (Some(ch_id), Some(ev_chat)) => ch_id == channel_id && ev_chat == chat_id,
        (Some(ch_id), None) => ch_id == channel_id,
        _ => false,
    }
}
