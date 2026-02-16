//! Hook system for extensible lifecycle hooks
//!
//! This module provides a plugin/hook system that allows extending agent behavior
//! at various lifecycle points. Hooks can:
//!
//! - Intercept and modify operations (before_agent_start, before_tool_call)
//! - React to events (after_agent_end, on_error)
//! - Transform data (before_response)
//! - Log and audit (logging hook)
//! - Enforce limits (rate_limit hook)
//!
//! # Example
//!
//! ```rust,ignore
//! use hooks::{HookManager, HookContext, HookEvent};
//! use hooks::builtin::LoggingHook;
//!
//! let manager = HookManager::new();
//! manager.register(Arc::new(LoggingHook::new()));
//!
//! let mut context = HookContext::new(HookEvent::BeforeAgentStart)
//!     .with_channel(123, Some(456));
//!
//! let result = manager.execute(HookEvent::BeforeAgentStart, &mut context).await;
//! if result.should_continue() {
//!     // Proceed with agent processing
//! }
//! ```

pub mod builtin;
mod manager;
mod types;

pub use manager::HookManager;
pub use types::{
    BoxedHook, Hook, HookConfig, HookContext, HookEvent, HookPriority, HookResult, HookStats,
};
