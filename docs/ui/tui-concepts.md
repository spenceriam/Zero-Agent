# TUI UI/UX Concepts

Zero-Agent targets the **Hermes default CLI layout** (scrollback transcript + fixed footer), not the separate `hermes --tui` Ink full-screen mode.

## Chosen direction: alternate screen + frame buffer

The TUI runs on the **alternate screen buffer** so shell scrollback cannot corrupt the UI. Transcript lines live in memory; each frame redraws the scrollable chat viewport between a **pinned header** and **pinned footer**.

```text
┌─ pinned header (rows 0–1, never scrolls) ──────────────────┐
│  Zero-Agent · session: 20260519-223202                     │
│  (blank spacer)                                            │
├─ scrollable chat viewport ─────────────────────────────────┤
│  Spencer                                                   │
│  list files in src/                                        │
│  Thinking:                                                 │
│  The user wants a directory listing…                       │
│  ZERO  Here are the files…                                 │
├─ pinned footer (never scrolls) ──────────────────────────────┤
│  ────────────────────────────────────────────────────────  │  ← prompt top rule
│  ❯ _                                                       │
│  ────────────────────────────────────────────────────────  │  ← prompt bottom rule
│  ⚕ mimo-v2.5-pro │ 0/128K │ CTX 0% │ ~/zero-agent          │  ← status (bottom-most)
└────────────────────────────────────────────────────────────┘
```

Implementation: [`bridge/rust/src/tui/layout.rs`](bridge/rust/src/tui/layout.rs) — `TerminalSession`, in-memory `transcript`, `render_frame()`, `scroll_offset`. User display name comes from [`profile.json`](../../.zero-agent/profile.json) (see onboarding below).

## Chat gutters and wrapping

The scrollable transcript uses a **2-column left/right gutter** (`content_width = terminal_width - 4`). The pinned header, status line, and prompt borders stay **full terminal width**.

All chat content — user messages, ZERO stream, thinking blocks, tool output, and diffs — wraps to `content_width` before the gutter prefix is applied. Resizing the terminal does not reflow existing transcript lines; new output uses the updated width.

User messages render as a plain **name + wrapped text** block (no Unicode box), with a **blank row** after the message before the next block. Thinking and ZERO stream body use the same content indent as user text; stream chunks flush on newline or turn end only.

Tool call lines use a **colored label pill** (`shell`, `read`, `write`, etc.) — yellow while running, green on success, red on failure. The same line updates in place when the tool finishes; command output appears on indented rows below (no pipe/`$` prefix). Agent prose may still include emojis.

## Debug mode

Enable with `--debug`, `ZERO_DEBUG=1`, or `/debug on`. Logs go to `{data_dir}/sessions/{session_id}/debug.log` (never the TUI transcript). Use `/debug status` to see the path and on/off state. Instrumentation covers input, agent loop iterations, provider streams, and tool execution.

## Tool approval

Destructive shell commands (`rm`, `git push`, `gh pr merge`, etc.) trigger a **frame-buffer modal overlay** (same pattern as onboarding — not println). Read-only commands (`ls`, `git status`, `gh pr view`) and mutating local ops (`git commit`, `write_file`) run without prompting by default.

## Edit/write diff view

After a successful `edit_file` or `write_file`, the TUI shows a **Pi-style diff** instead of dumping plain tool output:

- Header with path and `+N -M` stats
- **Auto layout**: split side-by-side at terminal width ≥ 120; unified below that
- **Compact/summary** on very narrow panes
- Body collapsed to 24 changed lines with a `… +N more lines` hint
- Classic `+`/`-` indicators with green/red coloring; long lines word-wrap within gutters

Syntax highlighting and streaming “pending edit” previews are not implemented yet.

## First-run setup overlay (separate from chat)

First launch (no profile) runs a **setup modal** — not the chat transcript:

