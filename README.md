# Zero-Agent

Zero-Agent is a GPL-3.0, Hermes-inspired daily-use agent built primarily in [Zero](https://zerolang.ai/learn).

It is designed for coding, co-working, productivity, chat, local tools, background jobs, sub-agents, and messaging providers starting with Telegram.

## What works today

The **Rust bridge TUI** (`bridge/rust/`) is the primary runnable interface:

- Unified scroll transcript (user, thinking, tools, agent replies)
- Fixed footer with status bar and prompt
- Destructive-tool approval modal (pill buttons, optional deny comment)
- OpenAI-compatible and Anthropic providers with streaming
- Local-first sessions, memory, and tool execution (`read`/`write`/`edit`/`shell`/`glob`)

Zero-language sources under `src/` and archived specs remain for long-term Zero-native implementation.

## Quick start

### From a clone

```bash
./scripts/run.sh
```

`./scripts/run.sh` runs the **pre-built binary** when possible. For a fresh build:

```bash
cargo build --release --manifest-path bridge/rust/Cargo.toml --features tui
./scripts/run.sh
```

Pass an explicit config path:

```bash
./scripts/run.sh --config /path/to/config.json
```

### Install script

```bash
curl -fsSL https://raw.githubusercontent.com/spenceriam/Zero-Agent/main/scripts/install.sh | bash
```

This installs to `$ZERO_HOME` (default `~/.zero-agent`), builds the TUI binary, and writes a starter `config.json` plus `.env` template.

## Configuration

Config is discovered by walking up from the current directory for `.zero-agent/config.json`, or loaded from `--config`.

**No API keys or secrets are committed to this repository.** Set keys via:

- `.zero-agent/.env` (see `scripts/install.sh` template)
- Environment variables: `OPENROUTER_API_KEY`, `OPENAI_API_KEY`, `ANTHROPIC_API_KEY`, `TELEGRAM_BOT_TOKEN`
- `api_key` fields in `config.json` (local only — never commit)

Default providers in code/install templates are **OpenRouter**, **OpenAI**, **Anthropic**, and **Ollama** (local or cloud). You choose `default_provider` in config.

Example minimal config:

```json
{
  "data_dir": ".zero-agent",
  "default_provider": "openrouter",
  "providers": [
    {
      "id": "openrouter",
      "name": "OpenRouter",
      "api_format": "openai",
      "base_url": "https://openrouter.ai/api/v1",
      "api_key": "",
      "default_model": "anthropic/claude-sonnet-4",
      "models": []
    }
  ],
  "tool_policy": {
    "allow_safe_without_prompt": true,
    "ask_before_mutating": true,
    "ask_before_destructive": true
  }
}
```

Set `ZERO_HOME` to use a global config directory under `$HOME/.zero-agent/`.

## TUI layout

The TUI uses a **fixed footer** (status bar + prompt) and a **scroll region** above it for the conversation transcript — the same structural model as Hermes default CLI.

Slash commands include `/model`, `/provider`, `/session`, `/clear`, `/debug`, and `/quit`. See [UI/UX Concepts](docs/ui/tui-concepts.md).

## Development

```bash
cd bridge/rust
cargo test --features tui
cargo clippy --features tui -- -D warnings
cargo fmt -- --check
```

## Goals

- Feel like Hermes Agent, but smaller, faster, and more Zero-native.
- Provide both a split-pane TUI and messaging gateway experience.
- Use Zero wherever it makes sense for durable core logic.
- Use a thin Rust bridge only where Zero cannot yet support required runtime APIs.
- Support native Anthropic and OpenAI tool-calling, OpenAI-compatible providers, OpenRouter, Ollama Cloud, and model discovery.
- Keep memory local-first and user-mutable.

## Documentation

- [Product Spec](docs/spec/product-spec.md)
- [Architecture](docs/spec/architecture.md)
- [Roadmap](docs/spec/roadmap.md)
- [UI/UX Concepts](docs/ui/tui-concepts.md)
- [BMAD Work Breakdown](docs/bmad/work-breakdown.md)

## License

GPL-3.0. See [LICENSE](LICENSE).
