# Open Questions

## Zero capability questions

1. Can Zero perform HTTPS requests with TLS today without a bridge?
2. Can Zero parse streaming SSE responses?
3. Can Zero spawn a subprocess with streaming stdout/stderr?
4. Can Zero write to subprocess stdin?
5. Can Zero implement raw terminal mode for split-pane TUI?
6. Can Zero handle Telegram polling directly?
7. Can Zero build usable artifacts for Windows/macOS/Linux today?
8. What is the cleanest Zero-to-Rust bridge boundary?

## Product questions

1. Should Anthropic or OpenAI be the first default provider?
2. Should Telegram actions that mutate files require an extra confirmation compared with TUI actions?
3. Should local memory be opt-in per fact, or should the agent proactively remember with notification?
4. Should extension creation be enabled by default in Telegram, or TUI-only at first?
5. Should background jobs be allowed to run shell tools unattended?
6. Should model discovery cache results locally?
7. Should provider API keys be stored only as env var references in v0.1?

## UX questions

1. Which TUI layout should be the first implementation: primary split-pane or workbench layout?
2. Should streaming in Telegram edit a single message or append chunks?
3. Should slash commands be identical between TUI and Telegram?
4. What approval levels should exist beyond approve once/session/deny?

## Extension ecosystem questions

1. What are trusted extension sources?
2. Should third-party extension install be disabled until signing/review exists?
3. Should extensions be Zero-only, or allow Rust/shell/PowerShell/Python scripts?
4. How should the agent sandbox untrusted extensions?

## Repository questions

1. Should the mistakenly-created `spenceriam/zer0cli` repo be deleted later?
2. Should this repo push the initial spec commit now or wait until scaffold review?
