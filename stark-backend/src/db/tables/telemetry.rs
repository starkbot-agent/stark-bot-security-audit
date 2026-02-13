//! Telemetry database operations - execution_spans, rollouts, attempts, resource_versions

use chrono::{DateTime, Utc};
use rusqlite::Result as SqliteResult;
use serde_json::Value;

use super::super::Database;
use crate::telemetry::resource_version::ResourceBundle;
use crate::telemetry::span::{Span, SpanStatus, SpanType};

impl Database {
    // ============================================
    // Span operations
    // ============================================

    pub fn insert_span(&self, span: &Span) -> SqliteResult<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT OR IGNORE INTO execution_spans
                (span_id, sequence_id, rollout_id, session_id, attempt_idx, parent_span_id,
                 span_type, name, status, started_at, completed_at, duration_ms, attributes, error)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            rusqlite::params![
                span.span_id,
                span.sequence_id,
                span.rollout_id,
                span.session_id,
                span.attempt_idx,
                span.parent_span_id,
                serde_json::to_string(&span.span_type).unwrap_or_default().trim_matches('"'),
                span.name,
                serde_json::to_string(&span.status).unwrap_or_default().trim_matches('"'),
                span.started_at.to_rfc3339(),
                span.completed_at.map(|t| t.to_rfc3339()),
                span.duration_ms,
                serde_json::to_string(&span.attributes).unwrap_or_default(),
                span.error,
            ],
        )?;
        Ok(())
    }

    pub fn get_spans_by_rollout(&self, rollout_id: &str) -> SqliteResult<Vec<Span>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT span_id, sequence_id, rollout_id, session_id, attempt_idx, parent_span_id,
                    span_type, name, status, started_at, completed_at, duration_ms, attributes, error
             FROM execution_spans WHERE rollout_id = ?1 ORDER BY sequence_id",
        )?;

        let spans = stmt
            .query_map([rollout_id], |row| Self::row_to_span(row))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(spans)
    }

    pub fn get_spans_by_session(&self, session_id: i64) -> SqliteResult<Vec<Span>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT span_id, sequence_id, rollout_id, session_id, attempt_idx, parent_span_id,
                    span_type, name, status, started_at, completed_at, duration_ms, attributes, error
             FROM execution_spans WHERE session_id = ?1 ORDER BY sequence_id",
        )?;

        let spans = stmt
            .query_map([session_id], |row| Self::row_to_span(row))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(spans)
    }

    pub fn query_spans(
        &self,
        span_type: Option<SpanType>,
        session_id: Option<i64>,
        since: Option<DateTime<Utc>>,
        limit: Option<usize>,
    ) -> SqliteResult<Vec<Span>> {
        let conn = self.conn();

        let mut sql = String::from(
            "SELECT span_id, sequence_id, rollout_id, session_id, attempt_idx, parent_span_id,
                    span_type, name, status, started_at, completed_at, duration_ms, attributes, error
             FROM execution_spans WHERE 1=1"
        );
        let mut params: Vec<Box<dyn rusqlite::types::ToSql>> = Vec::new();

        if let Some(ref st) = span_type {
            sql.push_str(&format!(
                " AND span_type = ?{}",
                params.len() + 1
            ));
            let type_str = serde_json::to_string(st).unwrap_or_default().trim_matches('"').to_string();
            params.push(Box::new(type_str));
        }

        if let Some(sid) = session_id {
            sql.push_str(&format!(" AND session_id = ?{}", params.len() + 1));
            params.push(Box::new(sid));
        }

        if let Some(ref s) = since {
            sql.push_str(&format!(" AND started_at >= ?{}", params.len() + 1));
            params.push(Box::new(s.to_rfc3339()));
        }

        sql.push_str(" ORDER BY sequence_id");

        if let Some(lim) = limit {
            sql.push_str(&format!(" LIMIT {}", lim));
        }

        let mut stmt = conn.prepare(&sql)?;
        let param_refs: Vec<&dyn rusqlite::types::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        let spans = stmt
            .query_map(param_refs.as_slice(), |row| Self::row_to_span(row))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(spans)
    }

    pub fn prune_spans_before(&self, before: &str) -> SqliteResult<usize> {
        let conn = self.conn();
        conn.execute(
            "DELETE FROM execution_spans WHERE started_at < ?1",
            [before],
        )
    }

    fn row_to_span(row: &rusqlite::Row) -> rusqlite::Result<Span> {
        let span_type_str: String = row.get(6)?;
        let status_str: String = row.get(8)?;
        let started_at_str: String = row.get(9)?;
        let completed_at_str: Option<String> = row.get(10)?;
        let attributes_str: String = row.get(12)?;

        Ok(Span {
            span_id: row.get(0)?,
            sequence_id: row.get::<_, i64>(1)? as u64,
            rollout_id: row.get(2)?,
            session_id: row.get(3)?,
            attempt_idx: row.get::<_, i64>(4)? as u32,
            parent_span_id: row.get(5)?,
            span_type: parse_span_type(&span_type_str),
            name: row.get(7)?,
            status: parse_span_status(&status_str),
            started_at: chrono::DateTime::parse_from_rfc3339(&started_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
            completed_at: completed_at_str.and_then(|s| {
                chrono::DateTime::parse_from_rfc3339(&s)
                    .map(|dt| dt.with_timezone(&Utc))
                    .ok()
            }),
            duration_ms: row.get(11)?,
            attributes: serde_json::from_str(&attributes_str).unwrap_or(Value::Null),
            error: row.get(13)?,
        })
    }

    // ============================================
    // Rollout operations
    // ============================================

    pub fn create_rollout(&self, rollout: &crate::telemetry::rollout::Rollout) -> SqliteResult<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO rollouts
                (rollout_id, session_id, channel_id, status, config, resources_id, created_at, metadata)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                rollout.rollout_id,
                rollout.session_id,
                rollout.channel_id,
                serde_json::to_string(&rollout.status).unwrap_or_default().trim_matches('"'),
                serde_json::to_string(&rollout.config).unwrap_or_default(),
                rollout.resources_id,
                rollout.created_at.to_rfc3339(),
                serde_json::to_string(&rollout.metadata).unwrap_or_default(),
            ],
        )?;

        // Create the first attempt
        if let Some(attempt) = rollout.attempts.first() {
            self.create_attempt(&rollout.rollout_id, attempt.attempt_idx)?;
        }

        Ok(())
    }

    pub fn update_rollout_status(&self, rollout_id: &str, status: &str) -> SqliteResult<()> {
        let conn = self.conn();
        conn.execute(
            "UPDATE rollouts SET status = ?1 WHERE rollout_id = ?2",
            [status, rollout_id],
        )?;
        Ok(())
    }

    pub fn complete_rollout(
        &self,
        rollout_id: &str,
        status: &str,
        result: Option<&str>,
        error: Option<&str>,
        duration_ms: Option<u64>,
    ) -> SqliteResult<()> {
        let conn = self.conn();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE rollouts SET status = ?1, completed_at = ?2, result = ?3, error = ?4, duration_ms = ?5
             WHERE rollout_id = ?6",
            rusqlite::params![status, now, result, error, duration_ms, rollout_id],
        )?;
        Ok(())
    }

    pub fn prune_rollouts_before(&self, before: &str) -> SqliteResult<usize> {
        let conn = self.conn();
        // Also clean up associated attempts and spans
        conn.execute(
            "DELETE FROM attempts WHERE rollout_id IN (SELECT rollout_id FROM rollouts WHERE created_at < ?1)",
            [before],
        )?;
        conn.execute(
            "DELETE FROM execution_spans WHERE rollout_id IN (SELECT rollout_id FROM rollouts WHERE created_at < ?1)",
            [before],
        )?;
        conn.execute(
            "DELETE FROM rollouts WHERE created_at < ?1",
            [before],
        )
    }

    // ============================================
    // Attempt operations
    // ============================================

    pub fn create_attempt(&self, rollout_id: &str, attempt_idx: u32) -> SqliteResult<()> {
        let conn = self.conn();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "INSERT OR IGNORE INTO attempts (rollout_id, attempt_idx, started_at) VALUES (?1, ?2, ?3)",
            rusqlite::params![rollout_id, attempt_idx, now],
        )?;
        Ok(())
    }

    pub fn update_attempt(
        &self,
        rollout_id: &str,
        attempt_idx: u32,
        succeeded: bool,
        error: Option<&str>,
    ) -> SqliteResult<()> {
        let conn = self.conn();
        let now = Utc::now().to_rfc3339();
        conn.execute(
            "UPDATE attempts SET completed_at = ?1, succeeded = ?2, error = ?3
             WHERE rollout_id = ?4 AND attempt_idx = ?5",
            rusqlite::params![now, succeeded as i32, error, rollout_id, attempt_idx],
        )?;
        Ok(())
    }

    // ============================================
    // Resource version operations
    // ============================================

    pub fn create_resource_bundle(&self, bundle: &ResourceBundle) -> SqliteResult<()> {
        let conn = self.conn();
        let resources_json = serde_json::to_string(&bundle.resources).unwrap_or_default();
        conn.execute(
            "INSERT INTO resource_versions (version_id, label, is_active, resources, description, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                bundle.version_id,
                bundle.label,
                bundle.is_active as i32,
                resources_json,
                bundle.description,
                bundle.created_at.to_rfc3339(),
            ],
        )?;
        Ok(())
    }

    pub fn activate_resource_bundle(&self, version_id: &str) -> SqliteResult<()> {
        let conn = self.conn();
        // Deactivate all
        conn.execute("UPDATE resource_versions SET is_active = 0", [])?;
        // Activate the target
        conn.execute(
            "UPDATE resource_versions SET is_active = 1 WHERE version_id = ?1",
            [version_id],
        )?;
        Ok(())
    }

    pub fn get_active_resource_bundle(&self) -> SqliteResult<Option<ResourceBundle>> {
        let conn = self.conn();
        let result = conn.query_row(
            "SELECT version_id, label, is_active, resources, description, created_at
             FROM resource_versions WHERE is_active = 1 LIMIT 1",
            [],
            |row| Self::row_to_resource_bundle(row),
        );
        match result {
            Ok(bundle) => Ok(Some(bundle)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn get_latest_resource_bundle(&self) -> SqliteResult<Option<ResourceBundle>> {
        let conn = self.conn();
        let result = conn.query_row(
            "SELECT version_id, label, is_active, resources, description, created_at
             FROM resource_versions ORDER BY created_at DESC LIMIT 1",
            [],
            |row| Self::row_to_resource_bundle(row),
        );
        match result {
            Ok(bundle) => Ok(Some(bundle)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    pub fn list_resource_bundles(&self) -> SqliteResult<Vec<ResourceBundle>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT version_id, label, is_active, resources, description, created_at
             FROM resource_versions ORDER BY created_at DESC",
        )?;
        let bundles = stmt
            .query_map([], |row| Self::row_to_resource_bundle(row))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(bundles)
    }

    fn row_to_resource_bundle(row: &rusqlite::Row) -> rusqlite::Result<ResourceBundle> {
        let resources_str: String = row.get(3)?;
        let created_at_str: String = row.get(5)?;
        let is_active_int: i32 = row.get(2)?;

        Ok(ResourceBundle {
            version_id: row.get(0)?,
            label: row.get(1)?,
            is_active: is_active_int != 0,
            resources: serde_json::from_str(&resources_str).unwrap_or_default(),
            description: row.get(4)?,
            created_at: chrono::DateTime::parse_from_rfc3339(&created_at_str)
                .map(|dt| dt.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc::now()),
        })
    }
}

// ============================================
// Helper functions
// ============================================

fn parse_span_type(s: &str) -> SpanType {
    match s {
        "tool_call" => SpanType::ToolCall,
        "llm_call" => SpanType::LlmCall,
        "planning" => SpanType::Planning,
        "reward" => SpanType::Reward,
        "annotation" => SpanType::Annotation,
        "rollout" => SpanType::Rollout,
        "watchdog" => SpanType::Watchdog,
        "resource_resolution" => SpanType::ResourceResolution,
        _ => SpanType::Annotation,
    }
}

fn parse_span_status(s: &str) -> SpanStatus {
    match s {
        "running" => SpanStatus::Running,
        "succeeded" => SpanStatus::Succeeded,
        "failed" => SpanStatus::Failed,
        "timed_out" => SpanStatus::TimedOut,
        "skipped" => SpanStatus::Skipped,
        "cancelled" => SpanStatus::Cancelled,
        _ => SpanStatus::Failed, // Unknown status likely indicates corruption; default to Failed not Succeeded
    }
}
