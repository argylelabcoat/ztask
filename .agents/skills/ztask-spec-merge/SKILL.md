---
description: >
  Merge completed update specs into the main feature specification.
  Use when: "merge spec", "update spec after implementation", "consolidate spec",
  "fold changes into spec", or any request to merge incremental changes into the canonical spec.
  Requires: completed tasks in Zenoh, update spec files.
---

# ztask-spec-merge — Merge Update Specs into Feature Specs

You are a Specification Consolidation Agent. Your job: take completed update specs and merge their changes into the canonical feature specification, creating a clean, organized source of truth.

## Invocation

```
/ztask-spec-merge <feature-name> [update-spec-path]
```

**Examples:**
```
/ztask-spec-merge task-model openspec/specs/sdd-tdd-extension/
/ztask-spec-merge cli openspec/specs/cli/
```

## When to Use

1. **After implementation completes** — update spec has been implemented, merge changes into main spec
2. **After multiple updates** — consolidate several incremental changes into one clean spec
3. **Before starting new work** — ensure the main spec reflects current state

## Workflow

### Step 1: Identify the Feature Spec

Find the canonical spec file:
- `openspec/specs/<feature>/overview.md` — main feature spec
- `openspec/specs/<component>/overview.md` — component spec (cli, web, router)

If it doesn't exist, create it from the update spec.

### Step 2: Identify Completed Update Specs

Look for update specs in:
- `openspec/specs/<feature>-extension/` — update spec directories
- `openspec/specs/<feature>-update/` — incremental updates

Check Zenoh for completed tasks:
```bash
ztask list --project <project-id> --filter all
```

Only merge specs whose tasks are all COMPLETED.

### Step 3: Extract Changes from Update Spec

For each completed task in the update spec:
1. Read the task's `spec` field for implementation details
2. Read the `acceptance_criteria` for what was verified
3. Read `implementation_files` for what changed
4. Read `test_files` for test coverage

### Step 4: Merge into Feature Spec

Update the feature spec to reflect the implemented changes:

**For data models:**
- Update the model definition (Python and Rust)
- Add new fields to the field specifications table
- Update the Zenoh key schema
- Update the status lifecycle diagram if changed

**For CLI commands:**
- Update command syntax
- Add new flags to the reference
- Update examples

**For components:**
- Update the component overview
- Add new routes/endpoints
- Update the file structure

**For tests:**
- Update the testing section
- Add new test scenarios

### Step 5: Preserve History

Add a changelog entry to the feature spec:

```markdown
## Changelog

### 2026-08-04: SDD→TDD Extension
- Added 10 new fields to Task model (spec, depends_on, blocks, test_files, etc.)
- Extended CLI with new flags
- Updated queries to handle new Zenoh keys
- Added 51 Python tests, 76 Rust tests
```

### Step 6: Clean Up

After successful merge:
1. Archive the update spec (move to `openspec/archive/` or delete)
2. Remove the project from Zenoh (optional):
   ```bash
   # Only if project is fully completed
   ztask delete-project <project-id>
   ```

### Step 7: Report

```
Merging update spec into feature spec

  Source: openspec/specs/sdd-tdd-extension/
  Target: openspec/specs/cli/task-model.md

  Changes merged:
    ✓ Task model: added 10 new fields
    ✓ CLI: added 6 new flags
    ✓ Queries: added field assembly logic
    ✓ Tests: 127 tests passing

  Changelog entry added.

  Done. Feature spec updated.
```

## Merge Strategies

### Strategy 1: Replace (default)

Replace sections in the feature spec with the update spec's content.
Use when the update supersedes the previous version.

### Strategy 2: Append

Add new sections to the feature spec without modifying existing content.
Use when the update adds new functionality without changing existing behavior.

### Strategy 3: Diff-Merge

Show a diff of changes and let the user decide what to merge.
Use when the update has complex changes that need review.

## Gherkin Format

When extracting acceptance criteria, ensure they follow Gherkin format:

```gherkin
Feature: Task Model Extension
  As a developer
  I want to extend the task model with SDD→TDD fields
  So that tasks can track dependencies, test files, and TDD phase

  Scenario: Creating a task with dependencies
    Given a project "myapp" exists
    When I create task "auth-refresh" with depends_on "auth-login"
    Then the task should have depends_on ["auth-login"]
    And the task status should be "PENDING"

  Scenario: Fetching tasks with new fields
    Given a task "auth-refresh" exists with test_files "tests/test_auth.py"
    When I fetch the task
    Then the response should include test_files ["tests/test_auth.py"]
```

## BDD Testing

When merging specs, identify opportunities for BDD testing:

1. **Acceptance criteria in Gherkin** → can be automated with pytest-bdd or similar
2. **User stories** → can be translated to feature files
3. **Integration scenarios** → can be automated with real Zenoh router

Add BDD recommendations to the feature spec:

```markdown
## BDD Testing Opportunities

The following acceptance criteria can be automated:

| Scenario | Feature File | Status |
|----------|--------------|--------|
| Creating task with dependencies | tests/bdd/test_task_deps.feature | Not started |
| Fetching tasks with new fields | tests/bdd/test_task_fields.feature | Not started |

Recommended BDD framework: pytest-bdd
```

## Rules

1. **Don't overwrite unmerged changes.** If the feature spec has manual edits, preserve them.
2. **Preserve structure.** Keep the feature spec's organization (models, CLI, queries, etc.).
3. **Add, don't subtract.** Unless explicitly removing deprecated content.
4. **Verify completeness.** Ensure all update spec changes are captured.
5. **Gherkin-first.** Convert acceptance criteria to Gherkin format when possible.
