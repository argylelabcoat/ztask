# Zenoh Task Tracker Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the `ztask` CLI package (Poetry-managed) for creating/querying/updating tasks in Zenoh, plus the `zenoh-router` container running zenohd with the `zenoh-backend-garry` storage backend, per `docs/superpowers/specs/2026-07-31-zenoh-task-tracker-design.md`.

**Architecture:** A typer-based CLI (`ztask`) opens a Zenoh client session against a router endpoint (configurable via `ZTASK_ZENOH_ENDPOINT`) and reads/writes hierarchical keys under `projects/<project_id>/tasks/<task_id>/*`. The router is a separate multi-stage-built Docker/Apple-container image running `zenohd` with the `garry` storage backend persisting that keyspace to disk.

**Tech Stack:** Python 3.11+, Poetry, typer, eclipse-zenoh (Python bindings), pytest + pytest-mock, Rust/cmake (router build stage), Docker/Apple `container` CLI.

## Global Constraints

- Key schema is fixed: `projects/<project_id>/tasks/<task_id>/{status,time_entered,time_accepted,time_completed,acceptance_criteria,history/<iso-timestamp>}` (spec: Key Schema).
- CLI connects via `ZTASK_ZENOH_ENDPOINT` env var, default `tcp/localhost:7447`; no reliance on multicast scouting (spec: Components).
- Router storage backend config: volume `garry`/backend `garry`, storage `key_expr: "projects/**"`, `db_path` under a mounted `/data` volume (spec: Router Container).
- Dependency management is Poetry, not pip/uv directly.
- Unit tests must not require a network or container; integration tests are marked `@pytest.mark.integration` and skipped by default (spec: Testing).

---

## File Structure

```
pyproject.toml
ztask/
  __init__.py
  models.py         # Task dataclass
  zenoh_client.py    # endpoint resolution + session context manager
  queries.py         # fetch_all_tasks, fetch_task, fetch_status
  cli.py             # typer app: list, get, create, update-status
tests/
  unit/
    fakes.py          # FakeReply/FakeOk/FakePayload test doubles
    test_models.py
    test_zenoh_client.py
    test_queries.py
    test_cli.py
  integration/
    conftest.py        # router container fixture
    test_cli_integration.py
docker/
  router/
    Dockerfile
    config.json5
scripts/
  up.sh
```

---

### Task 1: Poetry project scaffold

**Files:**
- Create: `pyproject.toml`
- Create: `ztask/__init__.py`
- Create: `tests/unit/__init__.py` (empty)
- Create: `tests/integration/__init__.py` (empty)

**Interfaces:**
- Produces: installable `ztask` package (empty), `poetry run pytest` working with an `integration` marker registered, `poetry run ztask` entry point wired to `ztask.cli:app` (app added in Task 5, so entry point will 404 until then — acceptable, this task only wires config).

- [ ] **Step 1: Write `pyproject.toml`**

```toml
[tool.poetry]
name = "ztask"
version = "0.1.0"
description = "CLI for LLMs and developers to manage tasks stored in Zenoh."
authors = []
readme = "README.md"
packages = [{ include = "ztask" }]

[tool.poetry.dependencies]
python = "^3.11"
typer = "^0.12"
eclipse-zenoh = "^1.0"

[tool.poetry.group.dev.dependencies]
pytest = "^8.0"
pytest-mock = "^3.14"

[tool.poetry.scripts]
ztask = "ztask.cli:app"

[tool.pytest.ini_options]
markers = [
    "integration: requires a running container runtime (docker/container)",
]

[build-system]
requires = ["poetry-core"]
build-backend = "poetry.core.masonry.api"
```

- [ ] **Step 2: Create empty package/test dirs**

```bash
mkdir -p ztask tests/unit tests/integration
touch ztask/__init__.py tests/unit/__init__.py tests/integration/__init__.py
echo "# Zenoh Task Tracker" > README.md
```

- [ ] **Step 3: Install and verify**

Run: `poetry install`
Expected: succeeds, creates/updates `poetry.lock`.

Run: `poetry run pytest`
Expected: `no tests ran` (exit code may be 5) — no errors/import failures.

- [ ] **Step 4: Commit**

```bash
git add pyproject.toml poetry.lock README.md ztask tests
git commit -m "chore: scaffold Poetry project for ztask CLI"
```

---

### Task 2: Task model

**Files:**
- Create: `ztask/models.py`
- Test: `tests/unit/test_models.py`

**Interfaces:**
- Consumes: nothing (pure data class).
- Produces: `ztask.models.Task` — dataclass with fields `id: str`, `status: str = "UNKNOWN"`, `time_entered: Optional[str] = None`, `time_accepted: Optional[str] = None`, `time_completed: Optional[str] = None`, `acceptance_criteria: Optional[str] = None`, `history: List[dict] = field(default_factory=list)`, and method `to_dict() -> dict`.

- [ ] **Step 1: Write the failing test**

