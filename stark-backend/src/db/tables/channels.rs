//! Channel database operations

use chrono::{DateTime, Duration, Utc};
use rusqlite::Result as SqliteResult;

use crate::models::Channel;
use super::super::Database;

/// Maximum number of safe mode channels allowed at once
const MAX_SAFE_MODE_CHANNELS: usize = 10;

/// Minimum age (in minutes) for a safe mode channel to be eligible for cleanup
const SAFE_MODE_CHANNEL_MIN_AGE_MINUTES: i64 = 5;

impl Database {
    /// Create a new external channel
    pub fn create_channel(
        &self,
        channel_type: &str,
        name: &str,
        bot_token: &str,
        app_token: Option<&str>,
    ) -> SqliteResult<Channel> {
        self.create_channel_with_safe_mode(channel_type, name, bot_token, app_token, false)
    }

    /// Create a new external channel with optional safe mode
    pub fn create_channel_with_safe_mode(
        &self,
        channel_type: &str,
        name: &str,
        bot_token: &str,
        app_token: Option<&str>,
        safe_mode: bool,
    ) -> SqliteResult<Channel> {
        let conn = self.conn();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO external_channels (channel_type, name, enabled, bot_token, app_token, safe_mode, created_at, updated_at)
             VALUES (?1, ?2, 0, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![channel_type, name, bot_token, app_token, if safe_mode { 1 } else { 0 }, &now, &now],
        )?;

        let id = conn.last_insert_rowid();
        self.cache.invalidate_channels();

        Ok(Channel {
            id,
            channel_type: channel_type.to_string(),
            name: name.to_string(),
            enabled: false,
            bot_token: bot_token.to_string(),
            app_token: app_token.map(|s| s.to_string()),
            safe_mode,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        })
    }

    /// Create a safe mode channel with rate limiting (max 10, FIFO with 5-min minimum age)
    /// Returns Ok(channel) if created, Err if rate limited
    pub fn create_safe_mode_channel(
        &self,
        channel_type: &str,
        name: &str,
        bot_token: &str,
        app_token: Option<&str>,
    ) -> Result<Channel, String> {
        // Count existing safe mode channels
        let safe_mode_count = self.count_safe_mode_channels()
            .map_err(|e| format!("Failed to count safe mode channels: {}", e))?;

        if safe_mode_count >= MAX_SAFE_MODE_CHANNELS {
            // Try to clean up oldest safe mode channel if it's old enough
            if let Some(oldest) = self.get_oldest_safe_mode_channel()
                .map_err(|e| format!("Failed to get oldest safe mode channel: {}", e))?
            {
                let age = Utc::now() - oldest.created_at;
                if age >= Duration::minutes(SAFE_MODE_CHANNEL_MIN_AGE_MINUTES) {
                    log::info!(
                        "Safe mode channel limit reached, deleting oldest channel {} (age: {} minutes)",
                        oldest.id,
                        age.num_minutes()
                    );
                    self.delete_channel(oldest.id)
                        .map_err(|e| format!("Failed to delete old safe mode channel: {}", e))?;
                } else {
                    return Err(format!(
                        "Safe mode channel limit reached ({}/{}). Oldest channel is only {} minutes old (minimum {} required for cleanup).",
                        safe_mode_count, MAX_SAFE_MODE_CHANNELS, age.num_minutes(), SAFE_MODE_CHANNEL_MIN_AGE_MINUTES
                    ));
                }
            } else {
                return Err(format!(
                    "Safe mode channel limit reached ({}/{}) but no channels found to clean up",
                    safe_mode_count, MAX_SAFE_MODE_CHANNELS
                ));
            }
        }

        // Create the new safe mode channel
        self.create_channel_with_safe_mode(channel_type, name, bot_token, app_token, true)
            .map_err(|e| format!("Failed to create safe mode channel: {}", e))
    }

