//! RewardEmitter with auto-scoring for tool success/failure,
//! session efficiency, and loop detection.

use serde_json::json;
use std::sync::Arc;

use super::span::{SpanCollector, SpanType};

/// Emits structured reward signals based on execution outcomes.
pub struct RewardEmitter {
    collector: Arc<SpanCollector>,
}

impl RewardEmitter {
    pub fn new(collector: Arc<SpanCollector>) -> Self {
        Self { collector }
    }

    /// Emit a reward for a completed tool call.
    ///
    /// Scoring:
    /// - Success: +1.0
    /// - Failure: -0.5
    /// - Bonus for fast execution (< 1s): +0.2
    pub fn tool_completed(&self, tool_name: &str, success: bool, duration_ms: u64) {
        let mut value = if success { 1.0 } else { -0.5 };

        // Bonus for fast successful tools
        if success && duration_ms < 1000 {
            value += 0.2;
        }

        let mut span = self.collector.start_span(SpanType::Reward, "tool_completed");
        span.attributes = json!({
            "reward_value": value,
            "reward_type": "tool_completed",
            "tool_name": tool_name,
            "success": success,
            "duration_ms": duration_ms,
        });
        span.succeed();
        self.collector.record(span);
    }

    /// Emit a reward for session completion.
    ///
    /// Scoring:
    /// - Base: +2.0 for successful completion
    /// - Efficiency bonus: scales inversely with iteration count
    /// - Penalty for excessive iterations: -0.1 per iteration over threshold
    pub fn session_completed(
        &self,
        success: bool,
        iterations: u32,
        tool_calls: u32,
        max_iterations: u32,
    ) {
        let mut value = if success { 2.0 } else { -1.0 };

        if success {
            // Efficiency bonus: fewer iterations = higher reward
            let efficiency_ratio = 1.0 - (iterations as f64 / max_iterations as f64);
            value += efficiency_ratio.max(0.0) * 1.0;

            // Penalty for high iteration counts
            let threshold = max_iterations / 3;
            if iterations > threshold {
                value -= (iterations - threshold) as f64 * 0.1;
            }
        }

        let mut span = self.collector.start_span(SpanType::Reward, "session_completed");
        span.attributes = json!({
            "reward_value": value,
            "reward_type": "session_completed",
            "success": success,
            "iterations": iterations,
            "tool_calls": tool_calls,
            "max_iterations": max_iterations,
            "efficiency_ratio": 1.0 - (iterations as f64 / max_iterations as f64),
        });
        span.succeed();
        self.collector.record(span);
    }

    /// Emit a negative reward when loop detection triggers.
    ///
    /// Scoring: -2.0 (loops are wasteful and indicate poor planning)
    pub fn loop_detected(&self, repeated_signatures: &[String], iteration: u32) {
        let mut span = self.collector.start_span(SpanType::Reward, "loop_detected");
        span.attributes = json!({
            "reward_value": -2.0,
            "reward_type": "loop_detected",
            "repeated_signatures": repeated_signatures,
            "iteration": iteration,
        });
        span.succeed();
        self.collector.record(span);
    }

    /// Emit a reward for successful error recovery (retry succeeded).
    pub fn retry_succeeded(&self, attempt_idx: u32) {
        let mut span = self.collector.start_span(SpanType::Reward, "retry_succeeded");
        span.attributes = json!({
            "reward_value": 0.5,
            "reward_type": "retry_succeeded",
            "attempt_idx": attempt_idx,
        });
        span.succeed();
        self.collector.record(span);
    }

    /// Emit a reward for a watchdog timeout event.
    pub fn watchdog_timeout(&self, operation: &str, timeout_ms: u64) {
        let mut span = self.collector.start_span(SpanType::Reward, "watchdog_timeout");
        span.attributes = json!({
            "reward_value": -1.5,
            "reward_type": "watchdog_timeout",
            "operation": operation,
            "timeout_ms": timeout_ms,
        });
        span.succeed();
        self.collector.record(span);
    }

    /// Emit a generic custom reward.
    pub fn custom(&self, name: &str, value: f64, metadata: serde_json::Value) {
        let mut span = self.collector.start_span(SpanType::Reward, name);
        span.attributes = json!({
            "reward_value": value,
            "reward_type": name,
            "metadata": metadata,
        });
        span.succeed();
        self.collector.record(span);
    }
}
