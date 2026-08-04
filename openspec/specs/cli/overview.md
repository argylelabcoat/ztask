# CLI (ztask)

Python CLI for LLM agents and developers to manage tasks in Zenoh.

## Overview

A Poetry-managed, pip-installable CLI (`ztask`) that opens a Zenoh session against a router and issues `list`/`get`/`create`/`update-status` commands. Defaults task creation to `entered_by: LLM`.

## Package Structure

```
ztask/
  __init__.py
  cli.py             # typer app: list, get, create, update-status
  models.py          # Task dataclass
  queries.py         # fetch_all_tasks, fetch_task, fetch_status
  zenoh_client.py    # endpoint resolution + session context manager
```

## Components

### `zenoh_client.py`

Session handling. Endpoint comes from `ZTASK_ZENOH_ENDPOINT` env var, defaulting to `tcp/localhost:7447` (local dev). Agent containers override it to `tcp/zenoh-router:7447` (Docker network) via env. No reliance on multicast scouting, since containers don't reliably see each other via multicast.

```python
DEFAULT_ENDPOINT = "tcp/localhost:7447"
ENDPOINT_ENV_VAR = "ZTASK_ZENOH_ENDPOINT"

def resolve_endpoint() -> str:
    return os.environ.get(ENDPOINT_ENV_VAR, DEFAULT_ENDPOINT)

@contextmanager
def open_session():
    endpoint = resolve_endpoint()
    config = zenoh.Config()
    config.insert_json5("connect/endpoints", f'["{endpoint}"]')
    with zenoh.open(config) as session:
        yield session
```

### `models.py`

Task dataclass. See `../task-model.md` for the full model specification.

### `queries.py`

Zenoh query helpers that assemble tasks from hierarchical keys:

- `fetch_all_tasks(session, project_id)` → `Dict[str, Task]`
  - Queries `projects/<project_id>/tasks/**`
  - Groups fields by task ID
  - Returns map of task_id → Task

- `fetch_task(session, project_id, task_id)` → `Optional[Task]`
  - Queries `projects/<project_id>/tasks/<task_id>/**`
  - Returns None if no keys found

- `fetch_status(session, project_id, task_id)` → `str`
  - Queries single key `projects/<project_id>/tasks/<task_id>/status`
  - Returns `"UNKNOWN"` if key not found

Field assembly logic (`_apply_field`):
- Scalar fields: set directly
- `history/*` entries: parsed as JSON, appended to history list
- Unknown fields: ignored

### `cli.py`

Typer app with four commands:

**`ztask list`**
```
ztask list --project <id> [--filter all|incomplete|wip|blocked]
```
- `all` — every task
- `incomplete` — excludes COMPLETED
- `wip` — only IN_PROGRESS/WIP/RUNNING
- `blocked` — tasks with unmet dependencies (future)
- Output: JSON array

**`ztask get`**
```
ztask get <task-id> --project <id>
```
- Output: single JSON object
- Exit code 1 if not found

**`ztask create`**
```
ztask create <task-id> --project <id>
    [--criteria "..."]
    [--spec "..."]
    [--depends-on task1,task2]
    [--test-files file1,file2]
    [--impl-files file1,file2]
    [--test-command "..."]
    [--verify-command "..."]
    [--entered-by llm|user]
```
- Creates task in PENDING state
- Writes only non-empty fields to Zenoh
- Appends history entry

**`ztask update-status`**
```
ztask update-status <task-id> <status> --project <id> [--note "..."]
```
- Normalizes status to uppercase
- Sets `time_accepted` when transitioning to WIP from non-WIP
- Sets `time_completed` when transitioning to COMPLETED
- Appends history entry
- Exit code 1 if task not found

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `ZTASK_ZENOH_ENDPOINT` | `tcp/localhost:7447` | Zenoh router endpoint |

## Dependencies

```toml
[tool.poetry.dependencies]
python = "^3.11"
typer = "^0.12"
eclipse-zenoh = "^1.0"

[tool.poetry.group.dev.dependencies]
pytest = "^8.0"
pytest-mock = "^3.14"
```

## Testing

- **Unit** (`tests/unit/`) — mock `zenoh.Session`/replies via `unittest.mock`; cover key construction, status-filtering logic, the `get_task` not-found path, and status-transition timestamp logic. No network or container required.
- **Integration** (`tests/integration/`, marked `@pytest.mark.integration`) — session-scoped fixture that builds and runs the real router container, runs CLI commands against it end-to-end. Skipped by default.

## Entry Point

```toml
[tool.poetry.scripts]
ztask = "ztask.cli:app"
```