```python
# tests/unit/test_models.py
from ztask.models import Task


def test_task_defaults():
    task = Task(id="t1")
    assert task.status == "UNKNOWN"
    assert task.time_entered is None
    assert task.history == []


def test_task_to_dict_includes_all_fields():
    task = Task(
        id="t1",
        status="PENDING",
        time_entered="2026-07-31T00:00:00+00:00",
        acceptance_criteria="Given X, When Y, Then Z",
        history=[{"timestamp": "t", "from_status": "NONE", "to_status": "PENDING", "note": ""}],
    )
    result = task.to_dict()
    assert result == {
        "id": "t1",
        "status": "PENDING",
        "time_entered": "2026-07-31T00:00:00+00:00",
        "time_accepted": None,
        "time_completed": None,
        "acceptance_criteria": "Given X, When Y, Then Z",
        "history": [{"timestamp": "t", "from_status": "NONE", "to_status": "PENDING", "note": ""}],
    }


def test_task_history_default_is_independent_per_instance():
    a = Task(id="a")
    b = Task(id="b")
    a.history.append({"x": 1})
    assert b.history == []
```

- [ ] **Step 2: Run test to verify it fails**

Run: `poetry run pytest tests/unit/test_models.py -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'ztask.models'`

- [ ] **Step 3: Write minimal implementation**

```python
# ztask/models.py
from dataclasses import dataclass, field
from typing import List, Optional


@dataclass
class Task:
    id: str
    status: str = "UNKNOWN"
    time_entered: Optional[str] = None
    time_accepted: Optional[str] = None
    time_completed: Optional[str] = None
    acceptance_criteria: Optional[str] = None
    history: List[dict] = field(default_factory=list)

    def to_dict(self) -> dict:
        return {
            "id": self.id,
            "status": self.status,
            "time_entered": self.time_entered,
            "time_accepted": self.time_accepted,
            "time_completed": self.time_completed,
            "acceptance_criteria": self.acceptance_criteria,
            "history": self.history,
        }
```

- [ ] **Step 4: Run test to verify it passes**

Run: `poetry run pytest tests/unit/test_models.py -v`
Expected: 3 passed

- [ ] **Step 5: Commit**

```bash
git add ztask/models.py tests/unit/test_models.py
git commit -m "feat: add Task model"
```

---

### Task 3: Zenoh endpoint resolution + session context manager

**Files:**
- Create: `ztask/zenoh_client.py`
- Test: `tests/unit/test_zenoh_client.py`

**Interfaces:**
- Consumes: `os.environ`, `zenoh.Config`, `zenoh.open` (from the `eclipse-zenoh` package, imported as `zenoh`).
- Produces: `ztask.zenoh_client.ENDPOINT_ENV_VAR = "ZTASK_ZENOH_ENDPOINT"`, `ztask.zenoh_client.DEFAULT_ENDPOINT = "tcp/localhost:7447"`, `resolve_endpoint() -> str`, `open_session()` — a context manager yielding a `zenoh.Session`.

- [ ] **Step 1: Write the failing test**

```python
# tests/unit/test_zenoh_client.py
from ztask import zenoh_client


def test_resolve_endpoint_default(monkeypatch):
    monkeypatch.delenv(zenoh_client.ENDPOINT_ENV_VAR, raising=False)
    assert zenoh_client.resolve_endpoint() == "tcp/localhost:7447"


def test_resolve_endpoint_from_env(monkeypatch):
    monkeypatch.setenv(zenoh_client.ENDPOINT_ENV_VAR, "tcp/zenoh-router:7447")
    assert zenoh_client.resolve_endpoint() == "tcp/zenoh-router:7447"


def test_open_session_configures_connect_endpoint(monkeypatch, mocker):
    monkeypatch.setenv(zenoh_client.ENDPOINT_ENV_VAR, "tcp/zenoh-router:7447")

    fake_config = mocker.MagicMock()
    fake_session = mocker.MagicMock()
    fake_session.__enter__.return_value = "the-session"
    fake_session.__exit__.return_value = False

    mocker.patch.object(zenoh_client.zenoh, "Config", return_value=fake_config)
    mock_open = mocker.patch.object(zenoh_client.zenoh, "open", return_value=fake_session)

    with zenoh_client.open_session() as session:
        assert session == "the-session"

    fake_config.insert_json5.assert_called_once_with(
        "connect/endpoints", '["tcp/zenoh-router:7447"]'
    )
    mock_open.assert_called_once_with(fake_config)
```

- [ ] **Step 2: Run test to verify it fails**

Run: `poetry run pytest tests/unit/test_zenoh_client.py -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'ztask.zenoh_client'`

- [ ] **Step 3: Write minimal implementation**

