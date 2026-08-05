# Task: Edit Modal HTTP Routes

## Depends On
- edit-task-backend

## Acceptance Criteria
Feature: Edit modal HTTP routes
  As a browser client
  I want GET/POST endpoints for the edit modal
  So that I can load a pre-filled edit form and save changes in one request

  Scenario: GET edit form for an existing task
    Given a task exists
    When GET /projects/{id}/tasks/{task_id}/edit is requested
    Then the response is 200
    And the form is pre-filled with the task's current status, criteria, spec, depends_on, and blocks

  Scenario: GET edit form for a missing task
    Given no task exists with the given ID
    When GET /projects/{id}/tasks/{task_id}/edit is requested
    Then the response is 404

  Scenario: POST edit saves the combined fields
    Given a task exists
    When POST /projects/{id}/tasks/{task_id}/edit is submitted with new status, criteria, spec, depends_on, and blocks
    Then the response is 200
    And the response body is the updated task_row.html fragment
    And depends_on/blocks are split from comma-separated input, trimmed, with empty entries dropped

  Scenario: Invalid project or task ID is rejected
    Given a project_id or task_id containing a wildcard character
    When GET or POST .../edit is requested
    Then the response is 400 and the store is never queried

  Scenario: Old status/criteria routes are removed
    Given the new combined edit route exists
    When POST /projects/{id}/tasks/{task_id}/status or /projects/{id}/tasks/{task_id}/criteria is requested
    Then the router returns 404 — the routes no longer exist

## Spec
Add to `web/src/handlers/task.rs`:
- `edit_form()`: validates `project_id`/`task_id` via `is_valid_id` (400 on failure), fetches the task via `queries::fetch_task` (404 if missing), renders `task_edit.html` pre-filled with the task's current editable field values (`depends_on`/`blocks` joined with `", "` for display in their text inputs)
- `edit()`: validates IDs (400 on failure), parses the combined form (`status`, `criteria`, `spec`, `depends_on`, `blocks` as comma-separated strings, `note`), splits/trims/drops-empty on `depends_on`/`blocks`, calls `tasks::edit_task`, returns the updated `task_row.html` fragment (404 if the task doesn't exist)

Remove the `update_status()` and `edit_criteria()` handlers and their routes. Register in `web/src/lib.rs`:
```
.route("/projects/{id}/tasks/{task_id}/edit", get(handlers::task::edit_form).post(handlers::task::edit))
```
removing the old `.../status` and `.../criteria` route registrations.

## Test Files
- web/src/handlers/task.rs (inline `#[cfg(test)]`)

## Implementation Files
- web/src/handlers/task.rs
- web/src/lib.rs

## Test Command
cd web && cargo test --lib handlers::task::
