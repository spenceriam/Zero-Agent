# BMAD Story 2.2: Anthropic Provider

## Status

Model layer implemented. HTTP/SSE streaming requires bridge http.stream_sse.

## Provider model

`src/providers/anthropic.0` defines:

- `AnthropicMessage` shape: role, content
- `AnthropicTool` shape: name, description, input_schema
- `AnthropicRequest` shape: model, max_tokens, messages, tools, stream
- `AnthropicToolUse` shape: id, name, input
- `AnthropicContent` shape: content_type, text, tool_use
- `AnthropicResponse` shape: stop_reason, content
- `buildRequest()` - construct a streaming Anthropic API request
- `toolSchema()` - translate Zero tool to Anthropic tool format
- `parseStreamEvent()` - parse SSE chunk into AgentEvent
- `thinkingEvent()`, `textEvent()`, `toolUseEvent()` - event constructors

## Streaming design

The Anthropic provider produces AgentEvent values from parsed SSE chunks:

- `thinkingEvent(text)` -> ProviderThinking (extended thinking/reasoning)
- `textEvent(text)` -> ProviderText (assistant text delta)
- `toolUseEvent(name, input)` -> ToolCall (tool use request)

Actual HTTP/SSE transport will be wired through `bridge.http.stream_sse` when implemented.

## Acceptance evidence

Commands:

```sh
export PATH="/c/Users/spenc/.zero/bin:$PATH"
zero build src/main.0
zero run src/main.0
```

## Remaining work

- Wire http.stream_sse bridge operation for real Anthropic API calls.
- Implement SSE line parser for Anthropic event types (message_start, content_block_delta, message_stop, etc.).
- Add API key env var reading (ANTHROPIC_API_KEY).
- Add model list (claude-sonnet-4-20250514, claude-haiku-4-20250514, etc.).
