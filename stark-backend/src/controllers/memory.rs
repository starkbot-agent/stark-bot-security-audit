//! Memory controller - REST API for QMD markdown-based memory system
//!
//! Provides endpoints for browsing, searching, and viewing memory files.

use actix_web::{web, HttpRequest, HttpResponse, Responder};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

use crate::qmd_memory::file_ops;
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

// ============================================================================
// Response Types
// ============================================================================

#[derive(Debug, Serialize)]
struct MemoryFile {
    /// Relative path from memory root (e.g., "MEMORY.md" or "user123/2024-01-15.md")
    path: String,
    /// File name only
    name: String,
    /// File type: "daily_log", "long_term", or "unknown"
    file_type: String,
    /// Parsed date if this is a daily log file
    date: Option<String>,
    /// Identity ID if in a subdirectory
    identity_id: Option<String>,
    /// File size in bytes
    size: u64,
    /// Last modified timestamp
    modified: Option<String>,
}

#[derive(Debug, Serialize)]
struct ListFilesResponse {
    success: bool,
    files: Vec<MemoryFile>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct ReadFileResponse {
    success: bool,
    path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    identity_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct SearchResult {
    file_path: String,
    snippet: String,
    score: f64,
}

#[derive(Debug, Serialize)]
struct SearchResponse {
    success: bool,
    query: String,
    results: Vec<SearchResult>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct MemoryStats {
    total_files: usize,
    daily_log_count: usize,
    long_term_count: usize,
    identity_count: usize,
    identities: Vec<String>,
    date_range: Option<DateRange>,
}

#[derive(Debug, Serialize)]
struct DateRange {
    oldest: String,
    newest: String,
}

#[derive(Debug, Serialize)]
struct StatsResponse {
    success: bool,
    stats: MemoryStats,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct AppendResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct MemoryInfoResponse {
    success: bool,
    memory_dir: String,
    exists: bool,
}

// ============================================================================
// Request Types
// ============================================================================

#[derive(Debug, Deserialize)]
struct ReadFileQuery {
    path: String,
}

#[derive(Debug, Deserialize)]
struct SearchQuery {
    query: String,
    #[serde(default = "default_search_limit")]
    limit: i32,
}

fn default_search_limit() -> i32 {
    20
}

#[derive(Debug, Deserialize)]
struct DailyLogQuery {
    date: Option<String>,
    identity_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct LongTermQuery {
    identity_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AppendBody {
    content: String,
    identity_id: Option<String>,
}

// ============================================================================
// Handlers
// ============================================================================

/// GET /api/memory/files - List all memory files
async fn list_files(data: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }

    let memory_store = match data.dispatcher.memory_store() {
        Some(store) => store,
        None => {
            return HttpResponse::ServiceUnavailable().json(ListFilesResponse {
                success: false,
                files: vec![],
                error: Some("Memory system not initialized".to_string()),
            });
        }
    };

    let file_list = match memory_store.list_files() {
        Ok(files) => files,
        Err(e) => {
            return HttpResponse::InternalServerError().json(ListFilesResponse {
                success: false,
                files: vec![],
                error: Some(format!("Failed to list files: {}", e)),
            });
        }
    };

    let memory_dir = memory_store.memory_dir();
    let mut files: Vec<MemoryFile> = Vec::new();

    for rel_path in file_list {
        let full_path = memory_dir.join(&rel_path);
        let name = std::path::Path::new(&rel_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        // Determine file type and parse date
        let (file_type, date) = if name == "MEMORY.md" {
            ("long_term".to_string(), None)
        } else if let Some(d) = file_ops::parse_date_from_filename(&name) {
            ("daily_log".to_string(), Some(d.format("%Y-%m-%d").to_string()))
        } else {
            ("unknown".to_string(), None)
        };

        // Extract identity_id if in subdirectory
        let identity_id = std::path::Path::new(&rel_path)
            .parent()
            .and_then(|p| p.to_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        // Get file metadata
        let (size, modified) = if let Ok(metadata) = std::fs::metadata(&full_path) {
            let mod_time = metadata.modified().ok().map(|t| {
                let datetime: chrono::DateTime<chrono::Utc> = t.into();
                datetime.format("%Y-%m-%d %H:%M:%S").to_string()
            });
            (metadata.len(), mod_time)
        } else {
            (0, None)
        };

        files.push(MemoryFile {
            path: rel_path,
            name,
            file_type,
            date,
            identity_id,
            size,
            modified,
        });
    }

    // Sort: MEMORY.md first, then daily logs by date descending
    files.sort_by(|a, b| {
        // Long-term memories first
        if a.file_type == "long_term" && b.file_type != "long_term" {
            return std::cmp::Ordering::Less;
        }
        if b.file_type == "long_term" && a.file_type != "long_term" {
            return std::cmp::Ordering::Greater;
        }
        // Then by date descending (newest first)
        match (&b.date, &a.date) {
            (Some(bd), Some(ad)) => bd.cmp(ad),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => a.name.cmp(&b.name),
        }
    });

    HttpResponse::Ok().json(ListFilesResponse {
        success: true,
        files,
        error: None,
    })
}

/// GET /api/memory/file - Read a specific memory file
async fn read_file(
    data: web::Data<AppState>,
    req: HttpRequest,
    query: web::Query<ReadFileQuery>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }

    let memory_store = match data.dispatcher.memory_store() {
        Some(store) => store,
        None => {
            return HttpResponse::ServiceUnavailable().json(ReadFileResponse {
                success: false,
                path: query.path.clone(),
                content: None,
                file_type: None,
                date: None,
                identity_id: None,
                error: Some("Memory system not initialized".to_string()),
            });
        }
    };

    let content = match memory_store.get_file(&query.path) {
        Ok(c) => c,
        Err(e) => {
            return HttpResponse::NotFound().json(ReadFileResponse {
                success: false,
                path: query.path.clone(),
                content: None,
                file_type: None,
                date: None,
                identity_id: None,
                error: Some(format!("Failed to read file: {}", e)),
            });
        }
    };

    let name = std::path::Path::new(&query.path)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    let (file_type, date) = if name == "MEMORY.md" {
        (Some("long_term".to_string()), None)
    } else if let Some(d) = file_ops::parse_date_from_filename(&name) {
        (
            Some("daily_log".to_string()),
            Some(d.format("%Y-%m-%d").to_string()),
        )
    } else {
        (Some("unknown".to_string()), None)
    };

    let identity_id = std::path::Path::new(&query.path)
        .parent()
        .and_then(|p| p.to_str())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    HttpResponse::Ok().json(ReadFileResponse {
        success: true,
        path: query.path.clone(),
        content: Some(content),
        file_type,
        date,
        identity_id,
        error: None,
    })
}

/// GET /api/memory/search - Search memories with BM25 full-text search
async fn search(
    data: web::Data<AppState>,
    req: HttpRequest,
    query: web::Query<SearchQuery>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }

    let memory_store = match data.dispatcher.memory_store() {
        Some(store) => store,
        None => {
            return HttpResponse::ServiceUnavailable().json(SearchResponse {
                success: false,
                query: query.query.clone(),
                results: vec![],
                error: Some("Memory system not initialized".to_string()),
            });
        }
    };

    let limit = query.limit.clamp(1, 100);

    match memory_store.search(&query.query, limit) {
        Ok(results) => {
            let results: Vec<SearchResult> = results
                .into_iter()
                .map(|r| SearchResult {
                    file_path: r.file_path,
                    snippet: r.snippet,
                    score: r.score,
                })
                .collect();

            HttpResponse::Ok().json(SearchResponse {
                success: true,
                query: query.query.clone(),
                results,
                error: None,
            })
        }
        Err(e) => HttpResponse::InternalServerError().json(SearchResponse {
            success: false,
            query: query.query.clone(),
            results: vec![],
            error: Some(format!("Search failed: {}", e)),
        }),
    }
}

/// GET /api/memory/daily - Get today's or a specific date's daily log
async fn get_daily_log(
    data: web::Data<AppState>,
    req: HttpRequest,
    query: web::Query<DailyLogQuery>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }

    let memory_store = match data.dispatcher.memory_store() {
        Some(store) => store,
        None => {
            return HttpResponse::ServiceUnavailable().json(ReadFileResponse {
                success: false,
                path: "".to_string(),
                content: None,
                file_type: None,
                date: None,
                identity_id: None,
                error: Some("Memory system not initialized".to_string()),
            });
        }
    };

    let identity_id = query.identity_id.as_deref();

    let (content, date_str) = if let Some(date_str) = &query.date {
        // Parse specific date
        match NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
            Ok(date) => match memory_store.get_daily_log_for_date(date, identity_id) {
                Ok(c) => (c, date_str.clone()),
                Err(e) => {
                    return HttpResponse::NotFound().json(ReadFileResponse {
                        success: false,
                        path: format!("{}.md", date_str),
                        content: None,
                        file_type: Some("daily_log".to_string()),
                        date: Some(date_str.clone()),
                        identity_id: identity_id.map(|s| s.to_string()),
                        error: Some(format!("Failed to read daily log: {}", e)),
                    });
                }
            },
            Err(_) => {
                return HttpResponse::BadRequest().json(ReadFileResponse {
                    success: false,
                    path: "".to_string(),
                    content: None,
                    file_type: None,
                    date: None,
                    identity_id: None,
                    error: Some("Invalid date format. Use YYYY-MM-DD".to_string()),
                });
            }
        }
    } else {
        // Get today's log
        let today = chrono::Local::now().date_naive();
        let date_str = today.format("%Y-%m-%d").to_string();
        match memory_store.get_daily_log(identity_id) {
            Ok(c) => (c, date_str),
            Err(e) => {
                let date_str = today.format("%Y-%m-%d").to_string();
                return HttpResponse::Ok().json(ReadFileResponse {
                    success: true,
                    path: format!("{}.md", date_str),
                    content: Some(String::new()), // Empty is fine for today's log
                    file_type: Some("daily_log".to_string()),
                    date: Some(date_str),
                    identity_id: identity_id.map(|s| s.to_string()),
                    error: Some(format!("No entries yet: {}", e)),
                });
            }
        }
    };

    HttpResponse::Ok().json(ReadFileResponse {
        success: true,
        path: format!("{}.md", date_str),
        content: Some(content),
        file_type: Some("daily_log".to_string()),
        date: Some(date_str),
        identity_id: identity_id.map(|s| s.to_string()),
        error: None,
    })
}

/// GET /api/memory/long-term - Get long-term memory (MEMORY.md)
async fn get_long_term(
    data: web::Data<AppState>,
    req: HttpRequest,
    query: web::Query<LongTermQuery>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }

    let memory_store = match data.dispatcher.memory_store() {
        Some(store) => store,
        None => {
            return HttpResponse::ServiceUnavailable().json(ReadFileResponse {
                success: false,
                path: "MEMORY.md".to_string(),
                content: None,
                file_type: None,
                date: None,
                identity_id: None,
                error: Some("Memory system not initialized".to_string()),
            });
        }
    };

    let identity_id = query.identity_id.as_deref();

    match memory_store.get_long_term(identity_id) {
        Ok(content) => HttpResponse::Ok().json(ReadFileResponse {
            success: true,
            path: "MEMORY.md".to_string(),
            content: Some(content),
            file_type: Some("long_term".to_string()),
            date: None,
            identity_id: identity_id.map(|s| s.to_string()),
            error: None,
        }),
        Err(e) => HttpResponse::Ok().json(ReadFileResponse {
            success: true,
            path: "MEMORY.md".to_string(),
            content: Some(String::new()), // Empty is fine
            file_type: Some("long_term".to_string()),
            date: None,
            identity_id: identity_id.map(|s| s.to_string()),
            error: Some(format!("No long-term memory yet: {}", e)),
        }),
    }
}

/// POST /api/memory/daily - Append to today's daily log
async fn append_daily_log(
    data: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<AppendBody>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }

    let memory_store = match data.dispatcher.memory_store() {
        Some(store) => store,
        None => {
            return HttpResponse::ServiceUnavailable().json(AppendResponse {
                success: false,
                message: None,
                error: Some("Memory system not initialized".to_string()),
            });
        }
    };

    let identity_id = body.identity_id.as_deref();

    match memory_store.append_daily_log(&body.content, identity_id) {
        Ok(()) => HttpResponse::Ok().json(AppendResponse {
            success: true,
            message: Some("Added to daily log".to_string()),
            error: None,
        }),
        Err(e) => HttpResponse::InternalServerError().json(AppendResponse {
            success: false,
            message: None,
            error: Some(format!("Failed to append: {}", e)),
        }),
    }
}

/// POST /api/memory/long-term - Append to long-term memory
async fn append_long_term(
    data: web::Data<AppState>,
    req: HttpRequest,
    body: web::Json<AppendBody>,
) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }

    let memory_store = match data.dispatcher.memory_store() {
        Some(store) => store,
        None => {
            return HttpResponse::ServiceUnavailable().json(AppendResponse {
                success: false,
                message: None,
                error: Some("Memory system not initialized".to_string()),
            });
        }
    };

    let identity_id = body.identity_id.as_deref();

    match memory_store.append_long_term(&body.content, identity_id) {
        Ok(()) => HttpResponse::Ok().json(AppendResponse {
            success: true,
            message: Some("Added to long-term memory".to_string()),
            error: None,
        }),
        Err(e) => HttpResponse::InternalServerError().json(AppendResponse {
            success: false,
            message: None,
            error: Some(format!("Failed to append: {}", e)),
        }),
    }
}

