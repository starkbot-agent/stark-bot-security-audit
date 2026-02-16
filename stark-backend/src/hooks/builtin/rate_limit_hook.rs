//! Rate limit hook - Enforces rate limits on agent operations
//!
//! This hook tracks request rates and blocks operations when limits are exceeded.

use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration as StdDuration;

use crate::hooks::types::{Hook, HookContext, HookEvent, HookPriority, HookResult};

/// Rate limit configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateLimitConfig {
    /// Maximum requests per window
    pub max_requests: u32,
    /// Window duration in seconds
    pub window_secs: u64,
    /// Maximum tool calls per message
    pub max_tool_calls_per_message: Option<u32>,
    /// Cooldown after hitting limit (seconds)
    pub cooldown_secs: u64,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests: 60,         // 60 requests
            window_secs: 60,          // per minute
            max_tool_calls_per_message: Some(10),
            cooldown_secs: 30,
        }
    }
}

/// Tracks rate limit state for a channel
#[derive(Debug)]
struct ChannelRateState {
    /// Timestamps of recent requests
    request_times: Vec<DateTime<Utc>>,
    /// Tool calls in current message
    tool_calls_in_message: AtomicU64,
    /// When cooldown ends (if in cooldown)
    cooldown_until: Option<DateTime<Utc>>,
    /// Current session ID (resets tool call count on new session)
    current_session: Option<i64>,
}

impl Default for ChannelRateState {
    fn default() -> Self {
        Self {
            request_times: Vec::new(),
            tool_calls_in_message: AtomicU64::new(0),
            cooldown_until: None,
            current_session: None,
        }
    }
}

/// Hook that enforces rate limits
pub struct RateLimitHook {
    config: RateLimitConfig,
    /// State by channel ID
    states: DashMap<i64, ChannelRateState>,
}

impl RateLimitHook {
    /// Create with default configuration
    pub fn new() -> Self {
        Self {
            config: RateLimitConfig::default(),
            states: DashMap::new(),
        }
    }

    /// Create with custom configuration
    pub fn with_config(config: RateLimitConfig) -> Self {
        Self {
            config,
            states: DashMap::new(),
        }
    }

    /// Check if a channel is in cooldown
    fn is_in_cooldown(&self, channel_id: i64) -> bool {
        if let Some(state) = self.states.get(&channel_id) {
            if let Some(cooldown_until) = state.cooldown_until {
                return Utc::now() < cooldown_until;
            }
        }
        false
    }

    /// Get remaining cooldown time
    fn remaining_cooldown(&self, channel_id: i64) -> Option<StdDuration> {
        if let Some(state) = self.states.get(&channel_id) {
            if let Some(cooldown_until) = state.cooldown_until {
                let now = Utc::now();
                if now < cooldown_until {
                    return Some((cooldown_until - now).to_std().unwrap_or(StdDuration::ZERO));
                }
            }
        }
        None
    }

    /// Check rate limit for a channel
    fn check_rate_limit(&self, channel_id: i64) -> Result<(), String> {
        let now = Utc::now();
        let window_start = now - Duration::seconds(self.config.window_secs as i64);

        let mut state = self.states.entry(channel_id).or_default();

        // Clean up old request times
        state.request_times.retain(|t| *t > window_start);

        // Check if over limit
        if state.request_times.len() >= self.config.max_requests as usize {
            // Enter cooldown
            state.cooldown_until = Some(now + Duration::seconds(self.config.cooldown_secs as i64));
            return Err(format!(
                "Rate limit exceeded: {} requests in {}s. Cooldown for {}s.",
                self.config.max_requests,
                self.config.window_secs,
                self.config.cooldown_secs
            ));
        }

        // Record this request
        state.request_times.push(now);

        Ok(())
    }

