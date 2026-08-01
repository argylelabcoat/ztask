use crate::models::Task;
use crate::queries::{self, TERMINAL_STATUS, WIP_STATUSES};
use crate::zenoh_store::ZenohStore;

pub const ENTERED_BY_USER: &str = "USER";

#[derive(Debug, Clone, PartialEq)]
pub enum TaskError {
    NotFound,
}

fn history_key(now: &str) -> String {
    now.replace(':', "-")
}

pub async fn create_task(
    store: &dyn ZenohStore,
    project_id: &str,
    task_id: &str,
    criteria: &str,
    now: &str,
) -> Task {
    let base = format!("projects/{project_id}/tasks/{task_id}");
    store.put(&format!("{base}/status"), "PENDING").await;
    store.put(&format!("{base}/time_entered"), now).await;
    store.put(&format!("{base}/entered_by"), ENTERED_BY_USER).await;
    if !criteria.is_empty() {
        store.put(&format!("{base}/acceptance_criteria"), criteria).await;
    }

    let history_value = serde_json::json!({
        "timestamp": now,
        "from_status": "NONE",
        "to_status": "PENDING",
        "note": "Task created via web UI",
    })
    .to_string();
    store
        .put(&format!("{base}/history/{}", history_key(now)), &history_value)
        .await;

    queries::fetch_task(store, project_id, task_id)
        .await
        .unwrap_or_else(|| Task::new(task_id))
}

pub async fn update_status(
    store: &dyn ZenohStore,
    project_id: &str,
    task_id: &str,
    status: &str,
    note: &str,
    now: &str,
) -> Result<Task, TaskError> {
    let base = format!("projects/{project_id}/tasks/{task_id}");
    let old_status = queries::fetch_status(store, project_id, task_id).await;
    if old_status == "UNKNOWN" {
        return Err(TaskError::NotFound);
    }

    let new_status = status.to_uppercase();
    store.put(&format!("{base}/status"), &new_status).await;

    let is_wip = |s: &str| WIP_STATUSES.contains(&s);
    if is_wip(&new_status) && !is_wip(&old_status) {
        store.put(&format!("{base}/time_accepted"), now).await;
    } else if new_status == TERMINAL_STATUS {
        store.put(&format!("{base}/time_completed"), now).await;
    }

    let history_value = serde_json::json!({
        "timestamp": now,
        "from_status": old_status,
        "to_status": new_status,
        "note": note,
    })
    .to_string();
    store
        .put(&format!("{base}/history/{}", history_key(now)), &history_value)
        .await;

    queries::fetch_task(store, project_id, task_id)
        .await
        .ok_or(TaskError::NotFound)
}

pub async fn edit_criteria(
    store: &dyn ZenohStore,
    project_id: &str,
    task_id: &str,
    criteria: &str,
    now: &str,
) -> Result<Task, TaskError> {
    let base = format!("projects/{project_id}/tasks/{task_id}");
    let status = queries::fetch_status(store, project_id, task_id).await;
    if status == "UNKNOWN" {
        return Err(TaskError::NotFound);
    }

    store.put(&format!("{base}/acceptance_criteria"), criteria).await;

    let history_value = serde_json::json!({
        "timestamp": now,
        "from_status": status,
        "to_status": status,
        "note": "criteria updated",
    })
    .to_string();
    store
        .put(&format!("{base}/history/{}", history_key(now)), &history_value)
        .await;

    queries::fetch_task(store, project_id, task_id)
        .await
        .ok_or(TaskError::NotFound)
}

pub async fn delete_task(store: &dyn ZenohStore, project_id: &str, task_id: &str) {
    store
        .delete(&format!("projects/{project_id}/tasks/{task_id}/**"))
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zenoh_store::fake::FakeStore;

    #[tokio::test]
    async fn create_task_writes_expected_fields_and_history() {
        let store = FakeStore::new();
        let task = create_task(&store, "p1", "t1", "Given X", "2026-08-01T00:00:00+00:00").await;

        assert_eq!(task.status, "PENDING");
        assert_eq!(task.entered_by.as_deref(), Some("USER"));
        assert_eq!(task.time_entered.as_deref(), Some("2026-08-01T00:00:00+00:00"));
        assert_eq!(task.acceptance_criteria.as_deref(), Some("Given X"));
        assert_eq!(task.history.len(), 1);
        assert_eq!(task.history[0].to_status, "PENDING");

        let put_calls = store.put_calls.lock().unwrap();
        assert!(put_calls.contains(&("projects/p1/tasks/t1/entered_by".to_string(), "USER".to_string())));
    }

    #[tokio::test]
    async fn create_task_without_criteria_leaves_it_unset() {
        let store = FakeStore::new();
        let task = create_task(&store, "p1", "t1", "", "2026-08-01T00:00:00+00:00").await;
        assert!(task.acceptance_criteria.is_none());
    }

    #[tokio::test]
    async fn update_status_to_wip_sets_time_accepted() {
        let store = FakeStore::new().seed("projects/p1/tasks/t1/status", "PENDING");
        let task = update_status(&store, "p1", "t1", "in_progress", "starting", "2026-08-01T01:00:00+00:00")
            .await
            .unwrap();
        assert_eq!(task.status, "IN_PROGRESS");
        assert_eq!(task.time_accepted.as_deref(), Some("2026-08-01T01:00:00+00:00"));
        assert!(task.time_completed.is_none());
    }

    #[tokio::test]
    async fn update_status_to_completed_sets_time_completed() {
        let store = FakeStore::new().seed("projects/p1/tasks/t1/status", "IN_PROGRESS");
        let task = update_status(&store, "p1", "t1", "completed", "", "2026-08-01T02:00:00+00:00")
            .await
            .unwrap();
        assert_eq!(task.status, "COMPLETED");
        assert_eq!(task.time_completed.as_deref(), Some("2026-08-01T02:00:00+00:00"));
    }

    #[tokio::test]
    async fn update_status_missing_task_returns_not_found() {
        let store = FakeStore::new();
        let result = update_status(&store, "p1", "missing", "completed", "", "2026-08-01T00:00:00+00:00").await;
        assert_eq!(result, Err(TaskError::NotFound));
    }

    #[tokio::test]
    async fn edit_criteria_updates_value_and_appends_history() {
        let store = FakeStore::new()
            .seed("projects/p1/tasks/t1/status", "PENDING")
            .seed("projects/p1/tasks/t1/acceptance_criteria", "old");
        let task = edit_criteria(&store, "p1", "t1", "new criteria", "2026-08-01T03:00:00+00:00")
            .await
            .unwrap();
        assert_eq!(task.acceptance_criteria.as_deref(), Some("new criteria"));
        assert!(task.history.iter().any(|h| h.note == "criteria updated"));
    }

    #[tokio::test]
    async fn edit_criteria_missing_task_returns_not_found() {
        let store = FakeStore::new();
        let result = edit_criteria(&store, "p1", "missing", "x", "2026-08-01T00:00:00+00:00").await;
        assert_eq!(result, Err(TaskError::NotFound));
    }

    #[tokio::test]
    async fn delete_task_removes_all_keys_under_the_task_prefix() {
        let store = FakeStore::new()
            .seed("projects/p1/tasks/t1/status", "PENDING")
            .seed("projects/p1/tasks/t1/time_entered", "now");
        delete_task(&store, "p1", "t1").await;
        assert!(queries::fetch_task(&store, "p1", "t1").await.is_none());
        assert_eq!(store.delete_calls.lock().unwrap().as_slice(), ["projects/p1/tasks/t1/**".to_string()]);
    }
}