/// GET /api/memory/stats - Get memory statistics
async fn get_stats(data: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }

    let memory_store = match data.dispatcher.memory_store() {
        Some(store) => store,
        None => {
            return HttpResponse::ServiceUnavailable().json(StatsResponse {
                success: false,
                stats: MemoryStats {
                    total_files: 0,
                    daily_log_count: 0,
                    long_term_count: 0,
                    identity_count: 0,
                    identities: vec![],
                    date_range: None,
                },
                error: Some("Memory system not initialized".to_string()),
            });
        }
    };

    let file_list = match memory_store.list_files() {
        Ok(files) => files,
        Err(e) => {
            return HttpResponse::InternalServerError().json(StatsResponse {
                success: false,
                stats: MemoryStats {
                    total_files: 0,
                    daily_log_count: 0,
                    long_term_count: 0,
                    identity_count: 0,
                    identities: vec![],
                    date_range: None,
                },
                error: Some(format!("Failed to list files: {}", e)),
            });
        }
    };

    let mut daily_log_count = 0;
    let mut long_term_count = 0;
    let mut identities: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut dates: Vec<NaiveDate> = Vec::new();

    for rel_path in &file_list {
        let name = std::path::Path::new(rel_path)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        if name == "MEMORY.md" {
            long_term_count += 1;
        } else if let Some(d) = file_ops::parse_date_from_filename(&name) {
            daily_log_count += 1;
            dates.push(d);
        }

        // Extract identity
        if let Some(parent) = std::path::Path::new(rel_path).parent() {
            if let Some(id) = parent.to_str().filter(|s| !s.is_empty()) {
                identities.insert(id.to_string());
            }
        }
    }

    dates.sort();
    let date_range = if !dates.is_empty() {
        Some(DateRange {
            oldest: dates.first().unwrap().format("%Y-%m-%d").to_string(),
            newest: dates.last().unwrap().format("%Y-%m-%d").to_string(),
        })
    } else {
        None
    };

    HttpResponse::Ok().json(StatsResponse {
        success: true,
        stats: MemoryStats {
            total_files: file_list.len(),
            daily_log_count,
            long_term_count,
            identity_count: identities.len(),
            identities: identities.into_iter().collect(),
            date_range,
        },
        error: None,
    })
}

