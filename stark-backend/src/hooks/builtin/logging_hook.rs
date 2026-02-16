//! Logging hook - Records all hook events for debugging and auditing
//!
//! This hook logs all events with configurable verbosity levels.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::hooks::types::{Hook, HookContext, HookEvent, HookPriority, HookResult};

/// Verbosity level for logging
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    /// Only log errors
    Error,
    /// Log warnings and errors
    Warn,
    /// Log info, warnings, and errors
    Info,
    /// Log everything including debug info
    Debug,
    /// Log with full context (may include sensitive data)
    Trace,
}

impl Default for LogLevel {
    fn default() -> Self {
        LogLevel::Info
    }
}

/// Hook that logs all events
pub struct LoggingHook {
    /// Log level
    level: LogLevel,
    /// Whether to include context details
    include_context: bool,
}

impl LoggingHook {
    /// Create a new logging hook with default settings
    pub fn new() -> Self {
        Self {
            level: LogLevel::Info,
            include_context: false,
        }
    }

    /// Create with a specific log level
    pub fn with_level(level: LogLevel) -> Self {
        Self {
            level,
            include_context: matches!(level, LogLevel::Debug | LogLevel::Trace),
        }
    }

    /// Create with full context logging
    pub fn verbose() -> Self {
        Self {
            level: LogLevel::Trace,
            include_context: true,
        }
    }

    fn format_context(&self, context: &HookContext) -> String {
        if !self.include_context {
            return String::new();
        }

        let mut parts = Vec::new();

        if let Some(channel_id) = context.channel_id {
            parts.push(format!("channel={}", channel_id));
        }
        if let Some(session_id) = context.session_id {
            parts.push(format!("session={}", session_id));
        }
        if let Some(ref tool_name) = context.tool_name {
            parts.push(format!("tool={}", tool_name));
        }
        if let Some(ref error) = context.error {
            parts.push(format!("error={}", error));
        }

        if parts.is_empty() {
            String::new()
        } else {
            format!(" [{}]", parts.join(", "))
        }
    }
}

impl Default for LoggingHook {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Hook for LoggingHook {
    fn id(&self) -> &str {
        "builtin.logging"
    }

    fn name(&self) -> &str {
        "Logging Hook"
    }

    fn description(&self) -> &str {
        "Logs all hook events for debugging and auditing"
    }

    fn events(&self) -> Vec<HookEvent> {
        vec![
            HookEvent::BeforeAgentStart,
            HookEvent::AfterAgentEnd,
            HookEvent::BeforeToolCall,
            HookEvent::AfterToolCall,
            HookEvent::OnModeTransition,
            HookEvent::OnError,
            HookEvent::BeforeResponse,
            HookEvent::OnMemoryUpdate,
        ]
    }

    fn priority(&self) -> HookPriority {
        // Run logging last so we capture the final state
        HookPriority::Lowest
    }

    async fn execute(&self, context: &mut HookContext) -> HookResult {
        let event_name = context.event.as_str();
        let ctx_str = self.format_context(context);

        match self.level {
            LogLevel::Error => {
                // Only log on error events
                if matches!(context.event, HookEvent::OnError) {
                    log::error!("[HOOK EVENT] {}{}", event_name, ctx_str);
                }
            }
            LogLevel::Warn => {
                if matches!(context.event, HookEvent::OnError) {
                    log::warn!("[HOOK EVENT] {}{}", event_name, ctx_str);
                }
            }
            LogLevel::Info => {
                log::info!("[HOOK EVENT] {}{}", event_name, ctx_str);
            }
            LogLevel::Debug => {
                log::debug!("[HOOK EVENT] {}{}", event_name, ctx_str);
            }
            LogLevel::Trace => {
                // Include full context in trace mode
                log::trace!(
                    "[HOOK EVENT] {} - context: {:?}",
                    event_name,
                    context
                );
            }
        }

        HookResult::Continue(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_logging_hook() {
        let hook = LoggingHook::new();
        let mut context = HookContext::new(HookEvent::BeforeAgentStart)
            .with_channel(123, Some(456));

        let result = hook.execute(&mut context).await;
        assert!(result.should_continue());
    }
}
