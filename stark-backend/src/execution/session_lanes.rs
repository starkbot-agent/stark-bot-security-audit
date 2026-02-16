//! Session Lane Serialization
//!
//! This module provides session-based request serialization to prevent race conditions
//! when multiple messages arrive for the same session concurrently.
//!
//! Inspired by OpenClaw's session lane system, this ensures:
//! - Messages for the same session are processed sequentially
//! - Tool executions don't interleave within a session
//! - Conversation history remains consistent
//! - Git operations don't conflict within a workspace

use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Maximum time a session lane can be held before warning
const LANE_HOLD_WARNING_SECS: u64 = 60;

/// Maximum number of session lanes to track (prevents memory leaks from abandoned sessions)
const MAX_SESSION_LANES: usize = 10000;

/// Time after which an idle lane can be pruned
const LANE_IDLE_TIMEOUT_SECS: u64 = 3600; // 1 hour

/// Metadata about a session lane
struct LaneMetadata {
    created_at: Instant,
    last_used: Instant,
    total_uses: u64,
}

impl Default for LaneMetadata {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            created_at: now,
            last_used: now,
            total_uses: 0,
        }
    }
}

/// Guard that releases the session lane when dropped
pub struct SessionLaneGuard {
    session_id: String,
    _permit: OwnedSemaphorePermit,
    acquired_at: Instant,
    manager: Arc<SessionLaneManager>,
}

impl SessionLaneGuard {
    /// Get the session ID this guard is for
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// Get how long this lane has been held
    pub fn held_duration(&self) -> Duration {
        self.acquired_at.elapsed()
    }
}

impl Drop for SessionLaneGuard {
    fn drop(&mut self) {
        let held = self.acquired_at.elapsed();
        if held.as_secs() > LANE_HOLD_WARNING_SECS {
            eprintln!(
                "[WARN] Session {} lane held for {} seconds (unusually long)",
                self.session_id, held.as_secs()
            );
        }

        // Update last_used timestamp
        if let Some(mut entry) = self.manager.metadata.get_mut(&self.session_id) {
            entry.last_used = Instant::now();
        }
    }
}

/// Manages session lanes for request serialization
pub struct SessionLaneManager {
    /// One semaphore per session - permits = 1 means only one request at a time
    lanes: DashMap<String, Arc<Semaphore>>,
    /// Metadata about each lane
    metadata: DashMap<String, LaneMetadata>,
    /// Global lane for cross-session operations (like git push to same repo)
    global_lane: Arc<Semaphore>,
    /// Optional workspace-based locking for git operations
    workspace_lanes: DashMap<String, Arc<Semaphore>>,
}

impl SessionLaneManager {
    /// Create a new session lane manager
    pub fn new() -> Arc<Self> {
        Arc::new(Self {
            lanes: DashMap::new(),
            metadata: DashMap::new(),
            global_lane: Arc::new(Semaphore::new(1)),
            workspace_lanes: DashMap::new(),
        })
    }

    /// Acquire a session lane for exclusive access
    ///
    /// This will block if another request is already processing for this session.
    /// Returns a guard that releases the lane when dropped.
    pub async fn acquire(self: &Arc<Self>, session_id: &str) -> SessionLaneGuard {
        // Get or create the semaphore for this session
        let semaphore = self.get_or_create_lane(session_id);

        // Acquire the permit (this will block if another request has it)
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .expect("Semaphore should not be closed");

        // Update metadata
        self.metadata
            .entry(session_id.to_string())
            .and_modify(|m| {
                m.last_used = Instant::now();
                m.total_uses += 1;
            })
            .or_insert_with(|| {
                let mut m = LaneMetadata::default();
                m.total_uses = 1;
                m
            });

        SessionLaneGuard {
            session_id: session_id.to_string(),
            _permit: permit,
            acquired_at: Instant::now(),
            manager: Arc::clone(self),
        }
    }

    /// Try to acquire a session lane without blocking
    ///
    /// Returns None if the lane is currently held by another request.
    pub fn try_acquire(self: &Arc<Self>, session_id: &str) -> Option<SessionLaneGuard> {
        let semaphore = self.get_or_create_lane(session_id);

        match semaphore.clone().try_acquire_owned() {
            Ok(permit) => {
                self.metadata
                    .entry(session_id.to_string())
                    .and_modify(|m| {
                        m.last_used = Instant::now();
                        m.total_uses += 1;
                    })
                    .or_insert_with(|| {
                        let mut m = LaneMetadata::default();
                        m.total_uses = 1;
                        m
                    });

                Some(SessionLaneGuard {
                    session_id: session_id.to_string(),
                    _permit: permit,
                    acquired_at: Instant::now(),
                    manager: Arc::clone(self),
                })
            }
            Err(_) => None,
        }
    }

    /// Acquire the global lane for operations that affect multiple sessions
    pub async fn acquire_global(self: &Arc<Self>) -> OwnedSemaphorePermit {
        self.global_lane
            .clone()
            .acquire_owned()
            .await
            .expect("Global semaphore should not be closed")
    }

