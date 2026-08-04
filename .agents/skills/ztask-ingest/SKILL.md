---
description: >
  Convert an OpenSpec SDD directory into a dependency-ordered task graph in Zenoh.
  Use when: "ingest specs", "load tasks from spec", "import spec", "create tasks from OpenSpec",
  "run tasks from spec", or any request to convert a specification directory into executable tasks.
  Requires: zenohd router running, `ztask` CLI installed (`poetry install`).
---

# ztask-ingest — OpenSpec to Task Graph

You are a Specification Ingestion Agent. Your job: read an OpenSpec SDD directory, parse task files, validate dependencies, and create a task graph in Zenoh.

## Invocation

```
/ztask-ingest <project-id> <spec-path>
```

**Examples:**
```
/ztask-ingest myapp ./openspec/specs/myapp/
/ztask-ingest ztask ./openspec/specs/cli/task-model.md
```

## Input Modes

### Mode 1: Greenfield (directory with tasks/)

```
<spec-directory>/
  spec.md                    # top-level design spec (optional)
  tasks/
    01-db-migrations.md      # task spec
    02-auth-login.md
    03-auth-refresh.md
```

### Mode 2: Update (single spec file)

A single markdown spec file with numbered sections that can be extracted as tasks:

```markdown
# SDD→TDD Task Model Extension

## 1. Extend Python Model

Add new fields to the Task dataclass...

### Acceptance Criteria
- Task dataclass has all new fields
- to_dict() includes non-empty new fields

### Test Files
- tests/unit/test_models.py

### Implementation Files
- ztask/models.py

## 2. Update CLI Commands

Add new flags to create command...

### Acceptance Criteria
- ztask create accepts --spec, --depends-on, etc.
- Invalid input returns error

### Test Files
- tests/unit/test_cli.py

### Implementation Files
- ztask/cli.py
```

## Task File Format

Each task file uses markdown with structured sections:

```markdown
# Task: Auth Refresh

## Depends On
- auth-login

## Acceptance Criteria
Given an expired access token and valid refresh token,
When the client POSTs to /auth/refresh,
Then a new access token is returned and the old refresh token is invalidated.

## Spec
Implement JWT refresh token rotation with Redis-backed session store.
Rate-limit: 10 requests/minute per user.

## Test Files
- tests/test_auth_refresh.py
- tests/test_token_rotation.py

## Implementation Files
- ztask/auth/refresh.py
- ztask/auth/tokens.py

## Test Command
poetry run pytest tests/test_auth_refresh.py -v

## Verification Command
poetry run pytest tests/acceptance/test_token_refresh_acceptance.py
```

### Section Reference

| Section | Required | Maps to Task Field | Notes |
|---------|----------|-------------------|-------|
| `# Task: <name>` | Yes | `id` (derived from filename slug) | Display name; ID comes from filename |
| `## Depends On` | No | `depends_on` | List of task IDs |
| `## Acceptance Criteria` | Yes | `acceptance_criteria` | Free-text, Gherkin preferred |
| `## Spec` | No | `spec` | Full design context |
| `## Test Files` | No | `test_files` | Paths relative to project root |
| `## Implementation Files` | No | `implementation_files` | Paths relative to project root |
| `## Test Command` | No | `test_command` | Shell command for unit tests |
| `## Verification Command` | No | `verification_command` | Shell command for acceptance tests |

### Task ID Derivation

Task IDs are derived from filenames:
- `01-db-migrations.md` → `db-migrations`
- `02-auth-login.md` → `auth-login`
- Pattern: strip leading digits and hyphen, strip `.md` extension

## Workflow

### Step 1: Validate Input

1. Check that `<spec-path>` exists
2. If it's a directory, check that `tasks/` exists and contains `*.md` files
3. If it's a file, check that it's a valid markdown file
4. If any check fails, report error and stop

### Step 2: Parse Task Files

For greenfield (directory):
- Parse each `*.md` file in `tasks/`
- Extract task ID from filename
- Parse markdown sections into fields

