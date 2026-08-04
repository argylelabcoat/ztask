# Ingest → Implementation Workflow

A guide for using OpenSpec specs and the ztask-ingest skill to drive autonomous implementation with LLM sub-agents.

## Overview

```
┌─────────────────────────────────────────────────────────────────┐
│  1. Write Spec                                                  │
│     openspec/specs/<feature>/tasks/*.md                         │
│                                                                 │
│  2. Ingest                                                      │
│     ztask-ingest <project> <spec-dir>                           │
│     → creates tasks with dependencies, test files, impl files   │
│                                                                 │
│  3. Orchestrate                                                 │
│     /ztask-orchestrator <project>                               │
│     → spawns sub-agents per task, respects dependencies         │
│                                                                 │
│  4. Verify                                                      │
│     poetry run pytest && cargo test                             │
│     → all tests pass, commit and push                           │
└─────────────────────────────────────────────────────────────────┘
```

## Step 1: Write the Spec

Create a spec directory with task files:

```
openspec/specs/<feature>/
  tasks/
    01-<task-name>.md
    02-<task-name>.md
    03-<task-name>.md
```

### Task File Template

```markdown
# Task: <Title>

## Depends On
- <other-task-id>   (optional)

## Acceptance Criteria
Given <context>,
When <action>,
Then <expected result>.

## Spec
<Implementation details, rationale, constraints>

## Test Files
- tests/unit/test_<module>.py

## Implementation Files
- <path/to/source.py>

## Test Command
poetry run pytest tests/unit/test_<module>.py -v

## Verification Command
<optional: full acceptance test command>
```

### Tips for Good Task Files

1. **One concern per task** — don't bundle unrelated changes
2. **Explicit dependencies** — list task IDs that must complete first
3. **Specific acceptance criteria** — Gherkin format works well
4. **List all files** — test files AND implementation files
5. **Include test command** — sub-agents need to know how to verify

### Example: SDD→TDD Extension

```
openspec/specs/sdd-tdd-extension/
  tasks/
    01-extend-python-model.md    # no dependencies
    02-update-cli.md             # depends on: extend-python-model
    03-update-queries.md         # depends on: extend-python-model
    04-update-web-model.md       # depends on: extend-python-model
```

## Step 2: Ingest the Spec

```bash
# Preview what will be created (dry-run)
ztask-ingest <project-id> <spec-dir> --dry-run

# Create tasks in Zenoh
ztask-ingest <project-id> <spec-dir>
```

### What Ingest Does

1. **Parses** task files — extracts ID, acceptance criteria, dependencies, files, commands
2. **Validates** dependency graph — detects cycles, verifies referenced tasks exist
3. **Topologically sorts** — dependencies come before dependents
4. **Creates tasks** in Zenoh — with all fields populated
5. **Reports** — shows created tasks, dependency graph, any warnings

### Example Output

```
Ingesting OpenSpec from openspec/specs/sdd-tdd-extension/

  Found 4 task(s)
  - extend-python-model: Extend Python Model
  - update-cli: Update CLI Commands
    Depends on: extend-python-model
  - update-queries: Update Queries
    Depends on: extend-python-model
  - update-web-model: Update Web Model
    Depends on: extend-python-model

  Dependency graph:
    extend-python-model (no deps)
    update-cli -> depends on [extend-python-model]
    update-queries -> depends on [extend-python-model]
    update-web-model -> depends on [extend-python-model]

  Creating tasks in project 'sdd-tdd-extension':
    ✓ extend-python-model — PENDING
    ✓ update-cli — PENDING (blocked by: extend-python-model)
    ✓ update-queries — PENDING (blocked by: extend-python-model)
    ✓ update-web-model — PENDING (blocked by: extend-python-model)

  Done. 4 tasks created, 0 skipped, 0 cycles detected.
```

## Step 3: Orchestrate Implementation

### Option A: Manual Sub-Agent Spawning

For each task (respecting dependency order):

```bash
# 1. Claim the task
ztask update-status <task-id> IN_PROGRESS --project <project-id> --note "Starting"

# 2. Spawn sub-agent
# Use actor tool with task details from ztask get <task-id> --project <project-id>

# 3. Wait for completion

# 4. Verify
ztask get <task-id> --project <project-id>
```

### Option B: Orchestrator Skill

```
/ztask-orchestrator <project-id>
```

The orchestrator will:
1. Fetch all incomplete tasks
2. Resolve dependencies
3. Spawn sub-agents for ready tasks
4. Monitor completion
5. Re-evaluate queue after each completion
6. Report summary

### Sub-Agent Prompt Template

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
- Test Command: {TEST_COMMAND}