    /// Acquire a workspace lane for git operations
    ///
    /// This prevents concurrent git operations in the same workspace across different sessions.
    pub async fn acquire_workspace(self: &Arc<Self>, workspace_path: &str) -> OwnedSemaphorePermit {
        let semaphore = self
            .workspace_lanes
            .entry(workspace_path.to_string())
            .or_insert_with(|| Arc::new(Semaphore::new(1)))
            .clone();

        semaphore
            .acquire_owned()
            .await
            .expect("Workspace semaphore should not be closed")
    }

    /// Check if a session currently has an active request
    pub fn is_session_busy(&self, session_id: &str) -> bool {
        self.lanes
            .get(session_id)
            .map(|s| s.available_permits() == 0)
            .unwrap_or(false)
    }

    /// Get statistics about session lanes
    pub fn stats(&self) -> SessionLaneStats {
        let mut active_count = 0;
        let mut total_uses = 0;

        for entry in self.metadata.iter() {
            total_uses += entry.total_uses;
        }

        for entry in self.lanes.iter() {
            if entry.available_permits() == 0 {
                active_count += 1;
            }
        }

        SessionLaneStats {
            total_lanes: self.lanes.len(),
            active_lanes: active_count,
            total_requests_processed: total_uses,
        }
    }

    /// Prune idle session lanes to free memory
    ///
    /// This should be called periodically to clean up abandoned sessions.
    pub fn prune_idle_lanes(&self) {
        let now = Instant::now();
        let idle_threshold = Duration::from_secs(LANE_IDLE_TIMEOUT_SECS);

        let mut to_remove = Vec::new();

        for entry in self.metadata.iter() {
            if now.duration_since(entry.last_used) > idle_threshold {
                // Only remove if not currently in use
                if let Some(lane) = self.lanes.get(entry.key()) {
                    if lane.available_permits() > 0 {
                        to_remove.push(entry.key().clone());
                    }
                }
            }
        }

        // Limit total lanes if needed
        if self.lanes.len() > MAX_SESSION_LANES {
            // Find oldest lanes to remove
            let mut lanes_by_age: Vec<_> = self.metadata.iter()
                .map(|e| (e.key().clone(), e.last_used))
                .collect();
            lanes_by_age.sort_by(|a, b| a.1.cmp(&b.1));

            let excess = self.lanes.len() - MAX_SESSION_LANES;
            for (key, _) in lanes_by_age.into_iter().take(excess) {
                if !to_remove.contains(&key) {
                    // Only remove if not currently in use
                    if let Some(lane) = self.lanes.get(&key) {
                        if lane.available_permits() > 0 {
                            to_remove.push(key);
                        }
                    }
                }
            }
        }

        for key in to_remove {
            self.lanes.remove(&key);
            self.metadata.remove(&key);
        }
    }

    fn get_or_create_lane(&self, session_id: &str) -> Arc<Semaphore> {
        self.lanes
            .entry(session_id.to_string())
            .or_insert_with(|| Arc::new(Semaphore::new(1)))
            .clone()
    }
}

impl Default for SessionLaneManager {
    fn default() -> Self {
        Self {
            lanes: DashMap::new(),
            metadata: DashMap::new(),
            global_lane: Arc::new(Semaphore::new(1)),
            workspace_lanes: DashMap::new(),
        }
    }
}

/// Statistics about session lanes
#[derive(Debug, Clone)]
pub struct SessionLaneStats {
    /// Total number of session lanes tracked
    pub total_lanes: usize,
    /// Number of lanes currently in use
    pub active_lanes: usize,
    /// Total number of requests processed across all lanes
    pub total_requests_processed: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::sleep;

    #[tokio::test]
    async fn test_sequential_access() {
        let manager = SessionLaneManager::new();
        let session_id = "test-session";

        // First acquire should succeed immediately
        let guard1 = manager.acquire(session_id).await;
        assert!(manager.is_session_busy(session_id));

        // Try acquire should fail while first is held
        assert!(manager.try_acquire(session_id).is_none());

        // Drop first guard
        drop(guard1);

        // Now try acquire should succeed
        assert!(manager.try_acquire(session_id).is_some());
    }

    #[tokio::test]
    async fn test_different_sessions_parallel() {
        let manager = SessionLaneManager::new();

        // Acquire two different sessions
        let guard1 = manager.acquire("session-1").await;
        let guard2 = manager.acquire("session-2").await;

        // Both should be busy
        assert!(manager.is_session_busy("session-1"));
        assert!(manager.is_session_busy("session-2"));

        drop(guard1);
        drop(guard2);
    }

    #[tokio::test]
    async fn test_stats() {
        let manager = SessionLaneManager::new();

        let _guard1 = manager.acquire("session-1").await;
        let _guard2 = manager.acquire("session-2").await;

        let stats = manager.stats();
        assert_eq!(stats.total_lanes, 2);
        assert_eq!(stats.active_lanes, 2);
        assert_eq!(stats.total_requests_processed, 2);
    }
}
