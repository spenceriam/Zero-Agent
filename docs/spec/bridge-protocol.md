# Bridge Protocol

Zero-Agent talks to the Rust bridge over JSON lines on stdio.

## Common request

```json
{
  "id": "request-id",
  "op": "operation.name"
}
```

v0.1 stubs also accept flat string fields such as `provider`.

## Common response

```json
{
  "id": "request-id",
  "ok": true,
  "event": "event-name",
  "output": {}
}
```

Error response:

```json
{
  "id": "request-id",
  "ok": false,
  "event": "error",
  "output": {},
  "error": "message"
}
```

## Current operations

### `ping`

Returns bridge readiness.

### `models.discover`

Stub operation that returns an empty model list and the requested provider.

### `http.request`

Reserved for provider and web requests. Not implemented.

### `http.stream_sse`

Reserved for streaming provider responses. Not implemented.

### `process.spawn`

Reserved for shell/tool execution. Not implemented.

### `telegram.poll`

Reserved for Telegram polling. Not implemented.

### `telegram.send`

Reserved for Telegram responses. Not implemented.