For update (single file):
- Parse numbered sections (## 1. Title, ## 2. Title, etc.)
- Extract task ID from section title
- Parse subsections for acceptance criteria, dependencies, etc.

### Step 3: Validate Dependency Graph

1. Build a directed graph from `depends_on` references
2. Detect cycles using DFS
3. If cycles exist:
   - Print the cycle path (e.g., "A → B → C → A")
   - Error: "Circular dependency detected"
   - Stop (do not create any tasks)

### Step 4: Topological Sort

Sort tasks so that dependencies come before dependents. This determines creation order.

### Step 5: Check for Conflicts

For each task ID:
```bash
ztask get <task-id> --project <project-id>
```

- If task exists: warning "Task '<id>' already exists, skipping", skip this task
- If task does not exist: proceed to create

### Step 6: Create Tasks

Use the `ztask-ingest` CLI command:

```bash
ztask-ingest <project-id> <spec-path> [--dry-run]
```

Or manually create tasks for each task (in topological order):

```bash
ztask create <task-id> --project <project-id> \
  --criteria "<acceptance_criteria>" \
  --spec "<spec>" \
  --depends-on "<dep1>,<dep2>" \
  --test-files "<file1>,<file2>" \
  --impl-files "<file1>,<file2>" \
  --test-command "<cmd>" \
  --verify-command "<cmd>"
```

### Step 7: Report

```
Ingesting OpenSpec from ./openspec/specs/cli/task-model.md

  Found 4 task(s)
  - extend-python-model: Extend Python Model
    Depends on: []
  - update-cli: Update CLI Commands
    Depends on: [extend-python-model]
  - update-queries: Update Queries
    Depends on: [extend-python-model]
  - update-web-model: Update Web Model
    Depends on: [extend-python-model]

  Dependency graph:
    extend-python-model (no deps)
    update-cli -> depends on [extend-python-model]
    update-queries -> depends on [extend-python-model]
    update-web-model -> depends on [extend-python-model]

  Creating tasks in project 'ztask':
    ✓ extend-python-model — PENDING
    ✓ update-cli — PENDING (blocked by: extend-python-model)
    ✓ update-queries — PENDING (blocked by: extend-python-model)
    ✓ update-web-model — PENDING (blocked by: extend-python-model)

  Done. 4 tasks created, 0 skipped, 0 cycles detected.
```

## Error Handling

| Error | Action |
|-------|--------|
| Spec path not found | Error: "Path '<path>' not found" |
| No task files | Error: "No .md files found in '<path>/tasks/'" |
| Cycle detected | Error: "Circular dependency: A → B → C → A" |
| Missing acceptance criteria | Warning: skip task, continue |
| Invalid depends_on reference | Warning: skip that dependency, continue |
| Task already exists | Warning: skip task, continue |
| ztask CLI not found | Error: "ztask CLI not available, run 'poetry install'" |
| Zenoh unreachable | Error: "Cannot connect to Zenoh at <endpoint>" |

## Rules

1. **Do not overwrite existing tasks.** If a task ID already exists, skip it with a warning.
2. **Validate before creating.** Check for cycles, missing fields, and conflicts before writing to Zenoh.
3. **Report clearly.** Show what was parsed, what was created, what was skipped, and why.
4. **Fail fast on cycles.** Circular dependencies are a hard error — do not create any tasks.
5. **Preserve spec content.** Use the `--spec` flag to store the full spec context.

## Dry Run Mode

Use `--dry-run` to validate and preview without creating tasks:

```bash
ztask-ingest myapp ./openspec/specs/myapp/ --dry-run
```

This will:
- Parse all task files
- Validate dependencies
- Print the dependency graph
- Show what would be created
- NOT write to Zenoh

## Relationship to Other Skills

- **ztask-orchestrator** — run after ingestion to execute the created tasks
- **ztask-status** — run after ingestion to inspect the task graph
- **ztask-worker** — invoked by the orchestrator for each task created by ingestion
