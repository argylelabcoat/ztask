import json

from tests.unit.fakes import FakeReply, FakeSession
from ztask.models import Task
from ztask.queries import _apply_field, fetch_all_tasks, fetch_status, fetch_task


def test_fetch_all_tasks_groups_fields_by_task_id():
    session = FakeSession({
        "projects/p1/tasks/**": [
            FakeReply("projects/p1/tasks/t1/status", "PENDING"),
            FakeReply("projects/p1/tasks/t1/time_entered", "2026-07-31T00:00:00+00:00"),
            FakeReply("projects/p1/tasks/t1/entered_by", "LLM"),
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
    assert tasks["t1"].entered_by == "LLM"
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


# --- _apply_field unit tests for new fields ---


def test_apply_field_depends_on_json_array():
    task = Task(id="t1")
    _apply_field(task, "depends_on", json.dumps(["t0", "t2"]))
    assert task.depends_on == ["t0", "t2"]


def test_apply_field_depends_on_comma_fallback():
    task = Task(id="t1")
    _apply_field(task, "depends_on", "t0, t2")
    assert task.depends_on == ["t0", "t2"]


def test_apply_field_blocks_json_array():
    task = Task(id="t1")
    _apply_field(task, "blocks", json.dumps(["t3", "t4"]))
    assert task.blocks == ["t3", "t4"]


def test_apply_field_blocks_comma_fallback():
    task = Task(id="t1")
    _apply_field(task, "blocks", "t3, t4")
    assert task.blocks == ["t3", "t4"]


def test_apply_field_test_files_json_array():
    task = Task(id="t1")
    _apply_field(task, "test_files", json.dumps(["tests/test_a.py", "tests/test_b.py"]))
    assert task.test_files == ["tests/test_a.py", "tests/test_b.py"]


def test_apply_field_test_files_comma_fallback():
    task = Task(id="t1")
    _apply_field(task, "test_files", "tests/test_a.py, tests/test_b.py")
    assert task.test_files == ["tests/test_a.py", "tests/test_b.py"]


def test_apply_field_implementation_files_json_array():
    task = Task(id="t1")
    _apply_field(task, "implementation_files", json.dumps(["src/a.py", "src/b.py"]))
    assert task.implementation_files == ["src/a.py", "src/b.py"]


def test_apply_field_implementation_files_comma_fallback():
    task = Task(id="t1")
    _apply_field(task, "implementation_files", "src/a.py, src/b.py")
    assert task.implementation_files == ["src/a.py", "src/b.py"]


def test_apply_field_attempt_count_integer():
    task = Task(id="t1")
    _apply_field(task, "attempt_count", "3")
    assert task.attempt_count == 3
    assert isinstance(task.attempt_count, int)


def test_apply_field_attempt_count_invalid_falls_back_to_zero():
    task = Task(id="t1")
    _apply_field(task, "attempt_count", "not_a_number")
    assert task.attempt_count == 0


def test_apply_field_string_fields():
    task = Task(id="t1")
    _apply_field(task, "spec", "my spec")
    _apply_field(task, "tdd_phase", "red")
    _apply_field(task, "test_command", "pytest")
    _apply_field(task, "verification_command", "cargo check")
    _apply_field(task, "failure_reason", "timeout")
    _apply_field(task, "acceptance_criteria", "done when tests pass")
    assert task.spec == "my spec"
    assert task.tdd_phase == "red"
    assert task.test_command == "pytest"
    assert task.verification_command == "cargo check"
    assert task.failure_reason == "timeout"
    assert task.acceptance_criteria == "done when tests pass"


def test_apply_field_tdd_phase_empty_becomes_none():
    task = Task(id="t1")
    _apply_field(task, "tdd_phase", "")
    assert task.tdd_phase is None


# --- Integration tests via fetch_task / fetch_all_tasks ---


def test_fetch_task_returns_all_new_fields():
    replies = {
        "projects/p1/tasks/t1/**": [
            FakeReply("projects/p1/tasks/t1/status", "IN_PROGRESS"),
            FakeReply("projects/p1/tasks/t1/spec", "implement auth"),
            FakeReply("projects/p1/tasks/t1/depends_on", json.dumps(["t0"])),
            FakeReply("projects/p1/tasks/t1/blocks", json.dumps(["t2"])),
            FakeReply("projects/p1/tasks/t1/test_files", json.dumps(["tests/test_auth.py"])),
            FakeReply("projects/p1/tasks/t1/implementation_files", json.dumps(["src/auth.py"])),
            FakeReply("projects/p1/tasks/t1/tdd_phase", "green"),
            FakeReply("projects/p1/tasks/t1/test_command", "pytest tests/test_auth.py"),
            FakeReply("projects/p1/tasks/t1/verification_command", "cargo test"),
            FakeReply("projects/p1/tasks/t1/failure_reason", "assertion error"),
            FakeReply("projects/p1/tasks/t1/attempt_count", "2"),
        ],
    }
    session = FakeSession(replies)
    task = fetch_task(session, "p1", "t1")

    assert task is not None
    assert task.spec == "implement auth"
    assert task.depends_on == ["t0"]
    assert task.blocks == ["t2"]
    assert task.test_files == ["tests/test_auth.py"]
    assert task.implementation_files == ["src/auth.py"]
    assert task.tdd_phase == "green"
    assert task.test_command == "pytest tests/test_auth.py"
    assert task.verification_command == "cargo test"
    assert task.failure_reason == "assertion error"
    assert task.attempt_count == 2


def test_fetch_all_tasks_groups_new_fields_by_task_id():
    replies = {
        "projects/p1/tasks/**": [
            FakeReply("projects/p1/tasks/t1/status", "PENDING"),
            FakeReply("projects/p1/tasks/t1/depends_on", json.dumps(["t0"])),
            FakeReply("projects/p1/tasks/t1/blocks", json.dumps(["t2"])),
            FakeReply("projects/p1/tasks/t1/test_files", json.dumps(["tests/test_x.py"])),
            FakeReply("projects/p1/tasks/t1/implementation_files", json.dumps(["src/x.py"])),
            FakeReply("projects/p1/tasks/t1/tdd_phase", "red"),
            FakeReply("projects/p1/tasks/t1/attempt_count", "1"),
        ],
    }
    session = FakeSession(replies)
    tasks = fetch_all_tasks(session, "p1")

    assert "t1" in tasks
    t = tasks["t1"]
    assert t.depends_on == ["t0"]
    assert t.blocks == ["t2"]
    assert t.test_files == ["tests/test_x.py"]
    assert t.implementation_files == ["src/x.py"]
    assert t.tdd_phase == "red"
    assert t.attempt_count == 1
