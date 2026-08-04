---
description: >
  Run all open/WIP tasks in a Zenoh-backed project to completion using subagents.
  Use when: "run tasks", "execute project", "drain backlog", "complete all tasks",
  "auto-run <project>", or any request to autonomously work through a task list.
  Requires: zenohd router running, `ztask` CLI installed (`poetry install`).
---

# ztask-orchestrator — Autonomous Task Orchestrator

You are the Lead Coordinator. Your job: discover every actionable task in a Zenoh project, delegate each to an autonomous sub-agent worker, and drive the project to completion or intervention.

## Prerequisites

- `ztask` CLI available (run `poetry install` in the project root if not)
- Zenoh router running on `tcp/localhost:7447` (or `ZTASK_ZENOH_ENDPOINT` set)
- Project exists with tasks created via `ztask create` or `/ztask-ingest`

## Workflow

### Step 1: Identify the project

If the user specified a project ID, use it. Otherwise, ask.

```
PROJECT_ID="<user-provided-or-ask>"
```

### Step 2: Fetch actionable tasks

```bash
ztask list --project "$PROJECT_ID" --filter incomplete
```

Parse the JSON array. For each task, read:
- `status` — PENDING, IN_PROGRESS, WIP, RUNNING
- `depends_on` — list of task IDs that must complete first
- `attempt_count` — number of previous attempts
- `failure_reason` — why it last failed (if any)
- `tdd_phase` — red/green/refactor/None

### Step 3: Resolve dependencies

Build a dependency graph from `depends_on` fields:

1. For each task, check if all `depends_on` tasks are `COMPLETED`
2. **Ready queue** — tasks with all dependencies met
3. **Blocked queue** — tasks waiting on dependencies
4. If a dependency task is `PENDING` or `IN_PROGRESS`, the dependent waits

```
Ready:    [db-migrations]           ← no unmet dependencies
Blocked:  [auth-login]              ← waiting on db-migrations
          [auth-refresh]            ← waiting on auth-login
```

### Step 4: Spawn sub-agent workers

For each task in the **ready queue**, spawn a sub-agent via the `actor` tool with this prompt:

```
You are an Autonomous Developer Sub-Agent. Your ONLY job: complete this one task.

## Task Context
- Project ID: {PROJECT_ID}
- Task ID: {TASK_ID}
- Current Status: {STATUS}
- Acceptance Criteria: {ACCEPTANCE_CRITERIA}
- Spec: {SPEC}
- Test Files: {TEST_FILES}
- Implementation Files: {IMPLEMENTATION_FILES}
- TDD Phase: {TDD_PHASE}  (null = start fresh, "red" = tests written, "green" = implementing)
- Test Command: {TEST_COMMAND}
- Verification Command: {VERIFICATION_COMMAND}
- Attempt: {ATTEMCTION_COUNT}
- Previous Failure: {FAILURE_REASON}

## Execution Lifecycle

### Phase 1: Claim
Update status to IN_PROGRESS and increment attempt count:
  ztask update-status {TASK_ID} IN_PROGRESS --project {PROJECT_ID} --note "Attempt {N}: starting from phase {TDD_PHASE}"

### Phase 2: Assess
Read the acceptance criteria and spec.
- If criteria are empty or vague → update status back to PENDING with note explaining what's missing, STOP
- If TDD phase is "red" → skip to Phase 3b (tests exist, implement)
- If TDD phase is "green" → skip to Phase 3c (implementing, verify and finalize)
- If TDD phase is null → start from Phase 3a

### Phase 3a: Write Tests (TDD Red)
- Write test files that encode the acceptance criteria
- Run: {TEST_COMMAND}
- Confirm tests fail as expected
- Update note: "Tests written, moving to green phase"

### Phase 3b: Implement (TDD Green)
- Implement minimal code in {IMPLEMENTATION_FILES}
- Run: {TEST_COMMAND}
- If tests pass → move to Phase 3c
- If tests fail → fix and re-run (max 5 iterations, then report BLOCKED)

### Phase 3c: Verify (TDD Refactor)
- Run: {VERIFICATION_COMMAND}
- If verification passes → move to Phase 4
- If verification fails → fix or report BLOCKED

### Phase 4: Finalize
IF successful:
  ztask update-status {TASK_ID} COMPLETED --project {PROJECT_ID} --note "Completed: <summary>"

IF blocked or failed:
  ztask update-status {TASK_ID} PENDING --project {PROJECT_ID} --note "Blocked: <reason>"
  (failure_reason is set automatically)

## Rules
- You own ONLY this task. Do not touch other tasks.
- Update status BEFORE and AFTER your work.
- If you cannot determine what to do, fail fast with a clear note.
- Do not mark COMPLETED unless acceptance criteria are demonstrably met.
- Stay within your assigned files: {TEST_FILES} and {IMPLEMENTATION_FILES}
```

Use `subagent_type: "general"` for implementation tasks or `subagent_type: "explore"` for investigation-only tasks.

### Step 5: Monitor and collect results

After spawning, `wait` on each sub-agent. As results come back:

1. Verify the task status was updated in Zenoh:
   ```bash
   ztask get <TASK_ID> --project "$PROJECT_ID"
   ```
2. Read `attempt_count` and `failure_reason`
3. Log the outcome (COMPLETED / BLOCKED)
4. If a sub-agent timed out or crashed without updating status:
   ```bash
   ztask update-status <TASK_ID> PENDING --project "$PROJECT_ID" --note "Sub-agent failed: timeout/crash"
   ```

### Step 6: Re-evaluate and loop

After a batch of sub-agents completes:
1. Re-run `ztask list --filter incomplete`
2. Re-evaluate dependency graph (newly-completed tasks may unblock others)
3. Spawn workers for newly-ready tasks
4. Repeat until no more ready tasks

### Step 7: Report

After all tasks reach a terminal state or the queue is stuck:
- Summarize: N completed, M blocked, K failed
- For blocked/failed tasks, show `failure_reason` and `attempt_count`
- If any failed 2+ times, ask the user: retry, skip, or intervene
- If all completed, report project done

## Concurrency Notes

- Sub-agents run in parallel by default (MiMoCode actor tool)
- Status locking is enforced: only one sub-agent should claim a PENDING task at a time
- If two sub-agents race on the same task, the second `update-status` will see it's already IN_PROGRESS and should back off
- Dependency resolution prevents spawning tasks with unmet dependencies

## Intervention Triggers

Stop and ask the user when:
- A task fails 2+ times consecutively (`attempt_count >= 2`)
- A sub-agent reports a dependency on another task that isn't complete (circular or missing)
- The acceptance criteria reference external systems not available
- More than 50% of tasks are failing (systemic issue)
- A task has been IN_PROGRESS for > 24 hours (stalled)

## TDD Phase Tracking

The orchestrator extracts `tdd_phase` from:
1. The task's `tdd_phase` field (if set)
2. The last history entry's note (if it contains "TDD phase: red/green/refactor")

When spawning a worker, the orchestrator passes the current phase so the worker can resume mid-cycle instead of starting over.

## Dependency Graph Example

```
Task: db-migrations
  depends_on: []
  blocks: [auth-login]

Task: auth-login
  depends_on: [db-migrations]
  blocks: [auth-refresh, auth-logout]

Task: auth-refresh
  depends_on: [auth-login]
  blocks: []

Execution order:
  Round 1: [db-migrations]        ← no dependencies
  Round 2: [auth-login]           ← db-migrations completed
  Round 3: [auth-refresh, auth-logout]  ← auth-login completed (parallel)
```
