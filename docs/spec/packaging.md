# Packaging

## Target install UX

Unix-like systems:

```sh
curl -fsSL https://raw.githubusercontent.com/spenceriam/Zero-Agent/main/scripts/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/spenceriam/Zero-Agent/main/scripts/install.ps1 | iex
```

## Current status

Installer scripts are placeholders until release artifacts exist.

## Artifact matrix

- Windows x64
- Linux x64
- macOS Apple Silicon
- macOS Intel if practical

## Build requirements today

- Zero CLI 0.1.1+
- Rust 1.95+
- Windows GNU target for bridge builds on this machine
- MSYS2 MinGW GCC on Windows
