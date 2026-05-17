#!/usr/bin/env sh
set -eu

INSTALL_DIR="${HOME}/.zero-agent/bin"
BRIDGE_DIR="$(cd "$(dirname "$0")/../bridge/rust" && pwd)"
ZERO_DIR="$(cd "$(dirname "$0")/.." && pwd)"

echo "Zero-Agent installer"
echo ""
echo "This script builds and installs Zero-Agent from source."
echo ""

# Check for required tools
command -v cargo >/dev/null 2>&1 || { echo "Error: cargo not found. Install Rust first."; exit 1; }
command -v zero >/dev/null 2>&1 || { echo "Warning: zero CLI not found. Zero source won't be compiled."; }

echo "Building Rust bridge..."
cd "$BRIDGE_DIR"
cargo build --release 2>&1 || { echo "Error: Bridge build failed."; exit 1; }

echo "Installing to ${INSTALL_DIR}..."
mkdir -p "$INSTALL_DIR"
cp "$BRIDGE_DIR/target/release/zero-agent-bridge" "$INSTALL_DIR/zero-agent-bridge" 2>/dev/null || \
cp "$BRIDGE_DIR/target/release/zero-agent-bridge.exe" "$INSTALL_DIR/zero-agent-bridge.exe" 2>/dev/null || \
{ echo "Error: Bridge binary not found."; exit 1; }

echo ""
echo "Zero-Agent bridge installed to ${INSTALL_DIR}"
echo ""
echo "Add to your PATH:"
echo "  export PATH=\"${INSTALL_DIR}:\$PATH\""
echo ""
echo "Build from source:"
echo "  cd ${ZERO_DIR}"
echo "  zero build src/main.0"
