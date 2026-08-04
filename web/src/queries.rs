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
        // SDD fields
        "spec" => task.spec = Some(value.to_string()),
        "depends_on" => {
            task.depends_on = serde_json::from_str(value)
                .unwrap_or_else(|_| {
                    value.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
                });
        }
        "blocks" => {
            task.blocks = serde_json::from_str(value)
                .unwrap_or_else(|_| {
                    value.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
                });
        }
        // TDD fields
        "test_files" => {
            task.test_files = serde_json::from_str(value)
                .unwrap_or_else(|_| {
                    value.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
                });
        }
        "implementation_files" => {
            task.implementation_files = serde_json::from_str(value)
                .unwrap_or_else(|_| {
                    value.split(',').map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect()
                });
        }
        "tdd_phase" => task.tdd_phase = Some(value.to_string()),
        "test_command" => task.test_command = Some(value.to_string()),
        "verification_command" => task.verification_command = Some(value.to_string()),
        // Execution metadata
        "failure_reason" => task.failure_reason = Some(value.to_string()),
        "attempt_count" => {
            task.attempt_count = value.parse().unwrap_or(0);
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
    pub last_activity: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Total,
    Incomplete,
    Wip,
    Activity,
}

impl SortKey {
    pub fn parse(s: &str) -> SortKey {
        match s {
            "total" => SortKey::Total,
            "incomplete" => SortKey::Incomplete,
            "wip" => SortKey::Wip,
            "activity" => SortKey::Activity,
            _ => SortKey::Name,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            SortKey::Name => "name",
            SortKey::Total => "total",
            SortKey::Incomplete => "incomplete",
            SortKey::Wip => "wip",
            SortKey::Activity => "activity",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}

impl SortDir {
    pub fn parse(s: &str) -> SortDir {
        if s == "desc" {
            SortDir::Desc
        } else {
            SortDir::Asc
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            SortDir::Asc => "asc",
            SortDir::Desc => "desc",
        }
    }

    pub fn flip(self) -> SortDir {
        match self {
            SortDir::Asc => SortDir::Desc,
            SortDir::Desc => SortDir::Asc,
        }
    }
}

pub fn sort_projects(projects: &mut [ProjectSummary], key: SortKey, dir: SortDir) {
    projects.sort_by(|a, b| {
        let ordering = match key {
            SortKey::Name => a.id.cmp(&b.id),
            SortKey::Total => a.total.cmp(&b.total),
            SortKey::Incomplete => a.incomplete.cmp(&b.incomplete),
            SortKey::Wip => a.wip.cmp(&b.wip),
            SortKey::Activity => a.last_activity.cmp(&b.last_activity),
        };
        if dir == SortDir::Desc {
            ordering.reverse()
        } else {
            ordering
        }
    });
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
            last_activity: None,
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

    for (key, value) in store.get("projects/*/tasks/*/history/*").await {
        let parts: Vec<&str> = key.split('/').collect();
        let Some(project_id) = parts.get(1) else { continue };
        let Some(summary) = summaries.get_mut(*project_id) else { continue };
        let Some(timestamp) = serde_json::from_str::<HistoryEntry>(&value).ok().map(|entry| entry.timestamp) else {
            continue;
        };
        let should_update = match &summary.last_activity {
            Some(existing) => timestamp.as_str() > existing.as_str(),
            None => true,
        };
        if should_update {
            summary.last_activity = Some(timestamp);
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
    async fn fetch_all_tasks_parses_sdd_tdd_fields() {
        let store = FakeStore::new()
            .seed("projects/p1/tasks/t1/status", "IN_PROGRESS")
            .seed("projects/p1/tasks/t1/spec", "# Spec\n- item 1")
            .seed("projects/p1/tasks/t1/depends_on", r#"["t0","t2"]"#)
            .seed("projects/p1/tasks/t1/blocks", r#"["t3"]"#)
            .seed("projects/p1/tasks/t1/test_files", r#"["src/test.rs"]"#)
            .seed("projects/p1/tasks/t1/implementation_files", r#"["src/lib.rs"]"#)
            .seed("projects/p1/tasks/t1/tdd_phase", "RED")
            .seed("projects/p1/tasks/t1/test_command", "cargo test")
            .seed("projects/p1/tasks/t1/verification_command", "cargo clippy")
            .seed("projects/p1/tasks/t1/failure_reason", "compile error")
            .seed("projects/p1/tasks/t1/attempt_count", "3");

        let tasks = fetch_all_tasks(&store, "p1").await;
        let task = &tasks["t1"];

        assert_eq!(task.spec.as_deref(), Some("# Spec\n- item 1"));
        assert_eq!(task.depends_on, vec!["t0", "t2"]);
        assert_eq!(task.blocks, vec!["t3"]);
        assert_eq!(task.test_files, vec!["src/test.rs"]);
        assert_eq!(task.implementation_files, vec!["src/lib.rs"]);
        assert_eq!(task.tdd_phase.as_deref(), Some("RED"));
        assert_eq!(task.test_command.as_deref(), Some("cargo test"));
        assert_eq!(task.verification_command.as_deref(), Some("cargo clippy"));
        assert_eq!(task.failure_reason.as_deref(), Some("compile error"));
        assert_eq!(task.attempt_count, 3);
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
        assert_eq!(
            projects[0],
            ProjectSummary { id: "p1".to_string(), total: 2, incomplete: 1, wip: 0, last_activity: None }
        );
        assert_eq!(
            projects[1],
            ProjectSummary { id: "p2".to_string(), total: 1, incomplete: 1, wip: 1, last_activity: None }
        );
    }

    #[tokio::test]
    async fn fetch_all_projects_computes_last_activity_from_latest_history_entry() {
        let store = FakeStore::new()
            .seed("projects/p1/tasks/t1/status", "PENDING")
            .seed(
                "projects/p1/tasks/t1/history/2026-07-31T00-00-00",
                r#"{"timestamp":"2026-07-31T00:00:00+00:00","from_status":"NONE","to_status":"PENDING","note":""}"#,
            )
            .seed(
                "projects/p1/tasks/t1/history/2026-08-02T00-00-00",
                r#"{"timestamp":"2026-08-02T00:00:00+00:00","from_status":"PENDING","to_status":"IN_PROGRESS","note":""}"#,
            )
            .seed("projects/p2/tasks/t1/status", "PENDING");

        let projects = fetch_all_projects(&store).await;

        let p1 = projects.iter().find(|p| p.id == "p1").unwrap();
        assert_eq!(p1.last_activity.as_deref(), Some("2026-08-02T00:00:00+00:00"));

        let p2 = projects.iter().find(|p| p.id == "p2").unwrap();
        assert_eq!(p2.last_activity, None);
    }

    fn sample_projects() -> Vec<ProjectSummary> {
        vec![
            ProjectSummary { id: "b".to_string(), total: 5, incomplete: 1, wip: 2, last_activity: Some("2026-08-01T00:00:00+00:00".to_string()) },
            ProjectSummary { id: "a".to_string(), total: 2, incomplete: 3, wip: 0, last_activity: Some("2026-08-03T00:00:00+00:00".to_string()) },
        ]
    }

    #[test]
    fn sort_key_parse_defaults_to_name_for_unknown_values() {
        assert_eq!(SortKey::parse("bogus"), SortKey::Name);
        assert_eq!(SortKey::parse("total"), SortKey::Total);
        assert_eq!(SortKey::parse("incomplete"), SortKey::Incomplete);
        assert_eq!(SortKey::parse("wip"), SortKey::Wip);
        assert_eq!(SortKey::parse("activity"), SortKey::Activity);
    }

    #[test]
    fn sort_dir_parse_defaults_to_asc_for_unknown_values() {
        assert_eq!(SortDir::parse("bogus"), SortDir::Asc);
        assert_eq!(SortDir::parse("desc"), SortDir::Desc);
        assert_eq!(SortDir::parse("asc"), SortDir::Asc);
    }

    #[test]
    fn sort_dir_flip_toggles() {
        assert_eq!(SortDir::Asc.flip(), SortDir::Desc);
        assert_eq!(SortDir::Desc.flip(), SortDir::Asc);
    }

    #[test]
    fn sort_projects_by_name_ascending() {
        let mut projects = sample_projects();
        sort_projects(&mut projects, SortKey::Name, SortDir::Asc);
        assert_eq!(projects.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
    }

    #[test]
    fn sort_projects_by_total_descending() {
        let mut projects = sample_projects();
        sort_projects(&mut projects, SortKey::Total, SortDir::Desc);
        assert_eq!(projects.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), vec!["b", "a"]);
    }

    #[test]
    fn sort_projects_by_incomplete_ascending() {
        let mut projects = sample_projects();
        sort_projects(&mut projects, SortKey::Incomplete, SortDir::Asc);
        assert_eq!(projects.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), vec!["b", "a"]);
    }

    #[test]
    fn sort_projects_by_wip_descending() {
        let mut projects = sample_projects();
        sort_projects(&mut projects, SortKey::Wip, SortDir::Desc);
        assert_eq!(projects.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), vec!["b", "a"]);
    }

    #[test]
    fn sort_projects_by_activity_descending_puts_most_recent_first() {
        let mut projects = sample_projects();
        sort_projects(&mut projects, SortKey::Activity, SortDir::Desc);
        assert_eq!(projects.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
    }
}
