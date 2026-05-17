# BMAD Story 8.1: Packaging

## Status

Implemented with build-from-source installers.

## Install scripts

- `scripts/install.sh` - Unix installer (builds Rust bridge, copies to ~/.zero-agent/bin)
- `scripts/install.ps1` - Windows installer (builds Rust bridge, copies to ~/.zero-agent/bin)

## Build requirements

- Rust 1.95+ (for bridge)
- Zero CLI 0.1.1+ (for source build)
- MSYS2 MinGW GCC on Windows (for GNU target)

## Install flow

1. Check for required tools (cargo, zero)
2. Build Rust bridge in release mode
3. Copy bridge binary to ~/.zero-agent/bin
4. Print PATH instructions

## Artifact matrix

- Windows x64: zero-agent-bridge.exe
- Linux x64: zero-agent-bridge
- macOS Apple Silicon: zero-agent-bridge

## Acceptance evidence

Build bridge from source:

```sh
cd bridge/rust
cargo build --release
```

Install script test:

```sh
./scripts/install.sh
```

## Remaining work

- Add CI/CD pipeline for automated builds.
- Add release artifact publishing to GitHub Releases.
- Add checksum verification in installer.
- Add auto-update mechanism.
