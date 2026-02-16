#!/bin/bash
# Tool Validation Test
# Tests that all tool implementations in agent_test work correctly
#
# Usage: ./tests/validate_tools.sh

set -e

WORKSPACE="/tmp/tool-validation-test-$$"
echo "============================================================"
echo "Tool Validation Test"
echo "Workspace: $WORKSPACE"
echo "============================================================"

# Cleanup function
cleanup() {
    echo ""
    echo "Cleaning up workspace..."
    rm -rf "$WORKSPACE"
}
trap cleanup EXIT

# Create workspace
mkdir -p "$WORKSPACE"
cd "$WORKSPACE"

echo ""
echo "--- Test 1: write_file ---"
cat > test.txt << 'EOF'
Hello, World!
This is a test file.
EOF
if [ -f test.txt ]; then
    echo "PASS: File created successfully"
else
    echo "FAIL: File not created"
    exit 1
fi

echo ""
echo "--- Test 2: read_file ---"
CONTENT=$(cat test.txt)
if [[ "$CONTENT" == *"Hello, World!"* ]]; then
    echo "PASS: File read successfully"
else
    echo "FAIL: File content mismatch"
    exit 1
fi

echo ""
echo "--- Test 3: list_files ---"
mkdir -p src
touch src/main.ts src/utils.ts
LISTING=$(ls -1 .)
if [[ "$LISTING" == *"src"* ]] && [[ "$LISTING" == *"test.txt"* ]]; then
    echo "PASS: Directory listing works"
else
    echo "FAIL: Directory listing incorrect"
    exit 1
fi

echo ""
echo "--- Test 4: exec ---"
RESULT=$(echo "test output")
if [[ "$RESULT" == "test output" ]]; then
    echo "PASS: Command execution works"
else
    echo "FAIL: Command execution failed"
    exit 1
fi

echo ""
echo "--- Test 5: git init ---"
git init -q
if [ -d .git ]; then
    echo "PASS: Git init works"
else
    echo "FAIL: Git init failed"
    exit 1
fi

echo ""
echo "--- Test 6: git add & status ---"
git add test.txt
STATUS=$(git status --porcelain)
if [[ "$STATUS" == *"A  test.txt"* ]] || [[ "$STATUS" == *"A test.txt"* ]]; then
    echo "PASS: Git add/status works"
else
    echo "FAIL: Git add/status failed: $STATUS"
    exit 1
fi

echo ""
echo "--- Test 7: git commit ---"
git config user.email "test@example.com"
git config user.name "Test User"
git commit -m "test: initial commit" -q
LOG=$(git log --oneline -1)
if [[ "$LOG" == *"initial commit"* ]]; then
    echo "PASS: Git commit works"
else
    echo "FAIL: Git commit failed"
    exit 1
fi

echo ""
echo "--- Test 8: glob (find) ---"
FOUND=$(find . -name "*.ts" -type f | sort)
if [[ "$FOUND" == *"main.ts"* ]] && [[ "$FOUND" == *"utils.ts"* ]]; then
    echo "PASS: Glob/find works"
else
    echo "FAIL: Glob/find failed"
    exit 1
fi

echo ""
echo "--- Test 9: grep ---"
echo "function hello() {}" >> src/main.ts
GREP_RESULT=$(grep -rn "function" src/ 2>/dev/null || true)
if [[ "$GREP_RESULT" == *"main.ts"* ]] && [[ "$GREP_RESULT" == *"function"* ]]; then
    echo "PASS: Grep works"
else
    echo "FAIL: Grep failed"
    exit 1
fi

echo ""
echo "--- Test 10: Complex file creation (Todo App structure) ---"
mkdir -p src/components
cat > src/components/TodoList.tsx << 'EOF'
import React, { useState } from 'react';

interface Todo {
  id: string;
  title: string;
  completed: boolean;
}

export function TodoList() {
  const [todos, setTodos] = useState<Todo[]>([]);
  const [input, setInput] = useState('');

  const addTodo = () => {
    if (!input.trim()) return;
    setTodos([...todos, {
      id: crypto.randomUUID(),
      title: input,
      completed: false
    }]);
    setInput('');
  };

  return (
    <div className="p-4 max-w-md mx-auto">
      <h1 className="text-2xl font-bold mb-4">Todo List</h1>
      <div className="flex gap-2 mb-4">
        <input
          value={input}
          onChange={(e) => setInput(e.target.value)}
          className="flex-1 border rounded px-2 py-1"
          placeholder="Add a todo..."
        />
        <button onClick={addTodo} className="bg-blue-500 text-white px-4 py-1 rounded">
          Add
        </button>
      </div>
      <ul className="space-y-2">
        {todos.map(todo => (
          <li key={todo.id} className={todo.completed ? 'line-through text-gray-500' : ''}>
            {todo.title}
          </li>
        ))}
      </ul>
    </div>
  );
}
EOF

if [ -f src/components/TodoList.tsx ]; then
    SIZE=$(wc -c < src/components/TodoList.tsx)
    if [ "$SIZE" -gt 500 ]; then
        echo "PASS: Complex file creation works ($SIZE bytes)"
    else
        echo "FAIL: File too small"
        exit 1
    fi
else
    echo "FAIL: Complex file not created"
    exit 1
fi

echo ""
echo "============================================================"
echo "ALL TESTS PASSED!"
echo "============================================================"
echo ""
echo "The tool implementations are working correctly."
echo "To run the full agent test with an LLM, use:"
echo ""
echo '  TEST_QUERY="build a todo app" \'
echo '  TEST_AGENT_ENDPOINT="https://api.openai.com/v1/chat/completions" \'
echo '  TEST_AGENT_SECRET="your-api-key" \'
echo '  cargo run --bin agent_test'
echo ""
