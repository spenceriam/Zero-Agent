# Phase 2 Provider Skeleton

## Implemented baseline

Provider metadata exists in Zero:

- `src/providers/registry.0`

Rust bridge operation dispatcher supports:

- `ping`
- `models.discover`
- explicit `not_implemented` responses for:
  - `http.request`
  - `http.stream_sse`
  - `process.spawn`
  - `telegram.poll`
  - `telegram.send`

## Current bridge protocol

Request:

```json
{"id":"1","op":"models.discover","provider":"openrouter"}
```

Response:

```json
{"id":"1","ok":true,"event":"models","output":{"provider":"openrouter","transport":"stub","models":[]}}
```

## Dependency decision

The bridge is currently dependency-free. Serde/serde_json were intentionally not used because Rust build scripts tried to link through the wrong MSVC `link.exe` from the bash environment, even for GNU target builds. The stub dispatcher now includes a minimal JSON string parser that rejects malformed escapes and decodes `\uXXXX` escapes, covered by unit tests. Real HTTP/provider work should revisit dependency/toolchain setup before expanding protocol complexity.

## Verified commands

```sh
export PATH="/c/msys64/mingw64/bin:/c/Users/spenc/.cargo/bin:$PATH"
cd C:/Users/spenc/GitHub/zero-agent/bridge/rust
cargo check --target x86_64-pc-windows-gnu
printf '{"id":"1","op":"ping"}\n{"id":"2","op":"models.discover","provider":"openrouter"}\n' | cargo run --quiet --target x86_64-pc-windows-gnu
```

## Next provider work

- Add real HTTP transport.
- Decide Rust dependency strategy or MSVC build tools.
- Implement OpenRouter/OpenAI-compatible model discovery.
- Implement Anthropic and OpenAI native streaming/tool-call protocol adapters.
- Map provider streaming chunks into normalized Zero-Agent events.
