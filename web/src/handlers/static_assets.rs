use axum::http::header;
use axum::response::IntoResponse;

const STYLE_CSS: &str = include_str!("../../static/pico.min.css");
const HTMX_JS: &str = include_str!("../../static/htmx.min.js");

pub async fn style_css() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/css")], STYLE_CSS)
}

pub async fn htmx_js() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "application/javascript")], HTMX_JS)
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
    async fn style_css_served_with_css_content_type() {
        let store = Arc::new(FakeStore::new()) as Arc<dyn ZenohStore>;
        let response = app(AppState { store })
            .oneshot(Request::builder().uri("/static/style.css").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("content-type").unwrap(), "text/css");
    }

    #[tokio::test]
    async fn htmx_js_served_with_js_content_type() {
        let store = Arc::new(FakeStore::new()) as Arc<dyn ZenohStore>;
        let response = app(AppState { store })
            .oneshot(Request::builder().uri("/static/htmx.min.js").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers().get("content-type").unwrap(), "application/javascript");
    }
}
