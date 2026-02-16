# Parent-Child Agentic Thread Architecture

## Problem Statement

Currently, the dispatcher has a single tool loop where:
1. LLM generates tool calls
2. Tools execute sequentially in the same loop
3. Results feed back to LLM
4. LLM decides to continue or stop

**Issue**: When `web3_tx` fails or returns an error, the LLM retries it multiple times because it's all in one loop. The LLM doesn't understand that `web3_tx` already broadcasts AND polls for confirmation internally.

## Solution Overview

Introduce a **parent-child thread architecture** where:
- **Primary agentic thread**: Main conversation loop that coordinates
- **Child agentic threads**: Spawned for each tool execution, run asynchronously
- Tools complete independently and send events back to the primary thread
- Primary thread resumes only after receiving tool completion events

```
┌─────────────────────────────────────────────────────────────┐
│                    PRIMARY AGENTIC THREAD                    │
│  ┌─────────────────────────────────────────────────────────┐│
│  │  1. Receive user message                                ││
│  │  2. Generate AI response with tool calls                ││
│  │  3. For each tool call:                                 ││
│  │     → Spawn CHILD THREAD                                ││
│  │     → Wait for completion event                         ││
│  │  4. Collect all tool results                            ││
│  │  5. Feed back to AI, repeat if more tools needed        ││
│  └─────────────────────────────────────────────────────────┘│
│                          │                                   │
│            ┌─────────────┼─────────────┐                     │
│            ▼             ▼             ▼                     │
│  ┌──────────────┐ ┌──────────────┐ ┌──────────────┐         │
│  │ CHILD THREAD │ │ CHILD THREAD │ │ CHILD THREAD │         │
│  │  Tool: exec  │ │ Tool: web3_tx│ │ Tool: fetch  │         │
│  │              │ │              │ │              │         │
│  │ Execute tool │ │ Execute tool │ │ Execute tool │         │
│  │ Send result  │ │ Send result  │ │ Send result  │         │
│  │ via channel  │ │ via channel  │ │ via channel  │         │
│  └──────────────┘ └──────────────┘ └──────────────┘         │
└─────────────────────────────────────────────────────────────┘
```

## Key Design Decisions

### 1. Tool Execution Isolation
Each tool runs in its own spawned task. The primary thread:
- Spawns child tasks for ALL tool calls from a single AI response
- Waits for ALL children to complete (or timeout)
- Collects results and feeds back to AI

### 2. No LLM-Level Retry
The child thread executes the tool ONCE. If it fails:
- Return error result to primary thread
- Primary thread feeds error to AI
- AI can decide to try a DIFFERENT approach, but NOT retry the same tool with same params

### 3. Event-Based Completion
Child threads communicate completion via `tokio::sync::mpsc` channel:
```rust
struct ToolCompletionEvent {
    tool_call_id: String,
    result: ToolResult,
    duration_ms: u64,
}
```

### 4. Parallel Tool Execution
When AI returns multiple tool calls in one response:
- Spawn all children in parallel
- Wait for all to complete (with overall timeout)
- Collect all results before next AI iteration

## Implementation Plan

### Phase 1: Create ToolExecutor Service

**File**: `stark-backend/src/execution/tool_executor.rs`

```rust
pub struct ToolExecutor {
    tool_registry: Arc<ToolRegistry>,
    execution_tracker: Arc<ExecutionTracker>,
    broadcaster: Arc<EventBroadcaster>,
}

impl ToolExecutor {
    /// Execute tool calls in parallel child threads
    /// Returns only after ALL tools complete (or timeout)
    pub async fn execute_tools(
        &self,
        tool_calls: Vec<ToolCall>,
        context: &ToolContext,
        timeout: Duration,
    ) -> Vec<ToolResponse> {
        let (tx, mut rx) = mpsc::channel::<ToolCompletionEvent>(tool_calls.len());

        // Spawn child thread for each tool
        for call in &tool_calls {
            let tx = tx.clone();
            let call = call.clone();
            let registry = self.tool_registry.clone();
            let context = context.clone();

            tokio::spawn(async move {
                let start = Instant::now();
                let result = registry.execute(&call.name, call.arguments, &context).await;
                let _ = tx.send(ToolCompletionEvent {
                    tool_call_id: call.id,
                    result,
                    duration_ms: start.elapsed().as_millis() as u64,
                }).await;
            });
        }

        drop(tx); // Close sender so rx knows when all are done

        // Collect all results with timeout
        let mut results = Vec::new();
        let deadline = Instant::now() + timeout;

        while let Ok(event) = tokio::time::timeout_at(
            deadline.into(),
            rx.recv()
        ).await {
            if let Some(event) = event {
                results.push(event);
            } else {
                break; // Channel closed, all done
            }
        }

        // Convert to ToolResponse format
        self.build_responses(tool_calls, results)
    }
}
```

