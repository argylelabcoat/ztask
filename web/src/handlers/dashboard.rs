use askama::Template;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use serde::Deserialize;

use crate::render::HtmlTemplate;
use crate::{queries, AppState};

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub projects: Vec<queries::ProjectSummary>,
    pub sort_name_href: String,
    pub sort_total_href: String,
    pub sort_incomplete_href: String,
    pub sort_wip_href: String,
    pub sort_activity_href: String,
    pub active_sort: String,
    pub active_dir: String,
    pub form_error: Option<String>,
    pub form_project_id: String,
    pub form_task_id: String,
    pub form_criteria: String,
}

#[derive(Deserialize)]
pub struct SortQuery {
    #[serde(default)]
    sort: String,
    #[serde(default)]
    dir: String,
}

fn sort_link(col: queries::SortKey, current_sort: queries::SortKey, current_dir: queries::SortDir) -> String {
    let dir = if col == current_sort { current_dir.flip() } else { queries::SortDir::Asc };
    format!("/?sort={}&dir={}", col.as_str(), dir.as_str())
}

pub(crate) async fn build_dashboard(
    state: &AppState,
    sort: queries::SortKey,
    dir: queries::SortDir,
    form_error: Option<String>,
    form_project_id: String,
    form_task_id: String,
    form_criteria: String,
) -> DashboardTemplate {
    let mut projects = queries::fetch_all_projects(state.store.as_ref()).await;
    queries::sort_projects(&mut projects, sort, dir);

    DashboardTemplate {
        projects,
        sort_name_href: sort_link(queries::SortKey::Name, sort, dir),
        sort_total_href: sort_link(queries::SortKey::Total, sort, dir),
        sort_incomplete_href: sort_link(queries::SortKey::Incomplete, sort, dir),
        sort_wip_href: sort_link(queries::SortKey::Wip, sort, dir),
        sort_activity_href: sort_link(queries::SortKey::Activity, sort, dir),
        active_sort: sort.as_str().to_string(),
        active_dir: dir.as_str().to_string(),
        form_error,
        form_project_id,
        form_task_id,
        form_criteria,
    }
}

pub async fn show(State(state): State<AppState>, Query(query): Query<SortQuery>) -> HtmlTemplate<DashboardTemplate> {
    let sort = queries::SortKey::parse(&query.sort);
    let dir = queries::SortDir::parse(&query.dir);
    HtmlTemplate(build_dashboard(&state, sort, dir, None, String::new(), String::new(), String::new()).await)
}

#[derive(Deserialize)]
pub struct CreateProjectForm {
    project_id: String,
    task_id: String,
    #[serde(default)]
    criteria: String,
}

pub async fn create(State(state): State<AppState>, Form(form): Form<CreateProjectForm>) -> Response {
    if !crate::is_valid_id(&form.project_id) || !crate::is_valid_id(&form.task_id) {
        let template = build_dashboard(
            &state,
            queries::SortKey::Name,
            queries::SortDir::Asc,
            Some("Invalid project or task ID.".to_string()),
            form.project_id,
            form.task_id,
            form.criteria,
        )
        .await;
        return (StatusCode::BAD_REQUEST, HtmlTemplate(template)).into_response();
    }

    let existing = queries::fetch_all_tasks(state.store.as_ref(), &form.project_id).await;
    if !existing.is_empty() {
        let template = build_dashboard(
            &state,
            queries::SortKey::Name,
            queries::SortDir::Asc,
            Some(format!("Project '{}' already exists.", form.project_id)),
            form.project_id,
            form.task_id,
            form.criteria,
        )
        .await;
        return (StatusCode::CONFLICT, HtmlTemplate(template)).into_response();
    }

    let now = crate::iso_now();
    crate::tasks::create_task(state.store.as_ref(), &form.project_id, &form.task_id, &form.criteria, &now).await;
    Redirect::to(&format!("/projects/{}", form.project_id)).into_response()
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
    async fn dashboard_lists_projects_with_counts() {
        let store = FakeStore::new()
            .seed("projects/p1/tasks/t1/status", "PENDING")
            .seed("projects/p1/tasks/t2/status", "COMPLETED")
            .seed("projects/p2/tasks/t1/status", "IN_PROGRESS");
        let state = AppState { store: Arc::new(store) };

        let response = app(state)
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains("p1"));
        assert!(html.contains("p2"));
    }

    #[tokio::test]
    async fn dashboard_sorts_by_total_descending_when_requested() {
        let store = FakeStore::new()
            .seed("projects/small/tasks/t1/status", "PENDING")
            .seed("projects/big/tasks/t1/status", "PENDING")
            .seed("projects/big/tasks/t2/status", "PENDING");
        let state = AppState { store: Arc::new(store) };

        let response = app(state)
            .oneshot(Request::builder().uri("/?sort=total&dir=desc").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8(body.to_vec()).unwrap();
        let big_pos = html.find("big").unwrap();
        let small_pos = html.find("small").unwrap();
        assert!(big_pos < small_pos, "expected 'big' (total=2) to appear before 'small' (total=1) when sorted by total desc");
    }

    #[tokio::test]
    async fn dashboard_falls_back_to_default_sort_for_unknown_sort_value() {
        let store = FakeStore::new()
            .seed("projects/p1/tasks/t1/status", "PENDING")
            .seed("projects/p2/tasks/t1/status", "PENDING");
        let state = AppState { store: Arc::new(store) };

        let response = app(state)
            .oneshot(Request::builder().uri("/?sort=nonsense").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn create_project_redirects_and_creates_first_task() {
        let store = Arc::new(FakeStore::new());
        let state = AppState { store: store.clone() as Arc<dyn crate::zenoh_store::ZenohStore> };

        let response = app(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("project_id=newproj&task_id=t1&criteria=Given+X"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response.headers().get("location").unwrap().to_str().unwrap(), "/projects/newproj");

        let put_calls = store.put_calls.lock().unwrap();
        assert!(put_calls.iter().any(|(k, v)| k == "projects/newproj/tasks/t1/status" && v == "PENDING"));
    }

    #[tokio::test]
    async fn create_project_rejects_invalid_project_id() {
        let store = FakeStore::new();
        let state = AppState { store: Arc::new(store) };

        let response = app(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("project_id=bad*id&task_id=t1&criteria="))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn create_project_rejects_project_that_already_has_tasks() {
        let store = FakeStore::new().seed("projects/existing/tasks/t1/status", "PENDING");
        let state = AppState { store: Arc::new(store) };

        let response = app(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("project_id=existing&task_id=t2&criteria="))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
    }
}
