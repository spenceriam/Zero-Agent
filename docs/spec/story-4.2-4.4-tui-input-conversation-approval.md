# BMAD Stories 4.2-4.4: TUI Input, Conversation, Approval

## Status

Implemented as model layers.

## Input handling (Story 4.2)

`src/apps/tui.0` defines:

- `InputState` shape: text, cursor
- `defaultInput()` - create empty input state

## Conversation pane (Story 4.3)

Conversation history is managed through the session store (Story 1.3). The TUI renders conversation lines from session messages.

## Approval flow (Story 4.4)

`src/apps/tui.0` defines:

- `ApprovalChoice` enum: Approve, Deny, AlwaysApprove
- `ApprovalPrompt` shape: visible, tool_name, description, risk
- `renderApproval(tool_name, description, risk)` - show approval prompt
- `hideApproval()` - hide approval prompt

## Approval flow design

1. Agent calls tool with Mutating/Destructive risk
2. TUI shows ApprovalPrompt with tool details
3. User chooses Approve/Deny/AlwaysApprove
4. If Approve, tool executes
5. If Deny, tool is skipped
6. If AlwaysApprove, tool risk is downgraded for session

## Acceptance evidence

Commands:

```sh
export PATH="/c/Users/spenc/.zero/bin:$PATH"
zero build src/main.0
zero run src/main.0
```

## Remaining work

- Wire input handling into actual terminal input via bridge.
- Wire approval flow into agent loop.
- Add conversation scrolling and pagination.
- Add input history (up/down arrows).