```python
# ztask/zenoh_client.py
import os
from contextlib import contextmanager

import zenoh

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

- [ ] **Step 4: Run test to verify it passes**

Run: `poetry run pytest tests/unit/test_zenoh_client.py -v`
Expected: 3 passed

- [ ] **Step 5: Commit**

```bash
git add ztask/zenoh_client.py tests/unit/test_zenoh_client.py
git commit -m "feat: add Zenoh session helper with configurable endpoint"
```

---

### Task 4: Query helpers (fetch_all_tasks, fetch_task, fetch_status)

**Files:**
- Create: `ztask/queries.py`
- Create: `tests/unit/fakes.py`
- Test: `tests/unit/test_queries.py`

**Interfaces:**
- Consumes: `ztask.models.Task` (Task 2). Takes a `zenoh.Session`-like object exposing `.get(key_expr) -> Iterable[reply]` where `reply.ok` is either `None` or an object with `.key_expr` (str-convertible) and `.payload.to_string() -> str`.
- Produces: `fetch_all_tasks(session, project_id: str) -> Dict[str, Task]`, `fetch_task(session, project_id: str, task_id: str) -> Optional[Task]`, `fetch_status(session, project_id: str, task_id: str) -> str` (returns `"UNKNOWN"` if no ok reply).

- [ ] **Step 1: Write the test doubles**

```python
# tests/unit/fakes.py
class FakePayload:
    def __init__(self, value: str):
        self._value = value

    def to_string(self) -> str:
        return self._value


class FakeOk:
    def __init__(self, key_expr: str, payload: str):
        self.key_expr = key_expr
        self.payload = FakePayload(payload)


class FakeReply:
    def __init__(self, key_expr: str = "", payload: str = "", ok: bool = True):
        self.ok = FakeOk(key_expr, payload) if ok else None


class FakeSession:
    """Maps key_expr strings (as passed to .get) to a list of FakeReply."""

    def __init__(self, replies_by_key_expr: dict):
        self._replies_by_key_expr = replies_by_key_expr
        self.put_calls = []

    def get(self, key_expr: str):
        return self._replies_by_key_expr.get(key_expr, [])

    def put(self, key_expr: str, value: str):
        self.put_calls.append((key_expr, value))
```

- [ ] **Step 2: Write the failing test**

```python
# tests/unit/test_queries.py
from tests.unit.fakes import FakeReply, FakeSession
from ztask.queries import fetch_all_tasks, fetch_status, fetch_task


def test_fetch_all_tasks_groups_fields_by_task_id():
    session = FakeSession({
        "projects/p1/tasks/**": [
            FakeReply("projects/p1/tasks/t1/status", "PENDING"),
            FakeReply("projects/p1/tasks/t1/time_entered", "2026-07-31T00:00:00+00:00"),
            FakeReply("projects/p1/tasks/t2/status", "COMPLETED"),
            FakeReply(
                "projects/p1/tasks/t1/history/2026-07-31T00-00-00",
                '{"timestamp": "t", "from_status": "NONE", "to_status": "PENDING", "note": ""}',
            ),
            FakeReply(ok=False),
        ]
    })

    tasks = fetch_all_tasks(session, "p1")

    assert set(tasks.keys()) == {"t1", "t2"}
    assert tasks["t1"].status == "PENDING"
    assert tasks["t1"].time_entered == "2026-07-31T00:00:00+00:00"
    assert tasks["t1"].history == [
        {"timestamp": "t", "from_status": "NONE", "to_status": "PENDING", "note": ""}
    ]
    assert tasks["t2"].status == "COMPLETED"


def test_fetch_task_queries_task_specific_prefix_and_returns_none_if_missing():
    session = FakeSession({
        "projects/p1/tasks/t1/**": [
            FakeReply("projects/p1/tasks/t1/status", "PENDING"),
        ],
        "projects/p1/tasks/missing/**": [],
    })

    found = fetch_task(session, "p1", "t1")
    assert found is not None
    assert found.id == "t1"
    assert found.status == "PENDING"

    missing = fetch_task(session, "p1", "missing")
    assert missing is None


def test_fetch_status_returns_unknown_when_no_ok_reply():
    session = FakeSession({"projects/p1/tasks/t1/status": [FakeReply(ok=False)]})
    assert fetch_status(session, "p1", "t1") == "UNKNOWN"


def test_fetch_status_returns_value_when_present():
    session = FakeSession({"projects/p1/tasks/t1/status": [FakeReply("projects/p1/tasks/t1/status", "IN_PROGRESS")]})
    assert fetch_status(session, "p1", "t1") == "IN_PROGRESS"
```

- [ ] **Step 3: Run test to verify it fails**

Run: `poetry run pytest tests/unit/test_queries.py -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'ztask.queries'`

- [ ] **Step 4: Write minimal implementation**

```python
# ztask/queries.py
import json
from typing import Dict, Optional

from ztask.models import Task


def _apply_field(task: Task, field_name: str, value: str) -> None:
    if field_name == "status":
        task.status = value
    elif field_name == "time_entered":
        task.time_entered = value
    elif field_name == "time_accepted":
        task.time_accepted = value
    elif field_name == "time_completed":
        task.time_completed = value
    elif field_name == "acceptance_criteria":
        task.acceptance_criteria = value
    elif field_name.startswith("history/"):
        try:
            task.history.append(json.loads(value))
        except json.JSONDecodeError:
            task.history.append(value)


