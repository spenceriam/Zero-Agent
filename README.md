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
