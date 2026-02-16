# Plan: Real-Time Tool/Skill Usage Display in Agent Chat

## Goal

When asking the agent chat something like "search the web and tell me about the 1932 olympics", display real-time indicators showing which tools are being used, similar to Claude Code's "EXPLORING... using tool web_search" display.

## Current State

### Already Have
- `GatewayEvent::tool_execution(channel_id, tool_name, parameters)` - broadcast BEFORE tool runs
- `GatewayEvent::tool_result(channel_id, tool_name, success, duration_ms)` - defined but NOT used
- `GatewayEvent::skill_invoked(channel_id, skill_name)` - defined but NOT used
- `EventBroadcaster` - broadcasts to all WebSocket clients
- WebSocket Gateway running on port 8081

### Missing
- `tool.result` event not being broadcast after tool completes
- Frontend not connected to WebSocket gateway
- Frontend has no UI for real-time tool indicators

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                         FRONTEND                                │
│                                                                 │
│  ┌──────────────────┐      ┌──────────────────────────────────┐│
│  │   Agent Chat     │      │      WebSocket Connection        ││
│  │   (/api/chat)    │      │      (ws://localhost:8081)       ││
│  └────────┬─────────┘      └──────────────┬───────────────────┘│
│           │ POST                          │ Events             │
│           │                               ▼                    │
│           │                    ┌─────────────────────┐         │
│           │                    │  Event Handlers     │         │
│           │                    │  - tool.execution   │         │
│           │                    │  - tool.result      │         │
│           │                    │  - skill.invoked    │         │
│           │                    └──────────┬──────────┘         │
│           │                               │                    │
│           │                               ▼                    │
│           │                    ┌─────────────────────┐         │
│           │                    │  Status Indicator   │         │
│           │                    │  "Using web_search" │         │
│           │                    └─────────────────────┘         │
└───────────┼───────────────────────────────────────────────────┘
            │
            ▼
┌─────────────────────────────────────────────────────────────────┐
│                         BACKEND                                 │
│                                                                 │
│  ┌──────────────────┐      ┌──────────────────────────────────┐│
│  │  Chat Controller │      │      Gateway WebSocket           ││
│  └────────┬─────────┘      │      Server (8081)               ││
│           │                └──────────────┬───────────────────┘│
│           ▼                               ▲                    │
│  ┌──────────────────┐                     │                    │
│  │ MessageDispatcher│─────────────────────┘                    │
│  │                  │  broadcasts events                       │
│  │  execute_tool()  │  - tool.execution (before)               │
│  │                  │  - tool.result (after)                   │
│  └──────────────────┘                                          │
└─────────────────────────────────────────────────────────────────┘
```

## Implementation Tasks

### Phase 1: Backend - Add Missing Events

**File: `src/channels/dispatcher.rs`**

1. Broadcast `tool.result` event AFTER each tool completes
2. Add `agent.thinking` event for when AI is processing
3. Broadcast thinking status at key points:
   - "Thinking..." when starting AI generation
   - "Using tool: {name}" before tool execution
   - "Processing results..." after tool execution

### Phase 2: Frontend - WebSocket Connection

**File: `stark-frontend/js/agent-chat.js`**

1. Connect to WebSocket gateway on page load
2. Handle reconnection on disconnect
3. Handle gateway events (tool.execution, tool.result, agent.thinking)

### Phase 3: Frontend - Status Indicator UI

**File: `stark-frontend/js/agent-chat.js`**

1. Create floating status indicator element
2. Show/update/hide based on events
3. Display tool icons and names
4. Show success/failure states

## File Changes Summary

| File | Changes |
|------|---------|
| `src/channels/dispatcher.rs` | Add `tool.result` broadcast, add `agent.thinking` events |
| `src/gateway/protocol.rs` | Add `agent_thinking()` event constructor |
| `stark-frontend/js/agent-chat.js` | Add WebSocket connection, event handlers, status UI |

## Testing

1. Start the server with a Brave/SerpAPI key configured
2. Open Agent Chat in browser
3. Open browser DevTools → Network → WS tab to see WebSocket messages
4. Ask: "Search the web and tell me about the 1932 Olympics"
5. Verify:
   - Status indicator appears: "🔍 Using web_search..."
   - Indicator updates: "✓ web_search complete"
   - Final response displays
