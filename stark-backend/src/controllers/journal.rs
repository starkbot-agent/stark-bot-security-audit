use actix_web::{web, HttpRequest, HttpResponse, Responder};
use serde::{Deserialize, Serialize};
use std::path::Path;
use tokio::fs;

use crate::config::journal_dir;
use crate::AppState;

/// Validate session token from request
fn validate_session_from_request(
    state: &web::Data<AppState>,
    req: &HttpRequest,
) -> Result<(), HttpResponse> {
    let token = req
        .headers()
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.trim_start_matches("Bearer ").to_string());

    let token = match token {
        Some(t) => t,
        None => {
            return Err(HttpResponse::Unauthorized().json(serde_json::json!({
                "error": "No authorization token provided"
            })));
        }
    };

    match state.db.validate_session(&token) {
        Ok(Some(_)) => Ok(()),
        Ok(None) => Err(HttpResponse::Unauthorized().json(serde_json::json!({
            "error": "Invalid or expired session"
        }))),
        Err(e) => {
            log::error!("Session validation error: {}", e);
            Err(HttpResponse::InternalServerError().json(serde_json::json!({
                "error": "Internal server error"
            })))
        }
    }
}

#[derive(Debug, Serialize)]
struct JournalEntry {
    name: String,
    path: String,
    is_dir: bool,
    size: u64,
    modified: Option<String>,
}

#[derive(Debug, Serialize)]
struct ListJournalResponse {
    success: bool,
    path: String,
    entries: Vec<JournalEntry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ListJournalQuery {
    path: Option<String>,
}

/// List files in the journal directory
async fn list_journal(
    data: web::Data<AppState>,
    req: HttpRequest,
    query: web::Query<ListJournalQuery>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }

    let journal = journal_dir();
    let journal_path = Path::new(&journal);

    // Resolve the requested path
    let relative_path = query.path.as_deref().unwrap_or("");
    let full_path = if relative_path.is_empty() {
        journal_path.to_path_buf()
    } else {
        journal_path.join(relative_path)
    };

    // Check if journal directory exists
    if !journal_path.exists() {
        return HttpResponse::Ok().json(ListJournalResponse {
            success: true,
            path: relative_path.to_string(),
            entries: vec![],
            error: Some("Journal directory does not exist yet".to_string()),
        });
    }

    // Security check: canonicalize and ensure we're within journal
    let canonical_journal = match journal_path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ListJournalResponse {
                success: false,
                path: relative_path.to_string(),
                entries: vec![],
                error: Some(format!("Journal not accessible: {}", e)),
            });
        }
    };

    let canonical_path = match full_path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            return HttpResponse::NotFound().json(ListJournalResponse {
                success: false,
                path: relative_path.to_string(),
                entries: vec![],
                error: Some("Path not found".to_string()),
            });
        }
    };

    // Ensure path is within journal
    if !canonical_path.starts_with(&canonical_journal) {
        return HttpResponse::Forbidden().json(ListJournalResponse {
            success: false,
            path: relative_path.to_string(),
            entries: vec![],
            error: Some("Access denied: path outside journal".to_string()),
        });
    }

    // Read directory contents
    let mut entries = Vec::new();
    let mut read_dir = match fs::read_dir(&canonical_path).await {
        Ok(rd) => rd,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ListJournalResponse {
                success: false,
                path: relative_path.to_string(),
                entries: vec![],
                error: Some(format!("Failed to read directory: {}", e)),
            });
        }
    };

    while let Ok(Some(entry)) = read_dir.next_entry().await {
        let name = match entry.file_name().to_str() {
            Some(n) => n.to_string(),
            None => continue,
        };

        let metadata = match entry.metadata().await {
            Ok(m) => m,
            Err(_) => continue,
        };

        let entry_path = entry.path();
        let rel_path = entry_path
            .strip_prefix(&canonical_journal)
            .unwrap_or(&entry_path)
            .to_string_lossy()
            .to_string();

        let modified = metadata.modified().ok().map(|t| {
            let datetime: chrono::DateTime<chrono::Utc> = t.into();
            datetime.format("%Y-%m-%d %H:%M:%S").to_string()
        });

        entries.push(JournalEntry {
            name,
            path: rel_path,
            is_dir: metadata.is_dir(),
            size: if metadata.is_dir() { 0 } else { metadata.len() },
            modified,
        });
    }

    // Sort: directories first, then by name (reverse for dates to show newest first)
    entries.sort_by(|a, b| {
        if a.is_dir != b.is_dir {
            b.is_dir.cmp(&a.is_dir)
        } else {
            // Reverse sort for dates (newest first)
            b.name.cmp(&a.name)
        }
    });

    HttpResponse::Ok().json(ListJournalResponse {
        success: true,
        path: relative_path.to_string(),
        entries,
        error: None,
    })
}

