# Rust Bridge Placeholder

This bridge exists only for runtime APIs that Zero cannot support directly yet.

Candidate bridge responsibilities:

- HTTPS and SSE streaming
- subprocess streaming
- PTY/raw terminal support
- Telegram polling/webhooks
- SQLite or other embedded persistence if JSONL is insufficient
- OS-specific shell integration

Rule: every bridge API must be narrow, documented, and replaceable by Zero-native code later.
