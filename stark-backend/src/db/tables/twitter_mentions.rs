//! Twitter mentions tracking - prevent duplicate processing of tweets
//!
//! Stores processed tweet IDs to avoid responding to the same mention twice.

use crate::db::Database;
use rusqlite::Result as SqliteResult;

impl Database {
    /// Check if a tweet has already been processed
    pub fn is_tweet_processed(&self, tweet_id: &str) -> SqliteResult<bool> {
        let conn = self.conn();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM twitter_processed_mentions WHERE tweet_id = ?1",
            [tweet_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Mark a tweet as processed
    pub fn mark_tweet_processed(
        &self,
        tweet_id: &str,
        channel_id: i64,
        author_id: &str,
        author_username: &str,
        tweet_text: &str,
    ) -> SqliteResult<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT OR IGNORE INTO twitter_processed_mentions
             (tweet_id, channel_id, author_id, author_username, tweet_text, processed_at)
             VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))",
            rusqlite::params![tweet_id, channel_id, author_id, author_username, tweet_text],
        )?;
        Ok(())
    }

    /// Get the most recent processed tweet ID for a channel
    /// Used for pagination with `since_id` parameter
    pub fn get_last_processed_tweet_id(&self, channel_id: i64) -> SqliteResult<Option<String>> {
        let conn = self.conn();
        let result = conn.query_row(
            "SELECT tweet_id FROM twitter_processed_mentions
             WHERE channel_id = ?1
             ORDER BY processed_at DESC LIMIT 1",
            [channel_id],
            |row| row.get(0),
        );
        match result {
            Ok(id) => Ok(Some(id)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Clean up old processed mentions (keep last 30 days by default)
    pub fn cleanup_old_processed_mentions(&self, days: i64) -> SqliteResult<usize> {
        let conn = self.conn();
        let deleted = conn.execute(
            "DELETE FROM twitter_processed_mentions
             WHERE processed_at < datetime('now', ?1)",
            [format!("-{} days", days)],
        )?;
        Ok(deleted)
    }

    /// Get count of processed mentions for a channel
    pub fn get_processed_mention_count(&self, channel_id: i64) -> SqliteResult<i64> {
        let conn = self.conn();
        conn.query_row(
            "SELECT COUNT(*) FROM twitter_processed_mentions WHERE channel_id = ?1",
            [channel_id],
            |row| row.get(0),
        )
    }
}
