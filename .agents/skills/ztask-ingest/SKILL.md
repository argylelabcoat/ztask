---
description: >
  Convert an OpenSpec SDD directory into a dependency-ordered task graph in Zenoh.
  Use when: "ingest specs", "load tasks from spec", "import spec", "create tasks from OpenSpec",
  "run tasks from spec", or any request to convert a specification directory into executable tasks.
  Requires: zenohd router running, `ztask` CLI installed (`poetry install`).
---

# ztask-ingest — OpenSpec to Task Graph

You are a Specification Ingestion Agent. Your job: read an OpenSpec SDD directory, parse task files, validate dependencies, and create a task graph in Zenoh. You ensure acceptance criteria are in Gherkin format and identify BDD testing opportunities.

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
Given the current Task dataclass,
When I add the SDD→TDD fields,
Then the dataclass has all new fields with proper defaults
And to_dict() includes non-empty new fields

### Test Files
- tests/unit/test_models.py

### Implementation Files
- ztask/models.py

## 2. Update CLI Commands

Add new flags to create command...

### Acceptance Criteria
Given the extended Task model,
When I run `ztask create`,
Then it accepts new flags: --spec, --depends-on, etc.
And invalid input returns an error

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
Feature: JWT Refresh Token Rotation
  As an authenticated user
  I want to refresh my expired access token
  So that I can continue using the API without re-logging in

  Scenario: Refresh with valid token
    Given an expired access token
    And a valid refresh token
    When the client POSTs to /auth/refresh
    Then a new access token is returned
    And the old refresh token is invalidated

  Scenario: Refresh with invalid token
    Given an expired access token
    And an invalid refresh token
    When the client POSTs to /auth/refresh
    Then the response status is 401
    And no new tokens are issued

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

## BDD Feature File
tests/bdd/auth_refresh.feature
```

### Section Reference

| Section | Required | Maps to Task Field | Notes |
|---------|----------|-------------------|-------|
| `# Task: <name>` | Yes | `id` (derived from filename slug) | Display name; ID comes from filename |
| `## Depends On` | No | `depends_on` | List of task IDs |
| `## Acceptance Criteria` | Yes | `acceptance_criteria` | **Must be in Gherkin format** |
| `## Spec` | No | `spec` | Full design context |
| `## Test Files` | No | `test_files` | Paths relative to project root |
| `## Implementation Files` | No | `implementation_files` | Paths relative to project root |
| `## Test Command` | No | `test_command` | Shell command for unit tests |
| `## Verification Command` | No | `verification_command` | Shell command for acceptance tests |
| `## BDD Feature File` | No | (generated) | Path to BDD feature file |

### Task ID Derivation

Task IDs are derived from filenames:
- `01-db-migrations.md` → `db-migrations`
- `02-auth-login.md` → `auth-login`
- Pattern: strip leading digits and hyphen, strip `.md` extension

## Gherkin Validation

**All acceptance criteria must be in Gherkin format.** If they're not, convert them during ingestion.

### Gherkin Template

```gherkin
Feature: <Feature Name>
  As a <role>
  I want <capability>
  So that <benefit>

  Scenario: <Scenario Name>
    Given <precondition>
    And <additional precondition>
    When <action>
    Then <expected result>
    And <additional result>
```

### Conversion Rules

**From bullet points:**
```
- Task dataclass has all new fields
- to_dict() includes non-empty new fields
```

**To Gherkin:**
```gherkin
Feature: Task Model Extension
  As a developer
  I want to extend the task model with SDD→TDD fields
  So that tasks can track dependencies, test files, and TDD phase

  Scenario: Task dataclass has all new fields
    Given the Task dataclass
    When I inspect its fields
    Then it should have spec, depends_on, blocks
    And it should have test_files, implementation_files
    And it should have tdd_phase, test_command, verification_command
    And it should have failure_reason, attempt_count

  Scenario: to_dict() includes non-empty new fields
    Given a Task with non-empty new fields
    When I call to_dict()
    Then the result should include all non-empty new fields
    And the result should omit empty new fields
```

**From free-text:**
```
Implement JWT refresh token rotation with Redis-backed session store.
Rate-limit: 10 requests/minute per user.
```

**To Gherkin:**
```gherkin
Feature: JWT Refresh Token Rotation
  As an authenticated user
  I want to refresh my expired access token
  So that I can continue using the API without re-logging in

  Scenario: Refresh with valid token
    Given an expired access token
    And a valid refresh token
    When the client POSTs to /auth/refresh
    Then a new access token is returned
    And the old refresh token is invalidated

  Scenario: Rate limiting
    Given a user with a valid refresh token
    When the user sends 11 refresh requests in 1 minute
    Then the 11th request should be rate-limited
    And the response status should be 429
```

### Validation Errors

If acceptance criteria cannot be converted to Gherkin:
- Warning: "Acceptance criteria not in Gherkin format, attempting conversion"
- If conversion fails: "Cannot convert acceptance criteria to Gherkin, please rewrite"
- Skip task and continue

## BDD Testing

**Identify BDD testing opportunities during ingestion.**

### BDD Detection

Look for:
1. **Gherkin acceptance criteria** → can be automated with pytest-bdd
2. **User stories** → can be translated to feature files
3. **Integration scenarios** → can be automated with real Zenoh router

### BDD Recommendations

When creating tasks, add BDD recommendations:

```markdown
## BDD Testing Opportunities

| Scenario | Feature File | Status |
|----------|--------------|--------|
| Refresh with valid token | tests/bdd/auth_refresh.feature | Not started |
| Rate limiting | tests/bdd/auth_refresh.feature | Not started |

Recommended BDD framework: pytest-bdd
Test command: poetry run pytest tests/bdd/ -v
```

