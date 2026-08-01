use std::process::Command;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

use ztask_web::zenoh_client::{open_session, RealZenohStore};
use ztask_web::zenoh_store::ZenohStore;
use ztask_web::{app, AppState};

const IMAGE: &str = "ztask-router:integration-test";
const CONTAINER_NAME: &str = "ztask-web-integration-test-router";
const PORT: u16 = 17448;

fn runtime() -> &'static str {
    if Command::new("container").arg("--version").output().is_ok() {
        "container"
    } else {
        "docker"
    }
}

fn wait_for_port(port: u16, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if std::net::TcpStream::connect(("127.0.0.1", port)).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(500));
    }
    panic!("router did not open port {port} within {timeout:?}");
}

struct RouterGuard {
    runtime: &'static str,
}

impl Drop for RouterGuard {
    fn drop(&mut self) {
        let _ = Command::new(self.runtime).args(["stop", CONTAINER_NAME]).output();
    }
}

fn start_router() -> RouterGuard {
    let rt = runtime();

    let status = Command::new(rt)
        .args(["build", "-f", "docker/router/Dockerfile", "-t", IMAGE, "."])
        .current_dir("..")
        .status()
        .expect("failed to run container build");
    assert!(status.success(), "router image build failed");

    let _ = Command::new(rt).args(["rm", "-f", CONTAINER_NAME]).output();

    let status = Command::new(rt)
        .args(["run", "--rm", "-d", "--name", CONTAINER_NAME, "-p", &format!("{PORT}:7447"), IMAGE])
        .status()
        .expect("failed to run container run");
    assert!(status.success(), "router container failed to start");

    wait_for_port(PORT, Duration::from_secs(30));
    RouterGuard { runtime: rt }
}

async fn build_app() -> axum::Router {
    let session = open_session(&format!("tcp/localhost:{PORT}"))
        .await
        .expect("failed to open zenoh session");

    // `zenoh::open()` can resolve before the transport link it just opened is
    // fully registered in the session's routing table: publishing (or
    // querying) immediately after `open()` returns can race that
    // registration and silently go nowhere (compute_data_route returns an
    // empty route). Give the transport a moment to finish wiring up before
    // this session is used for real traffic.
    tokio::time::sleep(Duration::from_millis(1000)).await;

    let store: Arc<dyn ZenohStore> = Arc::new(RealZenohStore::new(session));
    app(AppState { store })
}

/// The Garry storage-manager plugin persists puts asynchronously off of a
/// subscriber callback, so a `get` issued immediately after a `put` ack can
/// still race the write. Poll the task detail endpoint until it reports the
/// task exists (or time out), mirroring how a real client would tolerate the
/// router's eventual consistency instead of assuming a single query is enough.
async fn wait_for_task_visible(router: &axum::Router, project_id: &str, task_id: &str, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/projects/{project_id}/tasks/{task_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        if response.status() == StatusCode::OK {
            return;
        }
        if Instant::now() >= deadline {
            let logs = Command::new(runtime()).args(["logs", CONTAINER_NAME]).output();
            if let Ok(out) = logs {
                eprintln!("--- container logs (stdout) ---\n{}", String::from_utf8_lossy(&out.stdout));
                eprintln!("--- container logs (stderr) ---\n{}", String::from_utf8_lossy(&out.stderr));
            }
            panic!("task {project_id}/{task_id} did not become visible within {timeout:?}");
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

/// Same eventual-consistency reasoning as `wait_for_task_visible`, but for a
/// field value change (e.g. a status update) rather than initial creation.
async fn wait_for_task_status(router: &axum::Router, project_id: &str, task_id: &str, expected_status: &str, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    loop {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!("/projects/{project_id}/tasks/{task_id}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        if response.status() == StatusCode::OK {
            let body = response.into_body().collect().await.unwrap().to_bytes();
            let html = String::from_utf8(body.to_vec()).unwrap();
            if html.contains(expected_status) {
                return;
            }
        }
        if Instant::now() >= deadline {
            panic!("task {project_id}/{task_id} did not reach status {expected_status} within {timeout:?}");
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn create_then_get_round_trips_through_real_router() {
    let _guard = start_router();
    let router = build_app().await;

    let create_response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/projects/itest/tasks")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("task_id=task-1&criteria=Given+X"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::OK);

    wait_for_task_visible(&router, "itest", "task-1", Duration::from_secs(10)).await;

    let detail_response = router
        .oneshot(Request::builder().uri("/projects/itest/tasks/task-1").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(detail_response.status(), StatusCode::OK);
    let body = detail_response.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("task-1"));
    assert!(html.contains("Given X"));
}

#[tokio::test(flavor = "multi_thread")]
#[ignore]
async fn update_status_persists_and_appears_in_project_list() {
    let _guard = start_router();
    let router = build_app().await;

    let create_response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/projects/itest2/tasks")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("task_id=task-2&criteria="))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::OK, "create failed for task-2");

    wait_for_task_visible(&router, "itest2", "task-2", Duration::from_secs(15)).await;

    let update_response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/projects/itest2/tasks/task-2/status")
                .header("content-type", "application/x-www-form-urlencoded")
                .body(Body::from("status=in_progress&note=starting"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);

    wait_for_task_status(&router, "itest2", "task-2", "IN_PROGRESS", Duration::from_secs(10)).await;

    let list_response = router
        .oneshot(Request::builder().uri("/projects/itest2?filter=wip").body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);
    let body = list_response.into_body().collect().await.unwrap().to_bytes();
    let html = String::from_utf8(body.to_vec()).unwrap();
    assert!(html.contains("task-2"));
    assert!(html.contains("IN_PROGRESS"));
}
