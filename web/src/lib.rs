pub mod zenoh_store;

use std::sync::Arc;

use axum::routing::get;
use axum::Router;

use zenoh_store::ZenohStore;

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<dyn ZenohStore>,
}

pub fn app(state: AppState) -> Router {
    Router::new()
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