### BDD Feature File Generation

If acceptance criteria is in Gherkin format, generate a BDD feature file:

```gherkin
# tests/bdd/auth_refresh.feature
Feature: JWT Refresh Token Rotation
  As an authenticated user
  I want to refresh my expired access token
  So that I can continue using the API without re-logging in

  Scenario: Refresh with valid token
    Given an expired access token
    And a valid refresh token
    When the client POSTs to /auth/refresh
    Then a new access token is returned
    And the old refresh token is invalidated

  Scenario: Rate limiting
    Given a user with a valid refresh token
    When the user sends 11 refresh requests in 1 minute
    Then the 11th request should be rate-limited
    And the response status should be 429
```

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

### Step 3: Validate and Convert Acceptance Criteria

For each task:
1. Check if acceptance criteria is in Gherkin format
2. If not, attempt conversion:
   - Bullet points → Gherkin scenarios
   - Free-text → Gherkin feature + scenarios
3. If conversion fails, warning and skip task
4. Store converted Gherkin in `acceptance_criteria`

### Step 4: Identify BDD Opportunities

For each task:
1. Check if acceptance criteria is in Gherkin format
2. If yes, identify scenarios that can be automated
3. Generate BDD feature file path
4. Add BDD recommendations to task spec

### Step 5: Validate Dependency Graph

1. Build a directed graph from `depends_on` references
2. Detect cycles using DFS
3. If cycles exist:
   - Print the cycle path (e.g., "A → B → C → A")
   - Error: "Circular dependency detected"
   - Stop (do not create any tasks)

### Step 6: Topological Sort

Sort tasks so that dependencies come before dependents. This determines creation order.

### Step 7: Check for Conflicts

For each task ID:
```bash
ztask get <task-id> --project <project-id>
```

- If task exists: warning "Task '<id>' already exists, skipping", skip this task
- If task does not exist: proceed to create

### Step 8: Create Tasks

Use the `ztask-ingest` CLI command:

```bash
ztask-ingest <project-id> <spec-path> [--dry-run]
```

Or manually create tasks for each task (in topological order):

```bash
ztask create <task-id> --project <project-id> \
  --criteria "<gherkin_acceptance_criteria>" \
  --spec "<spec>" \
  --depends-on "<dep1>,<dep2>" \
  --test-files "<file1>,<file2>" \
  --impl-files "<file1>,<file2>" \
  --test-command "<cmd>" \
  --verify-command "<cmd>"
```

### Step 9: Generate BDD Feature Files

For tasks with Gherkin acceptance criteria:
1. Create `tests/bdd/` directory if it doesn't exist
2. Generate `.feature` file from acceptance criteria
3. Add feature file path to task's `test_files`

### Step 10: Report

```
Ingesting OpenSpec from ./openspec/specs/cli/task-model.md

  Found 4 task(s)
  - extend-python-model: Extend Python Model
    Depends on: []
    Acceptance Criteria: ✓ Gherkin format
    BDD: tests/bdd/extend_python_model.feature (2 scenarios)
  - update-cli: Update CLI Commands
    Depends on: [extend-python-model]
    Acceptance Criteria: ✓ Gherkin format (converted from bullets)
    BDD: tests/bdd/update_cli.feature (3 scenarios)
  - update-queries: Update Queries
    Depends on: [extend-python-model]
    Acceptance Criteria: ✓ Gherkin format (converted from bullets)
    BDD: tests/bdd/update_queries.feature (2 scenarios)
  - update-web-model: Update Web Model
    Depends on: [extend-python-model]
    Acceptance Criteria: ✓ Gherkin format (converted from bullets)
    BDD: tests/bdd/update_web_model.feature (2 scenarios)

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

  BDD feature files generated:
    ✓ tests/bdd/extend_python_model.feature
    ✓ tests/bdd/update_cli.feature
    ✓ tests/bdd/update_queries.feature
    ✓ tests/bdd/update_web_model.feature

  Done. 4 tasks created, 0 skipped, 0 cycles detected, 4 BDD feature files generated.
```

## Error Handling

| Error | Action |
|-------|--------|
| Spec path not found | Error: "Path '<path>' not found" |
| No task files | Error: "No .md files found in '<path>/tasks/'" |
| Cycle detected | Error: "Circular dependency: A → B → C → A" |
| Missing acceptance criteria | Warning: skip task, continue |
| Non-Gherkin acceptance criteria | Warning: attempt conversion, skip if fails |
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
6. **Gherkin-first.** Convert acceptance criteria to Gherkin format when possible.
7. **BDD-ready.** Generate BDD feature files for Gherkin acceptance criteria.

## Dry Run Mode

Use `--dry-run` to validate and preview without creating tasks:

```bash
ztask-ingest myapp ./openspec/specs/myapp/ --dry-run
```

This will:
- Parse all task files
- Validate and convert acceptance criteria to Gherkin
- Identify BDD opportunities
- Validate dependencies
- Print the dependency graph
- Show what would be created
- Show BDD feature files that would be generated
- NOT write to Zenoh

## Relationship to Other Skills

- **ztask-orchestrator** — run after ingestion to execute the created tasks
- **ztask-status** — run after ingestion to inspect the task graph
- **ztask-worker** — invoked by the orchestrator for each task created by ingestion
- **ztask-spec-merge** — merge completed update specs into feature specs
- **ztask-spec-organize** — maintain spec organization
