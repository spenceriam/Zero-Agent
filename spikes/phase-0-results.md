# Phase 0 Results

## Environment

- Zero CLI: 0.1.1
- Rust: 1.95.0
- Cargo: 1.95.0
- Host: Windows, Zero target `win32-x64.exe`

## Confirmed

- `zero check C:/Users/spenc/GitHub/zero-agent` returns `ok` after fixing `src/main.0` to current Zero syntax.
- `zero run C:/Users/spenc/GitHub/zero-agent` prints `zero-agent`.
- `zero graph --json` reports hosted target capabilities: `args`, `env`, `fs`, `memory`, `time`, `rand`, `net`, `proc`.
- `zero mem --json` works and shows no hidden heap allocation for the placeholder CLI.
- Zero direct backend for `win32-x64.exe` is available; target compiler is not required for the placeholder.

## Boundary decision

Zero should own durable core models and policy logic. Rust bridge remains required until proven otherwise for:

- HTTPS provider requests
- SSE streaming
- Telegram polling/webhooks
- process streaming and cancellation
- PTY/raw terminal split-pane TUI
- possibly SQLite

## Spike status

| Spike | Result | Notes |
| --- | --- | --- |
| zero-json-config | Zero-native candidate | `std.json` exists; validate/parse APIs are documented. Needs actual config model implementation. |
| session-jsonl-store | Zero-native candidate | `std.fs` exists; JSONL append/read API needs implementation proof. |
| tool-call-roundtrip | Zero-native candidate | JSON validation/schema storage likely Zero-native; provider transport still bridge-backed. |
| provider-http-call | Rust bridge required for v0.1 | Zero docs expose HTTP metadata but not complete HTTPS request/response runtime. |
| provider-sse-streaming | Rust bridge required for v0.1 | Needs streaming HTTPS/SSE. |
| process-streaming | Rust bridge required for v0.1 | Zero `proc` capability exists, but streaming stdout/stderr/cancel API is not proven. |
| split-pane-terminal | Rust bridge required for v0.1 | No documented raw terminal/TUI API. |
| telegram-polling | Rust bridge required for v0.1 | Requires HTTPS polling and maybe long polling/webhooks. |
| model-discovery | Rust bridge transport + Zero model logic | Discovery model/config can be Zero-owned, HTTP transport via Rust. |
| zero-rust-bridge | Required | Bridge should be narrow JSON-over-stdio first. |

## Commands run

```sh
export PATH="/c/Users/spenc/.zero/bin:/c/Users/spenc/.cargo/bin:$PATH"
zero --version
rustc --version
cargo --version
zero check "/c/Users/spenc/GitHub/zero-agent"
zero run "/c/Users/spenc/GitHub/zero-agent"
zero graph --json "/c/Users/spenc/GitHub/zero-agent"
zero size --json "/c/Users/spenc/GitHub/zero-agent"
zero mem --json "/c/Users/spenc/GitHub/zero-agent"
```
