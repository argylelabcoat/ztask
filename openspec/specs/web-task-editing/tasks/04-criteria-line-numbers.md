# Task: Line-Numbered Acceptance Criteria Textarea

## Depends On
- edit-modal-template

## Acceptance Criteria
Feature: Line-numbered acceptance criteria textarea
  As a human writing multi-line Gherkin acceptance criteria
  I want a line-number gutter next to the textarea
  So that I have better orientation while editing longer criteria

  Scenario: Textarea shows line numbers
    Given the edit modal is open
    When I view the acceptance criteria field
    Then it is a multi-line textarea with a line-number gutter beside it

  Scenario: Line numbers update as I type
    Given the criteria textarea has 3 lines
    When I add a new line
    Then the gutter shows a 4th line number

  Scenario: No new JS library introduced
    Given the project's static assets
    When line-numbers.js is added
    Then it is a small hand-rolled script, not a vendored library, served the same way as htmx.min.js

## Spec
Add `web/static/line-numbers.js`: a small hand-rolled script (no library) that, on `DOMContentLoaded` and on the criteria textarea's `input` event, renders a line-number gutter element beside it by counting newlines in the textarea's value. Serve it via a new `static_assets::line_numbers_js` handler, registered as `GET /static/line-numbers.js` in `web/src/lib.rs` (same pattern as `style_css`/`htmx_js`). Reference it from `task_edit.html` with a `<script src="/static/line-numbers.js"></script>` tag, and give the criteria `<textarea>` an id the script can target.

## Test Files
- web/src/handlers/static_assets.rs (inline `#[cfg(test)]`)

## Implementation Files
- web/static/line-numbers.js
- web/src/handlers/static_assets.rs
- web/src/lib.rs
- web/templates/task_edit.html

## Test Command
cd web && cargo test --lib handlers::static_assets::
