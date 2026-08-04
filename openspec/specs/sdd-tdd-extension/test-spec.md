# SDD→TDD Task Model Extension

This spec describes the extension of the task model to support SDD→TDD workflows.

## 1. Extend Python Model

Add new fields to the Task dataclass.

### Acceptance Criteria
- Task dataclass has all new fields with proper defaults
- to_dict() includes non-empty new fields

### Test Files
- tests/unit/test_models.py

### Implementation Files
- ztask/models.py

## 2. Update CLI Commands

Add new flags to create command.

### Acceptance Criteria
- ztask create accepts --spec, --depends-on, etc.
- Invalid input returns error

### Test Files
- tests/unit/test_cli.py

### Implementation Files
- ztask/cli.py

## 3. Update Queries

Handle new fields in queries module.

### Acceptance Criteria
- queries.py correctly assembles new fields from Zenoh keys

### Test Files
- tests/unit/test_queries.py

### Implementation Files
- ztask/queries.py
