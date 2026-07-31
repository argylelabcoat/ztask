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
    task_id: str,
    project_id: str = typer.Option(..., "--project", "-p", help="Project ID"),
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
