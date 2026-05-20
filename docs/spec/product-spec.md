# Product Spec: Zero-Agent

## Vision

Zero-Agent is a serious daily-use agent for coding, co-working, productivity, chatting, analysis, advice, background work, and messaging. It is inspired by Hermes Agent, but intentionally smaller, faster, and built primarily in Zero.

The project should prove that a modern AI agent can be mostly Zero-native while still supporting practical daily-use capabilities.

## Product identity

- Name: Zero-Agent
- Repository: `spenceriam/Zero-Agent`
- Local folder: `C:/Users/spenc/GitHub/zero-agent`
- License: AGPL-3.0
- Primary language: Zero
- Temporary bridge language: Rust, only where needed
- First interface targets: split-pane TUI and Telegram
- First owner model: single-owner local-first agent

## Primary users

1. Developer using the TUI for coding work.
2. Owner using Telegram to delegate work, ask questions, check status, and trigger background tasks.
3. Future advanced user who extends the agent by asking it to find, install, build, or create missing tools/skills/extensions.

## Core jobs to be done

Zero-Agent should help the user:

- build software
- modify files
- analyze codebases
- run commands and tests
- explain tradeoffs
- make plans
- execute background jobs
- use sub-agents
- remember local preferences and project facts
- communicate through Telegram
- discover and switch models/providers
- add tools or skills as needs emerge

## Guiding constraints

- Zero wherever it makes sense.
- Rust bridge only for runtime gaps in Zero.
- Do not hide bridge dependencies; call them out explicitly.
- Small install footprint.
- Cross-platform goal: Linux, macOS, Windows, WSL, bash, zsh, PowerShell.
- `curl | sh` style install for Unix-like systems and an equivalent Windows installer path.
- Local-only memory at first.
- Destructive actions require permission.
- Non-destructive actions should be fast and low-friction.
- AGPL-3.0 reciprocal open-source licensing.

## Must-have v0.1 capabilities

### Interfaces

- Split-pane TUI.
- Telegram single-owner gateway.
- Shared core agent loop used by both.

### Providers

- Anthropic native Messages API and tool calling.
- OpenAI native Responses/Chat Completions tool calling, depending on final API choice.
- OpenAI-compatible providers.
- OpenRouter.
- Ollama Cloud.
- Local Ollama if practical.
- Model discovery from provider endpoints where available.
- Provider/model setup flow from TUI and Telegram.

### Agent runtime

- Streaming responses.
- Thinking/reasoning event display when provider exposes it.
- Tool calling.
- Tool-result continuation loop.
- Background jobs.
- Sub-agent runs.
- Interrupt/stop.
- Persistent sessions.

### Tools

Initial Pi/Hermes-like baseline:

- read file
- write file
- edit file
- search files
- glob/list files
- run shell command
- run tests/builds
- web fetch/search where allowed
- task tracking
- memory save/list/forget
- schedule background prompt
- spawn sub-agent
- messaging send/reply
- provider/model discovery
- install/build/create extension

### Mutable tools/skills/extensions

The user should be able to say:

- "I need a tool that does X."
- "Find an extension for Y."
- "Build a skill for this workflow."
- "Add support for provider Z."

The agent should respond through an extension workflow:

1. Check built-in tools.
2. Check local extensions/skills.
3. Search configured extension sources if permitted.
4. Propose install/build/create plan.
5. Ask permission for risky install/build steps.
6. Add the tool/skill manifest locally.
7. Test it.
8. Make it available to future runs.

v0.1 should implement manifest-first local tools/skills, not a fully remote package ecosystem.

## Non-goals for v0.1

- Multi-owner/team auth.
- Full Slack/Discord/Matrix implementations.
- Full MCP compatibility.
- Browser automation.
- Cloud execution sandbox.
- Vector DB memory.
- Self-modifying core without review.
- Fully Zero-native networking if Zero APIs are not ready.

## Success criteria

Milestone 1 succeeds when Zero-Agent can, from both TUI and Telegram:

1. Configure endpoint/API key/provider.
2. Discover available models when provider supports it.
3. Stream a response.
4. Display reasoning/thinking events when available.
5. Execute a basic suite of tool calls.
6. Persist the session.
7. Run the same core agent loop across both interfaces.

## Open design posture

This project should be spec-driven. Each feature must be broken into chunks that another AI coding agent can implement independently with clear acceptance tests.
