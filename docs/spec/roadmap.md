# Roadmap

## Phase 0: feasibility spikes

Goal: learn what Zero can truly support today.

Spikes:

1. Zero config + JSON parsing.
2. Provider HTTP call.
3. Provider streaming/SSE.
4. Tool-call JSON roundtrip.
5. Process execution and streaming.
6. Split-pane TUI terminal control.
7. Telegram polling.
8. Model discovery.
9. Local session persistence.
10. Rust bridge ABI/API shape.

Exit criteria:

- Each spike has a result: Zero-native, Rust bridge required, or deferred.
- The bridge boundary is documented.

## Phase 1: shared core loop

Deliver:

- Config model.
- Provider registry.
- Tool registry.
- Session store.
- Basic memory store.
- Agent loop with tool continuation.
- Streaming event model.
- Policy engine.

Acceptance:

- A test provider can stream text and request tools.
- Tool results feed back into the loop.
- Sessions persist.

## Phase 2: providers and model discovery

Deliver:

- Anthropic native.
- OpenAI native.
- OpenAI-compatible.
- OpenRouter.
- Ollama Cloud.
- Model discovery where supported.

Acceptance:

- User can set endpoint/API key/provider/model.
- User can list models.
- User can switch models.
- Tool calling works with at least Anthropic and OpenAI native APIs.

## Phase 3: split-pane TUI

Deliver:

- Conversation pane.
- Input pane.
- Status/tool pane.
- Background jobs/sub-agents pane.
- Approval prompts.
- Slash commands.
- Interrupt/stop.

Acceptance:

- User can complete Milestone 1 from TUI.

## Phase 4: Telegram gateway

Deliver:

- Telegram polling.
- Single-owner auth.
- Per-chat session routing.
- Core slash commands.
- Tool permissions.
- Progress/final delivery.

Acceptance:

- User can complete Milestone 1 from Telegram.

## Phase 5: mutable tools, skills, extensions

Deliver:

- Local extension manifest.
- Skill manifest.
- Extension discovery.
- Extension install/build/create workflow.
- Tool tests.
- Permission model.

Acceptance:

- User can ask for a missing tool.
- Agent can propose creating it.
- Agent can create a local extension with manifest and test.
- Tool becomes available in later runs.

## Phase 6: background jobs and sub-agents

Deliver:

- Job store.
- One-shot jobs.
- Recurring jobs.
- Sub-agent run model.
- Job/sub-agent status UI.
- Telegram status delivery.

Acceptance:

- User can schedule work from TUI or Telegram.
- User can spawn a focused sub-agent.
- User can inspect/cancel jobs.

## Phase 7: packaging and hardening

Deliver:

- Release builds.
- OS artifacts.
- Install scripts.
- Size checks.
- Basic benchmark suite.
- Crash recovery.
- Log redaction.
- Security review.

Acceptance:

- Fresh machine can install and run Zero-Agent with one documented command per OS family.
