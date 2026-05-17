# Phases 3-7 Surface Skeleton

## Implemented stubs

TUI/app:

- `src/apps/mode.0`
- `src/apps/tui.0`

Telegram/gateway:

- `src/gateway/message.0`
- `src/gateway/telegram.0`

Jobs/sub-agents/memory/extensions:

- `src/core/jobs.0`
- `src/core/subagents.0`
- `src/core/memory.0`
- `src/tools/extensions.0`

Packaging:

- `scripts/install.sh`
- `scripts/install.ps1`
- `docs/spec/packaging.md`

## Current status

These are compile-checked design skeletons, not complete product implementations.

## Remaining implementation work

- Real split-pane TUI rendering through Rust PTY/terminal bridge.
- Telegram polling/send bridge operations.
- Extension manifest loader and creator workflow.
- Persistent job runner.
- Real sub-agent process/runtime isolation.
- Release artifact publishing.
- Installer artifact download/verification.
