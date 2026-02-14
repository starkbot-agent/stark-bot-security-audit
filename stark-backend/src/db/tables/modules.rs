//! Database operations for installed_modules table (plugin system)

use crate::db::Database;
use chrono::{DateTime, Utc};
use rusqlite::Result as SqliteResult;
use serde::{Deserialize, Serialize};

/// Represents an installed module in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledModule {
    pub id: i64,
    pub module_name: String,
    pub enabled: bool,
    pub version: String,
    pub description: String,
    pub has_tools: bool,
    pub has_dashboard: bool,
    pub installed_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Database {
    /// List all installed modules
    pub fn list_installed_modules(&self) -> SqliteResult<Vec<InstalledModule>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, module_name, enabled, version, description, has_tools, has_dashboard, installed_at, updated_at
             FROM installed_modules ORDER BY installed_at ASC",
        )?;

        let modules = stmt
            .query_map([], |row| Self::row_to_installed_module(row))?
            .filter_map(|r| r.ok())
            .collect();

        Ok(modules)
    }

    /// Check if a module is installed
    pub fn is_module_installed(&self, name: &str) -> SqliteResult<bool> {
        let conn = self.conn();
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM installed_modules WHERE module_name = ?1",
            [name],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    /// Check if a module is installed and enabled
    pub fn is_module_enabled(&self, name: &str) -> SqliteResult<bool> {
        let conn = self.conn();
        let result: Option<bool> = conn
            .query_row(
                "SELECT enabled FROM installed_modules WHERE module_name = ?1",
                [name],
                |row| row.get::<_, bool>(0),
            )
            .ok();
        Ok(result.unwrap_or(false))
    }

    /// Install a module (insert into installed_modules)
    pub fn install_module(
        &self,
        name: &str,
        description: &str,
        version: &str,
        has_tools: bool,
        has_dashboard: bool,
    ) -> SqliteResult<InstalledModule> {
        let conn = self.conn();
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO installed_modules (module_name, enabled, version, description, has_tools, has_dashboard, installed_at, updated_at)
             VALUES (?1, 1, ?2, ?3, ?4, ?5, ?6, ?6)",
            rusqlite::params![name, version, description, has_tools, has_dashboard, now],
        )?;

        let id = conn.last_insert_rowid();
        let installed_at = DateTime::parse_from_rfc3339(&now)
            .unwrap()
            .with_timezone(&Utc);

        Ok(InstalledModule {
            id,
            module_name: name.to_string(),
            enabled: true,
            version: version.to_string(),
            description: description.to_string(),
            has_tools,
            has_dashboard,
            installed_at,
            updated_at: installed_at,
        })
    }

    /// Uninstall a module (remove from installed_modules)
    pub fn uninstall_module(&self, name: &str) -> SqliteResult<bool> {
        let conn = self.conn();
        let rows = conn.execute(
            "DELETE FROM installed_modules WHERE module_name = ?1",
            [name],
        )?;
        Ok(rows > 0)
    }

    /// Enable or disable a module
    pub fn set_module_enabled(&self, name: &str, enabled: bool) -> SqliteResult<bool> {
        let conn = self.conn();
        let now = chrono::Utc::now().to_rfc3339();
        let rows = conn.execute(
            "UPDATE installed_modules SET enabled = ?1, updated_at = ?2 WHERE module_name = ?3",
            rusqlite::params![enabled, now, name],
        )?;
        Ok(rows > 0)
    }

    /// Get a single installed module by name
    pub fn get_installed_module(&self, name: &str) -> SqliteResult<Option<InstalledModule>> {
        let conn = self.conn();
        let result = conn.query_row(
            "SELECT id, module_name, enabled, version, description, has_tools, has_dashboard, installed_at, updated_at
             FROM installed_modules WHERE module_name = ?1",
            [name],
            |row| Self::row_to_installed_module(row),
        );
        match result {
            Ok(module) => Ok(Some(module)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn row_to_installed_module(row: &rusqlite::Row) -> rusqlite::Result<InstalledModule> {
        let installed_at_str: String = row.get(7)?;
        let updated_at_str: String = row.get(8)?;
        let installed_at = DateTime::parse_from_rfc3339(&installed_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());
        let updated_at = DateTime::parse_from_rfc3339(&updated_at_str)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now());

        Ok(InstalledModule {
            id: row.get(0)?,
            module_name: row.get(1)?,
            enabled: row.get(2)?,
            version: row.get(3)?,
            description: row.get(4)?,
            has_tools: row.get(5)?,
            has_dashboard: row.get(6)?,
            installed_at,
            updated_at,
        })
    }
}
