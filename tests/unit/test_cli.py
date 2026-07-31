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
