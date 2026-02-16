//! Async write-behind buffer for session messages.
//!
//! Tool call and tool result messages are sent to an in-memory channel
//! and written to the database by a background task. This keeps DB writes
//! off the agentic loop's hot path.

use crate::db::Database;
use crate::models::session_message::MessageRole;
use std::sync::Arc;
use tokio::sync::mpsc;

/// A queued session message waiting to be written to the database.
struct PendingMessage {
    session_id: i64,
    role: MessageRole,
    content: String,
    user_name: Option<String>,
}

/// Non-blocking writer that queues session messages for async DB persistence.
#[derive(Clone)]
pub struct SessionMessageWriter {
    tx: mpsc::UnboundedSender<PendingMessage>,
}

impl SessionMessageWriter {
    /// Create a new writer and spawn the background drain task.
    pub fn new(db: Arc<Database>) -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        tokio::spawn(Self::drain_loop(db, rx));
        Self { tx }
    }

    /// Queue a message for async DB write. Returns immediately.
    pub fn send(
        &self,
        session_id: i64,
        role: MessageRole,
        content: String,
        user_name: Option<&str>,
    ) {
        let _ = self.tx.send(PendingMessage {
            session_id,
            role,
            content,
            user_name: user_name.map(|s| s.to_string()),
        });
    }

    /// Background loop that drains the channel and writes to DB.
    /// Batches messages that have accumulated while processing.
    async fn drain_loop(db: Arc<Database>, mut rx: mpsc::UnboundedReceiver<PendingMessage>) {
        let mut batch: Vec<PendingMessage> = Vec::with_capacity(16);

        while let Some(msg) = rx.recv().await {
            batch.push(msg);

            // Drain any additional messages that arrived while we were waiting
            while let Ok(msg) = rx.try_recv() {
                batch.push(msg);
            }

            // Write the batch in a single transaction
            let entries: Vec<(i64, MessageRole, String, Option<String>, Option<String>)> = batch
                .drain(..)
                .map(|m| (m.session_id, m.role, m.content, None, m.user_name))
                .collect();

            if let Err(e) = db.add_session_messages_batch(&entries) {
                log::error!("[SESSION_WRITER] Failed to batch-write {} messages: {}", entries.len(), e);
                // Fall back to individual writes
                for (session_id, role, content, _, user_name) in entries {
                    if let Err(e) = db.add_session_message(
                        session_id,
                        role,
                        &content,
                        None,
                        user_name.as_deref(),
                        None,
                        None,
                    ) {
                        log::error!("[SESSION_WRITER] Individual write also failed: {}", e);
                    }
                }
            }
        }

        log::info!("[SESSION_WRITER] Background writer shutting down");
    }
}
