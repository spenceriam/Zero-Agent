# BMAD Story 1.2: Event Model

## Status

Implemented as a runtime slice.

## Event model

`src/core/events.0` defines:

- `AgentEventKind` enum: UserInput, ProviderText, ProviderThinking, AssistantText, ToolCall, ToolResult, ToolApproval, Status, Error, Done
- `AgentEvent` shape with kind, text, tool_name, source fields
- Constructor helpers for each event type

## Source tracking

Events carry an `InterfaceKind` source field so the core loop can produce a unified event stream that both TUI and Telegram consume identically.

## UI consumption

- `src/apps/tui.0` adds `renderEvent(event) -> String`
- `src/gateway/telegram.0` adds `formatEvent(event) -> String`

Both consume the same `AgentEvent` type.

## Acceptance evidence

Commands:

```sh
export PATH="/c/Users/spenc/.zero/bin:$PATH"
zero build src/main.0
zero run src/main.0
```

## Remaining work

- Wire event stream into real TUI rendering loop.
- Wire event stream into Telegram send adapter.
- Add session-scoped event buffering.