    /// Count the number of safe mode channels
    pub fn count_safe_mode_channels(&self) -> SqliteResult<usize> {
        let conn = self.conn();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM external_channels WHERE safe_mode = 1",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Get the oldest safe mode channel
    pub fn get_oldest_safe_mode_channel(&self) -> SqliteResult<Option<Channel>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, channel_type, name, enabled, bot_token, app_token, safe_mode, created_at, updated_at
             FROM external_channels WHERE safe_mode = 1 ORDER BY created_at ASC LIMIT 1"
        )?;

        let channel = stmt.query_row([], |row| Self::row_to_channel(row)).ok();
        Ok(channel)
    }

    /// Delete all safe mode channels older than the specified age
    pub fn cleanup_old_safe_mode_channels(&self, min_age_minutes: i64) -> SqliteResult<usize> {
        let conn = self.conn();
        let cutoff = (Utc::now() - Duration::minutes(min_age_minutes)).to_rfc3339();
        let deleted = conn.execute(
            "DELETE FROM external_channels WHERE safe_mode = 1 AND created_at < ?1",
            [&cutoff],
        )?;
        if deleted > 0 {
            self.cache.invalidate_channels();
        }
        Ok(deleted)
    }

    /// Get a channel by ID
    pub fn get_channel(&self, id: i64) -> SqliteResult<Option<Channel>> {
        if let Some(cached) = self.cache.get_channel(id) {
            return Ok(cached);
        }

        let conn = self.conn();

        let mut stmt = conn.prepare(
            "SELECT id, channel_type, name, enabled, bot_token, app_token, safe_mode, created_at, updated_at
             FROM external_channels WHERE id = ?1",
        )?;

        let channel = stmt
            .query_row([id], |row| Self::row_to_channel(row))
            .ok();

        self.cache.set_channel(id, channel.clone());
        Ok(channel)
    }

    /// List all channels
    pub fn list_channels(&self) -> SqliteResult<Vec<Channel>> {
        let conn = self.conn();

        let mut stmt = conn.prepare(
            "SELECT id, channel_type, name, enabled, bot_token, app_token, safe_mode, created_at, updated_at
             FROM external_channels ORDER BY channel_type, name",
        )?;

        let channels = stmt
            .query_map([], |row| Self::row_to_channel(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(channels)
    }

    /// List only enabled channels
    pub fn list_enabled_channels(&self) -> SqliteResult<Vec<Channel>> {
        if let Some(cached) = self.cache.get_enabled_channels() {
            return Ok((*cached).clone());
        }

        let conn = self.conn();

        let mut stmt = conn.prepare(
            "SELECT id, channel_type, name, enabled, bot_token, app_token, safe_mode, created_at, updated_at
             FROM external_channels WHERE enabled = 1 ORDER BY channel_type, name",
        )?;

        let channels: Vec<Channel> = stmt
            .query_map([], |row| Self::row_to_channel(row))?
            .filter_map(|r| r.ok())
            .collect();

        self.cache.set_enabled_channels(channels.clone());
        Ok(channels)
    }

    /// Update a channel
    pub fn update_channel(
        &self,
        id: i64,
        name: Option<&str>,
        enabled: Option<bool>,
        bot_token: Option<&str>,
        app_token: Option<Option<&str>>,
    ) -> SqliteResult<Option<Channel>> {
        let conn = self.conn();
        let now = Utc::now().to_rfc3339();

        // Build dynamic update query
        let mut updates = vec!["updated_at = ?1".to_string()];
        let mut param_idx = 2;

        if name.is_some() {
            updates.push(format!("name = ?{}", param_idx));
            param_idx += 1;
        }
        if enabled.is_some() {
            updates.push(format!("enabled = ?{}", param_idx));
            param_idx += 1;
        }
        if bot_token.is_some() {
            updates.push(format!("bot_token = ?{}", param_idx));
            param_idx += 1;
        }
        if app_token.is_some() {
            updates.push(format!("app_token = ?{}", param_idx));
            param_idx += 1;
        }

        let sql = format!(
            "UPDATE external_channels SET {} WHERE id = ?{}",
            updates.join(", "),
            param_idx
        );

        // Build params dynamically
        let mut params: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(now)];

        if let Some(n) = name {
            params.push(Box::new(n.to_string()));
        }
        if let Some(e) = enabled {
            params.push(Box::new(if e { 1 } else { 0 }));
        }
        if let Some(t) = bot_token {
            params.push(Box::new(t.to_string()));
        }
        if let Some(at) = app_token {
            params.push(Box::new(at.map(|s| s.to_string())));
        }
        params.push(Box::new(id));

        let params_ref: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();
        conn.execute(&sql, params_ref.as_slice())?;

        drop(conn);
        self.cache.invalidate_channels();
        self.get_channel(id)
    }

    /// Enable or disable a channel
    pub fn set_channel_enabled(&self, id: i64, enabled: bool) -> SqliteResult<bool> {
        let conn = self.conn();
        let now = Utc::now().to_rfc3339();

        let rows_affected = conn.execute(
            "UPDATE external_channels SET enabled = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![if enabled { 1 } else { 0 }, &now, id],
        )?;

        self.cache.invalidate_channels();
        Ok(rows_affected > 0)
    }

    /// Delete a channel
    pub fn delete_channel(&self, id: i64) -> SqliteResult<bool> {
        let conn = self.conn();
        let rows_affected = conn.execute(
            "DELETE FROM external_channels WHERE id = ?1",
            [id],
        )?;
        self.cache.invalidate_channels();
        Ok(rows_affected > 0)
    }

    /// Set the safe_mode flag on a channel
    pub fn set_channel_safe_mode(&self, id: i64, safe_mode: bool) -> SqliteResult<bool> {
        let conn = self.conn();
        let now = Utc::now().to_rfc3339();
        let rows_affected = conn.execute(
            "UPDATE external_channels SET safe_mode = ?1, updated_at = ?2 WHERE id = ?3",
            rusqlite::params![if safe_mode { 1 } else { 0 }, &now, id],
        )?;
        self.cache.invalidate_channels();
        Ok(rows_affected > 0)
    }

    /// List all non-safe-mode channels (for backup)
    pub fn list_channels_for_backup(&self) -> SqliteResult<Vec<Channel>> {
        let conn = self.conn();

        let mut stmt = conn.prepare(
            "SELECT id, channel_type, name, enabled, bot_token, app_token, safe_mode, created_at, updated_at
             FROM external_channels WHERE safe_mode = 0 ORDER BY channel_type, name",
        )?;

        let channels = stmt
            .query_map([], |row| Self::row_to_channel(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(channels)
    }

    /// Clear all non-safe-mode channels for restore (wipe before restore to prevent duplicates)
    pub fn clear_channels_for_restore(&self) -> SqliteResult<usize> {
        let conn = self.conn();
        // Only delete non-safe-mode channels; safe mode channels are temporary
        let rows_deleted = conn.execute("DELETE FROM external_channels WHERE safe_mode = 0", [])?;
        self.cache.invalidate_channels();
        Ok(rows_deleted)
    }

    fn row_to_channel(row: &rusqlite::Row) -> rusqlite::Result<Channel> {
        // Column order: id, channel_type, name, enabled, bot_token, app_token, safe_mode, created_at, updated_at
        let created_at_str: String = row.get(7)?;
        let updated_at_str: String = row.get(8)?;

        Ok(Channel {
            id: row.get(0)?,
            channel_type: row.get(1)?,
            name: row.get(2)?,
            enabled: row.get::<_, i32>(3)? != 0,
            bot_token: row.get(4)?,
            app_token: row.get(5)?,
            safe_mode: row.get::<_, i32>(6).unwrap_or(0) != 0,
            created_at: DateTime::parse_from_rfc3339(&created_at_str)
                .unwrap()
                .with_timezone(&Utc),
            updated_at: DateTime::parse_from_rfc3339(&updated_at_str)
                .unwrap()
                .with_timezone(&Utc),
        })
    }
}
