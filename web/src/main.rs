use std::sync::Arc;

use ztask_web::zenoh_client::{open_session, resolve_endpoint, RealZenohStore};
use ztask_web::zenoh_store::ZenohStore;
use ztask_web::{app, AppState};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let endpoint = resolve_endpoint();
    tracing::info!("connecting to zenoh router at {endpoint}");

    let session = match open_session(&endpoint).await {
        Ok(session) => session,
        Err(err) => {
            tracing::error!("failed to open zenoh session: {err}");
            std::process::exit(1);
        }
    };

    // zenoh::open() can return successfully before the transport link is
    // fully registered in the session's routing table — a request issued
    // in that window can silently compute an empty route. A short settle
    // delay avoids that cold-start race without adding meaningful startup
    // latency.
    tokio::time::sleep(std::time::Duration::from_millis(500)).await;

    let store: Arc<dyn ZenohStore> = Arc::new(RealZenohStore::new(session));
    let state = AppState { store };

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    tracing::info!("listening on 0.0.0.0:8080");
    axum::serve(listener, app(state)).await.unwrap();
}
