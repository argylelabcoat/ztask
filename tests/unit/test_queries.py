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
