# Task: Update Web Model

## Depends On
- extend-python-model

## Acceptance Criteria
Given the extended Task model in Python,
When I update the Rust web UI,
Then the Task struct has all new fields
And the queries module correctly assembles them

## Spec
Update the Rust web UI to match the Python model:
- Add new fields to Task struct in models.rs
- Add match arms in apply_field() in queries.rs
- Update templates to display new fields

## Test Files
- web/src/models.rs (inline tests)
- web/src/queries.rs (inline tests)

## Implementation Files
- web/src/models.rs
- web/src/queries.rs
- web/src/handlers/task.rs
- web/templates/task_detail.html

## Test Command
cd web && cargo test
