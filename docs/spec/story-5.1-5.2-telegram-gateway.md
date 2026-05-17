# BMAD Stories 5.1-5.2: Telegram Gateway

## Status

Implemented with bridge stubs.

## Zero model

`src/gateway/telegram.0` defines:

- `TelegramCommand` shape: name, description
- `TelegramMessage` shape: chat_id, text, from
- `TelegramUpdate` shape: update_id, message
- `TelegramConfig` shape: bot_token, allowed_users
- Commands: startCommand, helpCommand, statusCommand
- `formatEvent(event)` - format AgentEvent for Telegram
- `formatAssistantReply(text)` - format assistant reply
- `formatToolApproval(tool_name, description)` - format approval request
- `isAllowedUser(config, user_id)` - check user authorization

## Bridge operations

`bridge/rust/src/main.rs` supports:

- `telegram.poll` - stub (returns empty updates)
- `telegram.send` - stub (returns success)

Real implementation will call Telegram Bot API via http.request.

## Acceptance evidence

Commands:

```sh
export PATH="/c/Users/spenc/.zero/bin:$PATH"
zero build src/main.0
zero run src/main.0
```

```sh
export PATH="/c/msys64/mingw64/bin:/c/Users/spenc/.cargo/bin:$PATH"
CARGO_TARGET_DIR="/tmp/zero-agent-config-target" cargo test --target x86_64-pc-windows-gnu --manifest-path bridge/rust/Cargo.toml
```

Bridge smoke test:

```json
{"id":"1","op":"telegram.poll"}
{"id":"2","op":"telegram.send","chat_id":"12345","text":"Hello!"}
```

## Remaining work

- Wire real Telegram Bot API calls via http.request bridge.
- Add webhook support as alternative to polling.
- Add message formatting (Markdown, code blocks).
- Add inline keyboard for approval flow.
