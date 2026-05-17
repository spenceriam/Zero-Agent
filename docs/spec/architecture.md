# Architecture

## High-level shape

```text
Zero-Agent
  ├─ apps
  │  ├─ tui
  │  └─ telegram-gateway
  ├─ core
  │  ├─ agent-loop
  │  ├─ provider-registry
  │  ├─ tool-registry
  │  ├─ session-store
  │  ├─ memory-store
  │  ├─ job-runner
  │  ├─ sub-agent-runner
  │  └─ policy-engine
  ├─ providers
  │  ├─ anthropic
  │  ├─ openai
  │  ├─ openai-compatible
  │  ├─ openrouter
  │  └─ ollama-cloud
  ├─ tools
  │  ├─ fs
  │  ├─ search
  │  ├─ edit
  │  ├─ shell
  │  ├─ web
  │  ├─ memory
  │  ├─ scheduler
  │  ├─ sub-agent
  │  └─ extension-manager
  ├─ gateway
  │  ├─ normalized-message
  │  ├─ owner-auth
  │  ├─ telegram
  │  └─ coming-soon-stubs
  ├─ extension-system
  │  ├─ tool-manifest
  │  ├─ skill-manifest
  │  ├─ local-installer
  │  └─ builder
  └─ bridge
     └─ rust
        ├─ http
        ├─ sse
        ├─ process
        ├─ pty
        ├─ shell
        ├─ sqlite-or-jsonl
        └─ telegram
```

## Language boundary

### Zero-owned logic

Zero should own:

- application state
- agent loop
- prompt assembly
- provider abstraction
- tool schemas
- tool dispatch decisions
- policy decisions
- session model
- memory model
- config model
- job model
- extension manifest model
- UI state model
- normalized messaging model

### Rust bridge logic

Rust should be used only where current Zero APIs are insufficient:

- HTTPS requests
- SSE/streaming provider responses
- WebSocket support if needed later
- subprocess streaming
- PTY/raw terminal support
- shell detection and execution
- Telegram polling/webhooks
- filesystem watchers
- SQLite if JSONL becomes insufficient
- OS-specific installer/build helpers

Every Rust bridge API should be narrow and replaceable.

## Core agent loop

```text
InputEvent
  -> resolve session
  -> load memory/context
  -> build provider request
  -> stream provider events
  -> render/deliver assistant text
  -> collect tool calls
  -> classify tool risk
  -> request permission if needed
  -> execute tools
  -> append tool results
  -> continue until done/budget/cancelled
  -> persist session
```

## Provider interface

```text
Provider {
  id
  display_name
  api_kind
  capabilities
  configure(config)
  discover_models() -> Model[]
  stream(request) -> ProviderEventStream
}
```

Provider capabilities:

- streaming
- tool_calling
- native_thinking_events
- json_mode
- vision
- model_discovery
- local_models

Initial provider order:

1. Anthropic native.
2. OpenAI native.
3. OpenAI-compatible.
4. OpenRouter.
5. Ollama Cloud.
6. Local Ollama if the API surface aligns.

## Tool interface

```text
Tool {
  name
  description
  input_schema
  output_schema
  risk_level
  capabilities_required
  run(input, context) -> ToolResult
}
```

Risk levels:

- `safe`: read/search/list/status.
- `mutating`: file edits/writes, memory saves, local extension changes.
- `destructive`: deletes, overwrites, package installs, credential changes, pushes, external messages with broad impact.
- `blocked`: known dangerous patterns unless explicitly enabled.

Default policy:

- Safe tools run without approval.
- Mutating tools can run if user has granted the session/tool permission.
- Destructive tools always require approval.
- Messaging-triggered shell/file tools are owner-only.

## Extension architecture

v0.1 should be manifest-first.

```text
.zero-agent/extensions/<name>/
  extension.json
  README.md
  tool.zero | tool.rs | tool.sh | tool.ps1
  tests/
```

Extension manifest:

```json
{
  "name": "example-tool",
  "version": "0.1.0",
  "description": "Does one focused thing",
  "kind": "tool",
  "entry": "tool.sh",
  "schema": "schema.json",
  "risk_level": "safe",
  "permissions": ["read_files"]
}
```

Mutable tool flow:

1. User asks for capability.
2. Agent searches built-ins.
3. Agent searches local extensions.
4. Agent proposes installing/building/creating a new extension.
5. User approves risky steps.
6. Agent creates manifest + implementation.
7. Agent runs tests.
8. Tool becomes available.

## Session storage

Start with JSONL files for portability.

```text
~/.zero-agent/sessions/<session-id>.jsonl
~/.zero-agent/memory/*.jsonl
~/.zero-agent/jobs/*.json
~/.zero-agent/config.json
```

SQLite can be introduced when search/query needs justify it.

## Telegram gateway

Telegram should use the same core loop as the TUI.

```text
Telegram Update
  -> owner auth
  -> normalize message
  -> route to session
  -> run core loop
  -> send progress/final messages
```

MVP mode:

- polling first
- webhook later
- single owner allowlist
- text messages
- `/new`, `/stop`, `/status`, `/model`, `/provider`, `/tools`, `/help`

## Coming-soon provider stubs

Stubs can exist for:

- Discord
- Slack
- Matrix
- generic webhook

They should compile/configure as disabled providers, not pretend to work.

## Cross-platform shell strategy

Shell execution should detect:

- bash
- zsh
- fish later
- PowerShell
- cmd
- WSL boundary if applicable

The shell tool must represent destructive commands clearly before running them.

## Install strategy

Target:

- Unix: `curl -fsSL https://.../install.sh | sh`
- Windows PowerShell: `irm https://.../install.ps1 | iex`
- Package managers later: Homebrew, Scoop, npm wrapper only if useful.

The installer should download the right artifact, not require local compilation for normal users.
