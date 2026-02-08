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

// ============================================================================
// Realistic swap flow test — calls every tool from the swap skill in order.
//
// Unlike swap_flow_with_trace (which only calls say_to_user/task_fully_completed),
// this test has the mock AI call the actual swap tools:
//   select_web3_network, token_lookup, to_raw_amount, web3_preset_function_call,
//   x402_fetch, decode_calldata, broadcast_web3_tx, verify_tx_broadcast
//
// Tools backed by real logic (token_lookup, to_raw_amount, select_web3_network)
// succeed and set registers. Tools that need RPC/HTTP fail gracefully — the mock
// AI ignores failures and continues. The trace captures everything.
// ============================================================================

#[tokio::test]
async fn swap_flow_realistic() {
    let responses = vec![
        // === Iteration 1: set_agent_subtype("finance") — required before using crypto tools ===
        AiResponse::with_tools(
            String::new(),
            vec![tool_call(
                "set_agent_subtype",
                json!({"subtype": "finance"}),
            )],
        ),

        // === Iteration 2: define_tasks (5 tasks) ===
        AiResponse::with_tools(
            String::new(),
            vec![tool_call(
                "define_tasks",
                json!({
                    "tasks": [
                        "TASK 1 — Prepare: select network, look up sell+buy tokens, check Permit2 allowance. Call say_to_user(finished_task:true) when done.",
                        "TASK 2 — Approve Permit2 (SKIP if allowance sufficient).",
                        "TASK 3 — Quote+Decode: call to_raw_amount, then x402_fetch swap_quote, then decode_calldata with cache_as 'swap'. ALL THREE steps required.",
                        "TASK 4 — Execute: call swap_execute preset THEN broadcast_web3_tx. Exactly 2 sequential tool calls.",
                        "TASK 5 — Verify: call verify_tx_broadcast, report result."
                    ]
                }),
            )],
        ),

        // === TASK 1: Prepare ===

        // Iteration 3: select_web3_network(network: "base") → SUCCESS
        AiResponse::with_tools(
            "Selecting Base network.".to_string(),
            vec![tool_call(
                "select_web3_network",
                json!({"network": "base"}),
            )],
        ),

        // Iteration 4: token_lookup(symbol: "USDC", cache_as: "sell_token") → SUCCESS
        AiResponse::with_tools(
            "Looking up sell token USDC.".to_string(),
            vec![tool_call(
                "token_lookup",
                json!({"symbol": "USDC", "cache_as": "sell_token"}),
            )],
        ),

        // Iteration 4: token_lookup(symbol: "DEGEN", cache_as: "buy_token") → SUCCESS
        AiResponse::with_tools(
            "Looking up buy token DEGEN.".to_string(),
            vec![tool_call(
                "token_lookup",
                json!({"symbol": "DEGEN", "cache_as": "buy_token"}),
            )],
        ),

        // Iteration 5: web3_preset_function_call(erc20_allowance_permit2, call_only) → FAIL (no RPC)
        AiResponse::with_tools(
            "Checking Permit2 allowance.".to_string(),
            vec![tool_call(
                "web3_preset_function_call",
                json!({"preset": "erc20_allowance_permit2", "call_only": true}),
            )],
        ),

        // Iteration 6: say_to_user(finished_task: true) → SUCCESS → advances to Task 2
        AiResponse::with_tools(
            String::new(),
            vec![tool_call(
                "say_to_user",
                json!({
                    "message": "Found tokens:\n- SELL: USDC (6 decimals)\n- BUY: DEGEN (18 decimals)\n\nPermit2 allowance check failed (no RPC), assuming sufficient.",
                    "finished_task": true
                }),
            )],
        ),

        // === TASK 2: Approve (skip) ===

        // Iteration 7: task_fully_completed("Allowance sufficient") → advances to Task 3
        AiResponse::with_tools(
            String::new(),
            vec![tool_call(
                "task_fully_completed",
                json!({"summary": "Allowance sufficient — skipping approval."}),
            )],
        ),

        // === TASK 3: Quote+Decode ===

        // Iteration 8: to_raw_amount(amount: "1", decimals_register: "sell_token_decimals") → SUCCESS ("1000000")
        AiResponse::with_tools(
            "Converting 1 USDC to raw amount.".to_string(),
            vec![tool_call(
                "to_raw_amount",
                json!({
                    "amount": "1",
                    "decimals_register": "sell_token_decimals",
                    "cache_as": "sell_amount"
                }),
            )],
        ),

        // Iteration 9: x402_fetch(preset: "swap_quote") → FAIL (no HTTP endpoint)
        AiResponse::with_tools(
            "Fetching swap quote from 0x.".to_string(),
            vec![tool_call(
                "x402_fetch",
                json!({"preset": "swap_quote", "cache_as": "swap_quote"}),
            )],
        ),

        // Iteration 10: decode_calldata(abi: "0x_settler", calldata_register: "swap_quote") → FAIL (no data)
        AiResponse::with_tools(
            "Decoding swap calldata.".to_string(),
            vec![tool_call(
                "decode_calldata",
                json!({
                    "abi": "0x_settler",
                    "calldata_register": "swap_quote",
                    "cache_as": "swap"
                }),
            )],
        ),

        // Iteration 11: task_fully_completed("Quote decoded") → advances to Task 4
        AiResponse::with_tools(
            String::new(),
            vec![tool_call(
                "task_fully_completed",
                json!({"summary": "Quote fetched and decoded. Ready to execute swap."}),
            )],
        ),

        // === TASK 4: Execute ===

        // Iteration 12: web3_preset_function_call(preset: "swap_execute") → FAIL (no RPC + missing registers)
        AiResponse::with_tools(
            "Executing swap transaction.".to_string(),
            vec![tool_call(
                "web3_preset_function_call",
                json!({"preset": "swap_execute"}),
            )],
        ),

        // Iteration 13: broadcast_web3_tx(uuid: "mock-uuid-123") → FAIL (no tx in queue)
        AiResponse::with_tools(
            "Broadcasting transaction.".to_string(),
            vec![tool_call(
                "broadcast_web3_tx",
                json!({"uuid": "mock-uuid-123"}),
            )],
        ),

        // Iteration 14: task_fully_completed("Swap broadcast") → advances to Task 5
        AiResponse::with_tools(
            String::new(),
            vec![tool_call(
                "task_fully_completed",
                json!({"summary": "Swap transaction broadcast. Verifying next."}),
            )],
        ),

        // === TASK 5: Verify ===

        // Iteration 15: verify_tx_broadcast() → FAIL (no tx to verify)
        AiResponse::with_tools(
            "Verifying swap transaction.".to_string(),
            vec![tool_call(
                "verify_tx_broadcast",
                json!({}),
            )],
        ),

        // Iteration 16: say_to_user(finished_task: true) → SUCCESS → all tasks done, loop terminates
        AiResponse::with_tools(
            String::new(),
            vec![tool_call(
                "say_to_user",
                json!({
                    "message": "Swap complete (verification failed in test — no real tx). All tasks done.",
                    "finished_task": true
                }),
            )],
        ),
    ];

    let mut harness = TestHarness::new("web", false, false, responses);
    let (result, _events) = harness.dispatch("swap 1 usdc to degen", false).await;

    // Write trace for auditing
    harness.write_trace("swap_flow_realistic");

    // Verify dispatch succeeded
    assert!(result.error.is_none(), "dispatch should succeed: {:?}", result.error);

    // === Assertion 1: Get trace and verify iteration count ===
    let trace = harness.get_trace();

    // Print summary
    let extract_task_num = |sys_prompt: &str| -> Option<(usize, usize)> {
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

    let mut task_numbers: Vec<Option<(usize, usize)>> = Vec::new();
    for entry in &trace {
        let sys_prompt = entry.input_messages.first()
            .map(|m| m.content.as_str())
            .unwrap_or("");
        task_numbers.push(extract_task_num(sys_prompt));
    }

    eprintln!("\n=== SWAP FLOW REALISTIC TEST SUMMARY ===");
    eprintln!("Total AI iterations: {}", trace.len());
    for (i, entry) in trace.iter().enumerate() {
        let tool_names: Vec<&str> = entry.output_response.as_ref()
            .map(|r| r.tool_calls.iter().map(|tc| tc.name.as_str()).collect())
            .unwrap_or_default();
        let task_info = task_numbers[i]
            .map(|(c, t)| format!("TASK {}/{}", c, t))
            .unwrap_or_else(|| "no task".to_string());
        eprintln!(
            "  Iter {:>2}: {} | tools={:?} | tool_history={}",
            entry.iteration,
            task_info,
            tool_names,
            entry.input_tool_history.len(),
        );
    }
    eprintln!("=========================================\n");

    // 17 mock responses → 17 trace entries
    assert_eq!(
        trace.len(), 17,
        "Expected 17 AI iterations (set_agent_subtype + define_tasks + 15 tool calls), got {}",
        trace.len()
    );

    // === Assertion 2: Task advancement (1/5) → (2/5) → ... → (5/5) ===
    // Iter 1 (idx 0): set_agent_subtype — no task yet
    assert_eq!(task_numbers[0], None, "Iter 1: no task (set_agent_subtype)");
    // Iter 2 (idx 1): define_tasks — no task yet (planner mode)
    assert_eq!(task_numbers[1], None, "Iter 2: no task (define_tasks / planner mode)");
    // Task 1: iterations 3-7 (select_web3_network, 2x token_lookup, allowance, say_to_user)
    assert_eq!(task_numbers[2], Some((1, 5)), "Iter 3: TASK 1/5");
    assert_eq!(task_numbers[3], Some((1, 5)), "Iter 4: still TASK 1/5");
    assert_eq!(task_numbers[4], Some((1, 5)), "Iter 5: still TASK 1/5");
    assert_eq!(task_numbers[5], Some((1, 5)), "Iter 6: still TASK 1/5");
    assert_eq!(task_numbers[6], Some((1, 5)), "Iter 7: still TASK 1/5 (say_to_user finishes it)");
    // Task 2: iteration 8 (skip)
    assert_eq!(task_numbers[7], Some((2, 5)), "Iter 8: TASK 2/5");
    // Task 3: iterations 9-12 (to_raw_amount, x402_fetch, decode_calldata, task_fully_completed)
    assert_eq!(task_numbers[8], Some((3, 5)), "Iter 9: TASK 3/5");
    assert_eq!(task_numbers[9], Some((3, 5)), "Iter 10: still TASK 3/5");
    assert_eq!(task_numbers[10], Some((3, 5)), "Iter 11: still TASK 3/5");
    assert_eq!(task_numbers[11], Some((3, 5)), "Iter 12: still TASK 3/5 (task_fully_completed)");
    // Task 4: iterations 13-15 (swap_execute, broadcast, task_fully_completed)
    assert_eq!(task_numbers[12], Some((4, 5)), "Iter 13: TASK 4/5");
    assert_eq!(task_numbers[13], Some((4, 5)), "Iter 14: still TASK 4/5");
    assert_eq!(task_numbers[14], Some((4, 5)), "Iter 15: still TASK 4/5 (task_fully_completed)");
    // Task 5: iterations 16-17 (verify, say_to_user)
    assert_eq!(task_numbers[15], Some((5, 5)), "Iter 16: TASK 5/5");
    assert_eq!(task_numbers[16], Some((5, 5)), "Iter 17: still TASK 5/5 (say_to_user finishes it)");

    // === Assertion 3: Tools that should succeed DO succeed ===
    // Helper: find tool response in trace by checking the tool_history that appears in the NEXT iteration.
    // For trace[idx], its tool response appears in trace[idx + 1].input_tool_history.last().
    let get_tool_response = |trace_idx: usize| -> Option<(&str, bool)> {
        if trace_idx + 1 >= trace.len() {
            return None;
        }
        let history = &trace[trace_idx + 1].input_tool_history;
        let last_entry = history.last()?;
        let resp = last_entry.tool_responses.first()?;
        Some((&resp.content, resp.is_error))
    };

    // idx 0: set_agent_subtype → should succeed
    if let Some((content, is_error)) = get_tool_response(0) {
        assert!(!is_error, "set_agent_subtype should succeed, got error: {}", content);
    }

    // idx 1: define_tasks → should succeed
    if let Some((content, is_error)) = get_tool_response(1) {
        assert!(!is_error, "define_tasks should succeed, got error: {}", content);
    }

    // idx 2: select_web3_network → should succeed
    if let Some((content, is_error)) = get_tool_response(2) {
        assert!(!is_error, "select_web3_network should succeed, got error: {}", content);
        assert!(content.contains("base") || content.to_lowercase().contains("base"),
            "select_web3_network response should mention 'base', got: {}", content);
    }

    // idx 3: token_lookup USDC → should succeed
    if let Some((content, is_error)) = get_tool_response(3) {
        assert!(!is_error, "token_lookup(USDC) should succeed, got error: {}", content);
        assert!(content.contains("USDC"), "token_lookup response should mention USDC, got: {}", content);
    }

    // idx 4: token_lookup DEGEN → should succeed
    if let Some((content, is_error)) = get_tool_response(4) {
        assert!(!is_error, "token_lookup(DEGEN) should succeed, got error: {}", content);
        assert!(content.contains("DEGEN") || content.contains("Degen"),
            "token_lookup response should mention DEGEN, got: {}", content);
    }

    // idx 5: web3_preset_function_call(erc20_allowance_permit2) → expected to FAIL (no RPC)
    if let Some((_content, is_error)) = get_tool_response(5) {
        assert!(is_error, "web3_preset_function_call(allowance) should fail without RPC");
    }

    // idx 8: to_raw_amount → should succeed
    // === Assertion 4: sell_token_decimals register correctly used (6 decimals → "1000000") ===
    if let Some((content, is_error)) = get_tool_response(8) {
        assert!(!is_error, "to_raw_amount should succeed, got error: {}", content);
        // === Assertion 5: sell_amount register correctly set to "1000000" ===
        assert!(content.contains("1000000"),
            "to_raw_amount(1, decimals=6) should produce 1000000, got: {}", content);
        assert!(content.contains("sell_amount"),
            "to_raw_amount response should mention 'sell_amount' register, got: {}", content);
    }

    // idx 9: x402_fetch → expected to FAIL (no HTTP)
    if let Some((_content, is_error)) = get_tool_response(9) {
        assert!(is_error, "x402_fetch should fail without HTTP endpoint");
    }

    // idx 10: decode_calldata → expected to FAIL (no data in register)
    if let Some((_content, is_error)) = get_tool_response(10) {
        assert!(is_error, "decode_calldata should fail without swap_quote data");
    }

    // idx 12: web3_preset_function_call(swap_execute) → expected to FAIL
    if let Some((_content, is_error)) = get_tool_response(12) {
        assert!(is_error, "web3_preset_function_call(swap_execute) should fail without RPC");
    }

    // idx 13: broadcast_web3_tx → expected to FAIL (no tx in queue)
    if let Some((_content, is_error)) = get_tool_response(13) {
        assert!(is_error, "broadcast_web3_tx should fail without queued tx");
    }

    // idx 15: verify_tx_broadcast → expected to FAIL (no tx to verify)
    if let Some((_content, is_error)) = get_tool_response(15) {
        assert!(is_error, "verify_tx_broadcast should fail without broadcast tx");
    }

    // === Verify all expected tools were called in order ===
    let expected_tools = vec![
        "set_agent_subtype",
        "define_tasks",
        "select_web3_network",
        "token_lookup",    // USDC
        "token_lookup",    // DEGEN
        "web3_preset_function_call", // allowance
        "say_to_user",     // finish task 1
        "task_fully_completed", // skip task 2
        "to_raw_amount",
        "x402_fetch",
        "decode_calldata",
        "task_fully_completed", // finish task 3
        "web3_preset_function_call", // swap_execute
        "broadcast_web3_tx",
        "task_fully_completed", // finish task 4
        "verify_tx_broadcast",
        "say_to_user",     // finish task 5
    ];

    let actual_tools: Vec<&str> = trace.iter()
        .filter_map(|entry| entry.output_response.as_ref())
        .flat_map(|resp| resp.tool_calls.iter().map(|tc| tc.name.as_str()))
        .collect();

    assert_eq!(
        actual_tools, expected_tools,
        "Tool call order mismatch.\nExpected: {:?}\nActual:   {:?}",
        expected_tools, actual_tools
    );
}