### Phase 2: Refactor Dispatcher Tool Loop

**File**: `stark-backend/src/channels/dispatcher.rs`

Modify `generate_with_native_tools` and `generate_with_text_tools`:

```rust
// Before (current):
let tool_responses = self.execute_tool_calls(&tool_calls, ...).await;

// After (new):
let tool_responses = self.tool_executor.execute_tools(
    tool_calls,
    &tool_context,
    Duration::from_secs(300), // 5 min timeout per iteration
).await;
```

### Phase 3: Add Tool Execution Events

**File**: `stark-backend/src/gateway/protocol.rs`

Add new event types for child thread lifecycle:
```rust
// Existing events are fine, but add:
ToolExecutionSpawned,   // Child thread started
ToolExecutionProgress,  // Optional: for long-running tools
```

### Phase 4: Update Frontend

**File**: `stark-frontend/src/components/chat/ExecutionProgress.tsx`

Show child threads in the execution tree:
- Each tool shows as a child task (already works)
- Add visual indicator that tools run in parallel
- Show individual tool timeouts

### Phase 5: Prevent AI-Level Retries

Even with child threads, the AI might retry in the *next* loop iteration when it sees an error. We need to update tool descriptions to make it clear:

**File**: `stark-backend/src/tools/builtin/web3_tx.rs`

Update the tool description:
```rust
description: "Sign and broadcast an EVM transaction, then WAIT for it to be \
mined (up to 2 minutes). Returns final status: 'confirmed' or 'reverted'. \
IMPORTANT: This tool handles the ENTIRE transaction lifecycle - broadcast, \
polling, and confirmation. Call ONCE per transaction. If it fails or reverts, \
report the error to the user - do NOT retry with the same parameters."
```

This two-pronged approach ensures:
1. **Child threads**: Each tool call executes exactly once (no concurrent duplicates)
2. **Tool description**: AI understands not to retry on failure

## Files to Modify

| File | Changes |
|------|---------|
| `stark-backend/src/execution/mod.rs` | Export new `ToolExecutor` |
| `stark-backend/src/execution/tool_executor.rs` | **NEW FILE** - Child thread orchestration |
| `stark-backend/src/channels/dispatcher.rs` | Use `ToolExecutor` instead of inline execution |
| `stark-backend/src/gateway/protocol.rs` | Add spawn/progress events (optional) |
| `stark-backend/src/tools/types.rs` | Add `ToolCompletionEvent` struct |
| `stark-backend/src/tools/builtin/web3_tx.rs` | Update tool description to prevent AI retries |

## Key Benefits

1. **No accidental retries**: Each tool executes exactly once per tool call
2. **Parallel execution**: Multiple tools run concurrently
3. **Proper timeout handling**: Overall timeout for tool batch, not per-tool
4. **Clean separation**: Tool execution is decoupled from AI loop
5. **Better observability**: Clear child thread lifecycle events

## Edge Cases

1. **Tool timeout**: If a child doesn't complete in time, return timeout error
2. **Partial completion**: If some tools complete and others timeout, return mixed results
3. **Channel closed**: Handle gracefully if primary thread dies
4. **Long-running tools**: `web3_tx` can take 2+ minutes - set appropriate timeout

## Verification Plan

1. **Unit tests**: Test `ToolExecutor` with mock tools
2. **Integration test**: Run swap command, verify single `web3_tx` call
3. **Timeout test**: Verify timeout handling works correctly
4. **Parallel test**: Run multiple tools, verify they execute concurrently
5. **Frontend**: Verify execution progress shows correct hierarchy

## Migration Notes

- Backward compatible: No API changes
- Gradual rollout: Can enable per-tool-type initially
- Rollback: Keep old `execute_tool_calls` method as fallback
