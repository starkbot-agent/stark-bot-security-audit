//! QMD-style markdown memory system
//!
//! A simplified memory system where markdown files are the source of truth:
//! - MEMORY.md - Global long-term facts and preferences
//! - YYYY-MM-DD.md - Daily logs
//! - {identity_id}/ - Per-identity memories (optional)
//!
//! SQLite FTS5 provides fast BM25 full-text search across all memory files.

pub mod file_ops;
pub mod store;

pub use store::MemoryStore;
