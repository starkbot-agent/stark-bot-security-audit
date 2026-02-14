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
    /// Where this module came from: "builtin" or "starkhub"
    pub source: String,
    /// Path to the module.toml manifest (for dynamic modules)
    pub manifest_path: Option<String>,
    /// Path to the service binary (for dynamic modules)
    pub binary_path: Option<String>,
    /// Author identifier (e.g. "@ethereumdegen")
    pub author: Option<String>,
    /// SHA-256 checksum of the downloaded archive
    pub sha256_checksum: Option<String>,
    pub installed_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Database {
    /// List all installed modules
    pub fn list_installed_modules(&self) -> SqliteResult<Vec<InstalledModule>> {
        let conn = self.conn();
        let mut stmt = conn.prepare(
            "SELECT id, module_name, enabled, version, description, has_tools, has_dashboard,
                    source, manifest_path, binary_path, author, sha256_checksum,
                    installed_at, updated_at
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
        self.install_module_full(name, description, version, has_tools, has_dashboard,
            "builtin", None, None, None, None)
    }

    /// Install a module with all fields (used for dynamic/StarkHub modules)
    pub fn install_module_full(
        &self,
        name: &str,
        description: &str,
        version: &str,
        has_tools: bool,
        has_dashboard: bool,
        source: &str,
        manifest_path: Option<&str>,
        binary_path: Option<&str>,
        author: Option<&str>,
        sha256_checksum: Option<&str>,
    ) -> SqliteResult<InstalledModule> {
        let conn = self.conn();
        let now = chrono::Utc::now().to_rfc3339();

        conn.execute(
            "INSERT INTO installed_modules (module_name, enabled, version, description, has_tools, has_dashboard,
             source, manifest_path, binary_path, author, sha256_checksum, installed_at, updated_at)
             VALUES (?1, 1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?11)",
            rusqlite::params![
                name, version, description, has_tools, has_dashboard,
                source, manifest_path, binary_path, author, sha256_checksum, now
            ],
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
            source: source.to_string(),
            manifest_path: manifest_path.map(|s| s.to_string()),
            binary_path: binary_path.map(|s| s.to_string()),
            author: author.map(|s| s.to_string()),
            sha256_checksum: sha256_checksum.map(|s| s.to_string()),
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
            "SELECT id, module_name, enabled, version, description, has_tools, has_dashboard,
                    source, manifest_path, binary_path, author, sha256_checksum,
                    installed_at, updated_at
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
        let installed_at_str: String = row.get(12)?;
        let updated_at_str: String = row.get(13)?;
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
            source: row.get::<_, Option<String>>(7)?.unwrap_or_else(|| "builtin".to_string()),
            manifest_path: row.get(8)?,
            binary_path: row.get(9)?,
            author: row.get(10)?,
            sha256_checksum: row.get(11)?,
            installed_at,
            updated_at,
        })
    }
}