/// POST /api/memory/reindex - Force reindex the FTS database
async fn reindex(data: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }

    let memory_store = match data.dispatcher.memory_store() {
        Some(store) => store,
        None => {
            return HttpResponse::ServiceUnavailable().json(AppendResponse {
                success: false,
                message: None,
                error: Some("Memory system not initialized".to_string()),
            });
        }
    };

    match memory_store.reindex() {
        Ok(count) => HttpResponse::Ok().json(AppendResponse {
            success: true,
            message: Some(format!("Reindexed {} files", count)),
            error: None,
        }),
        Err(e) => HttpResponse::InternalServerError().json(AppendResponse {
            success: false,
            message: None,
            error: Some(format!("Reindex failed: {}", e)),
        }),
    }
}

/// GET /api/memory/info - Get memory system info
async fn memory_info(data: web::Data<AppState>, req: HttpRequest) -> impl Responder {
    if let Err(resp) = validate_session_from_request(&data, &req) {
        return resp;
    }

    let memory_store = data.dispatcher.memory_store();

    match memory_store {
        Some(store) => {
            let memory_dir = store.memory_dir();
            HttpResponse::Ok().json(MemoryInfoResponse {
                success: true,
                memory_dir: memory_dir.to_string_lossy().to_string(),
                exists: memory_dir.exists(),
            })
        }
        None => HttpResponse::Ok().json(MemoryInfoResponse {
            success: true,
            memory_dir: "Not configured".to_string(),
            exists: false,
        }),
    }
}

/// Configure memory routes
pub fn config(cfg: &mut web::ServiceConfig) {
    cfg.service(
        web::scope("/api/memory")
            .route("/files", web::get().to(list_files))
            .route("/file", web::get().to(read_file))
            .route("/search", web::get().to(search))
            .route("/daily", web::get().to(get_daily_log))
            .route("/daily", web::post().to(append_daily_log))
            .route("/long-term", web::get().to(get_long_term))
            .route("/long-term", web::post().to(append_long_term))
            .route("/stats", web::get().to(get_stats))
            .route("/reindex", web::post().to(reindex))
            .route("/info", web::get().to(memory_info)),
    );
}
