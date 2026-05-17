# BMAD Story 2.4: OpenAI-Compatible Provider

## Status

Implemented as model layer adapters.

## OpenRouter adapter

`src/providers/openrouter.0` defines:

- `OpenRouterConfig` shape: base_url, api_key_env, default_model
- `defaultConfig()` with OpenRouter API endpoint
- `buildRequest()` - delegates to OpenAI request builder
- `provider()` - returns ProviderConfig for registry

Default model: `anthropic/claude-sonnet-4`

## Ollama Cloud adapter

`src/providers/ollama.0` defines:

- `OllamaConfig` shape: base_url, default_model
- `defaultConfig()` with local Ollama endpoint
- `buildRequest()` - delegates to OpenAI request builder
- `provider()` - returns ProviderConfig for registry

Default model: `llama3.2`

## Design

Both adapters share the OpenAI request/response format. The only differences are:

- Base URL (OpenRouter vs local Ollama)
- API key handling (OpenRouter needs OPENROUTER_API_KEY, Ollama is local)
- Default model names

## Acceptance evidence

Commands:

```sh
export PATH="/c/Users/spenc/.zero/bin:$PATH"
zero build src/main.0
zero run src/main.0
```

## Remaining work

- Wire provider selection into agent loop based on config.default_provider.
- Add real HTTP calls through bridge.
- Add model list per provider.
