# ztask-ingest Skill

Convert an OpenSpec SDD directory structure into a dependency-ordered task graph in Zenoh.

## Invocation

```
/ztask-ingest <project-id> <spec-directory>
```

## Input: OpenSpec Directory Structure

```
<spec-directory>/
  spec.md                    # top-level design spec (optional)
  tasks/
    01-db-migrations.md      # task spec
    02-auth-login.md
    03-auth-refresh.md
```

### Task File Format

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

1. Check that `<spec-directory>` exists
2. Check that `<spec-directory>/tasks/` exists and contains `*.md` files
3. If `spec.md` exists at the directory root, read it for project-level context

### Step 2: Parse Task Files

For each `*.md` file in `tasks/`:
1. Extract task ID from filename
2. Parse markdown sections into fields
3. Validate:
   - `## Acceptance Criteria` section exists and is non-empty
   - Referenced `depends_on` task IDs exist in the set of task files

### Step 3: Validate Dependency Graph

1. Build a directed graph from `depends_on` references
2. Detect cycles using DFS
3. If cycles exist: print the cycle path, stop with error

### Step 4: Topological Sort

Sort tasks so that dependencies come before dependents. This determines creation order.

### Step 5: Check for Conflicts

For each task ID, query `ztask get <task-id> --project <project-id>`:
- If task exists: warning, skip (do not overwrite)
- If task does not exist: proceed to create

### Step 6: Create Tasks

For each task (in topological order):

```bash
ztask create <task-id> --project <project-id> \
  --criteria "<acceptance_criteria>" \
  --depends-on "<dep1>,<dep2>" \
  --test-files "<file1>,<file2>" \
  --impl-files "<file1>,<file2>" \
  --test-command "<cmd>" \
  --verify-command "<cmd>"
```

### Step 7: Report

```
Ingesting OpenSpec from ./openspec/specs/myapp/

  spec.md: loaded project spec (1,234 words)
  tasks/: found 4 task files

  Dependency graph:
    db-migrations (no deps)
    auth-login → depends on [db-migrations]
    auth-refresh → depends on [auth-login]
    auth-logout → depends on [auth-login]

  Creating tasks in project 'myapp':
    ✓ db-migrations — PENDING
    ✓ auth-login — PENDING (blocked by: db-migrations)
    ✓ auth-refresh — PENDING (blocked by: auth-login)
    ✓ auth-logout — PENDING (blocked by: auth-login)

  Done. 4 tasks created, 0 skipped, 0 cycles detected.
```

## Error Handling

| Error | Action |
|-------|--------|
| Spec directory not found | Error: "Directory '<path>' not found" |
| No task files | Error: "No .md files found in '<path>/tasks/'" |
| Cycle detected | Error: "Circular dependency: A → B → C → A" |
| Missing acceptance criteria | Warning: skip task, continue |
| Invalid depends_on reference | Warning: skip that dependency, continue |
| Task already exists | Warning: skip task, continue |
| ztask CLI not found | Error: "ztask CLI not available, run 'poetry install'" |
| Zenoh unreachable | Error: "Cannot connect to Zenoh at <endpoint>" |

## Relationship to Other Skills

- **ztask-orchestrator** — run after ingestion to execute the created tasks
- **ztask-status** — run after ingestion to inspect the task graph
- **ztask-worker** — invoked by the orchestrator for each task created by ingestion

## CLI Dependencies

- `ztask create` — create tasks with all new fields
- `ztask get` — check for existing tasks
