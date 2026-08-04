# Task: Extend Python Model

## Acceptance Criteria
Given the current Task dataclass,
When I add the SDD→TDD fields,
Then the dataclass has all new fields with proper defaults
And to_dict() includes non-empty new fields

## Spec
Add new fields to the Task dataclass:
- SDD fields: spec, depends_on, blocks
- TDD fields: test_files, implementation_files, tdd_phase, test_command, verification_command
- Execution metadata: failure_reason, attempt_count

## Test Files
- tests/unit/test_models.py

## Implementation Files
- ztask/models.py

## Test Command
poetry run pytest tests/unit/test_models.py -v
