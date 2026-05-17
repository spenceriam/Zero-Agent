# BMAD Story 1.3: Session Store

## Status

Implemented as a runtime slice.

## Zero model

`src/core/session.0` defines:

- `MessageRole` enum: User, Assistant, Tool, System
- `ChatMessage` shape with role, content, tool_name
- `Session` shape with id, title, provider, model
- Message constructor helpers: userMessage, assistantMessage, toolMessage, systemMessage
- `sessionPath` helper for JSONL file path resolution

## Bridge operations

`bridge/rust/src/main.rs` supports:

- `session.append` - append a JSONL line to a session file (creates parent dirs)
- `session.read` - read all lines from a session JSONL file
- `session.list` - list session files (.jsonl) in a directory

## Persistence format

Each session is a `.jsonl` file. Each line is a JSON string containing a serialized ChatMessage. Messages survive process restart.

## Acceptance evidence

Commands:

```sh
export PATH="/c/Users/spenc/.zero/bin:$PATH"
zero build src/main.0
zero run src/main.0
```

```sh
export PATH="/c/msys64/mingw64/bin:/c/Users/spenc/.cargo/bin:$PATH"
CARGO_TARGET_DIR="/tmp/zero-agent-config-target" cargo test --target x86_64-pc-windows-gnu --manifest-path bridge/rust/Cargo.toml
```

Bridge smoke test:

```json
{"id":"1","op":"session.append","path":"<data_dir>/sessions/test.jsonl","contents":"{\"role\":\"user\",\"content\":\"hello\"}"}
{"id":"2","op":"session.read","path":"<data_dir>/sessions/test.jsonl"}
{"id":"3","op":"session.list","dir":"<data_dir>/sessions"}
```

## Remaining work

- Wire session store into agent loop for automatic message persistence.
- Add session metadata file (title, created_at, provider, model).
- Add session resume logic (load messages, rebuild context window).
