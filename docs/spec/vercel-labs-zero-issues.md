# Zero-Agent Coverage of vercel-labs/zero Issues

## Addressed by Zero-Agent

### #4: AI-native primitives for agent-oriented language

Zero-Agent implements this at the application level:

- `src/providers/registry.0` - provider interface with capability metadata
- `src/providers/anthropic.0`, `openai.0` - model call primitives
- `src/tools/registry.0` - typed tool input/output with risk classification
- `src/core/policy.0` - approval boundaries (Safe/Mutating/Destructive/Blocked)
- `src/core/memory.0` - context/memory with persistence
- `src/core/session.0` - conversation context with budget tracking

The `ProviderRequest` shape and tool `input_schema` provide the typed tool call surface the issue describes.

### #5: generateText primitive

Zero-Agent's provider layer implements the generateText pattern:

- `src/providers/anthropic.0` - `buildRequest()` with model, messages, tools, stream
- `src/providers/openai.0` - `buildRequest()` with same interface
- `src/providers/openrouter.0`, `ollama.0` - OpenAI-compatible adapters

The provider model uses explicit `ProviderConfig` with `api_key_env` (no hidden global client), matching the issue's constraint of explicit capability.

### #7: Normalize capability and effect summary

Zero-Agent's tool registry provides capability metadata:

- `src/tools/registry.0` - `Tool` shape with name, description, risk, input_schema
- `src/providers/registry.0` - `ProviderCapability` with streaming, tool_calling, thinking_events, model_discovery
- `src/core/policy.0` - `decisionForRisk()` maps risk levels to permission decisions

This gives agents and editors a stable view of available tools and their risk profiles.

### #8: Agent-oriented benchmarks

Zero-Agent's stub provider serves as a benchmark/test surface:

- `src/providers/registry.0` - `stubStreamResponse()`, `stubToolCall()`
- Bridge has 6 unit tests covering JSON parsing, config validation, unicode escapes

The bridge smoke tests (config, session, filesystem, shell, memory, extension operations) exercise the kind of agent-oriented code paths the issue describes.

## Not addressed (Zero language level)

These are upstream Zero language issues:

- **#6: Structured edit previews** - Zero compiler feature, not Zero-Agent
- **#19: Lambda syntax** - Zero language feature request
- **#20: Structured concurrency** - Zero language feature request
- **#25: Zed extension** - Editor tooling
- **#28: Darwin dyld LC_UUID** - Zero runtime bug on macOS

## Recommendation

Issues #4, #5, #7, #8 are addressed by Zero-Agent's architecture. If vercel-labs/zero adopts similar primitives at the language level, Zero-Agent could migrate from bridge-based implementations to native stdlib calls.
