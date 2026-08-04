use askama::Template;
use axum::extract::{Path, State};
use axum::http::StatusCode;

use crate::render::HtmlTemplate;
use crate::{metrics, queries, AppState};

#[derive(Template)]
#[template(path = "metrics.html")]
pub struct MetricsTemplate {
    pub project_id: String,
    pub donut_segments: Vec<metrics::DonutSegment>,
    pub total_tasks: usize,
    pub velocity: Vec<metrics::VelocityPoint>,
    pub matrix: metrics::TransitionMatrix,
    pub timings: Vec<metrics::TaskTiming>,
    pub flagged: Vec<metrics::TaskTiming>,
}

pub async fn show(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<HtmlTemplate<MetricsTemplate>, StatusCode> {
    if !crate::is_valid_id(&project_id) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let tasks = queries::fetch_all_tasks(state.store.as_ref(), &project_id).await;
    let thresholds = queries::fetch_thresholds(state.store.as_ref(), &project_id).await;
    let now = chrono::Utc::now();

    let breakdown = metrics::compute_status_breakdown(&tasks);
    let donut_segments = metrics::compute_donut_segments(&breakdown);
    let velocity = metrics::compute_velocity(&tasks);
    let matrix = metrics::compute_transition_matrix(&tasks);
    let timings = metrics::compute_timing_table(&tasks, &thresholds, now);
    let flagged: Vec<metrics::TaskTiming> = timings.iter().filter(|t| t.stuck || t.churning).cloned().collect();

    Ok(HtmlTemplate(MetricsTemplate {
        project_id,
        donut_segments,
        total_tasks: tasks.len(),
        velocity,
        matrix,
        timings,
        flagged,
    }))
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
    async fn metrics_page_renders_for_project_with_tasks() {
        let store = FakeStore::new()
            .seed("projects/p1/tasks/t1/status", "COMPLETED")
            .seed(
                "projects/p1/tasks/t1/history/2026-08-01T00-00-00",
                r#"{"timestamp":"2026-08-01T00:00:00+00:00","from_status":"NONE","to_status":"PENDING","note":""}"#,
            )
            .seed(
                "projects/p1/tasks/t1/history/2026-08-01T01-00-00",
                r#"{"timestamp":"2026-08-01T01:00:00+00:00","from_status":"PENDING","to_status":"COMPLETED","note":""}"#,
            );
        let state = AppState { store: Arc::new(store) };

        let response = app(state)
            .oneshot(Request::builder().uri("/projects/p1/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains("t1"));
        assert!(html.contains("Completed"));
    }

    #[tokio::test]
    async fn metrics_page_rejects_invalid_project_id() {
        let store = FakeStore::new();
        let state = AppState { store: Arc::new(store) };

        let response = app(state)
            .oneshot(Request::builder().uri("/projects/*/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn metrics_page_handles_project_with_no_tasks() {
        let store = FakeStore::new();
        let state = AppState { store: Arc::new(store) };

        let response = app(state)
            .oneshot(Request::builder().uri("/projects/empty/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
