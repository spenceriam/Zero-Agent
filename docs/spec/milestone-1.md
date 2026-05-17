# Milestone 1: Both-Proof

## Purpose

Prove that Zero-Agent can use an LLM to do a basic suite of provider setup, model discovery, streaming, thinking/reasoning event display, tool calling, and session persistence from both the split-pane TUI and Telegram.

## Required interfaces

- TUI
- Telegram polling gateway

Both must use the same core agent loop.

## Required providers

At least two working providers:

1. Anthropic native or OpenAI native.
2. OpenRouter or Ollama Cloud.

OpenAI-compatible support may satisfy provider 2 if OpenRouter works through that path.

## Required setup flow

User can configure:

- provider
- endpoint/base URL where applicable
- API key env var or secret reference
- model

User can run model discovery:

```text
/provider add
/provider list
/model discover
/model set
```

## Required tool suite

Milestone tool calls:

- read file
- write file
- edit file
- list/glob files
- search text
- run shell command
- save/list/forget memory
- create/list/cancel background job
- spawn simple sub-agent

## Required safety behavior

- Read/search/list run without approval.
- Mutating tools require a clear status/permission model.
- Destructive tools always require approval.
- Telegram shell/file tools are owner-only.
- Dangerous shell patterns are blocked or require explicit confirmation.

## Required streaming behavior

TUI:

- stream assistant text into conversation pane
- show provider/model in status
- show tool calls in tool/status pane
- show thinking/reasoning event markers when available

Telegram:

- send progress message
- edit or append streamed chunks if practical
- send final response
- expose `/stop`

## Required persistence

- Sessions persist between runs.
- Telegram chat maps to a session.
- TUI can resume the last session.
- Local memory survives restart.
- Jobs survive restart.

## Acceptance test script

### TUI path

1. Start `zero-agent`.
2. Configure provider and API key.
3. Discover models.
4. Select a model.
5. Ask: "Create a file called hello.txt with a short greeting, then read it back."
6. Confirm any needed write permission.
7. Verify the file is created and read back.
8. Ask: "Run a command to show the current directory."
9. Verify command output appears.
10. Ask: "Remember that I prefer concise status updates."
11. Verify memory list shows the fact.
12. Spawn a sub-agent to summarize the project README.
13. Verify sub-agent result appears.

### Telegram path

1. Start gateway.
2. Send `/start` from owner account.
3. Send `/model discover`.
4. Select a model.
5. Ask the same hello-file task.
6. Confirm permissions if prompted.
7. Ask `/status`.
8. Schedule a one-shot reminder/job.
9. Stop a running response with `/stop`.

## Done definition

Milestone 1 is done only when both TUI and Telegram paths pass using the same core loop and documented provider configuration.
