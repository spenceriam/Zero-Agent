# Zero-Agent Coverage of vercel-labs/zero Issues and PRs

## Addressed by Zero-Agent (Open Issues)

### #4: AI-native primitives for agent-oriented language

Zero-Agent implements this at the application level:

- `src/providers/registry.0` - provider interface with capability metadata
- `src/providers/anthropic.0`, `openai.0` - model call primitives
- `src/tools/registry.0` - typed tool input/output with risk classification
- `src/core/policy.0` - approval boundaries (Safe/Mutating/Destructive/Blocked)
- `src/core/memory.0` - context/memory with persistence
- `src/core/session.0` - conversation context with budget tracking

### #5: generateText primitive

Zero-Agent's provider layer implements the generateText pattern:

- `src/providers/anthropic.0` - `buildRequest()` with model, messages, tools, stream
- `src/providers/openai.0` - `buildRequest()` with same interface
- `src/providers/openrouter.0`, `ollama.0` - OpenAI-compatible adapters

### #7: Normalize capability and effect summary

Zero-Agent's tool registry provides capability metadata:

- `src/tools/registry.0` - `Tool` shape with name, description, risk, input_schema
- `src/providers/registry.0` - `ProviderCapability` with streaming, tool_calling, thinking_events, model_discovery
- `src/core/policy.0` - `decisionForRisk()` maps risk levels to permission decisions

### #8: Agent-oriented benchmarks

Zero-Agent's stub provider and bridge test suite serve as agent-oriented benchmarks:

- `src/providers/registry.0` - `stubStreamResponse()`, `stubToolCall()`
- Bridge has 6 unit tests covering JSON parsing, config validation, unicode escapes

## Closed Issues (Resolved Upstream)

- **#31: Type checker accepts invalid minimal programs** - Fixed by PR #32 (1929 additions, conformance fixtures)
- **#36: Project license** - Resolved with Apache-2.0 (PR #37)
- **#38: Private vulnerability reporting** - Closed by maintainer

## Merged PRs (Recent Updates)

- **#32: Fix issue 31 checker regressions** - Major type checker fixes (missing returns, fallibility, borrow escapes, duplicate declarations)
- **#37: Add Apache-2.0 license** - Project now licensed
- **#35: Show Docs link in header on mobile** - Docs UX
- **#18: Fix dynamic cli strings** - CLI improvements
- **#17: Add toggle to homepage** - Website
- **#16: v0.1.1** - Release bump (Zero-Agent targets this)
- **#14: Improve bundled skills** - Skills system improvements
- **#13: Fix docs prose** - Docs cleanup
- **#11: `zero run` subcommand** - Zero-Agent uses this for execution

## Open PRs to Watch

### Directly relevant to Zero-Agent:

- **#39: New provenance model** (ctate, 2453 additions, reviewed by vercel) - Borrow system overhaul; may require Zero-Agent reference pattern updates
- **#42: NAM003 explain + catalog drift guard** (GodOnlyKn0w, supersedes #40) - Fixes `zero explain` for NAM003, adds coverage guard so diagnostic codes must have explain entries
- **#22: `zero new --json` envelope** (mkitsugi) - Adds JSON output to `zero new` with kind, name, path, manifest, entry, nextSteps
- **#9: `zero fmt --write` and `--check --json`** (mvanhorn) - Adds `--write` for in-place formatting, `--check --json` for structured diff output
- **#3: Target capability contract tests** (EfeDurmaz16) - Asserts host capabilities include process/runtime surface; unavailable caps reported with `available: false`
- **#2: Diagnostic catalog as JSON** (EfeDurmaz16) - `zero explain --json --all` for machine-readable diagnostic catalog with repair IDs and safety labels
- **#1: Fix-plan safety contract tests** (EfeDurmaz16) - CLI coverage for fix-plan safety metadata, covers TYP009 repair contract

### Platform/infra:

- **#30/#29: macOS Mach-O fixes** - Darwin runtime fixes (closes #28)
- **#26: Zed extension support** - Editor tooling
- **#24: Self-host target support check** - Build system
- **#23: Signal exit code portability** - Cross-platform

## Open Issues Not Addressed

These are upstream Zero language issues:

- **#6: Structured edit previews** - Zero compiler feature
- **#19: Lambda syntax** - Language feature request
- **#20: Structured concurrency** - Language feature request
- **#25: Zed extension** - Editor tooling
- **#28: Darwin dyld LC_UUID** - Runtime bug (PRs #29, #30 pending)

## Impact on Zero-Agent

- **PR #32** (type checker fixes) - Zero-Agent's `.0` files are now validated more strictly; verified build still passes
- **PR #39** (provenance model) - Borrow system overhaul may require updates to reference patterns in Zero code
- **PR #42** (NAM003 explain) - Diagnostic catalog drift guard ensures `zero explain` stays in sync with `zero check`
- **PRs #22, #9** (JSON envelopes) - `zero new --json` and `zero fmt --json` add structured output Zero-Agent could consume
- **PRs #3, #2, #1** (contract tests) - Target capabilities, diagnostic catalog, and fix-plan safety become machine-readable
- **Issue #7** (capability summary) - Partially addressed by PR #3's capability facts tests

## Action Items

- Monitor PR #39 (provenance model) for borrow pattern changes affecting Zero code
- Consume `zero explain --json` and `zero fix --plan --json` when PRs #2, #42 merge
- Use `zero fmt --check --json` for CI linting when PR #9 merges
- Reference Zero-Agent's architecture on issues #4, #5 if upstream adopts language-level AI primitives