    /// Check tool call limit for current message
    fn check_tool_limit(&self, channel_id: i64, session_id: Option<i64>) -> Result<(), String> {
        let max_calls = match self.config.max_tool_calls_per_message {
            Some(max) => max,
            None => return Ok(()),
        };

        let mut state = self.states.entry(channel_id).or_default();

        // Reset count if session changed
        if state.current_session != session_id {
            state.current_session = session_id;
            state.tool_calls_in_message.store(0, Ordering::SeqCst);
        }

        let current = state.tool_calls_in_message.fetch_add(1, Ordering::SeqCst);

        if current >= max_calls as u64 {
            return Err(format!(
                "Tool call limit exceeded: maximum {} calls per message",
                max_calls
            ));
        }

        Ok(())
    }

    /// Reset tool call count for a channel (call on new message)
    pub fn reset_tool_count(&self, channel_id: i64) {
        if let Some(mut state) = self.states.get_mut(&channel_id) {
            state.tool_calls_in_message.store(0, Ordering::SeqCst);
        }
    }
}

impl Default for RateLimitHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Hook for RateLimitHook {
    fn id(&self) -> &str {
        "builtin.rate_limit"
    }

    fn name(&self) -> &str {
        "Rate Limit Hook"
    }

    fn description(&self) -> &str {
        "Enforces rate limits on agent operations to prevent abuse"
    }

    fn events(&self) -> Vec<HookEvent> {
        vec![
            HookEvent::BeforeAgentStart,
            HookEvent::BeforeToolCall,
        ]
    }

    fn priority(&self) -> HookPriority {
        // Run rate limiting early
        HookPriority::High
    }

    async fn execute(&self, context: &mut HookContext) -> HookResult {
        let channel_id = match context.channel_id {
            Some(id) => id,
            None => return HookResult::Continue(None),
        };

        match context.event {
            HookEvent::BeforeAgentStart => {
                // Reset tool count for new message
                self.reset_tool_count(channel_id);

                // Check if in cooldown
                if let Some(remaining) = self.remaining_cooldown(channel_id) {
                    return HookResult::Cancel(format!(
                        "Rate limited. Please wait {} seconds.",
                        remaining.as_secs()
                    ));
                }

                // Check rate limit
                if let Err(msg) = self.check_rate_limit(channel_id) {
                    return HookResult::Cancel(msg);
                }

                HookResult::Continue(None)
            }
            HookEvent::BeforeToolCall => {
                // Check tool call limit
                if let Err(msg) = self.check_tool_limit(channel_id, context.session_id) {
                    log::warn!(
                        "[RATE LIMIT] Tool call limit exceeded for channel {}",
                        channel_id
                    );
                    return HookResult::Skip;
                }

                HookResult::Continue(None)
            }
            _ => HookResult::Continue(None),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_rate_limit_hook() {
        let hook = RateLimitHook::with_config(RateLimitConfig {
            max_requests: 2,
            window_secs: 60,
            max_tool_calls_per_message: Some(3),
            cooldown_secs: 10,
        });

        let channel_id = 123;

        // First request should pass
        let mut context = HookContext::new(HookEvent::BeforeAgentStart)
            .with_channel(channel_id, Some(1));
        let result = hook.execute(&mut context).await;
        assert!(result.should_continue());

        // Second request should pass
        let result = hook.execute(&mut context).await;
        assert!(result.should_continue());

        // Third request should be rate limited
        let result = hook.execute(&mut context).await;
        assert!(result.should_cancel());
    }

    #[tokio::test]
    async fn test_tool_call_limit() {
        let hook = RateLimitHook::with_config(RateLimitConfig {
            max_requests: 100,
            window_secs: 60,
            max_tool_calls_per_message: Some(2),
            cooldown_secs: 10,
        });

        let channel_id = 456;

        // First tool call should pass
        let mut context = HookContext::new(HookEvent::BeforeToolCall)
            .with_channel(channel_id, Some(1))
            .with_tool("test_tool".to_string(), serde_json::json!({}));
        let result = hook.execute(&mut context).await;
        assert!(result.should_continue());

        // Second tool call should pass
        let result = hook.execute(&mut context).await;
        assert!(result.should_continue());

        // Third tool call should be skipped
        let result = hook.execute(&mut context).await;
        assert!(result.should_skip());
    }
}
