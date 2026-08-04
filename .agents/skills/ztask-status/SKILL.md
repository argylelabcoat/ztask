---
description: >
  Show ztask CLI reference and project status overview.
  Use when: "task status", "project overview", "what's left", "show tasks",
  or any request to inspect the current state of a Zenoh project.
---

# ztask-status — Project Dashboard

Quick overview of a Zenoh project's task state.

## Usage

### 1. Get project ID

If not specified, ask the user or check the current project context.

### 2. Fetch all tasks

```bash
ztask list --project <PROJECT_ID> --filter all
```

### 3. Summarize

Group tasks by status and present:

```
Project: <PROJECT_ID>
─────────────────────────
  PENDING:      N tasks
  IN_PROGRESS:  N tasks
  COMPLETED:    N tasks
  OTHER:        N tasks
─────────────────────────
  Total:        N tasks
```

### 4. Detail open tasks

For each non-COMPLETED task, show:
- Task ID
- Status
- Acceptance criteria (truncated to 100 chars)
- Time in current status (computed from `time_accepted` or `time_entered`)
- Last note from history

### 5. Flag issues

Highlight:
- Tasks IN_PROGRESS for > 24 hours (potentially stalled)
- Tasks with empty acceptance criteria
- Tasks that have failed and returned to PENDING multiple times

## CLI Reference

```
ztask list   --project <id> [--filter all|incomplete|wip]
ztask get    <task-id> --project <id>
ztask create <task-id> --project <id> [--criteria "..."] [--entered-by llm|user]
ztask update-status <task-id> <status> --project <id> [--note "..."]
```

Environment: `ZTASK_ZENOH_ENDPOINT` (default: `tcp/localhost:7447`)
