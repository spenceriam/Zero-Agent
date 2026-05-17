# BMAD Story 2.1: Provider Registry

## Status

Implemented as a runtime slice.

## Provider model

`src/providers/registry.0` defines:

- `ProviderCapability` shape: streaming, tool_calling, thinking_events, model_discovery
- `Provider` shape: id, display_name, capability
- `ProviderRequest` shape: model, messages, tools
- `ProviderRegistry` shape: providers (list placeholder)

## Registered providers

- `openRouterProvider()` - streaming, tool_calling, model_discovery
- `anthropicProvider()` - streaming, tool_calling, thinking_events
- `openaiProvider()` - streaming, tool_calling, model_discovery
- `stubProvider()` - tool_calling only (for testing)

## Stub provider

The stub provider can:

- Stream a text response via `stubStreamResponse()`
- Request a tool call via `stubToolCall()`

Both return `AgentEvent` values that the core loop and UI can consume.

## Acceptance evidence

Commands:

```sh
export PATH="/c/Users/spenc/.zero/bin:$PATH"
zero build src/main.0
zero run src/main.0
```

## Remaining work

- Wire provider registry into agent loop for dynamic provider selection.
- Add real HTTP/SSE streaming via bridge (http.stream_sse).
- Implement OpenRouter model discovery via bridge.
- Add provider-specific request builders for Anthropic and OpenAI.
