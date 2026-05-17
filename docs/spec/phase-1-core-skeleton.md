# Phase 1 Core Skeleton

## Implemented baseline

Zero modules:

- `src/core/types.0`
- `src/core/config.0`
- `src/core/events.0`
- `src/core/session.0`
- `src/core/policy.0`
- `src/core/agent.0`
- `src/providers/registry.0`
- `src/tools/registry.0`
- `src/main.0`

Rust bridge:

- `bridge/rust/Cargo.toml`
- `bridge/rust/src/main.rs`

## Verified commands

```sh
export PATH="/c/Users/spenc/.zero/bin:/c/Users/spenc/.cargo/bin:$PATH"
zero check "/c/Users/spenc/GitHub/zero-agent"
zero build "/c/Users/spenc/GitHub/zero-agent"
zero run "/c/Users/spenc/GitHub/zero-agent"
```

```sh
export PATH="/c/msys64/mingw64/bin:/c/Users/spenc/.cargo/bin:$PATH"
cd "/c/Users/spenc/GitHub/zero-agent/bridge/rust"
cargo build --target x86_64-pc-windows-gnu
printf '{"id":"1","op":"ping"}\n' | cargo run --quiet --target x86_64-pc-windows-gnu
```

## Current constraints

- The Zero executable path stays primitive-only because Zero 0.1.1 direct Windows backend cannot emit user-defined shape returns from executable-reached functions.
- Richer core models are present as Zero modules, but runtime integration must proceed carefully around current codegen limits.
- Rust bridge execution works with MSYS2 MinGW and Rust GNU target.

## Next Phase 1 work

- Add concrete JSON schemas for events, tools, providers, sessions, jobs, and bridge operations.
- Expand Rust bridge from echo protocol to operation dispatch.
- Add provider transport operation skeletons.
- Add filesystem/process operation skeletons.
