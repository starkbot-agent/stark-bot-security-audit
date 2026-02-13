//! Rollout/Attempt lifecycle with retry logic.
//!
//! Each `dispatch()` becomes a `Rollout` with a formal status machine
//! (Queuing→Preparing→Running→Succeeded/Failed). Failed executions create
//! new `Attempt`s with configurable retry policy.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;

use super::span::SpanCollector;

/// The lifecycle status of a rollout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RolloutStatus {
    /// Waiting to be processed
    Queuing,
    /// Setting up context, loading resources
    Preparing,
    /// Actively executing the agentic loop
    Running,
    /// Completed successfully
    Succeeded,
    /// Failed after exhausting retries
    Failed,
    /// Cancelled by user or system
    Cancelled,
}

impl RolloutStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, RolloutStatus::Succeeded | RolloutStatus::Failed | RolloutStatus::Cancelled)
    }
}

/// Conditions under which a failed attempt should be retried.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryCondition {
    /// Retry on any failure
    OnAnyFailure,
    /// Retry on timeout
    OnTimeout,
    /// Retry on LLM errors (rate limit, server error)
    OnLlmError,
    /// Retry on tool execution errors
    OnToolError,
    /// Retry on context overflow (with history truncation)
    OnContextOverflow,
}

/// Configuration for rollout behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RolloutConfig {
    /// Maximum time for the entire rollout in seconds
    pub timeout_secs: u64,
    /// Maximum number of attempts (1 = no retry)
    pub max_attempts: u32,
    /// Which conditions trigger a retry
    pub retry_conditions: Vec<RetryCondition>,
    /// Delay between retry attempts in milliseconds
    pub retry_delay_ms: u64,
    /// Whether to use exponential backoff for retries
    pub exponential_backoff: bool,
    /// Maximum retry delay when using exponential backoff (ms)
    pub max_retry_delay_ms: u64,
}

impl Default for RolloutConfig {
    fn default() -> Self {
        Self {
            timeout_secs: 300,     // 5 minutes
            max_attempts: 3,
            retry_conditions: vec![
                RetryCondition::OnTimeout,
                RetryCondition::OnLlmError,
                RetryCondition::OnContextOverflow,
            ],
            retry_delay_ms: 1000,
            exponential_backoff: true,
            max_retry_delay_ms: 30_000,
        }
    }
}

impl RolloutConfig {
    /// Calculate the delay for a given attempt index (0-based).
    pub fn delay_for_attempt(&self, attempt_idx: u32) -> u64 {
        if self.exponential_backoff {
            let delay = self.retry_delay_ms * 2u64.pow(attempt_idx);
            delay.min(self.max_retry_delay_ms)
        } else {
            self.retry_delay_ms
        }
    }

    /// Check if a failure reason matches the retry conditions.
    pub fn should_retry(&self, reason: &FailureReason, attempt_count: u32) -> bool {
        if attempt_count >= self.max_attempts {
            return false;
        }
        self.retry_conditions.iter().any(|cond| match (cond, reason) {
            (RetryCondition::OnAnyFailure, _) => true,
            (RetryCondition::OnTimeout, FailureReason::Timeout) => true,
            (RetryCondition::OnLlmError, FailureReason::LlmError(_)) => true,
            (RetryCondition::OnToolError, FailureReason::ToolError(_)) => true,
            (RetryCondition::OnContextOverflow, FailureReason::ContextOverflow) => true,
            _ => false,
        })
    }
}

/// Why an attempt failed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureReason {
    Timeout,
    LlmError(String),
    ToolError(String),
    ContextOverflow,
    LoopDetected,
    Cancelled,
    Unknown(String),
}

impl FailureReason {
    /// Classify an error string into a failure reason.
    pub fn classify(error: &str) -> Self {
        let lower = error.to_lowercase();
        if lower.contains("timed out") || lower.contains("timeout") {
            FailureReason::Timeout
        } else if lower.contains("context") && (lower.contains("too large") || lower.contains("overflow")) {
            FailureReason::ContextOverflow
        } else if lower.contains("loop") && lower.contains("detect") {
            FailureReason::LoopDetected
        } else if lower.contains("cancelled") || lower.contains("canceled") {
            FailureReason::Cancelled
        } else if lower.contains("rate limit") || lower.contains("429") || lower.contains("500") || lower.contains("503") {
            FailureReason::LlmError(error.to_string())
        } else {
            FailureReason::Unknown(error.to_string())
        }
    }
}

