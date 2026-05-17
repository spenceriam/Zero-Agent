# BMAD Story 1.1: Config Model

## Status

Implemented as a first runtime slice.

## Zero model

`src/core/config.0` defines:

- `ProviderConfig`
- `ModelConfig`
- `ToolPolicyConfig`
- `AppConfig`
- default OpenRouter provider config
- default model config
- default tool policy config
- basic config validation predicate

## Bridge operations

`bridge/rust/src/main.rs` supports:

- `config.default`
- `config.validate`
- `config.write`
- `config.read`

## Validation rules

Current bridge validation checks:

- `data_dir` exists and is not empty
- `default_provider` exists and is not empty

## Acceptance evidence

Commands:

```sh
export PATH="/c/Users/spenc/.zero/bin:$PATH"
zero check "/c/Users/spenc/GitHub/zero-agent/zero.json"
```

```sh
export PATH="/c/msys64/mingw64/bin:/c/Users/spenc/.cargo/bin:$PATH"
CARGO_TARGET_DIR="/tmp/zero-agent-config-target" cargo test --target x86_64-pc-windows-gnu --manifest-path "/c/Users/spenc/GitHub/zero-agent/bridge/rust/Cargo.toml"
```

Expected bridge smoke examples:

```json
{"id":"1","op":"config.default"}
{"id":"2","op":"config.validate","data_dir":".zero-agent","default_provider":"openrouter"}
{"id":"3","op":"config.write","path":"C:/Users/spenc/AppData/Local/Temp/zero-agent-config.json","contents":"plain-config"}
{"id":"4","op":"config.read","path":"C:/Users/spenc/AppData/Local/Temp/zero-agent-config.json"}
```

## Remaining work

- Replace flat request fields with full nested JSON when bridge dependency/toolchain strategy supports a real JSON parser.
- Add Zero-side bridge invocation once process streaming/spawn is implemented.
- Add config path resolution for user/project config precedence.
