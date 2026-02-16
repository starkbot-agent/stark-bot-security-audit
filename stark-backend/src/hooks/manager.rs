//! HookManager - Manages hook registration, configuration, and execution
//!
//! The HookManager is responsible for:
//! - Registering and unregistering hooks
//! - Executing hooks in priority order
//! - Tracking hook statistics
//! - Managing hook configuration from database

use super::types::{BoxedHook, Hook, HookConfig, HookContext, HookEvent, HookPriority, HookResult, HookStats};
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::time::timeout;

/// Manager for hook registration and execution
pub struct HookManager {
    /// Registered hooks by ID
    hooks: DashMap<String, BoxedHook>,
    /// Hooks grouped by event (sorted by priority)
    hooks_by_event: DashMap<HookEvent, Vec<String>>,
    /// Hook configuration overrides
    configs: DashMap<String, HookConfig>,
    /// Hook statistics
    stats: DashMap<String, HookStats>,
    /// Whether to continue on hook errors
    continue_on_error: bool,
}

impl HookManager {
    /// Create a new HookManager
    pub fn new() -> Self {
        Self {
            hooks: DashMap::new(),
            hooks_by_event: DashMap::new(),
            configs: DashMap::new(),
            stats: DashMap::new(),
            continue_on_error: true,
        }
    }

    /// Create a HookManager that stops on first error
    pub fn strict() -> Self {
        Self {
            hooks: DashMap::new(),
            hooks_by_event: DashMap::new(),
            configs: DashMap::new(),
            stats: DashMap::new(),
            continue_on_error: false,
        }
    }

    /// Register a hook
    pub fn register(&self, hook: BoxedHook) {
        let id = hook.id().to_string();
        let events = hook.events();

        // Store the hook
        self.hooks.insert(id.clone(), hook);

        // Add to event mappings
        for event in events {
            self.hooks_by_event
                .entry(event)
                .or_insert_with(Vec::new)
                .push(id.clone());
        }

        // Sort hooks by priority for each event
        self.sort_hooks_by_priority();

        // Initialize stats
        self.stats.insert(id.clone(), HookStats::default());

        log::debug!("[HOOKS] Registered hook: {}", self.hooks.get(&id).map(|h| h.name().to_string()).unwrap_or_else(|| "unknown".to_string()));
    }

    /// Unregister a hook
    pub fn unregister(&self, id: &str) {
        // Remove from hooks map
        if self.hooks.remove(id).is_some() {
            // Remove from event mappings
            for mut entry in self.hooks_by_event.iter_mut() {
                entry.value_mut().retain(|hook_id| hook_id != id);
            }
            log::debug!("[HOOKS] Unregistered hook: {}", id);
        }
    }

    /// Sort hooks by priority for all events
    fn sort_hooks_by_priority(&self) {
        for mut entry in self.hooks_by_event.iter_mut() {
            let hooks = &self.hooks;
            let configs = &self.configs;

            entry.value_mut().sort_by_key(|id| {
                // Check for priority override in config
                if let Some(config) = configs.get(id) {
                    if let Some(priority) = config.priority {
                        return priority as i32;
                    }
                }
                // Fall back to hook's default priority
                hooks
                    .get(id)
                    .map(|h| h.priority() as i32)
                    .unwrap_or(HookPriority::Normal as i32)
            });
        }
    }

    /// Set hook configuration
    pub fn configure(&self, config: HookConfig) {
        let id = config.id.clone();
        self.configs.insert(id, config);
        self.sort_hooks_by_priority();
    }

    /// Check if a hook is enabled
    fn is_enabled(&self, hook: &dyn Hook) -> bool {
        // Check config override first
        if let Some(config) = self.configs.get(hook.id()) {
            return config.enabled;
        }
        // Fall back to hook's default
        hook.enabled()
    }

    /// Get timeout for a hook
    fn get_timeout(&self, hook: &dyn Hook) -> std::time::Duration {
        // Check config override first
        if let Some(config) = self.configs.get(hook.id()) {
            if let Some(timeout_secs) = config.timeout_secs {
                return std::time::Duration::from_secs(timeout_secs);
            }
        }
        // Fall back to hook's default
        hook.timeout()
    }

