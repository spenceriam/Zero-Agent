# BMAD Story 3.2: Filesystem Tools

## Status

Implemented with bridge operations.

## Bridge operations

`bridge/rust/src/main.rs` supports:

- `fs.read` - read a file (returns path and contents)
- `fs.write` - write a file (creates parent directories)
- `fs.edit` - search and replace in a file (rejects ambiguous matches)
- `fs.glob` - find files matching a glob pattern

## Tool implementations

- fs.read: reads file via `std::fs::read_to_string`
- fs.write: writes file via `std::fs::write` with `create_dir_all` for parent dirs
- fs.edit: reads file, validates match count (exactly 1), writes replacement
- fs.glob: uses `glob` crate to match patterns, returns file list

## Dependencies

- `glob = "0.3"` (pure Rust, no C dependencies)

## Acceptance evidence

Commands:

```sh
export PATH="/c/msys64/mingw64/bin:/c/Users/spenc/.cargo/bin:$PATH"
CARGO_TARGET_DIR="/tmp/zero-agent-config-target" cargo test --target x86_64-pc-windows-gnu --manifest-path bridge/rust/Cargo.toml
```

Bridge smoke test:

```json
{"id":"1","op":"fs.write","path":"<path>/test.txt","contents":"Hello, World!"}
{"id":"2","op":"fs.read","path":"<path>/test.txt"}
{"id":"3","op":"fs.edit","path":"<path>/test.txt","old_string":"World","new_string":"Zero-Agent"}
{"id":"4","op":"fs.glob","pattern":"*.txt","root":"<path>"}
```

## Remaining work

- Add fs.grep (regex search) - requires regex crate or manual implementation.
- Wire tools into agent loop for dynamic dispatch.
- Add tool result formatting for UI display.
