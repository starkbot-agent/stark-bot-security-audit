//! Broadcasted transactions database operations
//!
//! Persistent history of all crypto transaction broadcasts.

use chrono::{DateTime, Utc};
use rusqlite::Result as SqliteResult;
use serde::{Deserialize, Serialize};

use super::super::Database;

/// Status of a broadcasted transaction
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BroadcastedTxStatus {
    /// Transaction has been broadcast to the network
    Broadcast,
    /// Transaction confirmed on-chain
    Confirmed,
    /// Broadcast or confirmation failed
    Failed,
}

impl std::fmt::Display for BroadcastedTxStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BroadcastedTxStatus::Broadcast => write!(f, "broadcast"),
            BroadcastedTxStatus::Confirmed => write!(f, "confirmed"),
            BroadcastedTxStatus::Failed => write!(f, "failed"),
        }
    }
}

impl std::str::FromStr for BroadcastedTxStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "broadcast" => Ok(BroadcastedTxStatus::Broadcast),
            "confirmed" => Ok(BroadcastedTxStatus::Confirmed),
            "failed" => Ok(BroadcastedTxStatus::Failed),
            _ => Err(format!("Unknown status: {}", s)),
        }
    }
}

/// Broadcast mode - how the transaction was broadcast
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BroadcastMode {
    /// Broadcast via rogue mode (agent autonomously)
    Rogue,
    /// Broadcast via partner approval (user confirmed)
    Partner,
}

impl std::fmt::Display for BroadcastMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BroadcastMode::Rogue => write!(f, "rogue"),
            BroadcastMode::Partner => write!(f, "partner"),
        }
    }
}

impl std::str::FromStr for BroadcastMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "rogue" => Ok(BroadcastMode::Rogue),
            "partner" => Ok(BroadcastMode::Partner),
            _ => Err(format!("Unknown broadcast mode: {}", s)),
        }
    }
}

/// A broadcasted transaction record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BroadcastedTransaction {
    pub id: i64,
    pub uuid: String,
    pub network: String,
    pub from_address: String,
    pub to_address: String,
    pub value: String,
    pub value_formatted: String,
    pub tx_hash: Option<String>,
    pub explorer_url: Option<String>,
    pub status: BroadcastedTxStatus,
    pub broadcast_mode: BroadcastMode,
    pub error: Option<String>,
    pub broadcast_at: DateTime<Utc>,
    pub confirmed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

/// Data needed to record a new broadcast
pub struct RecordBroadcastRequest {
    pub uuid: String,
    pub network: String,
    pub from_address: String,
    pub to_address: String,
    pub value: String,
    pub value_formatted: String,
    pub tx_hash: Option<String>,
    pub explorer_url: Option<String>,
    pub broadcast_mode: BroadcastMode,
}

impl Database {
    /// Record a new broadcast transaction
    pub fn record_broadcast(&self, req: RecordBroadcastRequest) -> SqliteResult<i64> {
        let conn = self.conn();
        let now = Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO broadcasted_transactions
             (uuid, network, from_address, to_address, value, value_formatted,
              tx_hash, explorer_url, status, broadcast_mode, broadcast_at, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'broadcast', ?9, ?10, ?10)",
            rusqlite::params![
                req.uuid,
                req.network,
                req.from_address,
                req.to_address,
                req.value,
                req.value_formatted,
                req.tx_hash,
                req.explorer_url,
                req.broadcast_mode.to_string(),
                now,
            ],
        )?;

