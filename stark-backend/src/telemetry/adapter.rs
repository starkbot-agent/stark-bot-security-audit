//! Transform raw spans into useful views: session timelines,
//! execution summaries, state-action-reward triplets.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::span::{Span, SpanStatus, SpanType};

/// Generic adapter trait for transforming spans into different views.
pub trait Adapter<From, To> {
    fn transform(&self, input: &[From]) -> To;
}

// ─── Timeline ──────────────────────────────────────────────────────

/// A single entry in a session timeline.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimelineEntry {
    pub sequence_id: u64,
    pub timestamp: DateTime<Utc>,
    pub span_type: SpanType,
    pub name: String,
    pub status: SpanStatus,
    pub duration_ms: Option<u64>,
    pub summary: String,
    pub attributes: Value,
}

/// A chronologically ordered timeline of execution events.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Timeline {
    pub session_id: i64,
    pub rollout_id: String,
    pub entries: Vec<TimelineEntry>,
    pub total_duration_ms: u64,
    pub started_at: DateTime<Utc>,
    pub ended_at: Option<DateTime<Utc>>,
}

/// Transforms a collection of spans into a chronological timeline.
pub struct SpansToTimeline;

impl Adapter<Span, Timeline> for SpansToTimeline {
    fn transform(&self, spans: &[Span]) -> Timeline {
        let mut entries: Vec<TimelineEntry> = spans
            .iter()
            .map(|span| {
                let summary = match span.span_type {
                    SpanType::ToolCall => format!("Tool: {} ({})", span.name, status_label(span.status)),
                    SpanType::LlmCall => format!("LLM: {} ({})", span.name, status_label(span.status)),
                    SpanType::Planning => format!("Planning: {}", span.name),
                    SpanType::Reward => {
                        let value = span.attributes.get("reward_value")
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0);
                        format!("Reward: {} = {:.2}", span.name, value)
                    }
                    SpanType::Annotation => {
                        let key = span.attributes.get("annotation_key")
                            .and_then(|v| v.as_str())
                            .unwrap_or(&span.name);
                        format!("Note: {}", key)
                    }
                    SpanType::Watchdog => format!("Watchdog: {}", span.name),
                    SpanType::Rollout => format!("Rollout: {}", span.name),
                    SpanType::ResourceResolution => format!("Resource: {}", span.name),
                };

                TimelineEntry {
                    sequence_id: span.sequence_id,
                    timestamp: span.started_at,
                    span_type: span.span_type,
                    name: span.name.clone(),
                    status: span.status,
                    duration_ms: span.duration_ms,
                    summary,
                    attributes: span.attributes.clone(),
                }
            })
            .collect();

        entries.sort_by_key(|e| e.sequence_id);

        let started_at = entries.first().map(|e| e.timestamp).unwrap_or_else(Utc::now);
        let ended_at = entries.last().and_then(|e| {
            e.duration_ms.map(|d| e.timestamp + chrono::Duration::milliseconds(d as i64))
        });
        let total_duration_ms = ended_at
            .map(|end| (end - started_at).num_milliseconds().max(0) as u64)
            .unwrap_or(0);

        let rollout_id = spans.first()
            .map(|s| s.rollout_id.clone())
            .unwrap_or_default();
        let session_id = spans.first()
            .map(|s| s.session_id)
            .unwrap_or(0);

        Timeline {
            session_id,
            rollout_id,
            entries,
            total_duration_ms,
            started_at,
            ended_at,
        }
    }
}

// ─── Summary ───────────────────────────────────────────────────────

/// An aggregated summary of an execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionSummary {
    pub rollout_id: String,
    pub session_id: i64,
    pub total_spans: usize,
    pub tool_calls: usize,
    pub tool_successes: usize,
    pub tool_failures: usize,
    pub llm_calls: usize,
    pub rewards_emitted: usize,
    pub total_reward: f64,
    pub avg_tool_duration_ms: f64,
    pub avg_llm_duration_ms: f64,
    pub total_duration_ms: u64,
    pub attempt_count: u32,
    pub had_timeouts: bool,
    pub had_loops: bool,
}

/// Transforms spans into an aggregated execution summary.
pub struct SpansToSummary;