/// A single attempt within a rollout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Attempt {
    /// Attempt index (0-based)
    pub attempt_idx: u32,
    /// When this attempt started
    pub started_at: DateTime<Utc>,
    /// When this attempt completed
    pub completed_at: Option<DateTime<Utc>>,
    /// Duration in milliseconds
    pub duration_ms: Option<u64>,
    /// Whether this attempt succeeded
    pub succeeded: bool,
    /// Failure reason if failed
    pub failure_reason: Option<FailureReason>,
    /// Error message if failed
    pub error: Option<String>,
    /// Number of tool calls made in this attempt
    pub tool_calls: u32,
    /// Number of LLM calls made in this attempt
    pub llm_calls: u32,
    /// Total tokens consumed in this attempt
    pub tokens_used: u64,
}

impl Attempt {
    pub fn new(attempt_idx: u32) -> Self {
        Self {
            attempt_idx,
            started_at: Utc::now(),
            completed_at: None,
            duration_ms: None,
            succeeded: false,
            failure_reason: None,
            error: None,
            tool_calls: 0,
            llm_calls: 0,
            tokens_used: 0,
        }
    }

    pub fn succeed(&mut self) {
        let now = Utc::now();
        self.succeeded = true;
        self.completed_at = Some(now);
        self.duration_ms = Some((now - self.started_at).num_milliseconds().max(0) as u64);
    }

    pub fn fail(&mut self, reason: FailureReason, error: String) {
        let now = Utc::now();
        self.succeeded = false;
        self.failure_reason = Some(reason);
        self.error = Some(error);
        self.completed_at = Some(now);
        self.duration_ms = Some((now - self.started_at).num_milliseconds().max(0) as u64);
    }

    pub fn record_tool_call(&mut self) {
        self.tool_calls += 1;
    }

    pub fn record_llm_call(&mut self) {
        self.llm_calls += 1;
    }

    pub fn add_tokens(&mut self, tokens: u64) {
        self.tokens_used += tokens;
    }
}

/// A rollout represents a complete dispatch execution, potentially with retries.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rollout {
    /// Unique rollout identifier
    pub rollout_id: String,
    /// The session this rollout belongs to
    pub session_id: i64,
    /// The channel this rollout was triggered from
    pub channel_id: i64,
    /// Current rollout status
    pub status: RolloutStatus,
    /// Configuration for this rollout
    pub config: RolloutConfig,
    /// All attempts made in this rollout
    pub attempts: Vec<Attempt>,
    /// The resource version used for this rollout
    pub resources_id: Option<String>,
    /// When the rollout was created
    pub created_at: DateTime<Utc>,
    /// When the rollout completed
    pub completed_at: Option<DateTime<Utc>>,
    /// Total duration in milliseconds
    pub duration_ms: Option<u64>,
    /// The final result (if succeeded)
    pub result: Option<String>,
    /// The final error (if failed)
    pub error: Option<String>,
    /// Arbitrary metadata
    pub metadata: Value,
}

impl Rollout {
    pub fn new(session_id: i64, channel_id: i64, config: RolloutConfig) -> Self {
        Self {
            rollout_id: uuid::Uuid::new_v4().to_string(),
            session_id,
            channel_id,
            status: RolloutStatus::Queuing,
            config,
            attempts: Vec::new(),
            resources_id: None,
            created_at: Utc::now(),
            completed_at: None,
            duration_ms: None,
            result: None,
            error: None,
            metadata: Value::Object(serde_json::Map::new()),
        }
    }

    pub fn current_attempt(&self) -> Option<&Attempt> {
        self.attempts.last()
    }

    pub fn current_attempt_mut(&mut self) -> Option<&mut Attempt> {
        self.attempts.last_mut()
    }

    pub fn attempt_count(&self) -> u32 {
        self.attempts.len() as u32
    }
}

/// Manages the lifecycle of rollouts and attempts.
pub struct RolloutManager {
    db: Arc<crate::db::Database>,
}

impl RolloutManager {
    pub fn new(db: Arc<crate::db::Database>) -> Self {
        Self { db }
    }

    /// Create a new rollout and its first attempt.
    pub fn start_rollout(
        &self,
        session_id: i64,
        channel_id: i64,
        config: RolloutConfig,
    ) -> (Rollout, SpanCollector) {
        let mut rollout = Rollout::new(session_id, channel_id, config);
        rollout.status = RolloutStatus::Preparing;

        let attempt = Attempt::new(0);
        rollout.attempts.push(attempt);

        let collector = SpanCollector::new(rollout.rollout_id.clone(), session_id);

        // Persist the new rollout
        if let Err(e) = self.db.create_rollout(&rollout) {
            log::error!("[ROLLOUT] Failed to persist rollout: {}", e);
        }

        (rollout, collector)
    }