        Ok(conn.last_insert_rowid())
    }

    /// Update a broadcasted transaction status
    pub fn update_broadcast_status(
        &self,
        uuid: &str,
        status: BroadcastedTxStatus,
        error: Option<&str>,
    ) -> SqliteResult<bool> {
        let conn = self.conn();

        let confirmed_at = if status == BroadcastedTxStatus::Confirmed {
            Some(Utc::now().to_rfc3339())
        } else {
            None
        };

        let rows = conn.execute(
            "UPDATE broadcasted_transactions
             SET status = ?1, error = ?2, confirmed_at = COALESCE(?3, confirmed_at)
             WHERE uuid = ?4",
            rusqlite::params![status.to_string(), error, confirmed_at, uuid],
        )?;

        Ok(rows > 0)
    }

    /// List broadcasted transactions with optional filters
    pub fn list_broadcasted_transactions(
        &self,
        status: Option<&str>,
        network: Option<&str>,
        broadcast_mode: Option<&str>,
        limit: Option<usize>,
    ) -> SqliteResult<Vec<BroadcastedTransaction>> {
        let conn = self.conn();

        let mut sql = String::from(
            "SELECT id, uuid, network, from_address, to_address, value, value_formatted,
                    tx_hash, explorer_url, status, broadcast_mode, error,
                    broadcast_at, confirmed_at, created_at
             FROM broadcasted_transactions WHERE 1=1",
        );

        let mut params: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

        if let Some(s) = status {
            sql.push_str(&format!(" AND status = ?{}", params.len() + 1));
            params.push(Box::new(s.to_string()));
        }

        if let Some(n) = network {
            sql.push_str(&format!(" AND network = ?{}", params.len() + 1));
            params.push(Box::new(n.to_string()));
        }

        if let Some(m) = broadcast_mode {
            sql.push_str(&format!(" AND broadcast_mode = ?{}", params.len() + 1));
            params.push(Box::new(m.to_string()));
        }

        sql.push_str(" ORDER BY broadcast_at DESC");

        if let Some(l) = limit {
            sql.push_str(&format!(" LIMIT {}", l));
        }

        let params_refs: Vec<&dyn rusqlite::ToSql> = params.iter().map(|p| p.as_ref()).collect();

        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params_refs.as_slice(), |row| {
            let status_str: String = row.get(9)?;
            let mode_str: String = row.get(10)?;
            let broadcast_at_str: String = row.get(12)?;
            let confirmed_at_str: Option<String> = row.get(13)?;
            let created_at_str: String = row.get(14)?;

            Ok(BroadcastedTransaction {
                id: row.get(0)?,
                uuid: row.get(1)?,
                network: row.get(2)?,
                from_address: row.get(3)?,
                to_address: row.get(4)?,
                value: row.get(5)?,
                value_formatted: row.get(6)?,
                tx_hash: row.get(7)?,
                explorer_url: row.get(8)?,
                status: status_str.parse().unwrap_or(BroadcastedTxStatus::Broadcast),
                broadcast_mode: mode_str.parse().unwrap_or(BroadcastMode::Partner),
                error: row.get(11)?,
                broadcast_at: DateTime::parse_from_rfc3339(&broadcast_at_str)
                    .unwrap()
                    .with_timezone(&Utc),
                confirmed_at: confirmed_at_str.and_then(|s| {
                    DateTime::parse_from_rfc3339(&s)
                        .ok()
                        .map(|dt| dt.with_timezone(&Utc))
                }),
                created_at: DateTime::parse_from_rfc3339(&created_at_str)
                    .unwrap()
                    .with_timezone(&Utc),
            })
        })?;

        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Get a single broadcasted transaction by UUID
    pub fn get_broadcasted_transaction(&self, uuid: &str) -> SqliteResult<Option<BroadcastedTransaction>> {
        let txs = self.list_broadcasted_transactions(None, None, None, Some(1))?;
        let conn = self.conn();

        let mut stmt = conn.prepare(
            "SELECT id, uuid, network, from_address, to_address, value, value_formatted,
                    tx_hash, explorer_url, status, broadcast_mode, error,
                    broadcast_at, confirmed_at, created_at
             FROM broadcasted_transactions WHERE uuid = ?1",
        )?;

        let tx = stmt
            .query_row([uuid], |row| {
                let status_str: String = row.get(9)?;
                let mode_str: String = row.get(10)?;
                let broadcast_at_str: String = row.get(12)?;
                let confirmed_at_str: Option<String> = row.get(13)?;
                let created_at_str: String = row.get(14)?;

                Ok(BroadcastedTransaction {
                    id: row.get(0)?,
                    uuid: row.get(1)?,
                    network: row.get(2)?,
                    from_address: row.get(3)?,
                    to_address: row.get(4)?,
                    value: row.get(5)?,
                    value_formatted: row.get(6)?,
                    tx_hash: row.get(7)?,
                    explorer_url: row.get(8)?,
                    status: status_str.parse().unwrap_or(BroadcastedTxStatus::Broadcast),
                    broadcast_mode: mode_str.parse().unwrap_or(BroadcastMode::Partner),
                    error: row.get(11)?,
                    broadcast_at: DateTime::parse_from_rfc3339(&broadcast_at_str)
                        .unwrap()
                        .with_timezone(&Utc),
                    confirmed_at: confirmed_at_str.and_then(|s| {
                        DateTime::parse_from_rfc3339(&s)
                            .ok()
                            .map(|dt| dt.with_timezone(&Utc))
                    }),
                    created_at: DateTime::parse_from_rfc3339(&created_at_str)
                        .unwrap()
                        .with_timezone(&Utc),
                })
            })
            .ok();

        drop(txs); // unused binding from copy-paste
        Ok(tx)
    }
}
