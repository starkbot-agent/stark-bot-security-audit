//! Telegram chat message log - passive storage of ALL messages in Telegram chats
//!
//! Independent of the session system. Stores every text message the bot sees,
//! enabling cross-session readHistory queries.

use crate::db::Database;
use rusqlite::Result as SqliteResult;

#[derive(Debug, Clone)]
pub struct TelegramChatMessage {
    pub id: i64,
    pub channel_id: i64,
    pub chat_id: String,
    pub user_id: Option<String>,
    pub user_name: Option<String>,
    pub content: String,
    pub platform_message_id: Option<String>,
    pub is_bot_response: bool,
    pub created_at: String,
}

impl Database {
    /// Store a Telegram chat message in the passive log
    pub fn store_telegram_chat_message(
        &self,
        channel_id: i64,
        chat_id: &str,
        user_id: Option<&str>,
        user_name: Option<&str>,
        content: &str,
        platform_message_id: Option<&str>,
        is_bot_response: bool,
    ) -> SqliteResult<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO telegram_chat_messages
             (channel_id, chat_id, user_id, user_name, content, platform_message_id, is_bot_response, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'))",
            rusqlite::params![
                channel_id,
                chat_id,
                user_id,
                user_name,
                content,
                platform_message_id,
                is_bot_response as i32,
            ],
        )?;
        Ok(())
    }

    /// Get recent messages from a Telegram chat's passive log
    pub fn get_recent_telegram_chat_messages(
        &self,
        channel_id: i64,
        chat_id: &str,
        limit: i32,
    ) -> SqliteResult<Vec<TelegramChatMessage>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, channel_id, chat_id, user_id, user_name, content,
                    platform_message_id, is_bot_response, created_at
             FROM telegram_chat_messages
             WHERE channel_id = ?1 AND chat_id = ?2
             ORDER BY created_at DESC, id DESC
             LIMIT ?3",
        )?;

        let rows = stmt.query_map(rusqlite::params![channel_id, chat_id, limit], |row| {
            Ok(TelegramChatMessage {
                id: row.get(0)?,
                channel_id: row.get(1)?,
                chat_id: row.get(2)?,
                user_id: row.get(3)?,
                user_name: row.get(4)?,
                content: row.get(5)?,
                platform_message_id: row.get(6)?,
                is_bot_response: row.get::<_, i32>(7)? != 0,
                created_at: row.get(8)?,
            })
        })?;

        let mut messages: Vec<TelegramChatMessage> = rows.collect::<SqliteResult<Vec<_>>>()?;
        // Reverse so oldest is first (we fetched DESC for LIMIT)
        messages.reverse();
        Ok(messages)
    }

    /// Clean up old Telegram chat messages (keep last N days)
    pub fn cleanup_telegram_chat_messages(&self, days: i64) -> SqliteResult<usize> {
        let conn = self.conn();
        let deleted = conn.execute(
            "DELETE FROM telegram_chat_messages WHERE created_at < datetime('now', ?1)",
            [format!("-{} days", days)],
        )?;
        Ok(deleted)
    }
}
