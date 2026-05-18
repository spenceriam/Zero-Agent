#!/bin/bash
set -e

# Zero-Agent Test Runner
# Usage: ./scripts/run_tests.sh

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
BRIDGE_DIR="$PROJECT_DIR/bridge/rust"

echo "  Zero-Agent Test Suite"
echo "================================"
echo ""

# Set CI-like environment
export TZ=UTC
export LANG=C.UTF-8

# Run Rust tests
echo "  Running Rust tests..."
cd "$BRIDGE_DIR"
cargo test 2>&1
echo ""

# Run Rust tests with TUI feature
echo "  Running Rust tests (TUI feature)..."
cargo test --features tui 2>&1
echo ""

# Check for compilation warnings
echo "  Checking for warnings..."
cargo build 2>&1 | grep -q "warning:" && echo "  Warning: Build has warnings" || echo "  No warnings"
echo ""

# Run Zero check if available
if command -v zero &> /dev/null; then
    echo "  Checking Zero source..."
    cd "$PROJECT_DIR"
    zero check src/main.0 2>&1
    echo ""
else
    echo "  Skipping Zero check (zero not installed)"
    echo ""
fi

echo "================================"
echo "  All tests passed!"
