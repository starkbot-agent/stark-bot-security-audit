//! Agent contexts table - agent state persistence
//!
//! Stores AgentContext between messages so the agent can continue
//! across a multi-turn conversation.

use crate::ai::multi_agent::types::{ActiveSkill, AgentContext, AgentMode, AgentSubtype, TaskQueue};
use crate::db::Database;
use chrono::Utc;
use rusqlite::{params, Result as SqliteResult};

impl Database {
    /// Get agent context for a session (if exists)
    pub fn get_agent_context(&self, session_id: i64) -> SqliteResult<Option<AgentContext>> {
        let conn = self.conn();

        let mut stmt = conn.prepare(
            "SELECT original_request, mode, mode_iterations, total_iterations,
                    exploration_notes, scratchpad, subtype, active_skill_json
             FROM agent_contexts
             WHERE session_id = ?",
        )?;

        let result = stmt.query_row(params![session_id], |row| {
            let original_request: String = row.get(0)?;
            let mode_str: String = row.get(1)?;
            let mode_iterations: u32 = row.get(2)?;
            let total_iterations: u32 = row.get(3)?;
            let notes_json: String = row.get(4)?;
            let scratchpad: String = row.get(5)?;
            let subtype_str: Option<String> = row.get(6).ok();
            let active_skill_json: Option<String> = row.get(7).ok().flatten();

            // Parse mode (defaults to Assistant)
            let mode = AgentMode::from_str(&mode_str).unwrap_or_default();

            // Parse subtype
            let subtype = subtype_str
                .and_then(|s| AgentSubtype::from_str(&s))
                .unwrap_or_default();

            // Parse JSON fields
            let exploration_notes: Vec<String> =
                serde_json::from_str(&notes_json).unwrap_or_default();

            // Parse active skill
            let active_skill: Option<ActiveSkill> = active_skill_json
                .and_then(|json| serde_json::from_str(&json).ok());

            Ok(AgentContext {
                original_request,
                exploration_notes,
                mode,
                subtype,
                mode_iterations,
                total_iterations,
                scratchpad,
                active_skill,
                actual_tool_calls: 0,      // Reset on load
                no_tool_warnings: 0,       // Reset on load
                waiting_for_user_context: None, // Reset on load
                task_queue: TaskQueue::default(), // Reset on load
                planner_completed: false,  // Reset on load
                selected_network: None,    // Reset on load
            })
        });

        match result {
            Ok(ctx) => Ok(Some(ctx)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Create or update agent context for a session
    pub fn save_agent_context(
        &self,
        session_id: i64,
        context: &AgentContext,
    ) -> SqliteResult<()> {
        let conn = self.conn();
        let now = Utc::now().to_rfc3339();

        // Serialize JSON fields
        let notes_json = serde_json::to_string(&context.exploration_notes)
            .unwrap_or_else(|_| "[]".to_string());
        let active_skill_json: Option<String> = context.active_skill.as_ref()
            .and_then(|s| serde_json::to_string(s).ok());

        // Use INSERT OR REPLACE for upsert behavior
        // Note: Using simplified schema - old columns will be NULL/defaults
        conn.execute(
            "INSERT OR REPLACE INTO agent_contexts (
                session_id, original_request, mode, mode_iterations, total_iterations,
                exploration_notes, scratchpad, subtype, active_skill_json,
                context_sufficient, plan_ready, findings, plan_summary, tasks_json,
                created_at, updated_at
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9,
                0, 0, '[]', NULL, '{\"tasks\":[]}',
                COALESCE((SELECT created_at FROM agent_contexts WHERE session_id = ?1), ?10),
                ?10
            )",
            params![
                session_id,
                context.original_request,
                context.mode.to_string(),
                context.mode_iterations,
                context.total_iterations,
                notes_json,
                context.scratchpad,
                context.subtype.as_str(),
                active_skill_json,
                now,
            ],
        )?;

        Ok(())
    }

    /// Delete agent context for a session (e.g., on session reset)
    pub fn delete_agent_context(&self, session_id: i64) -> SqliteResult<()> {
        let conn = self.conn();
        conn.execute(
            "DELETE FROM agent_contexts WHERE session_id = ?",
            params![session_id],
        )?;
        Ok(())
    }

    /// Check if a session has an agent context
    pub fn has_agent_context(&self, session_id: i64) -> SqliteResult<bool> {
        let conn = self.conn();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM agent_contexts WHERE session_id = ?",
            params![session_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}