impl Adapter<Span, ExecutionSummary> for SpansToSummary {
    fn transform(&self, spans: &[Span]) -> ExecutionSummary {
        let mut tool_calls = 0usize;
        let mut tool_successes = 0usize;
        let mut tool_failures = 0usize;
        let mut llm_calls = 0usize;
        let mut rewards_emitted = 0usize;
        let mut total_reward = 0.0f64;
        let mut tool_durations = Vec::new();
        let mut llm_durations = Vec::new();
        let mut had_timeouts = false;
        let mut had_loops = false;
        let mut max_attempt: u32 = 0;

        for span in spans {
            max_attempt = max_attempt.max(span.attempt_idx);

            match span.span_type {
                SpanType::ToolCall => {
                    tool_calls += 1;
                    if span.status == SpanStatus::Succeeded {
                        tool_successes += 1;
                    } else if span.status == SpanStatus::Failed {
                        tool_failures += 1;
                    }
                    if let Some(d) = span.duration_ms {
                        tool_durations.push(d);
                    }
                    if span.status == SpanStatus::TimedOut {
                        had_timeouts = true;
                    }
                }
                SpanType::LlmCall => {
                    llm_calls += 1;
                    if let Some(d) = span.duration_ms {
                        llm_durations.push(d);
                    }
                    if span.status == SpanStatus::TimedOut {
                        had_timeouts = true;
                    }
                }
                SpanType::Reward => {
                    rewards_emitted += 1;
                    if let Some(v) = span.attributes.get("reward_value").and_then(|v| v.as_f64()) {
                        total_reward += v;
                    }
                    if span.name == "loop_detected" {
                        had_loops = true;
                    }
                }
                SpanType::Watchdog => {
                    if span.status == SpanStatus::TimedOut {
                        had_timeouts = true;
                    }
                }
                _ => {}
            }
        }

        let avg_tool_duration_ms = if tool_durations.is_empty() {
            0.0
        } else {
            tool_durations.iter().sum::<u64>() as f64 / tool_durations.len() as f64
        };

        let avg_llm_duration_ms = if llm_durations.is_empty() {
            0.0
        } else {
            llm_durations.iter().sum::<u64>() as f64 / llm_durations.len() as f64
        };

        let total_duration_ms = spans.iter()
            .filter_map(|s| s.completed_at.map(|end| (end - s.started_at).num_milliseconds().max(0) as u64))
            .sum();

        let rollout_id = spans.first().map(|s| s.rollout_id.clone()).unwrap_or_default();
        let session_id = spans.first().map(|s| s.session_id).unwrap_or(0);

        ExecutionSummary {
            rollout_id,
            session_id,
            total_spans: spans.len(),
            tool_calls,
            tool_successes,
            tool_failures,
            llm_calls,
            rewards_emitted,
            total_reward,
            avg_tool_duration_ms,
            avg_llm_duration_ms,
            total_duration_ms,
            attempt_count: max_attempt + 1,
            had_timeouts,
            had_loops,
        }
    }
}

// ─── Triplets ──────────────────────────────────────────────────────

/// A state-action-reward triplet for potential optimization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Triplet {
    /// The state before the action (context: previous spans, iteration count)
    pub state: Value,
    /// The action taken (tool name + args, or LLM prompt)
    pub action: Value,
    /// The reward signal following the action
    pub reward: f64,
    /// Timestamp of the action
    pub timestamp: DateTime<Utc>,
}

/// Transforms spans into state-action-reward triplets.
pub struct SpansToTriplets;

impl Adapter<Span, Vec<Triplet>> for SpansToTriplets {
    fn transform(&self, spans: &[Span]) -> Vec<Triplet> {
        let mut triplets = Vec::new();
        let mut sorted_spans: Vec<&Span> = spans.iter().collect();
        sorted_spans.sort_by_key(|s| s.sequence_id);

        // Pair each tool/LLM action with the next reward signal
        let mut pending_action: Option<&Span> = None;

        for span in &sorted_spans {
            match span.span_type {
                SpanType::ToolCall | SpanType::LlmCall => {
                    // If there's a pending action without a reward, assign 0.0
                    if let Some(prev) = pending_action.take() {
                        triplets.push(Triplet {
                            state: serde_json::json!({
                                "sequence_id": prev.sequence_id,
                                "attempt_idx": prev.attempt_idx,
                            }),
                            action: serde_json::json!({
                                "type": format!("{:?}", prev.span_type),
                                "name": prev.name,
                                "attributes": prev.attributes,
                            }),
                            reward: 0.0,
                            timestamp: prev.started_at,
                        });
                    }
                    pending_action = Some(span);
                }
                SpanType::Reward => {
                    let reward_value = span.attributes
                        .get("reward_value")
                        .and_then(|v| v.as_f64())
                        .unwrap_or(0.0);

                    if let Some(action) = pending_action.take() {
                        triplets.push(Triplet {
                            state: serde_json::json!({
                                "sequence_id": action.sequence_id,
                                "attempt_idx": action.attempt_idx,
                            }),
                            action: serde_json::json!({
                                "type": format!("{:?}", action.span_type),
                                "name": action.name,
                                "attributes": action.attributes,
                            }),
                            reward: reward_value,
                            timestamp: action.started_at,
                        });
                    }
                }
                _ => {}
            }
        }

        // Handle any remaining pending action
        if let Some(action) = pending_action {
            triplets.push(Triplet {
                state: serde_json::json!({
                    "sequence_id": action.sequence_id,
                    "attempt_idx": action.attempt_idx,
                }),
                action: serde_json::json!({
                    "type": format!("{:?}", action.span_type),
                    "name": action.name,
                    "attributes": action.attributes,
                }),
                reward: 0.0,
                timestamp: action.started_at,
            });
        }

        triplets
    }
}

// ─── Helpers ───────────────────────────────────────────────────────

fn status_label(status: SpanStatus) -> &'static str {
    match status {
        SpanStatus::Running => "running",
        SpanStatus::Succeeded => "ok",
        SpanStatus::Failed => "failed",
        SpanStatus::TimedOut => "timeout",
        SpanStatus::Skipped => "skipped",
        SpanStatus::Cancelled => "cancelled",
    }
}
