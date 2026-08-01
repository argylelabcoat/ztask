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
    if filter_type not in ("all", "incomplete", "wip"):
        typer.echo(
            f"Error: unknown filter '{filter_type}'. Expected 'all', 'incomplete', or 'wip'.",
            err=True,
        )
        raise typer.Exit(code=1)

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

        if old_status == "UNKNOWN":
            typer.echo(f"Error: Task '{task_id}' not found in project '{project_id}'.", err=True)
            raise typer.Exit(code=1)

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


if __name__ == "__main__":
    app()
