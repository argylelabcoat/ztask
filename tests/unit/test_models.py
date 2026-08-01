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
        entered_by="LLM",
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
        "entered_by": "LLM",
        "history": [{"timestamp": "t", "from_status": "NONE", "to_status": "PENDING", "note": ""}],
    }


def test_task_history_default_is_independent_per_instance():
    a = Task(id="a")
    b = Task(id="b")
    a.history.append({"x": 1})
    assert b.history == []