    /// Transition the rollout to running status.
    pub fn mark_running(&self, rollout: &mut Rollout) {
        rollout.status = RolloutStatus::Running;
        if let Err(e) = self.db.update_rollout_status(&rollout.rollout_id, "running") {
            log::error!("[ROLLOUT] Failed to update rollout status: {}", e);
        }
    }

    /// Mark the current attempt as succeeded and complete the rollout.
    pub fn succeed_rollout(&self, rollout: &mut Rollout, result: String) {
        if let Some(attempt) = rollout.current_attempt_mut() {
            attempt.succeed();
        }

        let now = Utc::now();
        rollout.status = RolloutStatus::Succeeded;
        rollout.completed_at = Some(now);
        rollout.duration_ms = Some((now - rollout.created_at).num_milliseconds().max(0) as u64);
        rollout.result = Some(result);

        self.persist_rollout_completion(rollout);
    }

    /// Fail the current attempt and determine if a retry should happen.
    ///
    /// Returns `true` if a new attempt was created (retry), `false` if the rollout is terminal.
    pub fn fail_attempt(
        &self,
        rollout: &mut Rollout,
        error: &str,
        collector: &SpanCollector,
    ) -> bool {
        let reason = FailureReason::classify(error);

        if let Some(attempt) = rollout.current_attempt_mut() {
            attempt.fail(reason.clone(), error.to_string());
        }

        // Persist the failed attempt
        if let Err(e) = self.db.update_attempt(
            &rollout.rollout_id,
            rollout.attempt_count().saturating_sub(1),
            false,
            Some(error),
        ) {
            log::error!("[ROLLOUT] Failed to persist attempt failure: {}", e);
        }

        // Check retry policy
        if rollout.config.should_retry(&reason, rollout.attempt_count()) {
            let new_idx = rollout.attempt_count();
            let new_attempt = Attempt::new(new_idx);
            rollout.attempts.push(new_attempt);
            collector.set_attempt(new_idx);

            log::info!(
                "[ROLLOUT] Retrying rollout {} (attempt {}/{}), reason: {:?}",
                rollout.rollout_id,
                new_idx + 1,
                rollout.config.max_attempts,
                reason
            );

            // Persist the new attempt
            if let Err(e) = self.db.create_attempt(&rollout.rollout_id, new_idx) {
                log::error!("[ROLLOUT] Failed to persist new attempt: {}", e);
            }

            true
        } else {
            // No more retries, fail the rollout
            let now = Utc::now();
            rollout.status = RolloutStatus::Failed;
            rollout.completed_at = Some(now);
            rollout.duration_ms = Some((now - rollout.created_at).num_milliseconds().max(0) as u64);
            rollout.error = Some(error.to_string());

            self.persist_rollout_completion(rollout);
            false
        }
    }

    /// Cancel the rollout.
    pub fn cancel_rollout(&self, rollout: &mut Rollout) {
        if let Some(attempt) = rollout.current_attempt_mut() {
            attempt.fail(FailureReason::Cancelled, "Cancelled".to_string());
        }

        let now = Utc::now();
        rollout.status = RolloutStatus::Cancelled;
        rollout.completed_at = Some(now);
        rollout.duration_ms = Some((now - rollout.created_at).num_milliseconds().max(0) as u64);

        self.persist_rollout_completion(rollout);
    }

    /// Get the retry delay for the current attempt.
    pub fn retry_delay(&self, rollout: &Rollout) -> u64 {
        let idx = rollout.attempt_count().saturating_sub(1);
        rollout.config.delay_for_attempt(idx)
    }

    fn persist_rollout_completion(&self, rollout: &Rollout) {
        let status = match rollout.status {
            RolloutStatus::Succeeded => "succeeded",
            RolloutStatus::Failed => "failed",
            RolloutStatus::Cancelled => "cancelled",
            _ => "running",
        };

        if let Err(e) = self.db.complete_rollout(
            &rollout.rollout_id,
            status,
            rollout.result.as_deref(),
            rollout.error.as_deref(),
            rollout.duration_ms,
        ) {
            log::error!("[ROLLOUT] Failed to persist rollout completion: {}", e);
        }
    }
}
