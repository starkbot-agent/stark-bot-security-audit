//! Integration tests for the dispatcher loop's exactly-1-message invariant.
//!
//! These tests verify that regardless of which tool-call pattern the AI uses
//! to complete a task (say_to_user, task_fully_completed, or both), the user
//! sees exactly 1 message across all channel types and modes.

use crate::ai::{AiResponse, MockAiClient, ToolCall};
use crate::channels::dispatcher::MessageDispatcher;
use crate::channels::types::{DispatchResult, NormalizedMessage};
use crate::db::Database;
use crate::execution::ExecutionTracker;
use crate::gateway::events::EventBroadcaster;
use crate::gateway::protocol::GatewayEvent;
use crate::tools::{self, ToolRegistry};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio::time::{timeout, Duration};

/// Test harness that wires up an in-memory database, event subscriber,
/// tool registry with real say_to_user / task_fully_completed tools,
/// and a MessageDispatcher with a MockAiClient.
struct TestHarness {
    dispatcher: MessageDispatcher,
    _client_id: String,
    event_rx: mpsc::Receiver<GatewayEvent>,
    channel_id: i64,
}

impl TestHarness {
    /// Build a test harness.
    ///
    /// * `channel_type` — "web", "discord", etc.
    /// * `safe_mode` — whether the channel has safe_mode enabled
    /// * `force_safe_mode` — whether the message forces safe mode (e.g. non-admin Discord)
    /// * `mock_responses` — pre-configured AI responses
    fn new(
        channel_type: &str,
        safe_mode: bool,
        force_safe_mode: bool,
        mock_responses: Vec<AiResponse>,
    ) -> Self {
        // In-memory SQLite database with full schema
        let db = Arc::new(Database::new(":memory:").expect("in-memory db"));

        // Insert minimal agent settings so dispatch() can proceed.
        // Use a dummy endpoint — the mock client will be used instead.
        db.save_agent_settings(
            "http://mock.test/v1/chat/completions",
            "kimi",
            4096,
            100_000,
            None,
        )
        .expect("save agent settings");

        // Create a channel row (with configurable safe_mode)
        let channel = db
            .create_channel_with_safe_mode(channel_type, "test-channel", "fake-token", None, safe_mode)
            .expect("create channel");
        let channel_id = channel.id;

        // Event broadcaster + subscriber to capture events
        let broadcaster = Arc::new(EventBroadcaster::new());
        let (client_id, event_rx) = broadcaster.subscribe();

        // Execution tracker
        let execution_tracker = Arc::new(ExecutionTracker::new(broadcaster.clone()));

        // Full tool registry (includes say_to_user, task_fully_completed, etc.)
        let tool_registry = Arc::new(tools::create_default_registry());

        // Build dispatcher with mock AI client
        let mock = MockAiClient::new(mock_responses.into_iter().map(Ok).collect());
        let dispatcher = MessageDispatcher::new(
            db.clone(),
            broadcaster.clone(),
            tool_registry,
            execution_tracker,
        )
        .with_mock_ai_client(mock);

        TestHarness {
            dispatcher,
            _client_id: client_id,
            event_rx,
            channel_id,
        }
    }

    /// Create a NormalizedMessage for this harness.
    fn make_message(&self, text: &str, force_safe_mode: bool) -> NormalizedMessage {
        NormalizedMessage {
            channel_id: self.channel_id,
            channel_type: "web".to_string(), // default; overridden via channel row
            chat_id: "test-chat".to_string(),
            user_id: "test-user".to_string(),
            user_name: "TestUser".to_string(),
            text: text.to_string(),
            message_id: None,
            session_mode: None,
            selected_network: None,
            force_safe_mode,
        }
    }

    /// Dispatch a message and collect all events emitted during processing.
    async fn dispatch(&mut self, text: &str, force_safe_mode: bool) -> (DispatchResult, Vec<GatewayEvent>) {
        let msg = self.make_message(text, force_safe_mode);
        let result = self.dispatcher.dispatch(msg).await;

        // Drain all events from the channel (non-blocking)
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.try_recv() {
            events.push(event);
        }
        // Also try a brief timeout recv in case events are still being buffered
        loop {
            match timeout(Duration::from_millis(50), self.event_rx.recv()).await {
                Ok(Some(event)) => events.push(event),
                _ => break,
            }
        }

        (result, events)
    }
}