## Execution Lifecycle

### Phase 1: Claim
Task is already claimed (IN_PROGRESS).

### Phase 2: Assess
Read the acceptance criteria and spec.
- If empty or vague → update status to PENDING with note, STOP
- If clear → proceed to Phase 3

### Phase 3: Execute (TDD)
1. Write tests in {TEST_FILES}
2. Run: {TEST_COMMAND}
3. Confirm tests fail (red)
4. Implement in {IMPLEMENTATION_FILES}
5. Run: {TEST_COMMAND}
6. Fix until tests pass (green)

### Phase 4: Finalize
IF successful:
  ztask update-status {TASK_ID} COMPLETED --project {PROJECT_ID} --note "Passed all tests"

IF blocked:
  ztask update-status {TASK_ID} PENDING --project {PROJECT_ID} --note "Blocked: <reason>"

## Rules
- You own ONLY this task
- Stay within your assigned files
- Don't mark COMPLETED unless tests pass
```

## Step 4: Verify and Commit

```bash
# Run all tests
poetry run pytest tests/unit/ -v
cd web && cargo test --lib

# Check task status
ztask list --project <project-id> --filter incomplete

# Commit
git add -A && git commit -m "feat: implement <feature>"

# Push
git push origin main
```

## Real-World Example: SDD→TDD Extension

### What We Did

1. **Spec** — Created `openspec/specs/sdd-tdd-extension/tasks/` with 4 task files
2. **Ingest** — `ztask-ingest sdd-tdd-extension openspec/specs/sdd-tdd-extension`
3. **Implement** — Spawned MiMo-V2.5 sub-agents for each task
4. **Verify** — 51 Python tests + 76 Rust tests pass
5. **Commit** — Pushed to `origin/main`

### What We Learned

1. **Specs are source of truth** — the spec drove implementation, not the other way around
2. **Dependencies work** — topological sort ensured correct execution order
3. **Sub-agents are effective** — each agent focused on one task, stayed scoped
4. **TDD is natural** — sub-agents wrote tests first, then implemented
5. **Existing code is fine** — sub-agents found that some code already existed and focused on test coverage

### Timing

- Spec writing: ~10 minutes
- Ingest: ~5 seconds
- Implementation (4 sub-agents): ~5 minutes
- Verification: ~30 seconds
- Commit/push: ~10 seconds

**Total: ~16 minutes from spec to merged code**

## Advanced Patterns

### Update Specs

For incremental changes to existing code:

```markdown
## 1. Extend Python Model

### Acceptance Criteria
- Task dataclass has all new fields

### Test Files
- tests/unit/test_models.py

### Implementation Files
- ztask/models.py
```

The sub-agent will check if code already exists and focus on test coverage.

### Greenfield Specs

For new features from scratch:

```markdown
## 1. Create Auth Module

### Acceptance Criteria
- Auth module exists with login/logout functions
- JWT tokens are generated and validated

### Test Files
- tests/test_auth.py

### Implementation Files
- src/auth/__init__.py
- src/auth/jwt.py
```

### Parallel Tasks

Tasks without dependencies run in parallel:

```
Task A (no deps)  ─┐
Task B (no deps)  ─┤─→ all run simultaneously
Task C (no deps)  ─┘
```

### Sequential Tasks

Tasks with dependencies run in order:

```
Task A (no deps) → Task B (depends on A) → Task C (depends on B)
```

### Mixed Dependencies

```
Task A (no deps)  ─┐
                    ├─→ Task D (depends on A, B, C)
Task B (no deps)  ─┤
                    │
Task C (no deps)  ─┘
```

## Troubleshooting

### Task Not Found After Ingest

The Zenoh router must be running. Check:
```bash
ps aux | grep zenoh
```

### Sub-Agent Can't Update Status

The task must exist in Zenoh. Verify:
```bash
ztask get <task-id> --project <project-id>
```

### Tests Fail After Implementation

Check if the sub-agent stayed scoped:
```bash
git diff --stat
```

Should only show changes to the assigned files.

### Dependency Cycle Detected

Review your task files. Common issues:
- Circular depends_on references
- Typo in task ID
- Missing task file

## Reference

| Command | Purpose |
|---------|---------|
| `ztask-ingest <project> <spec> --dry-run` | Preview tasks |
| `ztask-ingest <project> <spec>` | Create tasks |
| `ztask list --project <id>` | List tasks |
| `ztask get <task-id> --project <id>` | Task details |
| `ztask update-status <task-id> <status> --project <id>` | Update status |
| `/ztask-orchestrator <project>` | Auto-orchestrate |
| `/ztask-status <project>` | Project dashboard |
