# BMAD Story 2.3: OpenAI Provider

## Status

Model layer implemented. HTTP/SSE streaming requires bridge http.stream_sse.

## Provider model

`src/providers/openai.0` defines:

- `OpenAIMessage` shape: role, content
- `OpenAITool` shape: tool_type, function_name, function_description, function_parameters
- `OpenAIToolCall` shape: id, tool_type, function_name, function_arguments
- `OpenAIRequest` shape: model, messages, tools, stream, max_tokens
- `OpenAIChoice` shape: finish_reason, message
- `OpenAIResponse` shape: id, model, choices
- `buildRequest()` - construct a streaming OpenAI API request
- `toolSchema()` - translate Zero tool to OpenAI function calling format
- `parseStreamEvent()` - parse SSE chunk into AgentEvent
- `textDeltaEvent()`, `toolCallEvent()` - event constructors

## Streaming design

The OpenAI provider produces AgentEvent values from parsed SSE chunks:

- `textDeltaEvent(text)` -> ProviderText (assistant text delta)
- `toolCallEvent(name, arguments)` -> ToolCall (function call request)

Actual HTTP/SSE transport will be wired through `bridge.http.stream_sse` when implemented.

## Acceptance evidence

Commands:

```sh
export PATH="/c/Users/spenc/.zero/bin:$PATH"
zero build src/main.0
zero run src/main.0
```

## Remaining work

- Wire http.stream_sse bridge operation for real OpenAI API calls.
- Implement SSE line parser for OpenAI chunk format (data: [DONE], data: {...}).
- Add API key env var reading (OPENAI_API_KEY).
- Add model list (gpt-4o, gpt-4o-mini, etc.).
