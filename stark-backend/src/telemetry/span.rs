//! Span-based telemetry for structured execution traces.
//!
//! Every tool call, LLM call, and planning step emits a structured `Span`
//! with a monotonically increasing `sequence_id`, type, status, timing, and attributes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use parking_lot::Mutex;

/// The kind of operation a span represents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanType {
    /// A tool execution (e.g., web_fetch, read_file)
    ToolCall,
    /// An LLM generation call
    LlmCall,
    /// Task planning phase
    Planning,
    /// A reward signal emitted during execution
    Reward,
    /// An annotation (key-value metadata)
    Annotation,
    /// A rollout-level lifecycle event
    Rollout,
    /// A watchdog timeout or heartbeat event
    Watchdog,
    /// A resource version resolution
    ResourceResolution,
}

/// The completion status of a span.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SpanStatus {
    /// Span is still in progress
    Running,
    /// Completed successfully
    Succeeded,
    /// Completed with failure
    Failed,
    /// Timed out
    TimedOut,
    /// Skipped (e.g., validator blocked the call)
    Skipped,
    /// Cancelled by user or system
    Cancelled,
}

impl SpanStatus {
    pub fn is_terminal(&self) -> bool {
        !matches!(self, SpanStatus::Running)
    }
}

/// A structured execution span capturing a single operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    /// Unique span identifier
    pub span_id: String,
    /// Monotonically increasing sequence within a rollout
    pub sequence_id: u64,
    /// The rollout this span belongs to
    pub rollout_id: String,
    /// The session this span belongs to
    pub session_id: i64,
    /// The attempt within the rollout (0-indexed)
    pub attempt_idx: u32,
    /// Parent span ID (for nested spans)
    pub parent_span_id: Option<String>,
    /// What kind of operation this span represents
    pub span_type: SpanType,
    /// Human-readable name (e.g., tool name, "generate_with_tools")
    pub name: String,
    /// Current status of the span
    pub status: SpanStatus,
    /// When the span started
    pub started_at: DateTime<Utc>,
    /// When the span completed (None if still running)
    pub completed_at: Option<DateTime<Utc>>,
    /// Duration in milliseconds (computed on completion)
    pub duration_ms: Option<u64>,
    /// Structured attributes for this span
    pub attributes: Value,
    /// Optional error message if the span failed
    pub error: Option<String>,
}

impl Span {
    /// Create a new running span.
    pub fn new(
        sequence_id: u64,
        rollout_id: String,
        session_id: i64,
        attempt_idx: u32,
        span_type: SpanType,
        name: String,
    ) -> Self {
        Self {
            span_id: uuid::Uuid::new_v4().to_string(),
            sequence_id,
            rollout_id,
            session_id,
            attempt_idx,
            parent_span_id: None,
            span_type,
            name,
            status: SpanStatus::Running,
            started_at: Utc::now(),
            completed_at: None,
            duration_ms: None,
            attributes: Value::Object(serde_json::Map::new()),
            error: None,
        }
    }

    /// Set the parent span ID for nesting.
    pub fn with_parent(mut self, parent_id: String) -> Self {
        self.parent_span_id = Some(parent_id);
        self
    }

    /// Attach structured attributes.
    pub fn with_attributes(mut self, attrs: Value) -> Self {
        self.attributes = attrs;
        self
    }

    /// Mark the span as succeeded.
    pub fn succeed(&mut self) {
        let now = Utc::now();
        self.status = SpanStatus::Succeeded;
        self.completed_at = Some(now);
        self.duration_ms = Some((now - self.started_at).num_milliseconds().max(0) as u64);
    }

    /// Mark the span as failed with an error message.
    pub fn fail(&mut self, error: String) {
        let now = Utc::now();
        self.status = SpanStatus::Failed;
        self.completed_at = Some(now);
        self.duration_ms = Some((now - self.started_at).num_milliseconds().max(0) as u64);
        self.error = Some(error);
    }

    /// Mark the span as timed out.
    pub fn timeout(&mut self) {
        let now = Utc::now();
        self.status = SpanStatus::TimedOut;
        self.completed_at = Some(now);
        self.duration_ms = Some((now - self.started_at).num_milliseconds().max(0) as u64);
        self.error = Some("Operation timed out".to_string());
    }

    /// Mark the span as cancelled.
    pub fn cancel(&mut self) {
        let now = Utc::now();
        self.status = SpanStatus::Cancelled;
        self.completed_at = Some(now);
        self.duration_ms = Some((now - self.started_at).num_milliseconds().max(0) as u64);
    }
}

/// Thread-safe accumulator for spans within a rollout.
///
/// The SpanCollector owns a monotonically increasing sequence counter
/// and collects all spans emitted during an execution.
#[derive(Debug)]
pub struct SpanCollector {
    /// Monotonically increasing sequence counter
    sequence: AtomicU64,
    /// The rollout ID this collector is associated with
    rollout_id: String,
    /// The session ID (atomic to allow updating after rollout creation)
    session_id: AtomicI64,
    /// Current attempt index
    attempt_idx: AtomicU64,
    /// Collected spans (thread-safe)
    spans: Mutex<Vec<Span>>,
}

