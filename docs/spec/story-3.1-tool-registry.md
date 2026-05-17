# BMAD Story 3.1: Tool Registry

## Status

Implemented as a runtime slice.

## Tool model

`src/tools/registry.0` defines:

- `Tool` shape: name, description, risk, input_schema
- `ToolRegistry` shape: tools (list placeholder)
- `needsApproval(tool)` - checks if tool requires user approval based on risk level

## Registered tools

- `readFileTool()` - Safe risk, reads local files
- `writeFileTool()` - Mutating risk, writes content to files
- `editFileTool()` - Mutating risk, search and replace in files
- `shellTool()` - Mutating risk, runs shell commands
- `globTool()` - Safe risk, finds files by pattern
- `grepTool()` - Safe risk, searches file contents

## Risk classification

Tools use `RiskLevel` from `src/core/types.0`:

- Safe: read-only operations (read_file, glob, grep)
- Mutating: write operations that require approval (write_file, edit_file, shell)
- Destructive: dangerous operations requiring explicit confirmation
- Blocked: never allowed

Permission decisions come from `src/core/policy.0`:

- Safe -> Allow (no prompt)
- Mutating -> Ask (requires approval)
- Destructive -> Ask (requires approval)
- Blocked -> Deny (never allowed)

## Acceptance evidence

Commands:

```sh
export PATH="/c/Users/spenc/.zero/bin:$PATH"
zero build src/main.0
zero run src/main.0
```

## Remaining work

- Wire tool registry into agent loop for dynamic tool dispatch.
- Add tool execution via bridge operations.
- Add tool result formatting.
