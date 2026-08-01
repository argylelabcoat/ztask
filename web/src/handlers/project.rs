use askama::Template;
use axum::extract::{Path, Query, State};
use serde::Deserialize;

use crate::models::Task;
use crate::render::HtmlTemplate;
use crate::{queries, AppState};

#[derive(Template)]
#[template(path = "project.html")]
pub struct ProjectTemplate {
    pub project_id: String,
    pub tasks: Vec<Task>,
}

#[derive(Template)]
#[template(path = "task_row.html")]
pub struct TaskRowTemplate {
    pub project_id: String,
    pub task: Task,
}

#[derive(Deserialize)]
pub struct FilterQuery {
    #[serde(default = "default_filter")]
    filter: String,
}

fn default_filter() -> String {
    "all".to_string()
}

fn matches_filter(task: &Task, filter: &str) -> bool {
    let status = task.status.to_uppercase();
    match filter {
        "incomplete" => status != queries::TERMINAL_STATUS,
        "wip" => queries::WIP_STATUSES.contains(&status.as_str()),
        _ => true,
    }
}

pub async fn show(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Query(query): Query<FilterQuery>,
) -> HtmlTemplate<ProjectTemplate> {
    let all_tasks = queries::fetch_all_tasks(state.store.as_ref(), &project_id).await;
    let mut tasks: Vec<Task> = all_tasks.into_values().filter(|t| matches_filter(t, &query.filter)).collect();
    tasks.sort_by(|a, b| a.id.cmp(&b.id));

    HtmlTemplate(ProjectTemplate { project_id, tasks })
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use std::sync::Arc;
    use tower::ServiceExt;

    use crate::zenoh_store::fake::FakeStore;
    use crate::{app, AppState};

    #[tokio::test]
    async fn project_page_lists_only_incomplete_tasks_when_filtered() {
        let store = FakeStore::new()
            .seed("projects/p1/tasks/t1/status", "PENDING")
            .seed("projects/p1/tasks/t2/status", "COMPLETED");
        let state = AppState { store: Arc::new(store) };

        let response = app(state)
            .oneshot(Request::builder().uri("/projects/p1?filter=incomplete").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains("t1"));
        assert!(!html.contains("t2"));
    }

    #[tokio::test]
    async fn project_page_defaults_to_all_tasks() {
        let store = FakeStore::new()
            .seed("projects/p1/tasks/t1/status", "PENDING")
            .seed("projects/p1/tasks/t2/status", "COMPLETED");
        let state = AppState { store: Arc::new(store) };

        let response = app(state)
            .oneshot(Request::builder().uri("/projects/p1").body(Body::empty()).unwrap())
            .await
            .unwrap();

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains("t1"));
        assert!(html.contains("t2"));
    }
}
