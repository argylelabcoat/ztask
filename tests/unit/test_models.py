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


def test_task_sdd_fields_defaults():
    task = Task(id="t1")
    assert task.spec is None
    assert task.depends_on == []
    assert task.blocks == []


def test_task_tdd_fields_defaults():
    task = Task(id="t1")
    assert task.test_files == []
    assert task.implementation_files == []
    assert task.tdd_phase is None
    assert task.test_command is None
    assert task.verification_command is None


def test_task_execution_metadata_defaults():
    task = Task(id="t1")
    assert task.failure_reason is None
    assert task.attempt_count == 0


def test_task_to_dict_includes_non_empty_sdd_fields():
    task = Task(
        id="t1",
        spec="Task specification",
        depends_on=["t0"],
        blocks=["t2"],
    )
    result = task.to_dict()
    assert result["spec"] == "Task specification"
    assert result["depends_on"] == ["t0"]
    assert result["blocks"] == ["t2"]


def test_task_to_dict_includes_non_empty_tdd_fields():
    task = Task(
        id="t1",
        test_files=["tests/test_foo.py"],
        implementation_files=["src/foo.py"],
        tdd_phase="RED",
        test_command="pytest tests/test_foo.py",
        verification_command="poetry run pytest tests/test_foo.py -v",
    )
    result = task.to_dict()
    assert result["test_files"] == ["tests/test_foo.py"]
    assert result["implementation_files"] == ["src/foo.py"]
    assert result["tdd_phase"] == "RED"
    assert result["test_command"] == "pytest tests/test_foo.py"
    assert result["verification_command"] == "poetry run pytest tests/test_foo.py -v"


def test_task_to_dict_includes_non_empty_execution_metadata():
    task = Task(
        id="t1",
        failure_reason="Timeout exceeded",
        attempt_count=3,
    )
    result = task.to_dict()
    assert result["failure_reason"] == "Timeout exceeded"
    assert result["attempt_count"] == 3


def test_task_to_dict_omits_empty_new_fields():
    task = Task(id="t1")
    result = task.to_dict()
    assert "spec" not in result
    assert "depends_on" not in result
    assert "blocks" not in result
    assert "test_files" not in result
    assert "implementation_files" not in result
    assert "tdd_phase" not in result
    assert "test_command" not in result
    assert "verification_command" not in result
    assert "failure_reason" not in result
    assert "attempt_count" not in result
