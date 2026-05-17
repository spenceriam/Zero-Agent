# Zero Runtime Notes

## Direct Windows backend limitation

As of Zero CLI 0.1.1, `zero check` can accept module code that `zero build` cannot emit for the direct Windows backend.

Observed error:

```text
CGEN004: direct wasm return type is unsupported
expected: direct COFF x64 object MVP subset
actual: AgentEvent
```

Cause:

- Returning a user-defined shape from a function reached by the executable path is not supported by the current direct backend.

Current rule:

- Keep `src/main.0` primitive/direct-backend-safe.
- Keep richer Zero core model modules as the durable design surface.
- Exercise richer models through tests/spikes as backend support improves.
- Use the Rust bridge for runtime effects and complex host integration.

Verified commands:

```sh
zero check C:/Users/spenc/GitHub/zero-agent
zero build C:/Users/spenc/GitHub/zero-agent
zero run C:/Users/spenc/GitHub/zero-agent
```

Current executable output:

```text
Zero-Agent core bootstrap ready
```
