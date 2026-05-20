#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
cd "$ROOT"

BIN="$ROOT/bridge/rust/target/release/zero-agent-bridge"
MANIFEST="$ROOT/bridge/rust/Cargo.toml"
STAMP="$ROOT/bridge/rust/src"

needs_build() {
  if [[ ! -x "$BIN" ]]; then
    return 0
  fi
  find "$STAMP" -name '*.rs' -newer "$BIN" 2>/dev/null | grep -q .
}

if needs_build; then
  cargo build --release --manifest-path "$MANIFEST" --features tui -q 2>&1
fi

exec "$BIN" "$@"
