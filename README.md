# Zero-Agent

Zero-Agent is a GPL-3.0, Hermes-inspired daily-use agent built primarily in [Zero](https://zerolang.ai/learn).

It is designed for coding, co-working, productivity, chat, local tools, background jobs, sub-agents, and messaging providers starting with Telegram.

## Goals

- Feel like Hermes Agent, but smaller, faster, and more Zero-native.
- Provide both a split-pane TUI and messaging gateway experience.
- Use Zero wherever it makes sense for durable core logic.
- Use a thin Rust bridge only where Zero cannot yet support required runtime APIs.
- Support native Anthropic and OpenAI tool-calling, OpenAI-compatible providers, OpenRouter, Ollama Cloud, and model discovery.
- Keep memory local-first and user-mutable.
- Allow users to ask the agent to find, install, build, or create tools/skills/extensions when needed.

## Running the TUI

From the repository root (recommended — config is discovered by walking up to `.zero-agent/config.json`):

```bash
./scripts/run.sh
```

`./scripts/run.sh` runs the **pre-built binary** when possible, so you won't see compiler output on every launch. For development rebuilds:

```bash
cargo build --release --manifest-path bridge/rust/Cargo.toml --features tui
./scripts/run.sh
```

If you use `cargo run` directly, yellow `warning:` lines are **Rust compiler warnings**, not errors — the build succeeded if you see `Finished release profile`.

Pass an explicit config path:

```bash
./scripts/run.sh --config /path/to/config.json
```

Set `ZERO_HOME` to use a global config directory under `$HOME/.zero-agent/`.

### Layout

The TUI uses a **fixed footer** (status bar + prompt) and a **scroll region** above it for the conversation transcript — the same structural model as Hermes default CLI.

## Current stage

This repository starts as a spec-driven project. Implementation should proceed in BMAD-style chunks that are small enough for an AI coding agent to complete safely.

Read first:

- [Product Spec](docs/spec/product-spec.md)
- [Architecture](docs/spec/architecture.md)
- [Roadmap](docs/spec/roadmap.md)
- [Milestone 1](docs/spec/milestone-1.md)
- [UI/UX Concepts](docs/ui/tui-concepts.md)
- [BMAD Work Breakdown](docs/bmad/work-breakdown.md)
- [Open Questions](docs/spec/open-questions.md)

## License

GPL-3.0. See [LICENSE](LICENSE).
