//! Hook types and traits for extensible lifecycle hooks
//!
//! This module defines the core types and traits for the plugin/hook system.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::sync::Arc;
use std::time::Duration;

/// Events that hooks can subscribe to
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookEvent {
    /// Before the agent starts processing a message
    BeforeAgentStart,
    /// After the agent finishes processing (success or failure)
    AfterAgentEnd,
    /// Before a tool is called
    BeforeToolCall,
    /// After a tool call completes
    AfterToolCall,
    /// When the agent mode transitions (e.g., planning -> executing)
    OnModeTransition,
    /// When an error occurs during processing
    OnError,
    /// Before sending a response to the user
    BeforeResponse,
    /// After a memory is created or updated
    OnMemoryUpdate,
    /// Before a git commit is created
    BeforeCommit,
    /// After a git commit is created
    AfterCommit,
    /// Before code is pushed to remote
    BeforePush,
    /// After code is pushed to remote
    AfterPush,
    /// Before a PR is created
    BeforePrCreate,
    /// After a PR is created
    AfterPrCreate,
    /// Session started (new conversation)
    SessionStart,
    /// Session ended (conversation complete)
    SessionEnd,
    /// When a reward signal is emitted (telemetry)
    OnRewardEmitted,
    /// When a telemetry annotation is recorded
    OnAnnotation,
    /// When a rollout retry is triggered
    OnRolloutRetry,
    /// When a watchdog timeout fires
    OnWatchdogTimeout,
}

impl HookEvent {
    pub fn as_str(&self) -> &'static str {
        match self {
            HookEvent::BeforeAgentStart => "before_agent_start",
            HookEvent::AfterAgentEnd => "after_agent_end",
            HookEvent::BeforeToolCall => "before_tool_call",
            HookEvent::AfterToolCall => "after_tool_call",
            HookEvent::OnModeTransition => "on_mode_transition",
            HookEvent::OnError => "on_error",
            HookEvent::BeforeResponse => "before_response",
            HookEvent::OnMemoryUpdate => "on_memory_update",
            HookEvent::BeforeCommit => "before_commit",
            HookEvent::AfterCommit => "after_commit",
            HookEvent::BeforePush => "before_push",
            HookEvent::AfterPush => "after_push",
            HookEvent::BeforePrCreate => "before_pr_create",
            HookEvent::AfterPrCreate => "after_pr_create",
            HookEvent::SessionStart => "session_start",
            HookEvent::SessionEnd => "session_end",
            HookEvent::OnRewardEmitted => "on_reward_emitted",
            HookEvent::OnAnnotation => "on_annotation",
            HookEvent::OnRolloutRetry => "on_rollout_retry",
            HookEvent::OnWatchdogTimeout => "on_watchdog_timeout",
        }
    }
}

/// Result of a hook execution
#[derive(Debug, Clone)]
pub enum HookResult {
    /// Continue processing normally, optionally with modified data
    Continue(Option<Value>),
    /// Skip the current operation (e.g., skip a tool call)
    Skip,
    /// Cancel the entire operation with an error message
    Cancel(String),
    /// Replace the result with a different value
    Replace(Value),
    /// Hook execution failed
    Error(String),
}

impl HookResult {
    /// Check if the result allows continuing
    pub fn should_continue(&self) -> bool {
        matches!(self, HookResult::Continue(_) | HookResult::Replace(_))
    }

    /// Check if the operation should be skipped
    pub fn should_skip(&self) -> bool {
        matches!(self, HookResult::Skip)
    }

    /// Check if the operation should be cancelled
    pub fn should_cancel(&self) -> bool {
        matches!(self, HookResult::Cancel(_))
    }

    /// Get the error message if any
    pub fn error_message(&self) -> Option<&str> {
        match self {
            HookResult::Cancel(msg) | HookResult::Error(msg) => Some(msg),
            _ => None,
        }
    }
}

/// Priority levels for hook execution order
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookPriority {
    /// Execute first (e.g., security checks)
    Critical = 0,
    /// Execute early (e.g., rate limiting)
    High = 100,
    /// Normal execution order
    Normal = 500,
    /// Execute later (e.g., logging)
    Low = 900,
    /// Execute last (e.g., cleanup)
    Lowest = 1000,
}

impl Default for HookPriority {
    fn default() -> Self {
        HookPriority::Normal
    }
}

/// Context passed to hooks during execution
#[derive(Debug, Clone)]
pub struct HookContext {
    /// The event that triggered this hook
    pub event: HookEvent,
    /// Channel ID (if available)
    pub channel_id: Option<i64>,
    /// Session ID (if available)
    pub session_id: Option<i64>,
    /// User message (if applicable)
    pub message: Option<String>,
    /// Tool name (for tool-related events)
    pub tool_name: Option<String>,
    /// Tool arguments (for tool-related events)
    pub tool_args: Option<Value>,
    /// Tool result (for after_tool_call)
    pub tool_result: Option<Value>,
    /// Previous mode (for mode transitions)
    pub previous_mode: Option<String>,
    /// New mode (for mode transitions)
    pub new_mode: Option<String>,
    /// Error message (for on_error)
    pub error: Option<String>,
    /// Response content (for before_response)
    pub response: Option<String>,
    /// Git commit message (for commit hooks)
    pub commit_message: Option<String>,
    /// Files being committed/staged (for commit hooks)
    pub commit_files: Option<Vec<String>>,
    /// Git branch name (for git-related hooks)
    pub branch: Option<String>,
    /// Git remote name (for push hooks)
    pub remote: Option<String>,
    /// PR title (for PR hooks)
    pub pr_title: Option<String>,
    /// PR body/description (for PR hooks)
    pub pr_body: Option<String>,
    /// PR URL (for after_pr_create)
    pub pr_url: Option<String>,
    /// Workspace path (for file/git operations)
    pub workspace: Option<String>,
    /// Additional context data
    pub extra: Value,
}

