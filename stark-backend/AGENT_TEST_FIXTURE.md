# Agent Test Fixture

A standalone test harness for testing the agentic tool loop with **real** tool implementations. This binary runs a CodeEngineer agent that can actually build software by executing file operations, shell commands, and git operations.

## Quick Start

```bash
# Set up environment variables (or use .env file)
export TEST_AGENT_ENDPOINT="https://api.openai.com/v1/chat/completions"
export TEST_AGENT_SECRET="sk-..."

# Run the test
cargo run --bin agent_test
```

The binary automatically loads environment variables from `.env` in the project root.

## Environment Variables

### Required

| Variable | Description | Example |
|----------|-------------|---------|
| `TEST_AGENT_ENDPOINT` | OpenAI-compatible API endpoint | `https://api.openai.com/v1/chat/completions` |
| `TEST_AGENT_SECRET` | API key for the endpoint | `sk-...` |

### Optional

| Variable | Description | Default |
|----------|-------------|---------|
| `TEST_QUERY` | The task to give the agent | `"Build a simple todo app with TypeScript..."` |
| `TEST_AGENT_MODEL` | Model name to use | Auto-detected from endpoint |
| `TEST_WORKSPACE` | Directory for agent file operations | `/tmp/agent-test-workspace` |
| `TEST_SKILLS_DIR` | Path to skills directory | `./skills` |
| `TEST_MAX_ITERATIONS` | Max tool loop iterations | `25` |

## Model Auto-Detection

If `TEST_AGENT_MODEL` is not set, the model is auto-detected from the endpoint:

| Endpoint contains | Default Model |
|-------------------|---------------|
| `moonshot` | `moonshot-v1-128k` |
| `anthropic` | `claude-sonnet-4-20250514` |
| (other) | `gpt-4o` |

## Tools Available

The fixture provides **real** CodeEngineer tools that execute actual operations:

| Tool | Description | Parameters |
|------|-------------|------------|
| `read_file` | Read file contents | `path` |
| `write_file` | Create/overwrite files | `path`, `content` |
| `list_files` | List directory contents | `path` (optional) |
| `exec` | Execute shell commands | `command`, `timeout` (optional) |
| `git` | Git operations | `operation`, `files`, `message`, `branch`, `create` |
| `glob` | Find files by pattern | `pattern` |
| `grep` | Search in files | `pattern`, `path` (optional) |

### Git Operations

The `git` tool supports these operations:
- `status` - Show working tree status
- `diff` - Show changes
- `log` - Show recent commits
- `init` - Initialize repository
- `add` - Stage files (requires `files` array)
- `commit` - Create commit (requires `message`)
- `branch` - List or create branches
- `checkout` - Switch branches (with optional `create: true`)

## Example Usage

### Basic test (uses default query)

```bash
TEST_AGENT_ENDPOINT="https://api.openai.com/v1/chat/completions" \
TEST_AGENT_SECRET="sk-..." \
cargo run --bin agent_test
```

### Custom query

```bash
TEST_QUERY="Create a Python FastAPI server with a /health endpoint" \
TEST_AGENT_ENDPOINT="https://api.openai.com/v1/chat/completions" \
TEST_AGENT_SECRET="sk-..." \
TEST_WORKSPACE="/tmp/fastapi-test" \
cargo run --bin agent_test
```

### Using Moonshot/Kimi

```bash
TEST_QUERY="Build a React component that displays a counter" \
TEST_AGENT_ENDPOINT="https://api.moonshot.ai/v1/chat/completions" \
TEST_AGENT_SECRET="sk-..." \
TEST_AGENT_MODEL="moonshot-v1-128k" \
cargo run --bin agent_test
```

### Using .env file

Create a `.env` file in the project root:

```env
TEST_AGENT_ENDPOINT=https://api.openai.com/v1/chat/completions
TEST_AGENT_SECRET=sk-your-key-here
TEST_AGENT_MODEL=gpt-4o
TEST_WORKSPACE=/tmp/agent-workspace
TEST_MAX_ITERATIONS=30
```

Then just run:
```bash
cargo run --bin agent_test
```

## Output

The fixture prints detailed debug output for each iteration:

```
📤 ITERATION 1 / 25
============================================================

📋 Sending request to https://api.openai.com/v1/chat/completions (model: gpt-4o)

📊 Response:
   finish_reason: Some("tool_calls")
   content: I'll create the todo app...
   tool_calls: Some(2)

🔧 Processing 2 tool call(s):

   📍 Tool: write_file (id: call_abc123)
   🔧 Executing: write_file
   📥 Args: {"path":"package.json","content":"..."}
   📤 Result: Successfully wrote 234 bytes to package.json
```

### Final Output

On success, shows the agent's final response and workspace contents:

```
🎉 SUCCESS
============================================================
I've created a TypeScript todo CLI app with add, list, and remove commands...

📁 Workspace contents:
   📁 src/
     📄 index.ts
     📄 todo.ts
   📄 package.json
   📄 tsconfig.json
```

## Troubleshooting

### API Errors

- **401 Unauthorized**: Check `TEST_AGENT_SECRET` is valid
- **404 Not Found**: Check `TEST_AGENT_ENDPOINT` URL is correct
- **429 Rate Limited**: Wait and retry, or use a different API key

### Tool Execution Issues

- **Workspace permission errors**: Ensure `TEST_WORKSPACE` path is writable
- **Command not found**: The `exec` tool runs commands in bash; ensure required tools (npm, cargo, etc.) are installed

### Max Iterations Reached

If the agent doesn't complete within `TEST_MAX_ITERATIONS`:
- Increase the limit: `TEST_MAX_ITERATIONS=50`
- Simplify the query
- Check if the model is getting stuck in a loop (review the logs)

## Architecture

```
┌─────────────────────────────────────────────────┐
│                  agent_test.rs                   │
├─────────────────────────────────────────────────┤
│  1. Load .env and environment variables          │
│  2. Create/clean workspace directory             │
│  3. Build system prompt with tool descriptions   │
│  4. Enter agent loop:                            │
│     a. Send messages to LLM API                  │
│     b. If tool_calls in response:                │
│        - Execute each tool (REAL execution)      │
│        - Append results to messages              │
│        - Continue loop                           │
│     c. If no tool_calls:                         │
│        - Return final response                   │
│  5. Print workspace contents                     │
└─────────────────────────────────────────────────┘
```

## Adding New Tools

To add a new tool:

1. Add the `ToolSpec` in `get_code_engineer_tools()`
2. Add the execution handler in `execute_tool()` match
3. Implement the execution function `execute_<tool_name>()`

Example:
```rust
// In get_code_engineer_tools()
ToolSpec {
    tool_type: "function".to_string(),
    function: ToolFunction {
        name: "my_tool".to_string(),
        description: "Does something useful".to_string(),
        parameters: json!({
            "type": "object",
            "properties": {
                "param1": {"type": "string", "description": "..."}
            },
            "required": ["param1"]
        }),
    },
},

// In execute_tool()
"my_tool" => execute_my_tool(args, workspace),

// New function
fn execute_my_tool(args: &Value, workspace: &Path) -> String {
    let param1 = args.get("param1").and_then(|v| v.as_str()).unwrap_or("");
    // Implementation...
    format!("Result: {}", param1)
}
```