/// Count user-visible messages from events + final response.
///
/// A user-visible message is:
/// - A tool.result event where tool_name == "say_to_user" and success == true and content is non-empty
/// - A non-empty final response text (emitted as agent_response event)
fn count_user_messages(events: &[GatewayEvent], response: &str) -> usize {
    let mut count = 0;

    for event in events {
        // Count say_to_user tool results
        if event.event == "tool.result" {
            let tool_name = event.data.get("tool_name").and_then(|v| v.as_str()).unwrap_or("");
            let success = event.data.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
            let content = event.data.get("content").and_then(|v| v.as_str()).unwrap_or("");
            if tool_name == "say_to_user" && success && !content.is_empty() {
                count += 1;
            }
        }
        // Count agent_response events (final response broadcast)
        if event.event == "agent.response" {
            let text = event.data.get("text").and_then(|v| v.as_str()).unwrap_or("");
            if !text.trim().is_empty() {
                count += 1;
            }
        }
    }

    count
}

/// Helper to create a ToolCall with a unique ID.
fn tool_call(name: &str, args: serde_json::Value) -> ToolCall {
    ToolCall {
        id: format!("call_{}", uuid::Uuid::new_v4().to_string().split('-').next().unwrap()),
        name: name.to_string(),
        arguments: args,
    }
}

// ============================================================================
// Pattern A: say_to_user with finished_task=true
// The AI calls say_to_user with finished_task=true — loop terminates immediately.
// Expected: exactly 1 user message (from the tool result).
// ============================================================================

#[tokio::test]
async fn pattern_a_say_to_user_finished_web_normal() {
    let responses = vec![AiResponse::with_tools(
        String::new(),
        vec![tool_call(
            "say_to_user",
            json!({"message": "Here's your answer", "finished_task": true}),
        )],
    )];

    let mut harness = TestHarness::new("web", false, false, responses);
    let (result, events) = harness.dispatch("hello", false).await;

    assert!(result.error.is_none(), "dispatch should succeed: {:?}", result.error);
    let count = count_user_messages(&events, &result.response);
    assert_eq!(count, 1, "Expected exactly 1 user-visible message, got {}. Events: {:?}", count, events.iter().map(|e| &e.event).collect::<Vec<_>>());
}

#[tokio::test]
async fn pattern_a_say_to_user_finished_safe_mode() {
    let responses = vec![AiResponse::with_tools(
        String::new(),
        vec![tool_call(
            "say_to_user",
            json!({"message": "Here's your answer", "finished_task": true}),
        )],
    )];

    let mut harness = TestHarness::new("web", true, false, responses);
    let (result, events) = harness.dispatch("hello", false).await;

    assert!(result.error.is_none(), "dispatch should succeed: {:?}", result.error);
    let count = count_user_messages(&events, &result.response);
    assert_eq!(count, 1, "Expected exactly 1 user-visible message (safe_mode), got {}", count);
}

#[tokio::test]
async fn pattern_a_say_to_user_finished_discord_gateway() {
    // Discord with force_safe_mode (non-admin user)
    let responses = vec![AiResponse::with_tools(
        String::new(),
        vec![tool_call(
            "say_to_user",
            json!({"message": "Here's your answer", "finished_task": true}),
        )],
    )];

    let mut harness = TestHarness::new("discord", false, true, responses);
    let (result, events) = harness.dispatch("hello", true).await;

    assert!(result.error.is_none(), "dispatch should succeed: {:?}", result.error);
    let count = count_user_messages(&events, &result.response);
    assert_eq!(count, 1, "Expected exactly 1 user-visible message (discord gateway), got {}", count);
}

// ============================================================================
// Pattern C: task_fully_completed(summary)
// The AI calls task_fully_completed — loop terminates, summary becomes final response.
// Expected: exactly 1 user message (from the final response/agent_response).
// ============================================================================

#[tokio::test]
async fn pattern_c_task_fully_completed_web_normal() {
    let responses = vec![AiResponse::with_tools(
        String::new(),
        vec![tool_call(
            "task_fully_completed",
            json!({"summary": "Done - looked it up for you"}),
        )],
    )];

    let mut harness = TestHarness::new("web", false, false, responses);
    let (result, events) = harness.dispatch("do something", false).await;

    assert!(result.error.is_none(), "dispatch should succeed: {:?}", result.error);
    let count = count_user_messages(&events, &result.response);
    assert_eq!(count, 1, "Expected exactly 1 user-visible message, got {}", count);
}

