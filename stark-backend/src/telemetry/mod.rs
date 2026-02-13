//! Agent-Lightning inspired telemetry system.
//!
//! Provides structured execution traces, rollout/attempt lifecycle with retry,
//! reward signals, watchdog enforcement, resource versioning, and execution replay.
//!
//! Philosophy: "Agents emit spans, algorithms consume spans to improve resources."

pub mod span;
pub mod rollout;
pub mod emitter;
pub mod reward;
pub mod watchdog;
pub mod resource_version;
pub mod adapter;
pub mod store;

// Re-export key types for convenience
pub use span::{Span, SpanCollector, SpanGuard, SpanStatus, SpanType};
pub use rollout::{Attempt, FailureReason, Rollout, RolloutConfig, RolloutManager, RolloutStatus};
pub use emitter::{clear_active_collector, emit_annotation, set_active_collector};
pub use reward::RewardEmitter;
pub use watchdog::{Watchdog, WatchdogConfig, WatchdogError};
pub use resource_version::{Resource, ResourceBundle, ResourceManager, ResourceType};
pub use adapter::{Adapter, ExecutionSummary, SpansToSummary, SpansToTimeline, SpansToTriplets, Timeline, Triplet};
pub use store::{RetentionPolicy, RewardStats, TelemetryStore};
