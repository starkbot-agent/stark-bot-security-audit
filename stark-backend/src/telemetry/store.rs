//! SQLite-backed telemetry persistence with retention policy.
//!
//! Provides methods to persist spans, query execution data,
//! and prune old telemetry.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use super::adapter::{Adapter, ExecutionSummary, SpansToSummary, SpansToTimeline, SpansToTriplets, Timeline, Triplet};
use super::span::{Span, SpanCollector, SpanType};

/// Retention policy for telemetry data.
#[derive(Debug, Clone)]
pub struct RetentionPolicy {
    /// How long to keep spans (in days)
    pub span_retention_days: u64,
    /// How long to keep rollouts (in days)
    pub rollout_retention_days: u64,
    /// Maximum number of spans to keep per session
    pub max_spans_per_session: usize,
}

impl Default for RetentionPolicy {
    fn default() -> Self {
        Self {
            span_retention_days: 30,
            rollout_retention_days: 90,
            max_spans_per_session: 10_000,
        }
    }
}

/// Statistics about rewards over a time period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardStats {
    pub total_rewards: usize,
    pub total_value: f64,
    pub avg_value: f64,
    pub min_value: f64,
    pub max_value: f64,
    pub by_type: std::collections::HashMap<String, RewardTypeStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RewardTypeStat {
    pub count: usize,
    pub total_value: f64,
    pub avg_value: f64,
}

/// The telemetry store provides high-level persistence and query operations.
pub struct TelemetryStore {
    db: Arc<crate::db::Database>,
    retention: RetentionPolicy,
}

impl TelemetryStore {
    pub fn new(db: Arc<crate::db::Database>) -> Self {
        Self {
            db,
            retention: RetentionPolicy::default(),
        }
    }

    pub fn with_retention(mut self, retention: RetentionPolicy) -> Self {
        self.retention = retention;
        self
    }

    /// Persist all spans from a collector to the database.
    pub fn persist_spans(&self, collector: &SpanCollector) {
        let spans = collector.drain();
        if spans.is_empty() {
            return;
        }

        log::info!(
            "[TELEMETRY] Persisting {} spans for rollout {}",
            spans.len(),
            collector.rollout_id()
        );

        for span in &spans {
            if let Err(e) = self.db.insert_span(span) {
                log::error!("[TELEMETRY] Failed to persist span {}: {}", span.span_id, e);
            }
        }
    }

    /// Get all spans for a rollout.
    pub fn get_rollout_spans(&self, rollout_id: &str) -> Vec<Span> {
        match self.db.get_spans_by_rollout(rollout_id) {
            Ok(spans) => spans,
            Err(e) => {
                log::error!("[TELEMETRY] Failed to get rollout spans: {}", e);
                Vec::new()
            }
        }
    }

    /// Get all spans for a session.
    pub fn get_session_spans(&self, session_id: i64) -> Vec<Span> {
        match self.db.get_spans_by_session(session_id) {
            Ok(spans) => spans,
            Err(e) => {
                log::error!("[TELEMETRY] Failed to get session spans: {}", e);
                Vec::new()
            }
        }
    }

    /// Query spans by type and optional filters.
    pub fn query_spans(
        &self,
        span_type: Option<SpanType>,
        session_id: Option<i64>,
        since: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> Vec<Span> {
        match self.db.query_spans(span_type, session_id, since, limit) {
            Ok(spans) => spans,
            Err(e) => {
                log::error!("[TELEMETRY] Failed to query spans: {}", e);
                Vec::new()
            }
        }
    }

    /// Get a timeline view for a session.
    pub fn get_session_timeline(&self, session_id: i64) -> Timeline {
        let spans = self.get_session_spans(session_id);
        SpansToTimeline.transform(&spans)
    }

    /// Get an execution summary for a rollout.
    pub fn get_execution_summary(&self, rollout_id: &str) -> ExecutionSummary {
        let spans = self.get_rollout_spans(rollout_id);
        SpansToSummary.transform(&spans)
    }

    /// Get state-action-reward triplets for a rollout.
    pub fn get_triplets(&self, rollout_id: &str) -> Vec<Triplet> {
        let spans = self.get_rollout_spans(rollout_id);
        SpansToTriplets.transform(&spans)
    }

    /// Get reward statistics over a time period.
    pub fn get_reward_stats(&self, since: Option<DateTime<Utc>>) -> RewardStats {
        let reward_spans = self.query_spans(
            Some(SpanType::Reward),
            None,
            since,
            None,
        );

        let mut total_value = 0.0f64;
        let mut min_value = f64::MAX;
        let mut max_value = f64::MIN;
        let mut by_type: std::collections::HashMap<String, (usize, f64)> = std::collections::HashMap::new();

        for span in &reward_spans {
            let value = span.attributes.get("reward_value")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            let reward_type = span.attributes.get("reward_type")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();

            total_value += value;
            min_value = min_value.min(value);
            max_value = max_value.max(value);

            let entry = by_type.entry(reward_type).or_insert((0, 0.0));
            entry.0 += 1;
            entry.1 += value;
        }

        let total_rewards = reward_spans.len();
        let avg_value = if total_rewards > 0 { total_value / total_rewards as f64 } else { 0.0 };

        if min_value == f64::MAX { min_value = 0.0; }
        if max_value == f64::MIN { max_value = 0.0; }

        let by_type_stats = by_type
            .into_iter()
            .map(|(name, (count, total))| {
                (name, RewardTypeStat {
                    count,
                    total_value: total,
                    avg_value: if count > 0 { total / count as f64 } else { 0.0 },
                })
            })
            .collect();

        RewardStats {
            total_rewards,
            total_value,
            avg_value,
            min_value,
            max_value,
            by_type: by_type_stats,
        }
    }

    /// Prune telemetry data older than the retention policy.
    pub fn prune(&self) {
        let span_cutoff = Utc::now() - Duration::days(self.retention.span_retention_days as i64);
        let rollout_cutoff = Utc::now() - Duration::days(self.retention.rollout_retention_days as i64);

        match self.db.prune_spans_before(&span_cutoff.to_rfc3339()) {
            Ok(count) => {
                if count > 0 {
                    log::info!("[TELEMETRY] Pruned {} old spans", count);
                }
            }
            Err(e) => log::error!("[TELEMETRY] Failed to prune spans: {}", e),
        }

        match self.db.prune_rollouts_before(&rollout_cutoff.to_rfc3339()) {
            Ok(count) => {
                if count > 0 {
                    log::info!("[TELEMETRY] Pruned {} old rollouts", count);
                }
            }
            Err(e) => log::error!("[TELEMETRY] Failed to prune rollouts: {}", e),
        }
    }
}
