//! File operations for QMD memory system
//!
//! Handles reading/writing markdown files and directory structure.

use chrono::{Local, NaiveDate};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Get the path to a daily log file (YYYY-MM-DD.md)
pub fn daily_log_path(memory_dir: &Path, date: NaiveDate, identity_id: Option<&str>) -> PathBuf {
    let filename = format!("{}.md", date.format("%Y-%m-%d"));
    match identity_id {
        Some(id) => memory_dir.join(id).join(&filename),
        None => memory_dir.join(&filename),
    }
}

/// Get the path to the long-term memory file (MEMORY.md)
pub fn long_term_path(memory_dir: &Path, identity_id: Option<&str>) -> PathBuf {
    match identity_id {
        Some(id) => memory_dir.join(id).join("MEMORY.md"),
        None => memory_dir.join("MEMORY.md"),
    }
}

/// Ensure the memory directory structure exists
pub fn ensure_memory_dirs(memory_dir: &Path, identity_id: Option<&str>) -> io::Result<()> {
    fs::create_dir_all(memory_dir)?;
    if let Some(id) = identity_id {
        fs::create_dir_all(memory_dir.join(id))?;
    }
    Ok(())
}

/// Append content to a file with a timestamp header
pub fn append_to_file(path: &Path, content: &str) -> io::Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    // Read existing content to check if we need a newline prefix
    let existing = fs::read_to_string(path).unwrap_or_default();
    let needs_newline = !existing.is_empty() && !existing.ends_with('\n');

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;

    // Add newline if needed
    if needs_newline {
        writeln!(file)?;
    }

    // Add timestamp header and content
    let timestamp = Local::now().format("%H:%M");
    writeln!(file, "\n## {}\n{}", timestamp, content.trim())?;

    Ok(())
}

/// Append content to a file without timestamp (for bulk operations)
pub fn append_raw(path: &Path, content: &str) -> io::Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let existing = fs::read_to_string(path).unwrap_or_default();
    let needs_newline = !existing.is_empty() && !existing.ends_with('\n');

    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;

    if needs_newline {
        writeln!(file)?;
    }

    writeln!(file, "{}", content.trim())?;

    Ok(())
}

/// Read content from a file, returning empty string if file doesn't exist
pub fn read_file(path: &Path) -> io::Result<String> {
    match fs::read_to_string(path) {
        Ok(content) => Ok(content),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(String::new()),
        Err(e) => Err(e),
    }
}

/// List all markdown files in a directory (recursively)
pub fn list_memory_files(memory_dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut files = Vec::new();

    if !memory_dir.exists() {
        return Ok(files);
    }

    fn visit_dir(dir: &Path, files: &mut Vec<PathBuf>) -> io::Result<()> {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                visit_dir(&path, files)?;
            } else if path.extension().map(|e| e == "md").unwrap_or(false) {
                files.push(path);
            }
        }
        Ok(())
    }

    visit_dir(memory_dir, &mut files)?;
    Ok(files)
}

/// Get relative path from memory_dir for a file
pub fn relative_path(memory_dir: &Path, file_path: &Path) -> Option<String> {
    file_path
        .strip_prefix(memory_dir)
        .ok()
        .map(|p| p.to_string_lossy().to_string())
}

/// Parse a date from a filename like "2024-01-15.md"
pub fn parse_date_from_filename(filename: &str) -> Option<NaiveDate> {
    let stem = filename.strip_suffix(".md")?;
    NaiveDate::parse_from_str(stem, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_daily_log_path() {
        let dir = PathBuf::from("/memory");
        let date = NaiveDate::from_ymd_opt(2024, 1, 15).unwrap();

        assert_eq!(
            daily_log_path(&dir, date, None),
            PathBuf::from("/memory/2024-01-15.md")
        );

        assert_eq!(
            daily_log_path(&dir, date, Some("user123")),
            PathBuf::from("/memory/user123/2024-01-15.md")
        );
    }

    #[test]
    fn test_long_term_path() {
        let dir = PathBuf::from("/memory");

        assert_eq!(
            long_term_path(&dir, None),
            PathBuf::from("/memory/MEMORY.md")
        );

        assert_eq!(
            long_term_path(&dir, Some("user123")),
            PathBuf::from("/memory/user123/MEMORY.md")
        );
    }

    #[test]
    fn test_append_to_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("test.md");

        append_to_file(&path, "First entry").unwrap();
        append_to_file(&path, "Second entry").unwrap();

        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("First entry"));
        assert!(content.contains("Second entry"));
        assert!(content.contains("##")); // Has timestamp headers
    }

    #[test]
    fn test_list_memory_files() {
        let dir = tempdir().unwrap();
        let mem_dir = dir.path();

        // Create some files
        fs::write(mem_dir.join("MEMORY.md"), "content").unwrap();
        fs::write(mem_dir.join("2024-01-15.md"), "content").unwrap();
        fs::create_dir(mem_dir.join("user1")).unwrap();
        fs::write(mem_dir.join("user1/MEMORY.md"), "content").unwrap();

        let files = list_memory_files(mem_dir).unwrap();
        assert_eq!(files.len(), 3);
    }

    #[test]
    fn test_parse_date_from_filename() {
        assert_eq!(
            parse_date_from_filename("2024-01-15.md"),
            Some(NaiveDate::from_ymd_opt(2024, 1, 15).unwrap())
        );
        assert_eq!(parse_date_from_filename("MEMORY.md"), None);
        assert_eq!(parse_date_from_filename("invalid.md"), None);
    }
}
