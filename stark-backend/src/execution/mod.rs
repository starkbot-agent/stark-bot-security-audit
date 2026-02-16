//! Execution tracking module
//!
//! This module provides real-time execution progress tracking for AI agent tasks.
//! It manages a hierarchical task tree and emits gateway events for frontend
//! display of execution progress (similar to Claude Code's CLI display).
//!
//! Also provides session lane serialization to prevent race conditions when
//! multiple requests arrive for the same session.

mod tracker;
mod pending_confirmation;
mod process_manager;
mod session_lanes;

pub use tracker::ExecutionTracker;
pub use pending_confirmation::{PendingConfirmation, PendingConfirmationManager};
pub use process_manager::{ProcessInfo, ProcessManager, ProcessStatus};
pub use session_lanes::{SessionLaneGuard, SessionLaneManager, SessionLaneStats};
