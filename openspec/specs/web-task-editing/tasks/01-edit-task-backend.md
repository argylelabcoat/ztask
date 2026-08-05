# Task: Combined Edit Write Path

## Acceptance Criteria
Feature: Combined task edit write path
  As an admin using the web UI
  I want a single function that updates status, criteria, spec, depends_on, and blocks together
  So that editing a task in the modal writes one history entry instead of several

  Scenario: Editing status transitions to WIP sets time_accepted
    Given a task with status PENDING
    When edit_task is called with status IN_PROGRESS
    Then the task's time_accepted is set
    And a history entry records the transition

  Scenario: Editing status to COMPLETED sets time_completed
    Given a task with status IN_PROGRESS
    When edit_task is called with status COMPLETED
    Then the task's time_completed is set

  Scenario: Editing criteria and spec together
    Given a task with existing acceptance_criteria
    When edit_task is called with new criteria and a new spec value
    Then both fields are updated
    And exactly one history entry is appended

  Scenario: Editing depends_on and blocks
    Given a task with no depends_on
    When edit_task is called with depends_on containing "task-a" and "task-b"
    Then the task's depends_on contains exactly those two IDs

  Scenario: Editing a missing task returns not found
    Given no task exists with the given ID
    When edit_task is called
    Then it returns TaskError::NotFound

## Spec
Add `tasks::edit_task(store, project_id, task_id, status, criteria, spec, depends_on, blocks, note, now) -> Result<Task, TaskError>` to `web/src/tasks.rs`. This is the single write path behind the edit modal, replacing the current separate `update_status`/`edit_criteria` functions:

- Reads the current task (404/`TaskError::NotFound` if missing)
- Updates `status`; sets `time_accepted` on a WIP transition or `time_completed` on a COMPLETED transition, using the same rules the current `update_status` uses
- Updates `acceptance_criteria` and `spec` — always writes the given value (including empty-to-clear), matching the current `edit_criteria`'s behavior
- Updates `depends_on`/`blocks` — caller passes already-split `Vec<String>`; write back as a comma-joined string (the existing `queries.rs::apply_field` already falls back to comma-split parsing when a `depends_on`/`blocks` value isn't valid JSON, so no new read-path parsing is needed)
- Appends exactly one history entry ("edited via modal") regardless of how many fields changed

Remove `update_status` and `edit_criteria` from `web/src/tasks.rs` once `edit_task` covers their behavior — they become dead code once the routes that call them are removed in the next task.

## Test Files
- web/src/tasks.rs (inline `#[cfg(test)]`)

## Implementation Files
- web/src/tasks.rs

## Test Command
cd web && cargo test --lib tasks::
