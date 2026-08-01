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
    elif field_name == "entered_by":
        task.entered_by = value
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
