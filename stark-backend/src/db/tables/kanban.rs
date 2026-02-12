//! Kanban board database operations (kanban_items)

use chrono::{DateTime, Utc};
use rusqlite::Result as SqliteResult;
use serde::{Deserialize, Serialize};

use super::super::Database;

/// A kanban board item
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KanbanItem {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub status: String,
    pub priority: i32,
    pub session_id: Option<i64>,
    pub result: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Request to create a new kanban item
#[derive(Debug, Deserialize)]
pub struct CreateKanbanItemRequest {
    pub title: String,
    pub description: Option<String>,
    pub priority: Option<i32>,
}

/// Request to update a kanban item
#[derive(Debug, Default, Deserialize)]
pub struct UpdateKanbanItemRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub priority: Option<i32>,
    pub session_id: Option<i64>,
    pub result: Option<String>,
}

impl Database {
    /// Create a new kanban item
    pub fn create_kanban_item(&self, request: &CreateKanbanItemRequest) -> SqliteResult<KanbanItem> {
        let conn = self.conn();
        let now = Utc::now().to_rfc3339();
        let description = request.description.as_deref().unwrap_or("");
        let priority = request.priority.unwrap_or(0);

        conn.execute(
            "INSERT INTO kanban_items (title, description, priority, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            rusqlite::params![&request.title, description, priority, &now],
        )?;

        let id = conn.last_insert_rowid();
        let created_at = DateTime::parse_from_rfc3339(&now)
            .unwrap()
            .with_timezone(&Utc);

        Ok(KanbanItem {
            id,
            title: request.title.clone(),
            description: description.to_string(),
            status: "ready".to_string(),
            priority,
            session_id: None,
            result: None,
            created_at,
            updated_at: created_at,
        })
    }

    /// Get a kanban item by ID
    pub fn get_kanban_item(&self, id: i64) -> SqliteResult<Option<KanbanItem>> {
        let conn = self.conn();
        let item = conn
            .query_row(
                "SELECT id, title, description, status, priority, session_id, result, created_at, updated_at
                 FROM kanban_items WHERE id = ?1",
                [id],
                |row| Self::row_to_kanban_item(row),
            )
            .ok();
        Ok(item)
    }

    /// List all kanban items ordered by priority DESC, created_at ASC
    pub fn list_kanban_items(&self) -> SqliteResult<Vec<KanbanItem>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, title, description, status, priority, session_id, result, created_at, updated_at
             FROM kanban_items ORDER BY priority DESC, created_at ASC",
        )?;

        let items = stmt
            .query_map([], |row| Self::row_to_kanban_item(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(items)
    }

    /// List kanban items filtered by status
    pub fn list_kanban_items_by_status(&self, status: &str) -> SqliteResult<Vec<KanbanItem>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, title, description, status, priority, session_id, result, created_at, updated_at
             FROM kanban_items WHERE status = ?1 ORDER BY priority DESC, created_at ASC",
        )?;

        let items = stmt
            .query_map([status], |row| Self::row_to_kanban_item(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(items)
    }

    /// Update a kanban item with dynamic fields
    pub fn update_kanban_item(&self, id: i64, request: &UpdateKanbanItemRequest) -> SqliteResult<Option<KanbanItem>> {
        let conn = self.conn();
        let now = Utc::now().to_rfc3339();

        let mut updates = vec!["updated_at = ?1".to_string()];
        let mut param_idx = 2;

        if request.title.is_some() {
            updates.push(format!("title = ?{}", param_idx));
            param_idx += 1;
        }
        if request.description.is_some() {
            updates.push(format!("description = ?{}", param_idx));
            param_idx += 1;
        }
        if request.status.is_some() {
            updates.push(format!("status = ?{}", param_idx));
            param_idx += 1;
        }
        if request.priority.is_some() {
            updates.push(format!("priority = ?{}", param_idx));
            param_idx += 1;
        }
        if request.session_id.is_some() {
            updates.push(format!("session_id = ?{}", param_idx));
            param_idx += 1;
        }
        if request.result.is_some() {
            updates.push(format!("result = ?{}", param_idx));
            param_idx += 1;
        }

        let sql = format!(
            "UPDATE kanban_items SET {} WHERE id = ?{}",
            updates.join(", "),
            param_idx
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];
        if let Some(ref title) = request.title {
            params.push(Box::new(title.clone()));
        }
        if let Some(ref description) = request.description {
            params.push(Box::new(description.clone()));
        }
        if let Some(ref status) = request.status {
            params.push(Box::new(status.clone()));
        }
        if let Some(priority) = request.priority {
            params.push(Box::new(priority));
        }
        if let Some(session_id) = request.session_id {
            params.push(Box::new(session_id));
        }
        if let Some(ref result) = request.result {
            params.push(Box::new(result.clone()));
        }
        params.push(Box::new(id));

        let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, params_ref.as_slice())?;

        drop(conn);
        self.get_kanban_item(id)
    }

    /// Delete a kanban item
    pub fn delete_kanban_item(&self, id: i64) -> SqliteResult<bool> {
        let conn = self.conn();
        let rows_affected = conn.execute("DELETE FROM kanban_items WHERE id = ?1", [id])?;
        Ok(rows_affected > 0)
    }

    /// Atomically pick the highest-priority ready task and move it to in_progress
    pub fn pick_next_kanban_task(&self) -> SqliteResult<Option<KanbanItem>> {
        let conn = self.conn();
        let now = Utc::now().to_rfc3339();

        // Find highest-priority ready item
        let item_id: Option<i64> = conn
            .query_row(
                "SELECT id FROM kanban_items WHERE status = 'ready'
                 ORDER BY priority DESC, created_at ASC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .ok();

        let item_id = match item_id {
            Some(id) => id,
            None => return Ok(None),
        };

        // Atomically update to in_progress
        conn.execute(
            "UPDATE kanban_items SET status = 'in_progress', updated_at = ?1 WHERE id = ?2 AND status = 'ready'",
            rusqlite::params![&now, item_id],
        )?;

        drop(conn);
        self.get_kanban_item(item_id)
    }

    fn row_to_kanban_item(row: &rusqlite::Row) -> rusqlite::Result<KanbanItem> {
        let created_at_str: String = row.get(7)?;
        let updated_at_str: String = row.get(8)?;

        Ok(KanbanItem {
            id: row.get(0)?,
            title: row.get(1)?,
            description: row.get(2)?,
            status: row.get(3)?,
            priority: row.get(4)?,
            session_id: row.get(5)?,
            result: row.get(6)?,
            created_at: DateTime::parse_from_rfc3339(&created_at_str)
                .unwrap()
                .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                .unwrap()
                .with_timezone(&Utc),
        })
    }
}
