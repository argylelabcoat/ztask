# Task: Update Queries

## Depends On
- extend-python-model

## Acceptance Criteria
Given the extended Task model,
When I fetch tasks from Zenoh,
Then the queries module correctly assembles all new fields from hierarchical keys

## Spec
Update queries.py to handle new Zenoh keys:
- Add match arms in _apply_field() for new fields
- Handle JSON arrays for list fields (depends_on, blocks, test_files, implementation_files)
- Handle integer parsing for attempt_count

## Test Files
- tests/unit/test_queries.py

## Implementation Files
- ztask/queries.py

## Test Command
poetry run pytest tests/unit/test_queries.py -v
