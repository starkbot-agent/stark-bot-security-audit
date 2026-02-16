#!/usr/bin/env bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "=== Building frontend ==="
cd "$SCRIPT_DIR/stark-frontend"
npm run build

echo ""
echo "=== Running backend ==="
cd "$SCRIPT_DIR/stark-backend"
cargo r