impl SpanCollector {
    /// Create a new SpanCollector for a rollout.
    pub fn new(rollout_id: String, session_id: i64) -> Self {
        Self {
            sequence: AtomicU64::new(0),
            rollout_id,
            session_id: AtomicI64::new(session_id),
            attempt_idx: AtomicU64::new(0),
            spans: Mutex::new(Vec::new()),
        }
    }

    /// Get the rollout ID.
    pub fn rollout_id(&self) -> &str {
        &self.rollout_id
    }

    /// Get the session ID.
    pub fn session_id(&self) -> i64 {
        self.session_id.load(Ordering::Relaxed)
    }

    /// Update the session ID (called once the session is resolved).
    pub fn set_session(&self, session_id: i64) {
        self.session_id.store(session_id, Ordering::Relaxed);
    }

    /// Set the current attempt index.
    pub fn set_attempt(&self, idx: u32) {
        self.attempt_idx.store(idx as u64, Ordering::Relaxed);
    }

    /// Start a new span and return its ID for later completion.
    pub fn start_span(&self, span_type: SpanType, name: impl Into<String>) -> Span {
        let seq = self.sequence.fetch_add(1, Ordering::Relaxed);
        let attempt = self.attempt_idx.load(Ordering::Relaxed) as u32;
        Span::new(
            seq,
            self.rollout_id.clone(),
            self.session_id.load(Ordering::Relaxed),
            attempt,
            span_type,
            name.into(),
        )
    }

    /// Record a completed span.
    pub fn record(&self, span: Span) {
        self.spans.lock().push(span);
    }

    /// Start a span and return a guard that auto-completes it on drop.
    pub fn start_guarded(
        self: &Arc<Self>,
        span_type: SpanType,
        name: impl Into<String>,
    ) -> SpanGuard {
        let span = self.start_span(span_type, name);
        SpanGuard {
            collector: Arc::clone(self),
            span: Some(span),
        }
    }

    /// Drain all collected spans, returning them and clearing the internal buffer.
    pub fn drain(&self) -> Vec<Span> {
        let mut spans = self.spans.lock();
        std::mem::take(&mut *spans)
    }

    /// Get the number of collected spans.
    pub fn len(&self) -> usize {
        self.spans.lock().len()
    }

    /// Check if no spans have been collected.
    pub fn is_empty(&self) -> bool {
        self.spans.lock().is_empty()
    }

    /// Get a snapshot of all spans (clone).
    pub fn snapshot(&self) -> Vec<Span> {
        self.spans.lock().clone()
    }
}

/// RAII guard that automatically completes a span when dropped.
///
/// If the span hasn't been explicitly completed (via `succeed()`, `fail()`, etc.),
/// it will be marked as succeeded on drop.
pub struct SpanGuard {
    collector: Arc<SpanCollector>,
    span: Option<Span>,
}

impl SpanGuard {
    /// Get the span ID.
    pub fn span_id(&self) -> &str {
        self.span.as_ref().map(|s| s.span_id.as_str()).unwrap_or("")
    }

    /// Access the span mutably for adding attributes.
    pub fn span_mut(&mut self) -> Option<&mut Span> {
        self.span.as_mut()
    }

    /// Mark the span as succeeded and record it.
    pub fn succeed(mut self) {
        if let Some(mut span) = self.span.take() {
            span.succeed();
            self.collector.record(span);
        }
    }

    /// Mark the span as failed with an error and record it.
    pub fn fail(mut self, error: String) {
        if let Some(mut span) = self.span.take() {
            span.fail(error);
            self.collector.record(span);
        }
    }

    /// Mark the span as timed out and record it.
    pub fn timeout(mut self) {
        if let Some(mut span) = self.span.take() {
            span.timeout();
            self.collector.record(span);
        }
    }

    /// Mark the span as cancelled and record it.
    pub fn cancel(mut self) {
        if let Some(mut span) = self.span.take() {
            span.cancel();
            self.collector.record(span);
        }
    }

    /// Complete the span with a custom status and optional error.
    pub fn complete(mut self, status: SpanStatus, error: Option<String>) {
        if let Some(mut span) = self.span.take() {
            match status {
                SpanStatus::Succeeded => span.succeed(),
                SpanStatus::Failed => span.fail(error.unwrap_or_default()),
                SpanStatus::TimedOut => span.timeout(),
                SpanStatus::Cancelled => span.cancel(),
                SpanStatus::Skipped => {
                    let now = Utc::now();
                    span.status = SpanStatus::Skipped;
                    span.completed_at = Some(now);
                    span.duration_ms = Some((now - span.started_at).num_milliseconds().max(0) as u64);
                }
                SpanStatus::Running => {} // no-op, will be caught by Drop
            }
            self.collector.record(span);
        }
    }
}

impl Drop for SpanGuard {
    fn drop(&mut self) {
        // If the span hasn't been explicitly completed, mark as succeeded
        if let Some(mut span) = self.span.take() {
            if !span.status.is_terminal() {
                span.succeed();
            }
            self.collector.record(span);
        }
    }
}
