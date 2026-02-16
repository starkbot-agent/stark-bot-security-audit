# Mock AI Dispatcher Integration Tests

## Location

- **MockAiClient**: `stark-backend/src/ai/mod.rs`
- **Dispatcher wiring**: `stark-backend/src/channels/dispatcher.rs` (field, builder, `#[cfg(test)]` override)
- **Test file**: `stark-backend/src/channels/dispatcher_tests.rs`

## How it works

### MockAiClient

`MockAiClient` holds a `Arc<Mutex<VecDeque<Result<AiResponse, AiError>>>>`. Each call to `generate_with_tools()` pops the next response. When the queue is empty it returns `AiResponse::text("(mock exhausted)")`.

It's an enum variant on `AiClient`:
```rust
pub enum AiClient {
    Claude(ClaudeClient),
    OpenAI(OpenAIClient),
    Llama(LlamaClient),
    Mock(MockAiClient),  // test-only usage
}
```

All 5 match arms (`generate_text`, `generate_text_with_events`, `generate_with_tools`, `supports_tools`, `with_broadcaster`) handle the `Mock` variant.

### Dispatcher override

`MessageDispatcher` has a `#[cfg(test)]` field:
```rust
mock_ai_client: Option<crate::ai::MockAiClient>
```

Set via `.with_mock_ai_client(mock)` builder. In `dispatch()`, a `#[cfg(test)]` block checks this field and constructs `AiClient::Mock(mock.clone())` instead of calling `AiClient::from_settings_with_wallet_provider()`. The `#[cfg(not(test))]` block has the original logic — zero runtime cost in production.

### TestHarness

Sets up:
- **In-memory SQLite** (`Database::new(":memory:")`) — full schema auto-created
- **Agent settings row** — dummy endpoint, "kimi" archetype (needed for `dispatch()` to proceed past settings lookup)
- **Channel row** — configurable `channel_type` and `safe_mode`
- **EventBroadcaster** with a subscribed receiver to capture all events
- **Full ToolRegistry** via `tools::create_default_registry()` — real `say_to_user`, `task_fully_completed`, etc.
- **MessageDispatcher** with the MockAiClient injected

`harness.dispatch(text, force_safe_mode)` sends a `NormalizedMessage` through the real dispatcher and drains all events from the mpsc receiver.

### Counting user-visible messages

`count_user_messages(events, response)` counts:
- `tool.result` events where `tool_name == "say_to_user"` and `success == true` and content is non-empty
- `agent.response` events where text is non-empty

This is how we detect silence (0) vs duplication (2).

## Test patterns

| Pattern | Mock responses | What it tests |
|---------|---------------|---------------|
| A: say_to_user(finished_task=true) | 1 response with tool call | Loop terminates, final response suppressed, count=1 |
| C: task_fully_completed(summary) | 1 response with tool call | Loop terminates, summary becomes response, count=1 |
| D: say_to_user → task_fully_completed | 2 responses | say_to_user delivers message, task_fully_completed follows |

Each pattern tested in 3 modes:
- **web/normal** — `channel_type: "web"`, `safe_mode: false`
- **safe mode** — `channel_type: "web"`, `safe_mode: true`
- **discord gateway** — `channel_type: "discord"`, `force_safe_mode: true`

Total: 9 tests.

## Run command

```bash
cargo test -p stark-backend --bin stark-backend -- channels::dispatcher::dispatcher_tests
```

## Known limitations

1. **Pattern D web/normal has a weak assertion** — `count >= 1 && count <= 2` instead of `== 1`. The current code allows both messages through in non-safe mode. This should be tightened if the dispatcher is fixed to suppress the duplicate.

2. **Only tests the native tool-calling path** — `generate_with_native_tools_orchestrated`. The text-tools path (`generate_with_text_tools_orchestrated`, used by Kimi and other non-native archetypes) has its own duplicate loop logic and is NOT covered.

3. **No multi-tool-call-per-response tests** — e.g., AI returning say_to_user + another tool in the same response.

4. **Mock archetype defaults to Kimi** — which uses native tool calling. To test the text-tools path, would need an archetype that returns `uses_native_tool_calling() == false`.

## Key code paths exercised

```
dispatch()
  → get_active_agent_settings() (hits DB)
  → get_or_create_identity() (hits DB)
  → get_or_create_session() (hits DB)
  → get_channel() → safe_mode check
  → [#[cfg(test)] mock override] → AiClient::Mock
  → generate_with_tool_loop()
    → Orchestrator::new()
    → generate_with_native_tools_orchestrated()
      → loop:
        → client.generate_with_tools() → pops from mock queue
        → tool_registry.execute("say_to_user", ...) → real tool execution
        → broadcaster.broadcast(tool_result event)
        → finished_task/task_fully_completed metadata check → break
      → final response handling
  → broadcaster.broadcast(agent_response event) [if response non-empty]
```
