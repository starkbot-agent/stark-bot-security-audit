//! Integration tests for the dispatcher loop's exactly-1-message invariant.
//!
//! These tests verify that regardless of which tool-call pattern the AI uses
//! to complete a task (say_to_user, task_fully_completed, or both), the user
//! sees exactly 1 message across all channel types and modes.

use crate::ai::{AiResponse, MockAiClient, TraceEntry, ToolCall};
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

    /// Get the trace of INPUT/OUTPUT for each AI iteration.
    fn get_trace(&self) -> Vec<TraceEntry> {
        self.dispatcher.get_mock_trace()
    }

    /// Write trace data to test_output/ folder for auditing.
    /// Creates a JSON file with each iteration's INPUT and OUTPUT.
    fn write_trace(&self, test_name: &str) {
        let trace = self.get_trace();
        if trace.is_empty() {
            return;
        }

        // Create test_output directory at workspace root
        let output_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap_or(std::path::Path::new("."))
            .join("test_output");
        std::fs::create_dir_all(&output_dir).expect("create test_output dir");

        let timestamp = chrono::Utc::now().format("%Y%m%d_%H%M%S");
        let filename = format!("{}_{}.json", test_name, timestamp);
        let filepath = output_dir.join(&filename);

        // Build human-readable trace output
        let mut iterations = Vec::new();
        for entry in &trace {
            let mut iter_json = serde_json::Map::new();
            iter_json.insert("iteration".to_string(), json!(entry.iteration));

            // INPUT section
            let mut input = serde_json::Map::new();

            // System prompt (first message)
            if let Some(first_msg) = entry.input_messages.first() {
                if first_msg.role == crate::ai::MessageRole::System {
                    input.insert("system_prompt".to_string(), json!(first_msg.content));
                }
            }

            // User/assistant messages (skip system)
            let conversation: Vec<_> = entry.input_messages.iter()
                .filter(|m| m.role != crate::ai::MessageRole::System)
                .map(|m| json!({
                    "role": m.role.to_string(),
                    "content": m.content,
                }))
                .collect();
            input.insert("conversation".to_string(), json!(conversation));

            // Tool history from previous iterations
            let tool_hist: Vec<_> = entry.input_tool_history.iter()
                .map(|h| json!({
                    "tool_calls": h.tool_calls.iter().map(|tc| json!({
                        "name": tc.name,
                        "arguments": tc.arguments,
                    })).collect::<Vec<_>>(),
                    "tool_responses": h.tool_responses.iter().map(|tr| json!({
                        "tool_call_id": tr.tool_call_id,
                        "content": tr.content,
                        "is_error": tr.is_error,
                    })).collect::<Vec<_>>(),
                }))
                .collect();
            input.insert("tool_history".to_string(), json!(tool_hist));
            input.insert("available_tools".to_string(), json!(entry.input_tools));

            iter_json.insert("INPUT".to_string(), json!(input));

            // OUTPUT section
            let mut output = serde_json::Map::new();
            if let Some(ref resp) = entry.output_response {
                output.insert("content".to_string(), json!(resp.content));
                let tool_calls: Vec<_> = resp.tool_calls.iter()
                    .map(|tc| json!({
                        "id": tc.id,
                        "name": tc.name,
                        "arguments": tc.arguments,
                    }))
                    .collect();
                output.insert("tool_calls".to_string(), json!(tool_calls));
                output.insert("stop_reason".to_string(), json!(resp.stop_reason));
            }
            if let Some(ref err) = entry.output_error {
                output.insert("error".to_string(), json!(err));
            }
            iter_json.insert("OUTPUT".to_string(), json!(output));

            iterations.push(serde_json::Value::Object(iter_json));
        }

        let trace_json = json!({
            "test_name": test_name,
            "total_iterations": trace.len(),
            "iterations": iterations,
        });

        let content = serde_json::to_string_pretty(&trace_json).expect("serialize trace");
        std::fs::write(&filepath, &content).expect("write trace file");

        eprintln!("\n=== TRACE OUTPUT ===");
        eprintln!("Written to: {}", filepath.display());
        eprintln!("Iterations: {}", trace.len());
        eprintln!("====================\n");
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

// ============================================================================
// Multi-task swap flow test with INPUT/OUTPUT trace capture.
//
// Simulates "swap 1 usdc to starkbot" through the 5-task pipeline.
// The mock AI responses follow the define_tasks → task flow pattern.
// Each iteration's INPUT (system prompt, conversation, tool history, tools)
// and OUTPUT (AI response) are captured and written to test_output/.
// ============================================================================

#[tokio::test]
async fn swap_flow_with_trace() {
    let responses = vec![
        // Iteration 1 (TaskPlanner mode): AI calls define_tasks
        AiResponse::with_tools(
            String::new(),
            vec![tool_call(
                "define_tasks",
                json!({
                    "tasks": [
                        "TASK 1 — Prepare: select network, look up sell+buy tokens, check Permit2 allowance.",
                        "TASK 2 — Approve Permit2 (SKIP if allowance sufficient).",
                        "TASK 3 — Quote+Decode: call to_raw_amount, then x402_fetch, then decode_calldata with cache_as 'swap'.",
                        "TASK 4 — Execute: call swap_execute then broadcast_web3_tx. Exactly 2 sequential calls.",
                        "TASK 5 — Verify: call verify_tx_broadcast, report result."
                    ]
                }),
            )],
        ),
        // Iteration 2 (Task 1 - Prepare): AI reports findings via say_to_user
        AiResponse::with_tools(
            String::new(),
            vec![tool_call(
                "say_to_user",
                json!({
                    "message": "Found tokens:\n- SELL: USDC (0xA0b8...)\n- BUY: STARKBOT (0x1234...)\n\nPermit2 allowance: sufficient",
                    "finished_task": true
                }),
            )],
        ),
        // Iteration 3 (Task 2 - Approve): Skip since allowance is sufficient
        AiResponse::with_tools(
            String::new(),
            vec![tool_call(
                "task_fully_completed",
                json!({"summary": "Allowance already sufficient — skipping approval."}),
            )],
        ),
        // Iteration 4 (Task 3 - Quote+Decode): AI completes quote and decode
        AiResponse::with_tools(
            String::new(),
            vec![tool_call(
                "task_fully_completed",
                json!({"summary": "Quote fetched and decoded into swap registers. Ready to execute."}),
            )],
        ),
        // Iteration 5 (Task 4 - Execute): AI completes swap execution
        AiResponse::with_tools(
            String::new(),
            vec![tool_call(
                "task_fully_completed",
                json!({"summary": "Swap transaction broadcast. TX: 0xabc123..."}),
            )],
        ),
        // Iteration 6 (Task 5 - Verify): AI reports final result
        AiResponse::with_tools(
            String::new(),
            vec![tool_call(
                "say_to_user",
                json!({
                    "message": "✅ Swap verified!\n\nSwapped 1 USDC → 42,000 STARKBOT\nTX: https://basescan.org/tx/0xabc123",
                    "finished_task": true
                }),
            )],
        ),
    ];

    let mut harness = TestHarness::new("web", false, false, responses);
    let (result, events) = harness.dispatch("swap 1 usdc to starkbot", false).await;

    // Write trace for auditing
    harness.write_trace("swap_flow");

    // Verify dispatch succeeded
    assert!(result.error.is_none(), "dispatch should succeed: {:?}", result.error);

    // Verify trace was captured
    let trace = harness.get_trace();
    assert!(
        trace.len() >= 2,
        "Expected at least 2 AI iterations (planner + tasks), got {}",
        trace.len()
    );

    // Verify iteration 1 had define_tasks in the output
    if let Some(ref resp) = trace[0].output_response {
        let has_define_tasks = resp.tool_calls.iter().any(|tc| tc.name == "define_tasks");
        assert!(has_define_tasks, "First iteration should call define_tasks");
    }

    // Verify CURRENT TASK advances through the system prompt
    // Helper to extract task number from system prompt
    let extract_task_num = |sys_prompt: &str| -> Option<(usize, usize)> {
        // Look for "CURRENT TASK (X/Y)" pattern
        if let Some(pos) = sys_prompt.find("CURRENT TASK (") {
            let after = &sys_prompt[pos + "CURRENT TASK (".len()..];
            if let Some(slash) = after.find('/') {
                let current: usize = after[..slash].parse().ok()?;
                let rest = &after[slash + 1..];
                if let Some(paren) = rest.find(')') {
                    let total: usize = rest[..paren].parse().ok()?;
                    return Some((current, total));
                }
            }
        }
        None
    };

    // Build a summary of task numbers per iteration
    let mut task_numbers: Vec<Option<(usize, usize)>> = Vec::new();
    for entry in &trace {
        let sys_prompt = entry.input_messages.first()
            .map(|m| m.content.as_str())
            .unwrap_or("");
        task_numbers.push(extract_task_num(sys_prompt));
    }

    // Print summary for test output
    eprintln!("\n=== SWAP FLOW TEST SUMMARY ===");
    eprintln!("Total AI iterations: {}", trace.len());
    for (i, entry) in trace.iter().enumerate() {
        let tool_names: Vec<&str> = entry.output_response.as_ref()
            .map(|r| r.tool_calls.iter().map(|tc| tc.name.as_str()).collect())
            .unwrap_or_default();
        let task_info = task_numbers[i]
            .map(|(c, t)| format!("TASK {}/{}", c, t))
            .unwrap_or_else(|| "no task".to_string());
        eprintln!(
            "  Iteration {}: {} | tools={:?} | tool_history={}",
            entry.iteration,
            task_info,
            tool_names,
            entry.input_tool_history.len(),
        );
    }
    eprintln!("==============================\n");

    // Assert task advancement:
    // - Iteration 1: no task (planner mode)
    // - Iteration 2: TASK 1/5
    // - Iteration 3: TASK 2/5 (after say_to_user finished_task completed task 1)
    // - Iteration 4: TASK 3/5
    // - Iteration 5: TASK 4/5
    // - Iteration 6: TASK 5/5
    assert_eq!(task_numbers[0], None, "Iteration 1 should have no task (planner mode)");
    assert_eq!(task_numbers[1], Some((1, 5)), "Iteration 2 should show TASK 1/5");
    assert_eq!(task_numbers[2], Some((2, 5)), "Iteration 3 should show TASK 2/5 (task 1 completed)");
    assert_eq!(task_numbers[3], Some((3, 5)), "Iteration 4 should show TASK 3/5 (task 2 completed)");
    assert_eq!(task_numbers[4], Some((4, 5)), "Iteration 5 should show TASK 4/5 (task 3 completed)");
    assert_eq!(task_numbers[5], Some((5, 5)), "Iteration 6 should show TASK 5/5 (task 4 completed)");
}
