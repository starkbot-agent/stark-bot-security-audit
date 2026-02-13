//! Lightweight emit API for annotations.
//!
//! Provides thread-local access to the active SpanCollector so that
//! any code path can emit telemetry without explicit plumbing.
//!
//! Note: Reward emission is handled by `RewardEmitter` (reward.rs) which
//! provides richer scoring (efficiency bonuses, iteration penalties, etc.).

use serde_json::{json, Value};
use std::cell::RefCell;
use std::sync::Arc;

use super::span::{SpanCollector, SpanType};

thread_local! {
    /// The active SpanCollector for the current async task.
    /// Set at the start of a dispatch and cleared at the end.
    static ACTIVE_COLLECTOR: RefCell<Option<Arc<SpanCollector>>> = const { RefCell::new(None) };
}

/// Install a SpanCollector as the active collector for the current thread.
pub fn set_active_collector(collector: Arc<SpanCollector>) {
    ACTIVE_COLLECTOR.with(|c| {
        *c.borrow_mut() = Some(collector);
    });
}

/// Remove the active collector from the current thread.
pub fn clear_active_collector() {
    ACTIVE_COLLECTOR.with(|c| {
        *c.borrow_mut() = None;
    });
}

/// Get a reference to the active collector, if one is set.
pub fn with_active_collector<F, R>(f: F) -> Option<R>
where
    F: FnOnce(&Arc<SpanCollector>) -> R,
{
    ACTIVE_COLLECTOR.with(|c| {
        c.borrow().as_ref().map(f)
    })
}

/// Emit an annotation (key-value metadata) attached to the current execution.
pub fn emit_annotation(key: &str, value: Value) {
    with_active_collector(|collector| {
        let mut span = collector.start_span(SpanType::Annotation, key);
        span.attributes = json!({
            "annotation_key": key,
            "annotation_value": value,
        });
        span.succeed();
        collector.record(span);
    });
}