def fetch_all_tasks(session, project_id: str) -> Dict[str, Task]:
    prefix = f"projects/{project_id}/tasks/"
    replies = session.get(f"{prefix}**")

    tasks: Dict[str, Task] = {}
    for reply in replies:
        if not reply.ok:
            continue

        raw_key = str(reply.ok.key_expr)
        if not raw_key.startswith(prefix):
            continue

        relative_path = raw_key[len(prefix):]
        parts = relative_path.split("/", 1)
        task_id = parts[0]
        field_name = parts[1] if len(parts) > 1 else ""

        if task_id not in tasks:
            tasks[task_id] = Task(id=task_id)

        _apply_field(tasks[task_id], field_name, reply.ok.payload.to_string())

    return tasks


def fetch_task(session, project_id: str, task_id: str) -> Optional[Task]:
    prefix = f"projects/{project_id}/tasks/{task_id}/"
    replies = session.get(f"{prefix}**")

    task: Optional[Task] = None
    for reply in replies:
        if not reply.ok:
            continue

        raw_key = str(reply.ok.key_expr)
        if not raw_key.startswith(prefix):
            continue

        field_name = raw_key[len(prefix):]
        if task is None:
            task = Task(id=task_id)

        _apply_field(task, field_name, reply.ok.payload.to_string())

    return task


def fetch_status(session, project_id: str, task_id: str) -> str:
    key = f"projects/{project_id}/tasks/{task_id}/status"
    for reply in session.get(key):
        if reply.ok:
            return reply.ok.payload.to_string()
    return "UNKNOWN"
```

- [ ] **Step 5: Run test to verify it passes**

Run: `poetry run pytest tests/unit/test_queries.py -v`
Expected: 4 passed

- [ ] **Step 6: Commit**

```bash
git add ztask/queries.py tests/unit/fakes.py tests/unit/test_queries.py
git commit -m "feat: add task query helpers with direct single-task lookup"
```

---

### Task 5: CLI — `list` and `get` commands

**Files:**
- Create: `ztask/cli.py`
- Test: `tests/unit/test_cli.py`

**Interfaces:**
- Consumes: `ztask.zenoh_client.open_session` (Task 3), `ztask.queries.fetch_all_tasks`, `ztask.queries.fetch_task` (Task 4).
- Produces: `ztask.cli.app` (typer `Typer` instance) with commands `list` and `get` registered. `ztask.cli.WIP_STATUSES = {"IN_PROGRESS", "WIP", "RUNNING"}`, `ztask.cli.TERMINAL_STATUS = "COMPLETED"`.

- [ ] **Step 1: Write the failing test**

```python
# tests/unit/test_cli.py
import json

from typer.testing import CliRunner

from ztask.cli import app
from ztask.models import Task

runner = CliRunner()


def test_list_all_returns_every_task(mocker):
    tasks = {
        "t1": Task(id="t1", status="PENDING"),
        "t2": Task(id="t2", status="COMPLETED"),
    }
    mocker.patch("ztask.cli.open_session").return_value.__enter__.return_value = "session"
    mocker.patch("ztask.cli.fetch_all_tasks", return_value=tasks)

    result = runner.invoke(app, ["list", "--project", "p1", "--filter", "all"])

    assert result.exit_code == 0
    payload = json.loads(result.stdout)
    assert {t["id"] for t in payload} == {"t1", "t2"}


def test_list_incomplete_excludes_completed(mocker):
    tasks = {
        "t1": Task(id="t1", status="PENDING"),
        "t2": Task(id="t2", status="COMPLETED"),
    }
    mocker.patch("ztask.cli.open_session").return_value.__enter__.return_value = "session"
    mocker.patch("ztask.cli.fetch_all_tasks", return_value=tasks)

    result = runner.invoke(app, ["list", "--project", "p1", "--filter", "incomplete"])

    payload = json.loads(result.stdout)
    assert [t["id"] for t in payload] == ["t1"]


def test_get_found_prints_task_json(mocker):
    mocker.patch("ztask.cli.open_session").return_value.__enter__.return_value = "session"
    mocker.patch("ztask.cli.fetch_task", return_value=Task(id="t1", status="PENDING"))

    result = runner.invoke(app, ["get", "--project", "p1", "t1"])

    assert result.exit_code == 0
    assert json.loads(result.stdout)["id"] == "t1"


def test_get_not_found_exits_with_error(mocker):
    mocker.patch("ztask.cli.open_session").return_value.__enter__.return_value = "session"
    mocker.patch("ztask.cli.fetch_task", return_value=None)

    result = runner.invoke(app, ["get", "--project", "p1", "missing"])

    assert result.exit_code == 1
    assert "not found" in result.stdout + (result.stderr or "")
