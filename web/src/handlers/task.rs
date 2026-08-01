use askama::Template;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Form;
use serde::Deserialize;

use crate::models::Task;
use crate::render::HtmlTemplate;
use crate::AppState;

#[derive(Template)]
#[template(path = "task_row.html")]
pub struct TaskRowTemplate {
    pub project_id: String,
    pub task: Task,
}

#[derive(Deserialize)]
pub struct UpdateStatusForm {
    status: String,
    #[serde(default)]
    note: String,
}

pub async fn update_status(
    State(state): State<AppState>,
    Path((project_id, task_id)): Path<(String, String)>,
    Form(form): Form<UpdateStatusForm>,
) -> Result<HtmlTemplate<TaskRowTemplate>, StatusCode> {
    let now = crate::iso_now();
    match crate::tasks::update_status(state.store.as_ref(), &project_id, &task_id, &form.status, &form.note, &now).await {
        Ok(task) => Ok(HtmlTemplate(TaskRowTemplate { project_id, task })),
        Err(crate::tasks::TaskError::NotFound) => Err(StatusCode::NOT_FOUND),
    }
}

#[derive(Deserialize)]
pub struct EditCriteriaForm {
    criteria: String,
}

pub async fn edit_criteria(
    State(state): State<AppState>,
    Path((project_id, task_id)): Path<(String, String)>,
    Form(form): Form<EditCriteriaForm>,
) -> Result<HtmlTemplate<TaskRowTemplate>, StatusCode> {
    let now = crate::iso_now();
    match crate::tasks::edit_criteria(state.store.as_ref(), &project_id, &task_id, &form.criteria, &now).await {
        Ok(task) => Ok(HtmlTemplate(TaskRowTemplate { project_id, task })),
        Err(crate::tasks::TaskError::NotFound) => Err(StatusCode::NOT_FOUND),
    }
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use std::sync::Arc;
    use tower::ServiceExt;

    use crate::zenoh_store::fake::FakeStore;
    use crate::zenoh_store::ZenohStore;
    use crate::{app, AppState};

    #[tokio::test]
    async fn update_status_updates_row_and_persists() {
        let store = Arc::new(FakeStore::new().seed("projects/p1/tasks/t1/status", "PENDING"));
        let state = AppState { store: store.clone() as Arc<dyn ZenohStore> };

        let response = app(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects/p1/tasks/t1/status")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("status=completed&note=done"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let put_calls = store.put_calls.lock().unwrap();
        assert!(put_calls.iter().any(|(k, v)| k == "projects/p1/tasks/t1/status" && v == "COMPLETED"));
        assert!(put_calls.iter().any(|(k, _)| k == "projects/p1/tasks/t1/time_completed"));
    }

    #[tokio::test]
    async fn update_status_missing_task_returns_404() {
        let store = Arc::new(FakeStore::new());
        let state = AppState { store: store as Arc<dyn ZenohStore> };

        let response = app(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects/p1/tasks/missing/status")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("status=completed"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn edit_criteria_updates_row_and_persists() {
        let store = Arc::new(FakeStore::new().seed("projects/p1/tasks/t1/status", "PENDING"));
        let state = AppState { store: store.clone() as Arc<dyn ZenohStore> };

        let response = app(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects/p1/tasks/t1/criteria")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("criteria=Given+Y"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let put_calls = store.put_calls.lock().unwrap();
        assert!(put_calls.iter().any(|(k, v)| k == "projects/p1/tasks/t1/acceptance_criteria" && v == "Given Y"));
    }

    #[tokio::test]
    async fn edit_criteria_missing_task_returns_404() {
        let store = Arc::new(FakeStore::new());
        let state = AppState { store: store as Arc<dyn ZenohStore> };

        let response = app(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects/p1/tasks/missing/criteria")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("criteria=x"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
