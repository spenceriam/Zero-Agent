# BMAD Story 3.5: Extension Manager

## Status

Implemented with bridge operations.

## Zero model

`src/tools/extensions.0` defines:

- `ExtensionManifest` shape: name, kind, description, risk, version, author
- `ExtensionRegistry` shape: extensions (list placeholder)
- `localToolExtension(name, description)` - create tool extension manifest
- `localSkillExtension(name, description)` - create skill extension manifest
- `extensionPath(data_dir, name)` - resolve manifest path
- `extensionsDir(data_dir)` - resolve extensions directory

## Bridge operations

`bridge/rust/src/main.rs` supports:

- `extension.list` - list installed extension directories
- `extension.read` - read extension manifest JSON
- `extension.write` - write extension manifest (creates parent dirs)

## Install flow

1. User requests extension install
2. Agent looks up extension in registry
3. Agent writes manifest via `extension.write`
4. Extension is available for use

## Creator workflow

1. User asks agent to create new extension
2. Agent scaffolds manifest via `localToolExtension()` or `localSkillExtension()`
3. Agent writes manifest via `extension.write`
4. Extension is ready for development

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
{"id":"1","op":"extension.write","path":"<data_dir>/extensions/my-tool/manifest.json","contents":"{\"name\":\"my-tool\",\"kind\":\"tool\"}"}
{"id":"2","op":"extension.read","path":"<data_dir>/extensions/my-tool/manifest.json"}
{"id":"3","op":"extension.list","dir":"<data_dir>/extensions"}
```

## Remaining work

- Add remote extension registry (search/install from registry).
- Add extension validation (schema check).
- Add extension loader (load tool implementation from manifest).