```

- [ ] **Step 2: Run test to verify it fails**

Run: `poetry run pytest tests/unit/test_cli.py -v`
Expected: FAIL with `ModuleNotFoundError: No module named 'ztask.cli'`

- [ ] **Step 3: Write minimal implementation**

```python
# ztask/cli.py
import json
from datetime import datetime, timezone

import typer

from ztask.queries import fetch_all_tasks, fetch_status, fetch_task
from ztask.zenoh_client import open_session

app = typer.Typer(help="CLI tool for LLMs and developers to manage tasks in Zenoh.")

TERMINAL_STATUS = "COMPLETED"
WIP_STATUSES = {"IN_PROGRESS", "WIP", "RUNNING"}


def get_iso_timestamp() -> str:
    return datetime.now(timezone.utc).isoformat()


@app.command("list")
def list_tasks(
    project_id: str = typer.Option(..., "--project", "-p", help="Project ID"),
    filter_type: str = typer.Option(
        "all", "--filter", "-f", help="Filter mode: 'all', 'incomplete' (not COMPLETED), or 'wip' (IN_PROGRESS)"
    ),
):
    """List tasks filtered by state: all, incomplete, or wip."""
    with open_session() as session:
        all_tasks = fetch_all_tasks(session, project_id)

        filtered = []
        for task in all_tasks.values():
            status = task.status.upper()
            if filter_type == "all":
                filtered.append(task)
            elif filter_type == "incomplete" and status != TERMINAL_STATUS:
                filtered.append(task)
            elif filter_type == "wip" and status in WIP_STATUSES:
                filtered.append(task)

        typer.echo(json.dumps([t.to_dict() for t in filtered], indent=2))


@app.command("get")
def get_task(
    project_id: str = typer.Option(..., "--project", "-p", help="Project ID"),
    task_id: str = typer.Argument(..., help="Task ID"),
):
    """Fetch complete details and history for a single task."""
    with open_session() as session:
        task = fetch_task(session, project_id, task_id)

        if task is None:
            typer.echo(f"Error: Task '{task_id}' not found in project '{project_id}'.", err=True)
            raise typer.Exit(code=1)

        typer.echo(json.dumps(task.to_dict(), indent=2))


if __name__ == "__main__":
    app()
```

- [ ] **Step 4: Run test to verify it passes**

Run: `poetry run pytest tests/unit/test_cli.py -v`
Expected: 4 passed

- [ ] **Step 5: Commit**

```bash
git add ztask/cli.py tests/unit/test_cli.py
git commit -m "feat: add list and get CLI commands"
```

---

### Task 6: CLI — `create` and `update-status` commands

**Files:**
- Modify: `ztask/cli.py` (append two commands, import `fetch_status`)
- Modify: `tests/unit/test_cli.py` (append tests)

**Interfaces:**
- Consumes: `fetch_status` (Task 4, already imported unused since Task 5 — now used), `get_iso_timestamp` (defined in Task 5).
- Produces: `create` and `update-status` typer commands on `ztask.cli.app`.

- [ ] **Step 1: Write the failing test**

```python
# append to tests/unit/test_cli.py
from tests.unit.fakes import FakeSession


def test_create_puts_status_entered_and_history(mocker):
    session = FakeSession({})
    mocker.patch("ztask.cli.open_session").return_value.__enter__.return_value = session
    mocker.patch("ztask.cli.get_iso_timestamp", return_value="2026-07-31T00:00:00+00:00")

    result = runner.invoke(app, ["create", "--project", "p1", "t1", "--criteria", "Given X"])

    assert result.exit_code == 0
    keys = dict(session.put_calls)
    assert keys["projects/p1/tasks/t1/status"] == "PENDING"
    assert keys["projects/p1/tasks/t1/time_entered"] == "2026-07-31T00:00:00+00:00"
    assert keys["projects/p1/tasks/t1/acceptance_criteria"] == "Given X"
    history_key = "projects/p1/tasks/t1/history/2026-07-31T00-00-00+00-00"
    assert history_key in keys
    history_value = json.loads(keys[history_key])
    assert history_value["from_status"] == "NONE"
    assert history_value["to_status"] == "PENDING"


def test_update_status_to_in_progress_sets_time_accepted(mocker):
    session = FakeSession({"projects/p1/tasks/t1/status": []})
    mocker.patch("ztask.cli.open_session").return_value.__enter__.return_value = session
    mocker.patch("ztask.cli.fetch_status", return_value="PENDING")
    mocker.patch("ztask.cli.get_iso_timestamp", return_value="2026-07-31T01:00:00+00:00")

    result = runner.invoke(app, ["update-status", "--project", "p1", "t1", "in_progress", "--note", "starting"])

    assert result.exit_code == 0
    keys = dict(session.put_calls)
    assert keys["projects/p1/tasks/t1/status"] == "IN_PROGRESS"
    assert keys["projects/p1/tasks/t1/time_accepted"] == "2026-07-31T01:00:00+00:00"
    assert "projects/p1/tasks/t1/time_completed" not in keys


