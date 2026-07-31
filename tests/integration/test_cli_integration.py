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
