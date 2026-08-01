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


def test_list_unknown_filter_exits_with_error(mocker):
    mocker.patch("ztask.cli.open_session").return_value.__enter__.return_value = "session"
    fetch_all_tasks_mock = mocker.patch("ztask.cli.fetch_all_tasks")

    result = runner.invoke(app, ["list", "--project", "p1", "--filter", "bogus"])

    assert result.exit_code == 1
    assert "unknown filter" in result.stdout + (result.stderr or "")
    fetch_all_tasks_mock.assert_not_called()


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


def test_create_puts_status_entered_and_history(mocker):
    from tests.unit.fakes import FakeSession

    session = FakeSession({})
    mocker.patch("ztask.cli.open_session").return_value.__enter__.return_value = session
    mocker.patch("ztask.cli.get_iso_timestamp", return_value="2026-07-31T00:00:00+00:00")

    result = runner.invoke(app, ["create", "--project", "p1", "t1", "--criteria", "Given X"])

    assert result.exit_code == 0
    keys = dict(session.put_calls)
    assert keys["projects/p1/tasks/t1/status"] == "PENDING"
    assert keys["projects/p1/tasks/t1/time_entered"] == "2026-07-31T00:00:00+00:00"
    assert keys["projects/p1/tasks/t1/acceptance_criteria"] == "Given X"
    assert keys["projects/p1/tasks/t1/entered_by"] == "LLM"
    history_key = "projects/p1/tasks/t1/history/2026-07-31T00-00-00+00-00"
    assert history_key in keys
    history_value = json.loads(keys[history_key])
    assert history_value["from_status"] == "NONE"
    assert history_value["to_status"] == "PENDING"


def test_create_defaults_entered_by_to_llm(mocker):
    from tests.unit.fakes import FakeSession

    session = FakeSession({})
    mocker.patch("ztask.cli.open_session").return_value.__enter__.return_value = session
    mocker.patch("ztask.cli.get_iso_timestamp", return_value="2026-07-31T00:00:00+00:00")

    result = runner.invoke(app, ["create", "--project", "p1", "t1"])

    assert result.exit_code == 0
    keys = dict(session.put_calls)
    assert keys["projects/p1/tasks/t1/entered_by"] == "LLM"


def test_create_entered_by_user_is_normalized_uppercase(mocker):
    from tests.unit.fakes import FakeSession

    session = FakeSession({})
    mocker.patch("ztask.cli.open_session").return_value.__enter__.return_value = session
    mocker.patch("ztask.cli.get_iso_timestamp", return_value="2026-07-31T00:00:00+00:00")

    result = runner.invoke(app, ["create", "--project", "p1", "t1", "--entered-by", "user"])

    assert result.exit_code == 0
    keys = dict(session.put_calls)
    assert keys["projects/p1/tasks/t1/entered_by"] == "USER"


def test_create_unknown_entered_by_exits_with_error(mocker):
    from tests.unit.fakes import FakeSession

    session = FakeSession({})
    mocker.patch("ztask.cli.open_session").return_value.__enter__.return_value = session

    result = runner.invoke(app, ["create", "--project", "p1", "t1", "--entered-by", "bogus"])

    assert result.exit_code == 1
    assert "unknown entered-by" in result.stdout + (result.stderr or "")
    assert session.put_calls == []


def test_update_status_to_in_progress_sets_time_accepted(mocker):
    from tests.unit.fakes import FakeSession

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
    from tests.unit.fakes import FakeSession

    session = FakeSession({})
    mocker.patch("ztask.cli.open_session").return_value.__enter__.return_value = session
    mocker.patch("ztask.cli.fetch_status", return_value="IN_PROGRESS")
    mocker.patch("ztask.cli.get_iso_timestamp", return_value="2026-07-31T02:00:00+00:00")

    result = runner.invoke(app, ["update-status", "--project", "p1", "t1", "completed"])

    keys = dict(session.put_calls)
    assert keys["projects/p1/tasks/t1/status"] == "COMPLETED"
    assert keys["projects/p1/tasks/t1/time_completed"] == "2026-07-31T02:00:00+00:00"


def test_update_status_missing_task_exits_with_error(mocker):
    from tests.unit.fakes import FakeSession

    session = FakeSession({})
    mocker.patch("ztask.cli.open_session").return_value.__enter__.return_value = session
    mocker.patch("ztask.cli.fetch_status", return_value="UNKNOWN")

    result = runner.invoke(app, ["update-status", "--project", "p1", "missing", "completed"])

    assert result.exit_code == 1
    assert "not found" in result.stdout + (result.stderr or "")
    assert session.put_calls == []