def test_update_status_to_completed_sets_time_completed(mocker):
    session = FakeSession({})
    mocker.patch("ztask.cli.open_session").return_value.__enter__.return_value = session
    mocker.patch("ztask.cli.fetch_status", return_value="IN_PROGRESS")
    mocker.patch("ztask.cli.get_iso_timestamp", return_value="2026-07-31T02:00:00+00:00")

    result = runner.invoke(app, ["update-status", "--project", "p1", "t1", "completed"])

    keys = dict(session.put_calls)
    assert keys["projects/p1/tasks/t1/status"] == "COMPLETED"
    assert keys["projects/p1/tasks/t1/time_completed"] == "2026-07-31T02:00:00+00:00"
```

- [ ] **Step 2: Run test to verify it fails**

Run: `poetry run pytest tests/unit/test_cli.py -v`
Expected: 3 new FAILs — `create`/`update-status` are not registered typer commands (CliRunner reports usage error / non-zero exit).

- [ ] **Step 3: Write minimal implementation**

```python
# append to ztask/cli.py, and change the import line to:
# from ztask.queries import fetch_all_tasks, fetch_status, fetch_task


@app.command("create")
def create_task(
    project_id: str = typer.Option(..., "--project", "-p", help="Project ID"),
    task_id: str = typer.Argument(..., help="Task ID"),
    criteria: str = typer.Option("", "--criteria", "-c", help="Acceptance criteria or Gherkin spec"),
):
    """Create a new task in PENDING state."""
    base_key = f"projects/{project_id}/tasks/{task_id}"
    now = get_iso_timestamp()

    with open_session() as session:
        session.put(f"{base_key}/status", "PENDING")
        session.put(f"{base_key}/time_entered", now)
        if criteria:
            session.put(f"{base_key}/acceptance_criteria", criteria)

        history_key = f"{base_key}/history/{now.replace(':', '-')}"
        session.put(
            history_key,
            json.dumps({
                "timestamp": now,
                "from_status": "NONE",
                "to_status": "PENDING",
                "note": "Task created via CLI",
            }),
        )

        typer.echo(f"Created task '{task_id}' in project '{project_id}'.")


@app.command("update-status")
def update_status(
    project_id: str = typer.Option(..., "--project", "-p", help="Project ID"),
    task_id: str = typer.Argument(..., help="Task ID"),
    status: str = typer.Argument(..., help="New status (e.g., PENDING, IN_PROGRESS, COMPLETED)"),
    note: str = typer.Option("", "--note", "-n", help="Optional reason or execution log note"),
):
    """Update task status and push transition to history log."""
    base_key = f"projects/{project_id}/tasks/{task_id}"
    now = get_iso_timestamp()
    new_status = status.upper()

    with open_session() as session:
        old_status = fetch_status(session, project_id, task_id)

        session.put(f"{base_key}/status", new_status)

        if new_status in WIP_STATUSES and old_status not in WIP_STATUSES:
            session.put(f"{base_key}/time_accepted", now)
        elif new_status == TERMINAL_STATUS:
            session.put(f"{base_key}/time_completed", now)

        history_key = f"{base_key}/history/{now.replace(':', '-')}"
        session.put(
            history_key,
            json.dumps({
                "timestamp": now,
                "from_status": old_status,
                "to_status": new_status,
                "note": note,
            }),
        )

        typer.echo(f"Updated '{task_id}': {old_status} -> {new_status}")
