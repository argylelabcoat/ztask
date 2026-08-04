# Task: Update CLI Commands

## Depends On
- extend-python-model

## Acceptance Criteria
Given the extended Task model,
When I run `ztask create`,
Then it accepts new flags: --spec, --depends-on, --test-files, --impl-files, --test-command, --verify-command
And invalid input returns an error

## Spec
Extend the CLI to support the new fields:
- Add new flags to `ztask create` command
- Validate and store new fields in Zenoh
- Update `ztask list` to support `blocked` filter

## Test Files
- tests/unit/test_cli.py

## Implementation Files
- ztask/cli.py

## Test Command
poetry run pytest tests/unit/test_cli.py -v
