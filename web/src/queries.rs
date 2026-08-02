use std::collections::HashMap;

use crate::models::{HistoryEntry, Task};
use crate::zenoh_store::ZenohStore;

pub const TERMINAL_STATUS: &str = "COMPLETED";
pub const WIP_STATUSES: [&str; 3] = ["IN_PROGRESS", "WIP", "RUNNING"];

fn apply_field(task: &mut Task, field_name: &str, value: &str) {
    match field_name {
        "status" => task.status = value.to_string(),
        "time_entered" => task.time_entered = Some(value.to_string()),
        "time_accepted" => task.time_accepted = Some(value.to_string()),
        "time_completed" => task.time_completed = Some(value.to_string()),
        "acceptance_criteria" => task.acceptance_criteria = Some(value.to_string()),
        "entered_by" => task.entered_by = Some(value.to_string()),
        _ if field_name.starts_with("history/") => {
            if let Ok(entry) = serde_json::from_str::<HistoryEntry>(value) {
                task.history.push(entry);
            }
        }
        _ => {}
    }
}

pub async fn fetch_all_tasks(store: &dyn ZenohStore, project_id: &str) -> HashMap<String, Task> {
    let prefix = format!("projects/{project_id}/tasks/");
    let key_expr = format!("{prefix}**");
    let mut tasks: HashMap<String, Task> = HashMap::new();

    for (key, value) in store.get(&key_expr).await {
        let Some(relative) = key.strip_prefix(&prefix) else { continue };
        let mut parts = relative.splitn(2, '/');
        let task_id = parts.next().unwrap_or_default().to_string();
        let field_name = parts.next().unwrap_or_default();

        let task = tasks
            .entry(task_id.clone())
            .or_insert_with(|| Task::new(task_id.clone()));
        apply_field(task, field_name, &value);
    }

    tasks
}

pub async fn fetch_task(store: &dyn ZenohStore, project_id: &str, task_id: &str) -> Option<Task> {
    let prefix = format!("projects/{project_id}/tasks/{task_id}/");
    let key_expr = format!("{prefix}**");
    let mut result: Option<Task> = None;

    for (key, value) in store.get(&key_expr).await {
        let Some(field_name) = key.strip_prefix(&prefix) else { continue };
        let task = result.get_or_insert_with(|| Task::new(task_id));
        apply_field(task, field_name, &value);
    }

    result
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectSummary {
    pub id: String,
    pub total: usize,
    pub incomplete: usize,
    pub wip: usize,
}

pub async fn fetch_all_projects(store: &dyn ZenohStore) -> Vec<ProjectSummary> {
    let mut summaries: HashMap<String, ProjectSummary> = HashMap::new();

    for (key, value) in store.get("projects/*/tasks/*/status").await {
        let parts: Vec<&str> = key.split('/').collect();
        let Some(project_id) = parts.get(1) else { continue };
        let summary = summaries.entry(project_id.to_string()).or_insert_with(|| ProjectSummary {
            id: project_id.to_string(),
            total: 0,
            incomplete: 0,
            wip: 0,
        });
        summary.total += 1;
        let status = value.to_uppercase();
        if status != TERMINAL_STATUS {
            summary.incomplete += 1;
        }
        if WIP_STATUSES.contains(&status.as_str()) {
            summary.wip += 1;
        }
    }

    let mut result: Vec<ProjectSummary> = summaries.into_values().collect();
    result.sort_by(|a, b| a.id.cmp(&b.id));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zenoh_store::fake::FakeStore;

    #[tokio::test]
    async fn fetch_all_tasks_groups_fields_by_task_id() {
        let store = FakeStore::new()
            .seed("projects/p1/tasks/t1/status", "PENDING")
            .seed("projects/p1/tasks/t1/time_entered", "2026-07-31T00:00:00+00:00")
            .seed("projects/p1/tasks/t1/entered_by", "LLM")
            .seed("projects/p1/tasks/t2/status", "COMPLETED")
            .seed(
                "projects/p1/tasks/t1/history/2026-07-31T00-00-00",
                r#"{"timestamp":"t","from_status":"NONE","to_status":"PENDING","note":""}"#,
            );

        let tasks = fetch_all_tasks(&store, "p1").await;

        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks["t1"].status, "PENDING");
        assert_eq!(tasks["t1"].time_entered.as_deref(), Some("2026-07-31T00:00:00+00:00"));
        assert_eq!(tasks["t1"].entered_by.as_deref(), Some("LLM"));
        assert_eq!(tasks["t1"].history.len(), 1);
        assert_eq!(tasks["t2"].status, "COMPLETED");
    }

    #[tokio::test]
    async fn fetch_task_queries_task_specific_prefix_and_returns_none_if_missing() {
        let store = FakeStore::new().seed("projects/p1/tasks/t1/status", "PENDING");

        let found = fetch_task(&store, "p1", "t1").await;
        assert_eq!(found.unwrap().status, "PENDING");

        let missing = fetch_task(&store, "p1", "missing").await;
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn fetch_all_projects_groups_and_counts_by_status() {
        let store = FakeStore::new()
            .seed("projects/p1/tasks/t1/status", "PENDING")
            .seed("projects/p1/tasks/t2/status", "COMPLETED")
            .seed("projects/p2/tasks/t1/status", "IN_PROGRESS");

        let projects = fetch_all_projects(&store).await;

        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0], ProjectSummary { id: "p1".to_string(), total: 2, incomplete: 1, wip: 0 });
        assert_eq!(projects[1], ProjectSummary { id: "p2".to_string(), total: 1, incomplete: 1, wip: 1 });
    }
}
