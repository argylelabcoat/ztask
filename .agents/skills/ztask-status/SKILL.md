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
- TDD Phase (red/green/refactor/—)
- Acceptance criteria (truncated to 100 chars)
- Dependencies (depends_on list, met/unmet)
- Time in current status (computed from `time_accepted` when IN_PROGRESS/WIP/RUNNING — set by `update-status` on entry to a WIP status — otherwise from `time_entered`)
- Attempt count (if > 0)
- Failure reason (if set and status is PENDING)
- Last note from history

### 5. Flag issues

Highlight:
- Tasks IN_PROGRESS for > 24 hours (potentially stalled)
- Tasks with empty acceptance criteria
- Tasks that have failed and returned to PENDING multiple times (`attempt_count >= 2`)
- Tasks blocked by unmet dependencies

### 6. Dependency graph (optional)

If any tasks have `depends_on` set, show the dependency graph:

```
Dependency Graph:
  db-migrations (no deps) ✓ COMPLETED
  auth-login → depends on [db-migrations] ✓ COMPLETED
  auth-refresh → depends on [auth-login] ● IN_PROGRESS (green)
  auth-logout → depends on [auth-login] ○ PENDING (blocked)
```

## CLI Reference

```
ztask list   --project <id> [--filter all|incomplete|wip]
ztask get    <task-id> --project <id>
ztask create <task-id> --project <id> [--criteria "..."] [--spec "..."] [--depends-on "..."] [--entered-by llm|user]
ztask update-status <task-id> <status> --project <id> [--note "..."]
```

> The CLI only supports the filters `all`, `incomplete`, and `wip`
> (see `ztask/cli.py`). There is no `blocked` filter — compute
> blocked tasks client-side from `depends_on` vs. the set of
> `COMPLETED` task IDs.

Environment: `ZTASK_ZENOH_ENDPOINT` (default: `tcp/localhost:7447`)

## New Fields (SDD→TDD)

When displaying tasks, show these fields if present:

| Field | Display | Notes |
|-------|---------|-------|
| `spec` | Collapsible section | Full text, truncated to 200 chars in list view |
| `depends_on` | "Blocked by: [task-1, task-2]" | Show with met/unmet status |
| `blocks` | "Blocks: [task-3, task-4]" | Show count |
| `test_files` | List of paths | Clickable in web UI |
| `implementation_files` | List of paths | Clickable in web UI |
| `tdd_phase` | Badge: red/green/refactor/— | Color-coded |
| `test_command` | Code block | For reference |
| `verification_command` | Code block | For reference |
| `failure_reason` | Warning banner | Show when status is PENDING |
| `attempt_count` | Count badge | Warning style if >= 2 |
