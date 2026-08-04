pub mod handlers;
pub mod metrics;
pub mod models;
pub mod queries;
pub mod render;
pub mod tasks;
pub mod zenoh_client;
pub mod zenoh_store;

use std::sync::Arc;

use axum::routing::{get, post};
use axum::Router;

use zenoh_store::ZenohStore;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<dyn ZenohStore>,
}

pub fn iso_now() -> String {
    chrono::Utc::now().to_rfc3339()
}

pub fn is_valid_id(value: &str) -> bool {
    !value.is_empty() && !value.contains(['*', '?', '#', '$', '/'])
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/", get(handlers::dashboard::show))
        .route("/projects", post(handlers::dashboard::create))
        .route("/projects/{id}", get(handlers::project::show))
        .route("/projects/{id}/tasks", post(handlers::project::create))
        .route("/projects/{id}/tasks/{task_id}/status", post(handlers::task::update_status))
        .route("/projects/{id}/tasks/{task_id}/criteria", post(handlers::task::edit_criteria))
        .route("/projects/{id}/tasks/{task_id}", get(handlers::task::show).delete(handlers::task::delete))
        .route("/static/style.css", get(handlers::static_assets::style_css))
        .route("/static/htmx.min.js", get(handlers::static_assets::htmx_js))
        .route("/healthz", get(|| async { "ok" }))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    struct NullStore;

    #[async_trait::async_trait]
    impl ZenohStore for NullStore {
        async fn get(&self, _key_expr: &str) -> Vec<(String, String)> {
            Vec::new()
        }
        async fn put(&self, _key_expr: &str, _value: &str) {}
        async fn delete(&self, _key_expr: &str) {}
    }

    #[tokio::test]
    async fn healthz_returns_ok() {
        let state = AppState { store: Arc::new(NullStore) };
        let response = app(state)
            .oneshot(Request::builder().uri("/healthz").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"ok");
    }
}