#[derive(Debug, Serialize)]
struct ReadJournalResponse {
    success: bool,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReadJournalQuery {
    path: String,
}

/// Read a file from the journal
async fn read_journal(
    data: web::Data<AppState>,
    req: HttpRequest,
    query: web::Query<ReadJournalQuery>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }

    let journal = journal_dir();
    let journal_path = Path::new(&journal);
    let full_path = journal_path.join(&query.path);

    // Check if journal exists
    if !journal_path.exists() {
        return HttpResponse::NotFound().json(ReadJournalResponse {
            success: false,
            path: query.path.clone(),
            content: None,
            size: None,
            error: Some("Journal directory does not exist".to_string()),
        });
    }

    // Security check
    let canonical_journal = match journal_path.canonicalize() {
        Ok(p) => p,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ReadJournalResponse {
                success: false,
                path: query.path.clone(),
                content: None,
                size: None,
                error: Some(format!("Journal not accessible: {}", e)),
            });
        }
    };

    let canonical_path = match full_path.canonicalize() {
        Ok(p) => p,
        Err(_) => {
            return HttpResponse::NotFound().json(ReadJournalResponse {
                success: false,
                path: query.path.clone(),
                content: None,
                size: None,
                error: Some("File not found".to_string()),
            });
        }
    };

    if !canonical_path.starts_with(&canonical_journal) {
        return HttpResponse::Forbidden().json(ReadJournalResponse {
            success: false,
            path: query.path.clone(),
            content: None,
            size: None,
            error: Some("Access denied: path outside journal".to_string()),
        });
    }

    // Check if it's a file
    let metadata = match fs::metadata(&canonical_path).await {
        Ok(m) => m,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ReadJournalResponse {
                success: false,
                path: query.path.clone(),
                content: None,
                size: None,
                error: Some(format!("Failed to read file metadata: {}", e)),
            });
        }
    };

    if metadata.is_dir() {
        return HttpResponse::BadRequest().json(ReadJournalResponse {
            success: false,
            path: query.path.clone(),
            content: None,
            size: None,
            error: Some("Path is a directory, not a file".to_string()),
        });
    }

    // Read file content (limit to 1MB for safety)
    const MAX_SIZE: u64 = 1024 * 1024;
    if metadata.len() > MAX_SIZE {
        return HttpResponse::Ok().json(ReadJournalResponse {
            success: true,
            path: query.path.clone(),
            content: None,
            size: Some(metadata.len()),
            error: Some(format!("File too large to display ({} bytes)", metadata.len())),
        });
    }

    let content = match fs::read(&canonical_path).await {
        Ok(c) => c,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ReadJournalResponse {
                success: false,
                path: query.path.clone(),
                content: None,
                size: None,
                error: Some(format!("Failed to read file: {}", e)),
            });
        }
    };

    let text = String::from_utf8_lossy(&content).to_string();

    HttpResponse::Ok().json(ReadJournalResponse {
        success: true,
        path: query.path.clone(),
        content: Some(text),
        size: Some(metadata.len()),
        error: None,
    })
}

#[derive(Debug, Serialize)]
struct JournalInfoResponse {
    success: bool,
    journal_path: String,
    exists: bool,
}

/// Get journal info
async fn journal_info(data: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }

    let journal = journal_dir();
    let exists = Path::new(&journal).exists();

    HttpResponse::Ok().json(JournalInfoResponse {
        success: true,
        journal_path: journal,
        exists,
    })
}

pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/journal")
            .route("", web::get().to(list_journal))
            .route("/read", web::get().to(read_journal))
            .route("/info", web::get().to(journal_info)),
    );
}
