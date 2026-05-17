# TUI UI/UX Concepts

Zero-Agent should feel like Hermes Agent: active, tool-aware, status-rich, and comfortable for long-running work.

## Concept A: Primary split-pane layout

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ Zero-Agent  provider: anthropic  model: claude-*  session: project/main      │
├───────────────────────────────────────────────┬──────────────────────────────┤
│ Conversation                                  │ Activity                     │
│                                               │                              │
│ You: fix the parser                           │ ┌ Tools ───────────────────┐ │
│ Agent: I’ll inspect the parser...             │ │ read src/parser.0   done │ │
│                                               │ │ search "parse"     done │ │
│ Tool: read_file src/parser.0                  │ │ edit parser.0    pending│ │
│ Agent: Found the failing branch...            │ └──────────────────────────┘ │
│                                               │                              │
│ Agent streaming response appears here.        │ ┌ Jobs/Sub-agents ─────────┐ │
│                                               │ │ #12 tests        running │ │
│                                               │ │ #13 review       queued  │ │
│                                               │ └──────────────────────────┘ │
├───────────────────────────────────────────────┴──────────────────────────────┤
│ Input                                                                         │
│ > Ask, command, or /help                                                       │
└──────────────────────────────────────────────────────────────────────────────┘
```

Best for daily coding.

## Concept B: Workbench layout

```text
┌──────────────────────────────────────────────────────────────────────────────┐
│ Header: model/provider/session/tokens/status                                 │
├──────────────────────────────┬──────────────────────────────┬────────────────┤
│ Chat                         │ Files / Diff                 │ Tools / Jobs   │
│                              │                              │                │
│ Agent conversation           │ selected file, patch, diff   │ tool calls     │
│                              │                              │ sub-agents     │
│                              │                              │ background     │
├──────────────────────────────┴──────────────────────────────┴────────────────┤
│ Composer: multiline prompt + approvals                                       │
└──────────────────────────────────────────────────────────────────────────────┘
```

Best once edits/diffs are first-class.

## Concept C: Telegram mental model

Telegram should mirror the TUI status model with compact commands.

```text
User: /status
Zero-Agent:
Session: telegram/main
Provider: OpenRouter
Model: ...
Running: job #14 summarize repo
Tools enabled: read, search, shell-confirm, edit-confirm
Memory: local

User: create a tool that checks package licenses
Zero-Agent:
I don’t have that tool yet. I can create a local extension:
- manifest: license-checker
- implementation: shell/Rust bridge initially
- risk: read-only
Approve?
```

## Slash commands

Core:

```text
/help
/new
/sessions
/status
/stop
/provider
/model
/tools
/memory
/jobs
/agents
/extensions
/config
```

Provider:

```text
/provider list
/provider add
/provider test
/model discover
/model set <model>
```

Extensions:

```text
/extensions list
/extensions search <capability>
/extensions create <description>
/extensions install <source>
/extensions test <name>
/extensions disable <name>
```

Jobs/sub-agents:

```text
/jobs list
/jobs cancel <id>
/agents spawn <task>
/agents list
/agents stop <id>
```

## Approval UX

Approval should be concise and specific.

```text
Tool request: shell
Command: rm -rf dist
Risk: destructive
Reason: removes build output directory

Approve once / always for this session / deny
```

For Telegram:

```text
Approve destructive command?
rm -rf dist
[Approve once] [Deny]
```

## Design rules

- Show active tools without overwhelming the chat.
- Make long-running work visible.
- Let the user interrupt anytime.
- Make model/provider state obvious.
- Keep destructive approvals impossible to miss.
- Do not make safe read/search actions noisy.
- Make extension creation feel like a normal agent capability, not a separate developer workflow.
