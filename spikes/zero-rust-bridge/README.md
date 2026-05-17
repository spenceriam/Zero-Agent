# Zero/Rust Bridge Spike

## Decision

Use Rust as the temporary runtime bridge for capabilities that Zero cannot reliably provide yet.

## Initial protocol

Use JSON-over-stdio first because it is easy for Zero to model and easy to replace later.

Request shape:

```json
{
  "id": "request-id",
  "op": "http.request",
  "input": {}
}
```

Response shape:

```json
{
  "id": "request-id",
  "ok": true,
  "event": "done",
  "output": {}
}
```

Streaming response shape:

```json
{
  "id": "request-id",
  "ok": true,
  "event": "chunk",
  "output": {}
}
```

## Bridge operations for v0.1

- `http.request`
- `http.stream_sse`
- `process.spawn`
- `process.write_stdin`
- `process.cancel`
- `terminal.start_tui`
- `telegram.poll`
- `telegram.send`
- `models.discover`

## Rules

- Keep operations narrow.
- Zero owns policy and state.
- Rust executes runtime effects only.
- No hidden provider logic in Rust unless transport requires it.
- Every bridge op must have a documented schema and test fixture.
