# BMAD Story 3.3: Shell Tool

## Status

Implemented with bridge operation.

## Bridge operation

`bridge/rust/src/main.rs` supports:

- `shell.run` - run a shell command and capture output

## Implementation

- Uses `cmd /C` on Windows, `sh -c` on Unix
- Captures stdout, stderr, and exit code
- Returns all output in structured JSON response

## Tool model

Shell tool is registered in `src/tools/registry.0` as `shellTool()` with:

- Risk level: Mutating (requires approval)
- Input schema: `{ "command": string }`

## Approval flow

The shell tool has `RiskLevel.Mutating` so `needsApproval()` returns true. The agent loop should:

1. Check if tool needs approval via `policy.decisionForRisk()`
2. If Ask, present command to user for approval
3. If approved, execute via `shell.run` bridge operation
4. Return result as ToolResult event

## Acceptance evidence

Commands:

```sh
export PATH="/c/msys64/mingw64/bin:/c/Users/spenc/.cargo/bin:$PATH"
CARGO_TARGET_DIR="/tmp/zero-agent-config-target" cargo test --target x86_64-pc-windows-gnu --manifest-path bridge/rust/Cargo.toml
```

Bridge smoke test:

```json
{"id":"1","op":"shell.run","command":"echo Hello from Zero-Agent"}
{"id":"2","op":"shell.run","command":"ls -la"}
```

## Remaining work

- Wire shell tool into agent loop with approval flow.
- Add command timeout support.
- Add cancel support (kill process).
- Add working directory option.
