use askama::Template;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Form;
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
) -> Result<HtmlTemplate<ProjectTemplate>, StatusCode> {
    if !crate::is_valid_id(&project_id) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let all_tasks = queries::fetch_all_tasks(state.store.as_ref(), &project_id).await;
    let mut tasks: Vec<Task> = all_tasks.into_values().filter(|t| matches_filter(t, &query.filter)).collect();
    tasks.sort_by(|a, b| a.id.cmp(&b.id));

    Ok(HtmlTemplate(ProjectTemplate { project_id, tasks }))
}

#[derive(Deserialize)]
pub struct CreateTaskForm {
    task_id: String,
    #[serde(default)]
    criteria: String,
}

pub async fn create(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
    Form(form): Form<CreateTaskForm>,
) -> Result<HtmlTemplate<crate::handlers::task::TaskRowTemplate>, StatusCode> {
    if !crate::is_valid_id(&project_id) || !crate::is_valid_id(&form.task_id) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let now = crate::iso_now();
    let task = crate::tasks::create_task(state.store.as_ref(), &project_id, &form.task_id, &form.criteria, &now).await;
    Ok(HtmlTemplate(crate::handlers::task::TaskRowTemplate { project_id, task }))
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

    #[tokio::test]
    async fn project_page_with_wildcard_id_rejected() {
        let store = FakeStore::new();
        let state = AppState { store: Arc::new(store) };

        let response = app(state)
            .oneshot(Request::builder().uri("/projects/*").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn create_task_adds_row_and_persists_fields() {
        let store = Arc::new(FakeStore::new());
        let state = AppState { store: store.clone() as Arc<dyn crate::zenoh_store::ZenohStore> };

        let response = app(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects/p1/tasks")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("task_id=t1&criteria=Given+X"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains("t1"));

        let put_calls = store.put_calls.lock().unwrap();
        assert!(put_calls.iter().any(|(k, v)| k == "projects/p1/tasks/t1/status" && v == "PENDING"));
        assert!(put_calls.iter().any(|(k, v)| k == "projects/p1/tasks/t1/entered_by" && v == "USER"));
    }
}