impl HookContext {
    /// Create a new context for an event
    pub fn new(event: HookEvent) -> Self {
        Self {
            event,
            channel_id: None,
            session_id: None,
            message: None,
            tool_name: None,
            tool_args: None,
            tool_result: None,
            previous_mode: None,
            new_mode: None,
            error: None,
            response: None,
            commit_message: None,
            commit_files: None,
            branch: None,
            remote: None,
            pr_title: None,
            pr_body: None,
            pr_url: None,
            workspace: None,
            extra: Value::Null,
        }
    }

    /// Set channel context
    pub fn with_channel(mut self, channel_id: i64, session_id: Option<i64>) -> Self {
        self.channel_id = Some(channel_id);
        self.session_id = session_id;
        self
    }

    /// Set message context
    pub fn with_message(mut self, message: String) -> Self {
        self.message = Some(message);
        self
    }

    /// Set tool context
    pub fn with_tool(mut self, name: String, args: Value) -> Self {
        self.tool_name = Some(name);
        self.tool_args = Some(args);
        self
    }

    /// Set tool result
    pub fn with_tool_result(mut self, result: Value) -> Self {
        self.tool_result = Some(result);
        self
    }

    /// Set mode transition context
    pub fn with_mode_transition(mut self, previous: String, new: String) -> Self {
        self.previous_mode = Some(previous);
        self.new_mode = Some(new);
        self
    }

    /// Set error context
    pub fn with_error(mut self, error: String) -> Self {
        self.error = Some(error);
        self
    }

    /// Set response context
    pub fn with_response(mut self, response: String) -> Self {
        self.response = Some(response);
        self
    }

    /// Set commit context
    pub fn with_commit(mut self, message: String, files: Vec<String>) -> Self {
        self.commit_message = Some(message);
        self.commit_files = Some(files);
        self
    }

    /// Set git branch context
    pub fn with_branch(mut self, branch: String) -> Self {
        self.branch = Some(branch);
        self
    }

    /// Set git remote context
    pub fn with_remote(mut self, remote: String) -> Self {
        self.remote = Some(remote);
        self
    }

    /// Set PR context
    pub fn with_pr(mut self, title: String, body: Option<String>) -> Self {
        self.pr_title = Some(title);
        self.pr_body = body;
        self
    }

    /// Set PR URL (after creation)
    pub fn with_pr_url(mut self, url: String) -> Self {
        self.pr_url = Some(url);
        self
    }

    /// Set workspace path
    pub fn with_workspace(mut self, workspace: String) -> Self {
        self.workspace = Some(workspace);
        self
    }

    /// Set extra data
    pub fn with_extra(mut self, extra: Value) -> Self {
        self.extra = extra;
        self
    }
}

/// The main Hook trait that all hooks must implement
#[async_trait]
pub trait Hook: Send + Sync {
    /// Unique identifier for this hook
    fn id(&self) -> &str;

    /// Human-readable name for this hook
    fn name(&self) -> &str;

    /// Description of what this hook does
    fn description(&self) -> &str {
        ""
    }

    /// Events this hook subscribes to
    fn events(&self) -> Vec<HookEvent>;

    /// Priority for execution order (lower = earlier)
    fn priority(&self) -> HookPriority {
        HookPriority::Normal
    }

    /// Timeout for hook execution
    fn timeout(&self) -> Duration {
        Duration::from_secs(5)
    }

    /// Whether this hook is enabled
    fn enabled(&self) -> bool {
        true
    }

    /// Execute the hook
    async fn execute(&self, context: &mut HookContext) -> HookResult;
}

/// Configuration for a hook
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookConfig {
    /// Hook ID
    pub id: String,
    /// Whether this hook is enabled
    pub enabled: bool,
    /// Priority override
    pub priority: Option<HookPriority>,
    /// Timeout override in seconds
    pub timeout_secs: Option<u64>,
    /// Custom configuration for the hook
    pub config: Option<Value>,
}

/// Statistics for a hook
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookStats {
    /// Total number of executions
    pub executions: u64,
    /// Number of successful executions
    pub successes: u64,
    /// Number of failed executions
    pub failures: u64,
    /// Number of skips
    pub skips: u64,
    /// Number of cancellations
    pub cancellations: u64,
    /// Average execution time in milliseconds
    pub avg_execution_ms: f64,
    /// Maximum execution time in milliseconds
    pub max_execution_ms: u64,
}

impl HookStats {
    pub fn record_execution(&mut self, duration_ms: u64, result: &HookResult) {
        self.executions += 1;

        // Update average
        let total = self.avg_execution_ms * (self.executions - 1) as f64;
        self.avg_execution_ms = (total + duration_ms as f64) / self.executions as f64;

        // Update max
        if duration_ms > self.max_execution_ms {
            self.max_execution_ms = duration_ms;
        }

        // Record result type
        match result {
            HookResult::Continue(_) | HookResult::Replace(_) => self.successes += 1,
            HookResult::Skip => self.skips += 1,
            HookResult::Cancel(_) => self.cancellations += 1,
            HookResult::Error(_) => self.failures += 1,
        }
    }
}

/// A boxed hook for storage in collections
pub type BoxedHook = Arc<dyn Hook>;
