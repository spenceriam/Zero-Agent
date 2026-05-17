# BMAD Stories 6.1-6.2: Jobs and Sub-Agents

## Status

Implemented as model layers.

## Job runner (Story 6.1)

`src/core/jobs.0` defines:

- `JobStatus` enum: Queued, Running, Completed, Failed, Cancelled
- `Job` shape: id, prompt, status, enabled, session_id, result
- Constructor helpers: newJob, runningJob, completedJob, failedJob

## Sub-agent isolation (Story 6.2)

`src/core/subagents.0` defines:

- `SubAgentStatus` enum: Queued, Running, Completed, Failed
- `SubAgentRun` shape: id, task, status, result, parent_session
- Constructor helpers: queuedSubAgent, runningSubAgent, completedSubAgent, failedSubAgent

## Design

Jobs are background tasks that run independently. Sub-agents are isolated task runners that report back to the parent session.

- Job lifecycle: Queued -> Running -> Completed/Failed/Cancelled
- Sub-agent lifecycle: Queued -> Running -> Completed/Failed

## Acceptance evidence

Commands:

```sh
export PATH="/c/Users/spenc/.zero/bin:$PATH"
zero build src/main.0
zero run src/main.0
```

## Remaining work

- Wire job runner into agent loop for background task execution.
- Add job persistence (JSONL like sessions).
- Add sub-agent process isolation via bridge.
- Add job/sub-agent status reporting to UI.
