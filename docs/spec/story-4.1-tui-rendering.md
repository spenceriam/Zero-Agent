# BMAD Story 4.1: TUI Rendering Model

## Status

Implemented as model layer. Actual terminal rendering requires bridge.

## Rendering model

`src/apps/tui.0` defines:

- `Pane` enum: Conversation, Activity, Input
- `TuiLayout` shape: conversation, activity, input, active_pane
- `RenderLine` shape: text, kind (AgentEventKind)
- `ThinkingPanel` shape: visible, content
- `ProgressState` shape: active, message
- Render helpers: renderEvent, renderAssistantText, renderThinking, renderProgress, renderStatus, renderError

## Event to render mapping

- ProviderText -> RenderLine in conversation pane
- ProviderThinking -> ThinkingPanel (collapsible)
- AssistantText -> RenderLine in conversation pane
- ToolCall -> RenderLine in activity pane
- ToolResult -> RenderLine in activity pane
- Status -> ProgressState in activity pane
- Error -> RenderLine with error styling

## Acceptance evidence

Commands:

```sh
export PATH="/c/Users/spenc/.zero/bin:$PATH"
zero build src/main.0
zero run src/main.0
```

## Remaining work

- Wire rendering model into actual terminal output via bridge.
- Add split-pane layout with borders.
- Add scrolling and pagination.
- Add syntax highlighting for code blocks.
- Add markdown rendering.
