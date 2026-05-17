# BMAD Story 3.4: Memory Tool

## Status

Implemented with bridge operations.

## Zero model

`src/core/memory.0` defines:

- `MemoryKind` enum: User, Session, Fact
- `MemoryItem` shape: id, kind, text, session_id
- Constructor helpers: userMemory, sessionMemory, factMemory
- `memoryPath(data_dir)` - resolves memory file path

## Bridge operations

`bridge/rust/src/main.rs` supports:

- `memory.save` - append a memory item to JSONL file
- `memory.list` - read all memory items from JSONL file

## Persistence

Memory is stored as JSONL (one item per line) in `<data_dir>/memory.jsonl`. Items survive restart.

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
{"id":"1","op":"memory.save","path":"<data_dir>/memory.jsonl","contents":"{\"id\":\"m1\",\"kind\":\"user\",\"text\":\"prefers dark mode\"}"}
{"id":"2","op":"memory.list","path":"<data_dir>/memory.jsonl"}
```

## Remaining work

- Add memory recall (search by kind or text).
- Wire memory into agent loop for context injection.
- Add memory cleanup/pruning.
