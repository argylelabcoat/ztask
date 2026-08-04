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
- `attempt_count` — number of previous attempts (auto-incremented by `update-status` on entry to a WIP status)
- `failure_reason` — why it last failed (if set; note: the CLI never writes this field — check the most recent `history` entry's `note` for failure details)
- `tdd_phase` — red/green/refactor/None (note: the CLI never writes this field — infer from the most recent `history` note)

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

For each task in the **ready queue**, spawn a sub-agent. The mechanism
depends on the host agent platform:

- **OpenCode:** use the `task` tool with `subagent_type: "general"` for
  implementation tasks, or `subagent_type: "explore"` for
  investigation-only tasks.
- **MiMoCode:** use the `actor` tool.
- **Claude Code:** use the `Task` tool with `subagent_type: "general"`.

Use this prompt template (fill `{...}` placeholders from the task's
fields):

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
- Attempt: {ATTEMPT_COUNT}
- Previous Failure: {FAILURE_REASON}

## Execution Lifecycle

### Phase 1: Claim
Update status to IN_PROGRESS. The CLI auto-increments attempt_count
on this transition — do NOT increment it yourself.
  ztask update-status {TASK_ID} IN_PROGRESS --project {PROJECT_ID} --note "Attempt {N}: starting from phase {TDD_PHASE}"

### Phase 2: Assess
Read the acceptance criteria and spec.
- If criteria are empty or vague → update status back to PENDING with note explaining what's missing, STOP
- If TDD phase is "red" → skip to Phase 3b (tests exist, implement)
- If TDD phase is "green" → skip to Phase 3c (implementing, verify and finalize)
- If TDD phase is null → start from Phase 3a

> The CLI does not persist `tdd_phase` — encode the phase in the
> `--note` of each update so it can be recovered from history.

### Phase 3a: Write Tests (TDD Red)
- Write test files that encode the acceptance criteria
- Run: {TEST_COMMAND}
- Confirm tests fail as expected
- Update note: "red: tests written, moving to green phase"

### Phase 3b: Implement (TDD Green)
- Implement minimal code in {IMPLEMENTATION_FILES}
- Run: {TEST_COMMAND}
- If tests pass → move to Phase 3c
- If tests fail → fix and re-run (max 5 iterations, then update status to PENDING with note "Blocked: tests failing after 5 attempts" and STOP)

### Phase 3c: Verify (TDD Refactor)
- Run: {VERIFICATION_COMMAND}
- If verification passes → move to Phase 4
- If verification fails → fix, or update status to PENDING with note "Blocked: verification failed: <details>" and STOP

### Phase 4: Finalize
IF successful:
  ztask update-status {TASK_ID} COMPLETED --project {PROJECT_ID} --note "Completed: <summary>"

IF blocked or failed:
  ztask update-status {TASK_ID} PENDING --project {PROJECT_ID} --note "Blocked: <reason>"
  (Put the failure detail in the note — the CLI does not auto-populate
   the `failure_reason` field.)

## Rules
- You own ONLY this task. Do not touch other tasks.
- Update status BEFORE and AFTER your work.
- If you cannot determine what to do, fail fast with a clear note.
- Do not mark COMPLETED unless acceptance criteria are demonstrably met.
- Stay within your assigned files: {TEST_FILES} and {IMPLEMENTATION_FILES}
- There is no `BLOCKED` status — use PENDING with a blocking note.
```

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

### Step 7: Post-completion spec management

After all tasks complete (or at key milestones), run spec management:

#### 7a: Merge completed update specs

If the project was an update spec (e.g., `sdd-tdd-extension`), merge changes into the feature spec:

```
/ztask-spec-merge <feature-name> <update-spec-path>
```

This will:
- Extract implementation details from completed tasks
- Update the feature spec with new content
- Add changelog entry
- Convert acceptance criteria to Gherkin format
- Identify BDD testing opportunities

#### 7b: Organize specs

Run spec organization to ensure clean structure:

```
/ztask-spec-organize audit
```

This will:
- Check directory structure
- Validate naming conventions
- Update cross-references
- Report any issues

#### 7c: Archive completed specs

If the update spec is fully implemented, archive it:

```
/ztask-spec-organize archive
```

This will:
- Move completed update specs to `openspec/archive/`
- Create archive index
- Clean up the specs directory

### Step 8: Report

After all tasks reach a terminal state or the queue is stuck:
- Summarize: N completed, M blocked, K failed
- For blocked/failed tasks, show `failure_reason` and `attempt_count`
- If any failed 2+ times, ask the user: retry, skip, or intervene
- If all completed, report project done
- Report spec management actions taken

## Concurrency Notes

- Sub-agents may run in parallel depending on the host platform's
  sub-agent tool (OpenCode `task`, MiMoCode `actor`, Claude `Task`).
- **No status locking:** `ztask update-status` does not check the
  current status before writing — the last write wins. Two workers
  racing on the same task ID will both succeed and clobber each other.
  The orchestrator must prevent this by never spawning two workers
  for the same task ID in the same batch.
- Dependency resolution prevents spawning tasks with unmet dependencies.

## Intervention Triggers

Stop and ask the user when:
- A task fails 2+ times consecutively (`attempt_count >= 2`)
- A sub-agent reports a dependency on another task that isn't complete (circular or missing)
- The acceptance criteria reference external systems not available
- More than 50% of tasks are failing (systemic issue)
- A task has been IN_PROGRESS for > 24 hours (stalled)
- Spec merge/organize reports errors

## TDD Phase Tracking

The orchestrator infers the current TDD phase for a task from:

1. The task's `tdd_phase` field — **if set**. Note: the `ztask` CLI
   never writes this field (only `create` and `update-status` exist,
   and neither sets `tdd_phase`), so it will be `null` for tasks
   created via the CLI. It may be populated by direct Zenoh writes
   from other tools.
2. Otherwise, the most recent `history` entry's `note` — if it
   contains a phase marker like `"red: ..."`, `"green: ..."`, or
   `"refactor: ..."`. This is the recommended way to track phase,
   since workers encode their phase in `update-status --note`.

When spawning a worker, the orchestrator passes the inferred phase so
the worker can resume mid-cycle instead of starting over.

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

## Spec Management Integration

The orchestrator integrates with spec management skills to maintain clean specifications:

### After each task completes
- No immediate action (tasks are independent)

### After all tasks in a batch complete
- Run `/ztask-spec-organize audit` to check organization
- Report any issues found

### After all tasks in a project complete
- Run `/ztask-spec-merge` to merge update spec into feature spec
- Run `/ztask-spec-organize archive` to archive completed specs
- Report spec management actions

### Spec management error handling
- If spec merge fails: warn but don't block project completion
- If spec organize fails: warn but don't block project completion
- If archive fails: warn but don't block project completion
- Report all spec management issues in final summary

## Example: Full Workflow

```
1. User: /ztask-orchestrator sdd-tdd-extension

2. Orchestrator:
   - Fetches 4 tasks from sdd-tdd-extension project
   - Resolves dependencies: extend-python-model first, then 3 parallel
   - Spawns sub-agent for extend-python-model
   - Waits for completion
   - Spawns 3 sub-agents in parallel (update-cli, update-queries, update-web-model)
   - Waits for all to complete
   - All 4 tasks COMPLETED

3. Post-completion:
   - Runs /ztask-spec-merge task-model openspec/specs/sdd-tdd-extension/
   - Merges changes into openspec/specs/cli/task-model.md
   - Adds changelog entry
   - Runs /ztask-spec-organize audit
   - Reports: "Spec organization: ✓ All checks pass"
   - Runs /ztask-spec-organize archive
   - Archives sdd-tdd-extension/ to openspec/archive/2026-08-04-sdd-tdd-extension/

4. Final report:
   "Project sdd-tdd-extension complete.
    4 tasks completed, 0 blocked, 0 failed.
    Spec merged into cli/task-model.md.
    Update spec archived."
```
