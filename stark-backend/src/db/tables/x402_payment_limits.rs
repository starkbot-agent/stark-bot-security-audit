//! Database methods for x402_payment_limits table

use crate::db::Database;
use rusqlite::Result as SqliteResult;

/// A single payment-limit row.
#[derive(Debug, Clone)]
pub struct X402PaymentLimitRow {
    pub asset: String,
    pub max_amount: String,
    pub decimals: u8,
    pub display_name: String,
    pub address: Option<String>,
}

impl Database {
    /// Return all configured payment limits.
    pub fn get_all_x402_payment_limits(&self) -> SqliteResult<Vec<X402PaymentLimitRow>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT asset, max_amount, decimals, display_name, address FROM x402_payment_limits ORDER BY asset",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(X402PaymentLimitRow {
                asset: row.get(0)?,
                max_amount: row.get(1)?,
                decimals: row.get::<_, i32>(2)? as u8,
                display_name: row.get(3)?,
                address: row.get(4)?,
            })
        })?;
        rows.collect()
    }

    /// Upsert a single payment limit.
    pub fn set_x402_payment_limit(
        &self,
        asset: &str,
        max_amount: &str,
        decimals: u8,
        display_name: &str,
        address: Option<&str>,
    ) -> SqliteResult<()> {
        let conn = self.conn();
        conn.execute(
            "INSERT INTO x402_payment_limits (asset, max_amount, decimals, display_name, address, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, datetime('now'))
             ON CONFLICT(asset) DO UPDATE SET
                max_amount = excluded.max_amount,
                decimals = excluded.decimals,
                display_name = excluded.display_name,
                address = excluded.address,
                updated_at = datetime('now')",
            rusqlite::params![asset.to_uppercase(), max_amount, decimals as i32, display_name, address],
        )?;
        Ok(())
    }

    /// Delete a specific payment limit.
    pub fn delete_x402_payment_limit(&self, asset: &str) -> SqliteResult<bool> {
        let conn = self.conn();
        let affected = conn.execute(
            "DELETE FROM x402_payment_limits WHERE asset = ?1",
            [asset.to_uppercase()],
        )?;
        Ok(affected > 0)
    }
}
