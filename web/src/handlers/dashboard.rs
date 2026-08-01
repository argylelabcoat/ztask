use askama::Template;
use axum::extract::State;

use crate::render::HtmlTemplate;
use crate::{queries, AppState};

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub projects: Vec<queries::ProjectSummary>,
}

pub async fn show(State(state): State<AppState>) -> HtmlTemplate<DashboardTemplate> {
    let projects = queries::fetch_all_projects(state.store.as_ref()).await;
    HtmlTemplate(DashboardTemplate { projects })
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
}
