# BMAD Work Breakdown

This project should be implemented in small slices that an AI coding agent can complete without needing to understand the whole system at once.

## Epic 0: feasibility and repo baseline

### Story 0.1: repo baseline

Deliver:

- README
- AGPL-3.0 license
- docs/spec pack
- minimal Zero project manifest
- empty source layout
- Rust bridge placeholder

Acceptance:

- Repo is initialized.
- Specs explain scope and milestone 1.

### Story 0.2: Zero runtime capability matrix

Deliver:

- Spike results for HTTP, SSE, process streaming, terminal input, JSON, filesystem, Telegram polling.

Acceptance:

- Each capability is marked Zero-native, Rust-bridge, or deferred.

## Epic 1: core models

### Story 1.1: config model

Deliver:

- provider config type
- model config type
- tool policy config type
- local file loading/saving

Acceptance:

- Config can be read/written and validated.

### Story 1.2: event model

Deliver:

- input event
- provider event
- tool event
- UI render event
- gateway event

Acceptance:

- TUI and Telegram can consume the same event stream.

### Story 1.3: session store

Deliver:

- JSONL session persistence
- session create/list/resume

Acceptance:

- Messages survive restart.

## Epic 2: providers

### Story 2.1: provider registry

Deliver:

- provider interface
- registry
- capability metadata

Acceptance:

- Stub provider can stream events and request a tool.

### Story 2.2: Anthropic provider

Deliver:

- native request builder
- tool schema translation
- streaming parser
- thinking/reasoning event mapping if exposed

Acceptance:

- Streams text and completes a tool call.

### Story 2.3: OpenAI provider

Deliver:

- native request builder
- tool schema translation
- streaming parser
- reasoning event mapping if exposed

Acceptance:

- Streams text and completes a tool call.

### Story 2.4: OpenAI-compatible/OpenRouter/Ollama Cloud

Deliver:

- base URL configuration
- model discovery
- streaming
- tool calling where compatible

Acceptance:

- Model discovery and one tool call work.

## Epic 3: tool system

### Story 3.1: tool registry and policy

Deliver:

- tool interface
- risk levels
- permission decision model

Acceptance:

- Safe, mutating, destructive behavior is testable.

### Story 3.2: filesystem tools

Deliver:

- read
- write
- edit
- glob/list
- search

Acceptance:

- Milestone hello-file task passes.

### Story 3.3: shell tool

Deliver:

- shell detection
- command execution
- output streaming
- destructive classifier

Acceptance:

- Non-destructive commands run; destructive commands prompt.

### Story 3.4: memory tool

Deliver:

- save
- list
- forget
- local persistence

Acceptance:

- User preference can be saved and listed.

## Epic 4: TUI

### Story 4.1: split-pane shell

Deliver:

- conversation pane
- input pane
- activity pane
- status header

Acceptance:

- User can type messages and see streamed output.

### Story 4.2: approval UI

Deliver:

- inline approval prompts
- once/session/deny options

Acceptance:

- Destructive tool calls cannot proceed silently.

## Epic 5: Telegram

### Story 5.1: Telegram polling adapter

Deliver:

- polling loop
- owner allowlist
- normalized message mapping

Acceptance:

- Owner can send message and receive response.

### Story 5.2: Telegram commands

Deliver:

- `/start`, `/help`, `/status`, `/stop`, `/model`, `/provider`, `/tools`

Acceptance:

- Milestone Telegram path passes.

## Epic 6: jobs and sub-agents

### Story 6.1: background jobs

Deliver:

- job model
- one-shot jobs
- cancel/status

Acceptance:

- User schedules and cancels a job.

### Story 6.2: sub-agent runner

Deliver:

- focused child run model
- status events
- result return

Acceptance:

- User spawns a sub-agent to summarize README.

## Epic 7: mutable extensions

### Story 7.1: manifest-first extensions

Deliver:

- extension manifest schema
- extension loader
- local extension directory

Acceptance:

- Local extension appears as a tool.

### Story 7.2: create missing tool workflow

Deliver:

- detect missing capability
- propose build/create plan
- generate manifest + implementation
- run tests

Acceptance:

- User asks for a missing tool and agent creates a local extension.

## Epic 8: packaging

### Story 8.1: release artifact design

Deliver:

- artifact naming
- OS matrix
- install script spec

Acceptance:

- Fresh install path is documented per OS family.