```

Note: move the `if __name__ == "__main__": app()` block back to the bottom of the file after these two commands.

- [ ] **Step 4: Run test to verify it passes**

Run: `poetry run pytest tests/unit/test_cli.py -v`
Expected: 7 passed

- [ ] **Step 5: Commit**

```bash
git add ztask/cli.py tests/unit/test_cli.py
git commit -m "feat: add create and update-status CLI commands"
```

---

### Task 7: Router container (zenohd + garry backend)

**Files:**
- Create: `docker/router/Dockerfile`
- Create: `docker/router/config.json5`

**Interfaces:**
- Produces: an image tagged `ztask-router:local` that runs `zenohd -c /config.json5`, exposing TCP `7447`, with a `garry` storage on `projects/**` persisting under `/data`.

- [ ] **Step 1: Write `docker/router/config.json5`**

```json5
{
  plugins: {
    storage_manager: {
      volumes: [{
        name: "garry",
        backend: "garry",
        storages: [{
          name: "projects",
          key_expr: "projects/**",
          volume: {
            db_path: "/data/zenoh-garry",
            pool_size: 256,
            max_record_size: 1048576,
            max_versions: 64,
            compression: "lz4"
          }
        }]
      }]
    }
  }
}
```

- [ ] **Step 2: Write `docker/router/Dockerfile`**

```dockerfile
# syntax=docker/dockerfile:1

FROM rust:1-bookworm AS build

RUN apt-get update && apt-get install -y --no-install-recommends \
    cmake build-essential pkg-config git \
    && rm -rf /var/lib/apt/lists/*

# Build Garry (C library) from source.
RUN git clone --depth 1 https://github.com/Argylelabcoat/Garry.git /src/Garry
RUN cmake -S /src/Garry -B /src/Garry/build -DCMAKE_BUILD_TYPE=Release \
    && cmake --build /src/Garry/build -j \
    && cmake --install /src/Garry/build --prefix /src/Garry/install

ENV PKG_CONFIG_PATH=/src/Garry/install/lib/pkgconfig

# Build zenohd and the garry backend plugin from source.
RUN cargo install zenohd --locked
RUN cargo install zenoh-backend-garry --locked

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /src/Garry/install/lib/*.so* /usr/local/lib/
RUN ldconfig

COPY --from=build /usr/local/cargo/bin/zenohd /usr/local/bin/zenohd
COPY --from=build /usr/local/cargo/bin/libzenoh_backend_garry.so /root/.zenoh/lib/libzenoh_backend_garry.so
COPY docker/router/config.json5 /config.json5

VOLUME ["/data"]
EXPOSE 7447

ENTRYPOINT ["zenohd", "-c", "/config.json5"]
```

Note for whoever runs this: `cargo install zenoh-backend-garry` produces a `libzenoh_backend_garry.so` under `cargo install`'s target — the exact output filename/location may need adjusting after a first build (check `find /usr/local/cargo -name '*garry*'` inside the build stage) since cargo-installed plugin cdylibs aren't always placed in `bin/`. If it's not under `cargo/bin`, build from a cloned source checkout instead (`cargo build -p zenoh-backend-garry --release` against a `git clone` of `zenoh-backend-garry`, then copy from `target/release/`).

- [ ] **Step 3: Build and manually verify**

Run: `docker build -f docker/router/Dockerfile -t ztask-router:local .`
Expected: image builds successfully.

Run: `docker run --rm -p 7447:7447 -v ztask-data:/data ztask-router:local`
Expected: logs show zenohd starting and the `garry` storage plugin loading on `projects/**` without errors. Stop with Ctrl-C.

- [ ] **Step 4: Commit**

```bash
git add docker/router/Dockerfile docker/router/config.json5
git commit -m "feat: add zenohd+garry router container"
```

---

### Task 8: Local dev bring-up script

**Files:**
- Create: `scripts/up.sh`

**Interfaces:**
- Produces: a script that builds `docker/router/Dockerfile`, creates the `ztask-net` bridge network if missing, and runs the router container attached to it, publishing `7447` to the host so a locally-run CLI (`ZTASK_ZENOH_ENDPOINT=tcp/localhost:7447`) can reach it.

- [ ] **Step 1: Write `scripts/up.sh`**

```bash
#!/usr/bin/env bash
set -euo pipefail

RUNTIME="${ZTASK_CONTAINER_RUNTIME:-docker}"
NETWORK="ztask-net"
IMAGE="ztask-router:local"

if ! "$RUNTIME" network inspect "$NETWORK" >/dev/null 2>&1; then
  "$RUNTIME" network create "$NETWORK"
fi

"$RUNTIME" build -f docker/router/Dockerfile -t "$IMAGE" .

"$RUNTIME" run --rm -d \
  --name zenoh-router \
  --network "$NETWORK" \
  -p 7447:7447 \
  -v ztask-data:/data \
  "$IMAGE"

echo "Router running as 'zenoh-router' on network '$NETWORK', published on localhost:7447."
echo "Local CLI: export ZTASK_ZENOH_ENDPOINT=tcp/localhost:7447"
echo "In-network agent containers: export ZTASK_ZENOH_ENDPOINT=tcp/zenoh-router:7447 (--network $NETWORK)"
```

- [ ] **Step 2: Make executable and run it**

Run: `chmod +x scripts/up.sh && ./scripts/up.sh`
Expected: prints the running container name and connection instructions; `docker ps` shows `zenoh-router` up.

Run: `nc -z localhost 7447 && echo OPEN`
Expected: prints `OPEN`.

Tear down after verifying: `docker stop zenoh-router`

- [ ] **Step 3: Commit**

```bash
git add scripts/up.sh
git commit -m "feat: add local dev bring-up script for router container"
```

---

### Task 9: Integration tests against the real router container

**Files:**
- Create: `tests/integration/conftest.py`
- Create: `tests/integration/test_cli_integration.py`

**Interfaces:**
- Consumes: `docker/router/Dockerfile` (Task 7), `ztask.cli.app` (Tasks 5–6), `ZTASK_ZENOH_ENDPOINT` (Task 3).
- Produces: a session-scoped `router` pytest fixture that builds+starts the container and tears it down after the test session; tests marked `@pytest.mark.integration`.

- [ ] **Step 1: Write the fixture**

```python
# tests/integration/conftest.py
import shutil
import socket
import subprocess
import time

import pytest

IMAGE = "ztask-router:integration-test"
CONTAINER_NAME = "ztask-router-integration-test"
PORT = 17447


def _runtime() -> str:
    for candidate in ("container", "docker"):
        if shutil.which(candidate):
            return candidate
    pytest.skip("no container runtime (docker/container) found on PATH")


def _wait_for_port(host: str, port: int, timeout: float = 30.0) -> None:
    deadline = time.time() + timeout
    while time.time() < deadline:
        try:
            with socket.create_connection((host, port), timeout=1):
                return
        except OSError:
            time.sleep(0.5)
    raise TimeoutError(f"router did not open port {port} within {timeout}s")


@pytest.fixture(scope="session")
def router():
    runtime = _runtime()
    subprocess.run(
        [runtime, "build", "-f", "docker/router/Dockerfile", "-t", IMAGE, "."],
        check=True,
    )
    subprocess.run(
        [runtime, "rm", "-f", CONTAINER_NAME],
        check=False,
        capture_output=True,
    )
    subprocess.run(
        [
            runtime, "run", "--rm", "-d",
            "--name", CONTAINER_NAME,
            "-p", f"{PORT}:7447",
            IMAGE,
        ],
        check=True,
    )
    try:
        _wait_for_port("localhost", PORT)
        yield f"tcp/localhost:{PORT}"
    finally:
        subprocess.run([runtime, "stop", CONTAINER_NAME], check=False)
```

- [ ] **Step 2: Write the integration test**

```python
# tests/integration/test_cli_integration.py
import json
import os

import pytest
from typer.testing import CliRunner

from ztask.cli import app

runner = CliRunner()


@pytest.mark.integration
def test_create_then_get_round_trips_through_real_router(router, monkeypatch):
    monkeypatch.setenv("ZTASK_ZENOH_ENDPOINT", router)

    create_result = runner.invoke(
        app, ["create", "--project", "itest", "task-1", "--criteria", "Given X, When Y, Then Z"]
    )
    assert create_result.exit_code == 0, create_result.stdout

    get_result = runner.invoke(app, ["get", "--project", "itest", "task-1"])
    assert get_result.exit_code == 0, get_result.stdout

    task = json.loads(get_result.stdout)
    assert task["id"] == "task-1"
    assert task["status"] == "PENDING"
    assert task["acceptance_criteria"] == "Given X, When Y, Then Z"


@pytest.mark.integration
def test_update_status_persists_and_appears_in_list(router, monkeypatch):
    monkeypatch.setenv("ZTASK_ZENOH_ENDPOINT", router)

    runner.invoke(app, ["create", "--project", "itest", "task-2"])
    update_result = runner.invoke(
        app, ["update-status", "--project", "itest", "task-2", "in_progress", "--note", "starting"]
    )
    assert update_result.exit_code == 0, update_result.stdout

    list_result = runner.invoke(app, ["list", "--project", "itest", "--filter", "wip"])
    tasks = json.loads(list_result.stdout)
    assert any(t["id"] == "task-2" and t["status"] == "IN_PROGRESS" for t in tasks)
```

- [ ] **Step 3: Run to verify (requires a container runtime)**

Run: `poetry run pytest -m integration -v`
Expected: 2 passed (skipped automatically if no `docker`/`container` binary is on PATH, per the fixture's `pytest.skip`).

Run (default suite, should NOT attempt container work): `poetry run pytest -v`
Expected: integration tests are deselected/skipped by default; only unit tests run. If plain `pytest` still picks up integration tests, add `addopts = "-m 'not integration'"` to `[tool.pytest.ini_options]` in `pyproject.toml` and re-run.

- [ ] **Step 4: Commit**

```bash
git add tests/integration
git commit -m "test: add integration tests against real router container"
```

---

## Self-Review Notes

- **Spec coverage:** Key schema (Task 4/6), endpoint config (Task 3), `get_task` direct-prefix fix (Task 4's `fetch_task`), `update-status` old-status robustness fix (Task 4's `fetch_status` + Task 6), Poetry (Task 1), router Dockerfile/config (Task 7), unit tests (Tasks 2–6), integration tests (Task 9), local dev script (Task 8) — all covered. Human UI and Claude skills are explicitly out of scope per spec and have no tasks here.
- **Placeholder scan:** No TBD/TODO markers. The one caveat note in Task 7 Step 2 flags a real, unavoidable uncertainty (exact cargo-install output path for a third-party cdylib plugin) with a concrete fallback command, not a placeholder.
- **Type consistency:** `Task` fields (Task 2) match `_apply_field`/`to_dict` usage in Task 4 and the JSON assertions in Task 5/6 tests. `fetch_status` return type (`str`, `"UNKNOWN"` default) matches its use in `update_status`. `open_session()` context manager shape matches its mocking pattern (`.return_value.__enter__.return_value`) used consistently across Tasks 5, 6, 9.