    /// Execute all hooks for an event
    pub async fn execute(&self, event: HookEvent, context: &mut HookContext) -> HookResult {
        // Get hook IDs for this event
        let hook_ids: Vec<String> = self
            .hooks_by_event
            .get(&event)
            .map(|v| v.clone())
            .unwrap_or_default();

        if hook_ids.is_empty() {
            return HookResult::Continue(None);
        }

        log::debug!("[HOOKS] Executing {} hooks for event {:?}", hook_ids.len(), event);

        let mut final_result = HookResult::Continue(None);

        for hook_id in hook_ids {
            // Get the hook
            let hook = match self.hooks.get(&hook_id) {
                Some(h) => h.clone(),
                None => continue,
            };

            // Check if enabled
            if !self.is_enabled(hook.as_ref()) {
                continue;
            }

            // Execute with timeout
            let hook_timeout = self.get_timeout(hook.as_ref());
            let start = Instant::now();

            let result = match timeout(hook_timeout, hook.execute(context)).await {
                Ok(result) => result,
                Err(_) => {
                    log::warn!("[HOOKS] Hook {} timed out after {:?}", hook.id(), hook_timeout);
                    HookResult::Error(format!("Hook timed out after {:?}", hook_timeout))
                }
            };

            // Record stats
            let duration_ms = start.elapsed().as_millis() as u64;
            if let Some(mut stats) = self.stats.get_mut(&hook_id) {
                stats.record_execution(duration_ms, &result);
            }

            log::debug!(
                "[HOOKS] Hook {} completed in {}ms with result: {:?}",
                hook.id(),
                duration_ms,
                match &result {
                    HookResult::Continue(_) => "continue",
                    HookResult::Skip => "skip",
                    HookResult::Cancel(_) => "cancel",
                    HookResult::Replace(_) => "replace",
                    HookResult::Error(_) => "error",
                }
            );

            // Handle result
            match &result {
                HookResult::Continue(data) => {
                    // Merge data if provided
                    if data.is_some() {
                        final_result = result.clone();
                    }
                }
                HookResult::Skip => {
                    return HookResult::Skip;
                }
                HookResult::Cancel(msg) => {
                    return HookResult::Cancel(msg.clone());
                }
                HookResult::Replace(value) => {
                    final_result = HookResult::Replace(value.clone());
                }
                HookResult::Error(msg) => {
                    if !self.continue_on_error {
                        return HookResult::Error(msg.clone());
                    }
                    log::warn!("[HOOKS] Hook {} error (continuing): {}", hook.id(), msg);
                }
            }
        }

        final_result
    }

    /// Get hooks registered for an event
    pub fn get_hooks_for_event(&self, event: HookEvent) -> Vec<BoxedHook> {
        self.hooks_by_event
            .get(&event)
            .map(|ids| {
                ids.iter()
                    .filter_map(|id| self.hooks.get(id).map(|h| h.clone()))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Get all registered hooks
    pub fn get_all_hooks(&self) -> Vec<BoxedHook> {
        self.hooks.iter().map(|entry| entry.value().clone()).collect()
    }

    /// Get statistics for a hook
    pub fn get_stats(&self, id: &str) -> Option<HookStats> {
        self.stats.get(id).map(|s| s.clone())
    }

    /// Get all statistics
    pub fn get_all_stats(&self) -> HashMap<String, HookStats> {
        self.stats
            .iter()
            .map(|entry| (entry.key().clone(), entry.value().clone()))
            .collect()
    }

    /// Get hook count
    pub fn hook_count(&self) -> usize {
        self.hooks.len()
    }
}

impl Default for HookManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct TestHook {
        id: String,
        events: Vec<HookEvent>,
        priority: HookPriority,
    }

    #[async_trait]
    impl Hook for TestHook {
        fn id(&self) -> &str {
            &self.id
        }

        fn name(&self) -> &str {
            &self.id
        }

        fn events(&self) -> Vec<HookEvent> {
            self.events.clone()
        }

        fn priority(&self) -> HookPriority {
            self.priority
        }

        async fn execute(&self, _context: &mut HookContext) -> HookResult {
            HookResult::Continue(None)
        }
    }

    #[tokio::test]
    async fn test_hook_registration() {
        let manager = HookManager::new();

        let hook = Arc::new(TestHook {
            id: "test_hook".to_string(),
            events: vec![HookEvent::BeforeAgentStart],
            priority: HookPriority::Normal,
        });

        manager.register(hook);
        assert_eq!(manager.hook_count(), 1);

        manager.unregister("test_hook");
        assert_eq!(manager.hook_count(), 0);
    }

    #[tokio::test]
    async fn test_hook_execution() {
        let manager = HookManager::new();

        let hook = Arc::new(TestHook {
            id: "test_hook".to_string(),
            events: vec![HookEvent::BeforeAgentStart],
            priority: HookPriority::Normal,
        });

        manager.register(hook);

        let mut context = HookContext::new(HookEvent::BeforeAgentStart);
        let result = manager.execute(HookEvent::BeforeAgentStart, &mut context).await;

        assert!(result.should_continue());
    }
}