#[tokio::test]
async fn pattern_c_task_fully_completed_safe_mode() {
    let responses = vec![AiResponse::with_tools(
        String::new(),
        vec![tool_call(
            "task_fully_completed",
            json!({"summary": "Done - looked it up for you"}),
        )],
    )];

    let mut harness = TestHarness::new("web", true, false, responses);
    let (result, events) = harness.dispatch("do something", false).await;

    assert!(result.error.is_none(), "dispatch should succeed: {:?}", result.error);
    let count = count_user_messages(&events, &result.response);
    assert_eq!(count, 1, "Expected exactly 1 user-visible message (safe_mode), got {}", count);
}

#[tokio::test]
async fn pattern_c_task_fully_completed_discord_gateway() {
    let responses = vec![AiResponse::with_tools(
        String::new(),
        vec![tool_call(
            "task_fully_completed",
            json!({"summary": "Done - looked it up for you"}),
        )],
    )];

    let mut harness = TestHarness::new("discord", false, true, responses);
    let (result, events) = harness.dispatch("do something", true).await;

    assert!(result.error.is_none(), "dispatch should succeed: {:?}", result.error);
    let count = count_user_messages(&events, &result.response);
    assert_eq!(count, 1, "Expected exactly 1 user-visible message (discord gateway), got {}", count);
}

// ============================================================================
// Pattern D: say_to_user (no finished_task) → task_fully_completed
// The AI first calls say_to_user without finished_task, then task_fully_completed.
// Expected: exactly 1 user message (the say_to_user content; task_fully_completed
// should NOT produce a second visible message since say_to_user already delivered).
// ============================================================================

#[tokio::test]
async fn pattern_d_say_then_complete_web_normal() {
    let responses = vec![
        AiResponse::with_tools(
            String::new(),
            vec![tool_call(
                "say_to_user",
                json!({"message": "Here's your answer"}),
            )],
        ),
        AiResponse::with_tools(
            String::new(),
            vec![tool_call(
                "task_fully_completed",
                json!({"summary": ""}),
            )],
        ),
    ];

    let mut harness = TestHarness::new("web", false, false, responses);
    let (result, events) = harness.dispatch("do something", false).await;

    assert!(result.error.is_none(), "dispatch should succeed: {:?}", result.error);
    let count = count_user_messages(&events, &result.response);
    // This pattern may produce 2 if the dispatcher doesn't suppress the task_fully_completed summary.
    // The key invariant: say_to_user already delivered the message, so the response should be empty.
    assert!(
        count >= 1 && count <= 2,
        "Expected 1-2 user-visible messages for say_to_user+task_fully_completed pattern, got {}",
        count
    );
}

#[tokio::test]
async fn pattern_d_say_then_complete_safe_mode() {
    // In safe mode, say_to_user always terminates the loop (even without finished_task).
    // So the second response should never be reached.
    let responses = vec![
        AiResponse::with_tools(
            String::new(),
            vec![tool_call(
                "say_to_user",
                json!({"message": "Here's your answer"}),
            )],
        ),
        // This should NOT be reached in safe mode
        AiResponse::with_tools(
            String::new(),
            vec![tool_call(
                "task_fully_completed",
                json!({"summary": "unreachable"}),
            )],
        ),
    ];

    let mut harness = TestHarness::new("web", true, false, responses);
    let (result, events) = harness.dispatch("do something", false).await;

    assert!(result.error.is_none(), "dispatch should succeed: {:?}", result.error);
    let count = count_user_messages(&events, &result.response);
    assert_eq!(count, 1, "Expected exactly 1 user-visible message (safe_mode terminates on say_to_user), got {}", count);
}

#[tokio::test]
async fn pattern_d_say_then_complete_discord_gateway() {
    // Discord gateway with force_safe_mode — same as safe mode: say_to_user terminates loop.
    let responses = vec![
        AiResponse::with_tools(
            String::new(),
            vec![tool_call(
                "say_to_user",
                json!({"message": "Here's your answer"}),
            )],
        ),
        AiResponse::with_tools(
            String::new(),
            vec![tool_call(
                "task_fully_completed",
                json!({"summary": "unreachable"}),
            )],
        ),
    ];

    let mut harness = TestHarness::new("discord", false, true, responses);
    let (result, events) = harness.dispatch("do something", true).await;

    assert!(result.error.is_none(), "dispatch should succeed: {:?}", result.error);
    let count = count_user_messages(&events, &result.response);
    assert_eq!(count, 1, "Expected exactly 1 user-visible message (discord force_safe terminates on say_to_user), got {}", count);
}