- Chat chrome (pinned header, dimmed status + prompt) renders underneath the modal
- A centered modal card collects name, communication style, and optional preferences
- The chat prompt is disabled (`(setup)` hint) until setup completes
- Input is captured only by the modal; the main chat loop does not run

After setup completes, the TUI **hard-resets** into chat mode: transcript cleared, modal dismissed, prompt enabled.

Implementation: [`bridge/rust/src/tui/onboarding.rs`](bridge/rust/src/tui/onboarding.rs), [`bridge/rust/src/tui/modal.rs`](bridge/rust/src/tui/modal.rs), `TuiMode::OnboardingOverlay` in [`layout.rs`](bridge/rust/src/tui/layout.rs).

## Frame-buffer invariants

- Alternate screen with full clear on enter (no scroll-region hacks)
- Terminal size synced on init, resize, and each frame
- Pinned header at rows `0..HEADER_ROWS`; chat fills `HEADER_ROWS..footer_start`
- Footer pinned at bottom: top rule → prompt → bottom rule → status (bottom-most row)
- All drawable rows cleared each frame to prevent ghost/overdraw

## Status line

Bottom-most footer row shows model, context window (`0/128K`), context usage (`CTX N%`), and cwd. No graphical progress bar, no `idle` label, and no elapsed timer when idle. During an active turn (`thinking…` / `running tool…`), a turn-scoped elapsed timer appears.

## Growing prompt composer

The footer **grows upward** as you type long lines — typed text soft-wraps at terminal width and adds prompt rows above the status bar. The chat viewport shrinks accordingly (no row cap for typed input).

```text
│  ────────────────────────────────────────────────────────  │
│  ❯ This is a long typed line that wraps onto              │
│    the next row automatically                           │
│  ────────────────────────────────────────────────────────  │
│  ⚕ model │ 0/128K │ CTX 12% │ 1.2s │ thinking… │ ~/proj   │
```

**Paste / attachment policy (Hermes-style):**

- Multiline paste → compact badge only: `[pasted: N lines, M chars — press Enter to send]`
- Large single-line paste (>120 chars or wider than terminal) → same badge treatment
- Full pasted content is sent on Enter; it never expands the prompt box inline
- Future image attachments will use a similar badge: `[attached: filename.png]` (not implemented yet)

**Keyboard:**

- Alt+Enter or `\` + Enter — explicit newline in typed input
- ↑/↓ — move cursor within wrapped prompt; at first/last row, fall through to input history
- ←/→ — move cursor within the buffer

**First-run onboarding:** Runs as a modal overlay before the first chat turn (see **First-run setup overlay** above). Saved to `.zero-agent/profile.json` and injected into the agent system prompt.

**Not in scope:** Hermes mega-banner, skills wall, yellow skin, or Ink `--tui` modals.

## Legacy: DECSTBM scroll regions (removed)

Earlier builds used DECSTBM scroll regions on the main buffer; mouse scroll corrupted fixed footer rows. Replaced by the frame-buffer model above.

## Legacy Concept A: split-pane layout (not current)

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

Typing `/` in the prompt opens a **slash command palette** rendered as part of the frame buffer (bottom of the chat viewport, above the footer). The cursor stays in the prompt composer — the palette is not a separate println overlay.

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

Only **destructive** tool invocations prompt (e.g. `rm`, `git push`, `gh pr merge`). Read-only shell commands (`ls`, `git status`) and mutating local ops (`git commit`, `write_file`) run without prompting.

The TUI shows a **centered frame-buffer modal overlay** (same pattern as onboarding — chat dims underneath, header/footer stay pinned):

```text
┌─ Action Required ─────────────────────┐
│ Tool: shell                           │
│ Command: rm -rf dist                  │
│ Risk: Destructive                     │
│                                       │
│ > [D] Deny                            │
│   [O] Approve Once                    │
│   [S] Approve for Session             │
│   [A] Approve Always                  │
└───────────────────────────────────────┘
  ↑↓ navigate · Enter confirm · Esc Deny
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
