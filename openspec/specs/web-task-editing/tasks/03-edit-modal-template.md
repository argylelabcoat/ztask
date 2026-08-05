# Task: Edit Modal UI

## Depends On
- edit-modal-routes

## Acceptance Criteria
Feature: Edit modal UI
  As a human admin
  I want to edit a task via a modal dialog instead of inline row forms
  So that I have one focused editing surface for all editable fields

  Scenario: Row shows a single Edit button
    Given a project's task list is rendered
    When I view a task row
    Then it shows one "Edit" button instead of the separate status-select and criteria-input forms
    And Delete remains its own separate column

  Scenario: Clicking Edit opens the modal pre-filled
    Given a task row's Edit button
    When I click it
    Then the shared <dialog id="edit-modal"> opens
    And it is pre-filled with the task's current status, criteria, spec, depends_on, and blocks

  Scenario: Saving closes the modal and updates the row
    Given the edit modal is open with changed field values
    When I click Save
    Then the dialog closes
    And the task row reflects the saved values without a full page reload

  Scenario: Task detail page also has an Edit button
    Given the task detail page for a task
    When I view it
    Then it has an Edit button that opens the same modal

## Spec
- `web/templates/base.html`: add a shared, initially-empty `<dialog id="edit-modal">` present on every page
- `web/templates/task_edit.html` (new): the modal's form fragment — `GET .../edit`'s response. A `<form>` posting to `POST /projects/{project_id}/tasks/{task.id}/edit`, containing: status select, acceptance-criteria textarea, spec textarea, depends_on text input, blocks text input, Save button. On the Edit button: `hx-get` to `GET .../edit`, `hx-target` the dialog body, `hx-on::after-request="document.getElementById('edit-modal').showModal()"`. On the form: normal `hx-post`/`hx-target="#task-{{ task.id }}"`/`hx-swap="outerHTML"` (matching the existing row-fragment-swap pattern), plus `hx-on::after-request="if(event.detail.successful) this.closest('dialog').close()"`
- `web/templates/task_row.html`: replace the Update-status and Save-criteria forms with a single "Edit" button (Delete stays its own column, unchanged from the prior feature)
- `web/templates/task_detail.html`: add the same "Edit" button

## Test Files
- web/src/handlers/task.rs (assert task_edit.html fragment content, GET/POST round-trip)
- web/src/handlers/project.rs (assert task_row.html renders one Edit control, not the old two forms)

## Implementation Files
- web/templates/base.html
- web/templates/task_row.html
- web/templates/task_detail.html
- web/templates/task_edit.html

## Test Command
cd web && cargo test --lib
