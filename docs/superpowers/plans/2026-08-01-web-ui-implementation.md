# Zenoh Task Tracker Web UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build a Rust (axum + askama + htmx) admin web UI at `zenoh-tasks/web/` with an all-projects dashboard and a per-project dashboard supporting create/update-status/edit-criteria/delete, backed directly by the zenoh Rust SDK against the existing router.

**Architecture:** A `ZenohStore` trait (get/put/delete) abstracts all zenoh access behind a testable seam; `queries.rs`/`tasks.rs` hold pure business logic against that trait (unit-testable with an in-memory `FakeStore`); axum handlers wire HTTP routes to that logic and render Askama templates, with htmx driving partial-page swaps for writes; a thin `main.rs` wires the real zenoh-backed store at startup.

**Tech Stack:** Rust, axum 0.8, askama 0.16, zenoh 1.9.0 (Rust SDK), tokio, serde/serde_json, chrono, async-trait, htmx (vendored), Pico.css (vendored).

## Global Constraints

- Crate lives at `zenoh-tasks/web/` (package `ztask-web`, lib target auto-named `ztask_web`).
- `zenoh = "=1.9.0"` — exact pin, matching the router's pinned version (see `docker/router/Dockerfile`; zenoh's wire/plugin compatibility is not guaranteed across versions).
- Connects via the `ZTASK_ZENOH_ENDPOINT` env var — same name the Python CLI (`ztask/zenoh_client.py`) already uses — defaulting to `tcp/localhost:7447`.
- No schema changes: reuses `projects/<id>/tasks/<task_id>/{status,time_entered,time_accepted,time_completed,acceptance_criteria,entered_by,history/<ts>}` exactly as written by `ztask/cli.py`.
- Tasks created through the web UI always write `entered_by = "USER"` (no user-facing selector — this UI is for humans; the CLI defaults to `"LLM"`).
- `TERMINAL_STATUS = "COMPLETED"`, `WIP_STATUSES = ["IN_PROGRESS", "WIP", "RUNNING"]` — mirrors `ztask/cli.py`'s constants exactly.
- axum 0.8 uses `{param}` path syntax (not the old `:param` syntax — that panics on registration in 0.8).
- No auth, no live/real-time updates (refresh-on-load only) — out of scope per the design spec.
- Every new Rust file with logic gets unit tests using the in-memory `FakeStore` (Task 2); no test talks to a real network until the final opt-in integration test (Task 18).

---

### Task 1: Scaffold the `web` crate with a health-check route

**Files:**
- Create: `web/Cargo.toml`
- Create: `web/src/lib.rs`
- Create: `web/src/zenoh_store.rs`

**Interfaces:**
- Produces: `trait ZenohStore: Send + Sync { async fn get(&self, key_expr: &str) -> Vec<(String, String)>; async fn put(&self, key_expr: &str, value: &str); async fn delete(&self, key_expr: &str); }` in `zenoh_store.rs`
- Produces: `struct AppState { pub store: Arc<dyn ZenohStore> }` (Clone) in `lib.rs`
- Produces: `fn app(state: AppState) -> axum::Router` in `lib.rs`

- [ ] **Step 1: Create the crate**

Run from the repo root:

```bash
mkdir -p web/src
```

Create `web/Cargo.toml`:

```toml
[package]
name = "ztask-web"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = "0.8"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }
askama = "0.16"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
async-trait = "0.1"
zenoh = "=1.9.0"
chrono = { version = "0.4", default-features = false, features = ["clock"] }
tracing = "0.1"
tracing-subscriber = "0.3"

[dev-dependencies]
tower = { version = "0.5", features = ["util"] }
http-body-util = "0.1"
```

- [ ] **Step 2: Write `zenoh_store.rs`**

```rust
use async_trait::async_trait;

#[async_trait]
pub trait ZenohStore: Send + Sync {
    async fn get(&self, key_expr: &str) -> Vec<(String, String)>;
    async fn put(&self, key_expr: &str, value: &str);
    async fn delete(&self, key_expr: &str);
}
```

- [ ] **Step 3: Write `lib.rs` with a `/healthz` route and a failing-first test**

```rust
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
```

- [ ] **Step 4: Run the test**

Run: `cd web && cargo test`
Expected: `test tests::healthz_returns_ok ... ok` (this compiles and passes immediately since there's no separate "write test then implement" cycle for scaffolding — the route is trivial. If it fails to compile, check that `web/Cargo.toml` deps match Step 1 exactly.)

- [ ] **Step 5: Commit**

```bash
git add web/Cargo.toml web/Cargo.lock web/src/lib.rs web/src/zenoh_store.rs
git commit -m "feat(web): scaffold ztask-web crate with health check"
```

---

### Task 2: In-memory `FakeStore` test double

**Files:**
- Modify: `web/src/zenoh_store.rs`

**Interfaces:**
- Consumes: `ZenohStore` trait (Task 1)
- Produces: `zenoh_store::fake::FakeStore` — `FakeStore::new() -> Self`, `.seed(key: &str, value: &str) -> Self` (builder), `pub put_calls: Mutex<Vec<(String,String)>>`, `pub delete_calls: Mutex<Vec<String>>`. Implements `ZenohStore` with real prefix/wildcard-matching semantics (not canned responses) so put-then-get and delete-then-get behave like the real store within a single test.

This fake only needs to support the exact three key-expression shapes this app issues: an exact key, a `prefix**` wildcard suffix, and the fixed single-segment-wildcard shape `projects/*/tasks/*/status`. It is not a general zenoh key-expression matcher.

- [ ] **Step 1: Write the failing tests**

Append to `web/src/zenoh_store.rs`:

```rust
#[cfg(test)]
pub mod fake {
    use super::ZenohStore;
    use async_trait::async_trait;
    use std::collections::BTreeMap;
    use std::sync::Mutex;

    #[derive(Default)]
    pub struct FakeStore {
        data: Mutex<BTreeMap<String, String>>,
        pub put_calls: Mutex<Vec<(String, String)>>,
        pub delete_calls: Mutex<Vec<String>>,
    }

    impl FakeStore {
        pub fn new() -> Self {
            Self::default()
        }

        pub fn seed(self, key: &str, value: &str) -> Self {
            self.data.lock().unwrap().insert(key.to_string(), value.to_string());
            self
        }
    }

    fn single_segment_match(pattern: &str, key: &str) -> bool {
        let pattern_parts: Vec<&str> = pattern.split('/').collect();
        let key_parts: Vec<&str> = key.split('/').collect();
        pattern_parts.len() == key_parts.len()
            && pattern_parts
                .iter()
                .zip(key_parts.iter())
                .all(|(p, k)| *p == "*" || p == k)
    }

    fn matches(key_expr: &str, key: &str) -> bool {
        if let Some(prefix) = key_expr.strip_suffix("**") {
            key.starts_with(prefix)
        } else if key_expr.contains('*') {
            single_segment_match(key_expr, key)
        } else {
            key == key_expr
        }
    }

    #[async_trait]
    impl ZenohStore for FakeStore {
        async fn get(&self, key_expr: &str) -> Vec<(String, String)> {
            self.data
                .lock()
                .unwrap()
                .iter()
                .filter(|(k, _)| matches(key_expr, k))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        }

        async fn put(&self, key_expr: &str, value: &str) {
            self.put_calls
                .lock()
                .unwrap()
                .push((key_expr.to_string(), value.to_string()));
            self.data
                .lock()
                .unwrap()
                .insert(key_expr.to_string(), value.to_string());
        }

        async fn delete(&self, key_expr: &str) {
            self.delete_calls.lock().unwrap().push(key_expr.to_string());
            let mut data = self.data.lock().unwrap();
            if let Some(prefix) = key_expr.strip_suffix("**") {
                data.retain(|k, _| !k.starts_with(prefix));
            } else {
                data.remove(key_expr);
            }
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[tokio::test]
        async fn get_returns_seeded_exact_match() {
            let store = FakeStore::new().seed("a/b", "v1");
            assert_eq!(store.get("a/b").await, vec![("a/b".to_string(), "v1".to_string())]);
        }

        #[tokio::test]
        async fn get_matches_wildcard_suffix() {
            let store = FakeStore::new()
                .seed("a/b/c", "v1")
                .seed("a/b/d", "v2")
                .seed("a/x", "v3");
            let mut results = store.get("a/b/**").await;
            results.sort();
            assert_eq!(
                results,
                vec![("a/b/c".to_string(), "v1".to_string()), ("a/b/d".to_string(), "v2".to_string())]
            );
        }

        #[tokio::test]
        async fn get_matches_single_segment_wildcard_shape() {
            let store = FakeStore::new()
                .seed("projects/p1/tasks/t1/status", "PENDING")
                .seed("projects/p1/tasks/t1/time_entered", "now");
            let results = store.get("projects/*/tasks/*/status").await;
            assert_eq!(results, vec![("projects/p1/tasks/t1/status".to_string(), "PENDING".to_string())]);
        }

        #[tokio::test]
        async fn put_then_get_round_trips_and_records_call() {
            let store = FakeStore::new();
            store.put("a/b", "v1").await;
            assert_eq!(store.get("a/b").await, vec![("a/b".to_string(), "v1".to_string())]);
            assert_eq!(store.put_calls.lock().unwrap().as_slice(), [("a/b".to_string(), "v1".to_string())]);
        }

        #[tokio::test]
        async fn delete_removes_matching_prefix_and_records_call() {
            let store = FakeStore::new().seed("a/b/c", "v1").seed("a/x", "v2");
            store.delete("a/b/**").await;
            assert_eq!(store.get("a/b/**").await, Vec::<(String, String)>::new());
            assert_eq!(store.get("a/x").await, vec![("a/x".to_string(), "v2".to_string())]);
            assert_eq!(store.delete_calls.lock().unwrap().as_slice(), ["a/b/**".to_string()]);
        }
    }
}
```

- [ ] **Step 2: Run the tests to verify they pass**

Run: `cd web && cargo test zenoh_store`
Expected: all 5 new tests `ok` (this is TDD-by-construction — the implementation and tests were written together above since the fake's correctness *is* the test; there's no separate red step here because a fake test double with no tests would be unverified).

- [ ] **Step 3: Commit**

```bash
git add web/src/zenoh_store.rs
git commit -m "test(web): add in-memory FakeStore test double"
```

---

### Task 3: `models.rs` — `Task` and `HistoryEntry`

**Files:**
- Create: `web/src/models.rs`
- Modify: `web/src/lib.rs` (add `pub mod models;`)

**Interfaces:**
- Produces: `struct Task { pub id: String, pub status: String, pub time_entered: Option<String>, pub time_accepted: Option<String>, pub time_completed: Option<String>, pub acceptance_criteria: Option<String>, pub entered_by: Option<String>, pub history: Vec<HistoryEntry> }`, `Task::new(id: impl Into<String>) -> Self`
- Produces: `struct HistoryEntry { pub timestamp: String, pub from_status: String, pub to_status: String, pub note: String }` (Deserialize)

- [ ] **Step 1: Write the failing test**

Create `web/src/models.rs`:

```rust
use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct HistoryEntry {
    pub timestamp: String,
    pub from_status: String,
    pub to_status: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    pub id: String,
    pub status: String,
    pub time_entered: Option<String>,
    pub time_accepted: Option<String>,
    pub time_completed: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub entered_by: Option<String>,
    pub history: Vec<HistoryEntry>,
}

impl Task {
    pub fn new(id: impl Into<String>) -> Self {
        Task {
            id: id.into(),
            status: "UNKNOWN".to_string(),
            time_entered: None,
            time_accepted: None,
            time_completed: None,
            acceptance_criteria: None,
            entered_by: None,
            history: Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_task_has_unknown_status_and_empty_history() {
        let task = Task::new("t1");
        assert_eq!(task.id, "t1");
        assert_eq!(task.status, "UNKNOWN");
        assert!(task.time_entered.is_none());
        assert!(task.history.is_empty());
    }

    #[test]
    fn history_entry_deserializes_from_json_with_default_note() {
        let entry: HistoryEntry =
            serde_json::from_str(r#"{"timestamp":"t","from_status":"NONE","to_status":"PENDING"}"#).unwrap();
        assert_eq!(entry.timestamp, "t");
        assert_eq!(entry.from_status, "NONE");
        assert_eq!(entry.to_status, "PENDING");
        assert_eq!(entry.note, "");
    }
}
```

- [ ] **Step 2: Register the module**

In `web/src/lib.rs`, add `pub mod models;` above `pub mod zenoh_store;`.

- [ ] **Step 3: Run the tests**

Run: `cd web && cargo test models`
Expected: both tests `ok`.

- [ ] **Step 4: Commit**

```bash
git add web/src/models.rs web/src/lib.rs
git commit -m "feat(web): add Task and HistoryEntry models"
```

---

### Task 4: `queries.rs` — read-side business logic

**Files:**
- Create: `web/src/queries.rs`
- Modify: `web/src/lib.rs` (add `pub mod queries;`)

**Interfaces:**
- Consumes: `ZenohStore` (Task 1), `zenoh_store::fake::FakeStore` (Task 2), `Task`/`HistoryEntry` (Task 3)
- Produces: `const TERMINAL_STATUS: &str`, `const WIP_STATUSES: [&str; 3]`
- Produces: `async fn fetch_all_tasks(store: &dyn ZenohStore, project_id: &str) -> HashMap<String, Task>`
- Produces: `async fn fetch_task(store: &dyn ZenohStore, project_id: &str, task_id: &str) -> Option<Task>`
- Produces: `async fn fetch_status(store: &dyn ZenohStore, project_id: &str, task_id: &str) -> String`
- Produces: `struct ProjectSummary { pub id: String, pub total: usize, pub incomplete: usize, pub wip: usize }`
- Produces: `async fn fetch_all_projects(store: &dyn ZenohStore) -> Vec<ProjectSummary>` (sorted by `id`)

- [ ] **Step 1: Write the failing tests**

Create `web/src/queries.rs`:

```rust
use std::collections::HashMap;

use crate::models::{HistoryEntry, Task};
use crate::zenoh_store::ZenohStore;

pub const TERMINAL_STATUS: &str = "COMPLETED";
pub const WIP_STATUSES: [&str; 3] = ["IN_PROGRESS", "WIP", "RUNNING"];

fn apply_field(task: &mut Task, field_name: &str, value: &str) {
    match field_name {
        "status" => task.status = value.to_string(),
        "time_entered" => task.time_entered = Some(value.to_string()),
        "time_accepted" => task.time_accepted = Some(value.to_string()),
        "time_completed" => task.time_completed = Some(value.to_string()),
        "acceptance_criteria" => task.acceptance_criteria = Some(value.to_string()),
        "entered_by" => task.entered_by = Some(value.to_string()),
        _ if field_name.starts_with("history/") => {
            if let Ok(entry) = serde_json::from_str::<HistoryEntry>(value) {
                task.history.push(entry);
            }
        }
        _ => {}
    }
}

pub async fn fetch_all_tasks(store: &dyn ZenohStore, project_id: &str) -> HashMap<String, Task> {
    let prefix = format!("projects/{project_id}/tasks/");
    let key_expr = format!("{prefix}**");
    let mut tasks: HashMap<String, Task> = HashMap::new();

    for (key, value) in store.get(&key_expr).await {
        let Some(relative) = key.strip_prefix(&prefix) else { continue };
        let mut parts = relative.splitn(2, '/');
        let task_id = parts.next().unwrap_or_default().to_string();
        let field_name = parts.next().unwrap_or_default();

        let task = tasks
            .entry(task_id.clone())
            .or_insert_with(|| Task::new(task_id.clone()));
        apply_field(task, field_name, &value);
    }

    tasks
}

pub async fn fetch_task(store: &dyn ZenohStore, project_id: &str, task_id: &str) -> Option<Task> {
    let prefix = format!("projects/{project_id}/tasks/{task_id}/");
    let key_expr = format!("{prefix}**");
    let mut result: Option<Task> = None;

    for (key, value) in store.get(&key_expr).await {
        let Some(field_name) = key.strip_prefix(&prefix) else { continue };
        let task = result.get_or_insert_with(|| Task::new(task_id));
        apply_field(task, field_name, &value);
    }

    result
}

pub async fn fetch_status(store: &dyn ZenohStore, project_id: &str, task_id: &str) -> String {
    let key = format!("projects/{project_id}/tasks/{task_id}/status");
    store
        .get(&key)
        .await
        .into_iter()
        .next()
        .map(|(_, value)| value)
        .unwrap_or_else(|| "UNKNOWN".to_string())
}

#[derive(Debug, Clone, PartialEq)]
pub struct ProjectSummary {
    pub id: String,
    pub total: usize,
    pub incomplete: usize,
    pub wip: usize,
}

pub async fn fetch_all_projects(store: &dyn ZenohStore) -> Vec<ProjectSummary> {
    let mut summaries: HashMap<String, ProjectSummary> = HashMap::new();

    for (key, value) in store.get("projects/*/tasks/*/status").await {
        let parts: Vec<&str> = key.split('/').collect();
        let Some(project_id) = parts.get(1) else { continue };
        let summary = summaries.entry(project_id.to_string()).or_insert_with(|| ProjectSummary {
            id: project_id.to_string(),
            total: 0,
            incomplete: 0,
            wip: 0,
        });
        summary.total += 1;
        let status = value.to_uppercase();
        if status != TERMINAL_STATUS {
            summary.incomplete += 1;
        }
        if WIP_STATUSES.contains(&status.as_str()) {
            summary.wip += 1;
        }
    }

    let mut result: Vec<ProjectSummary> = summaries.into_values().collect();
    result.sort_by(|a, b| a.id.cmp(&b.id));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zenoh_store::fake::FakeStore;

    #[tokio::test]
    async fn fetch_all_tasks_groups_fields_by_task_id() {
        let store = FakeStore::new()
            .seed("projects/p1/tasks/t1/status", "PENDING")
            .seed("projects/p1/tasks/t1/time_entered", "2026-07-31T00:00:00+00:00")
            .seed("projects/p1/tasks/t1/entered_by", "LLM")
            .seed("projects/p1/tasks/t2/status", "COMPLETED")
            .seed(
                "projects/p1/tasks/t1/history/2026-07-31T00-00-00",
                r#"{"timestamp":"t","from_status":"NONE","to_status":"PENDING","note":""}"#,
            );

        let tasks = fetch_all_tasks(&store, "p1").await;

        assert_eq!(tasks.len(), 2);
        assert_eq!(tasks["t1"].status, "PENDING");
        assert_eq!(tasks["t1"].time_entered.as_deref(), Some("2026-07-31T00:00:00+00:00"));
        assert_eq!(tasks["t1"].entered_by.as_deref(), Some("LLM"));
        assert_eq!(tasks["t1"].history.len(), 1);
        assert_eq!(tasks["t2"].status, "COMPLETED");
    }

    #[tokio::test]
    async fn fetch_task_queries_task_specific_prefix_and_returns_none_if_missing() {
        let store = FakeStore::new().seed("projects/p1/tasks/t1/status", "PENDING");

        let found = fetch_task(&store, "p1", "t1").await;
        assert_eq!(found.unwrap().status, "PENDING");

        let missing = fetch_task(&store, "p1", "missing").await;
        assert!(missing.is_none());
    }

    #[tokio::test]
    async fn fetch_status_returns_unknown_when_key_absent() {
        let store = FakeStore::new();
        assert_eq!(fetch_status(&store, "p1", "t1").await, "UNKNOWN");
    }

    #[tokio::test]
    async fn fetch_status_returns_value_when_present() {
        let store = FakeStore::new().seed("projects/p1/tasks/t1/status", "IN_PROGRESS");
        assert_eq!(fetch_status(&store, "p1", "t1").await, "IN_PROGRESS");
    }

    #[tokio::test]
    async fn fetch_all_projects_groups_and_counts_by_status() {
        let store = FakeStore::new()
            .seed("projects/p1/tasks/t1/status", "PENDING")
            .seed("projects/p1/tasks/t2/status", "COMPLETED")
            .seed("projects/p2/tasks/t1/status", "IN_PROGRESS");

        let projects = fetch_all_projects(&store).await;

        assert_eq!(projects.len(), 2);
        assert_eq!(projects[0], ProjectSummary { id: "p1".to_string(), total: 2, incomplete: 1, wip: 0 });
        assert_eq!(projects[1], ProjectSummary { id: "p2".to_string(), total: 1, incomplete: 1, wip: 1 });
    }
}
```

- [ ] **Step 2: Register the module**

In `web/src/lib.rs`, add `pub mod queries;`.

- [ ] **Step 3: Run the tests**

Run: `cd web && cargo test queries`
Expected: all 5 tests `ok`.

- [ ] **Step 4: Commit**

```bash
git add web/src/queries.rs web/src/lib.rs
git commit -m "feat(web): add queries module (fetch_all_tasks/fetch_task/fetch_status/fetch_all_projects)"
```

---

### Task 5: `zenoh_client.rs` — endpoint resolution and the real zenoh-backed store

**Files:**
- Create: `web/src/zenoh_client.rs`
- Modify: `web/src/lib.rs` (add `pub mod zenoh_client;`)

**Interfaces:**
- Consumes: `ZenohStore` (Task 1)
- Produces: `const ENDPOINT_ENV_VAR: &str = "ZTASK_ZENOH_ENDPOINT"`, `fn resolve_endpoint() -> String`
- Produces: `async fn open_session(endpoint: &str) -> zenoh::Result<zenoh::Session>`
- Produces: `struct RealZenohStore`, `RealZenohStore::new(session: zenoh::Session) -> Self`, implements `ZenohStore`

This wraps the real zenoh SDK; `resolve_endpoint` is a pure function and gets a real unit test. `open_session`/`RealZenohStore` are thin glue over the SDK verified against a real router — no dedicated unit test here; covered by the opt-in integration test in Task 18 (same pragmatic split the Python CLI uses: `zenoh_client.py`'s `resolve_endpoint` is unit tested, `open_session` is not).

- [ ] **Step 1: Write the failing test**

Create `web/src/zenoh_client.rs`:

```rust
use std::env;

use async_trait::async_trait;
use zenoh::Session;

use crate::zenoh_store::ZenohStore;

pub const ENDPOINT_ENV_VAR: &str = "ZTASK_ZENOH_ENDPOINT";
const DEFAULT_ENDPOINT: &str = "tcp/localhost:7447";

pub fn resolve_endpoint() -> String {
    env::var(ENDPOINT_ENV_VAR).unwrap_or_else(|_| DEFAULT_ENDPOINT.to_string())
}

pub async fn open_session(endpoint: &str) -> zenoh::Result<Session> {
    let mut config = zenoh::Config::default();
    config.insert_json5("connect/endpoints", &format!(r#"["{endpoint}"]"#))?;
    zenoh::open(config).await
}

pub struct RealZenohStore {
    session: Session,
}

impl RealZenohStore {
    pub fn new(session: Session) -> Self {
        Self { session }
    }
}

#[async_trait]
impl ZenohStore for RealZenohStore {
    async fn get(&self, key_expr: &str) -> Vec<(String, String)> {
        let mut results = Vec::new();
        let Ok(replies) = self.session.get(key_expr).await else {
            return results;
        };
        while let Ok(reply) = replies.recv_async().await {
            if let Ok(sample) = reply.result() {
                let value = sample
                    .payload()
                    .try_to_string()
                    .map(|s| s.to_string())
                    .unwrap_or_default();
                results.push((sample.key_expr().to_string(), value));
            }
        }
        results
    }

    async fn put(&self, key_expr: &str, value: &str) {
        if let Err(err) = self.session.put(key_expr, value).await {
            tracing::warn!("zenoh put {key_expr} failed: {err}");
        }
    }

    async fn delete(&self, key_expr: &str) {
        if let Err(err) = self.session.delete(key_expr).await {
            tracing::warn!("zenoh delete {key_expr} failed: {err}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn resolve_endpoint_defaults_when_unset() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::remove_var(ENDPOINT_ENV_VAR);
        assert_eq!(resolve_endpoint(), "tcp/localhost:7447");
    }

    #[test]
    fn resolve_endpoint_reads_env_var() {
        let _guard = ENV_LOCK.lock().unwrap();
        env::set_var(ENDPOINT_ENV_VAR, "tcp/zenoh-router:7447");
        let result = resolve_endpoint();
        env::remove_var(ENDPOINT_ENV_VAR);
        assert_eq!(result, "tcp/zenoh-router:7447");
    }
}
```

- [ ] **Step 2: Register the module**

In `web/src/lib.rs`, add `pub mod zenoh_client;`.

- [ ] **Step 3: Run the tests**

Run: `cd web && cargo test zenoh_client`
Expected: both `resolve_endpoint` tests `ok`. (The crate must also still compile cleanly with the real `zenoh` dependency now exercised by `RealZenohStore` — if `cargo test` fails to *compile*, check the method names above against `zenoh = "=1.9.0"`'s actual API before changing anything else; they were verified against the crate source at the `1.9.0` tag while writing this plan.)

- [ ] **Step 4: Commit**

```bash
git add web/src/zenoh_client.rs web/src/lib.rs
git commit -m "feat(web): add zenoh_client with resolve_endpoint and RealZenohStore"
```

---

### Task 6: `tasks.rs` — write-side business logic

**Files:**
- Create: `web/src/tasks.rs`
- Modify: `web/src/lib.rs` (add `pub mod tasks;`)

**Interfaces:**
- Consumes: `ZenohStore` (Task 1), `Task` (Task 3), `queries::{fetch_status, fetch_task, TERMINAL_STATUS, WIP_STATUSES}` (Task 4)
- Produces: `const ENTERED_BY_USER: &str = "USER"`
- Produces: `enum TaskError { NotFound }` (Debug, Clone, PartialEq)
- Produces: `async fn create_task(store: &dyn ZenohStore, project_id: &str, task_id: &str, criteria: &str, now: &str) -> Task`
- Produces: `async fn update_status(store: &dyn ZenohStore, project_id: &str, task_id: &str, status: &str, note: &str, now: &str) -> Result<Task, TaskError>`
- Produces: `async fn edit_criteria(store: &dyn ZenohStore, project_id: &str, task_id: &str, criteria: &str, now: &str) -> Result<Task, TaskError>`
- Produces: `async fn delete_task(store: &dyn ZenohStore, project_id: &str, task_id: &str)`

Note the timestamp (`now`) is always passed in by the caller rather than read internally — this makes every function here a pure(ish) function of its inputs plus the store, with no need to mock a clock in tests (the HTTP handler layer, wired in later tasks, is the only place that calls the real `chrono::Utc::now()`).

- [ ] **Step 1: Write the failing tests**

Create `web/src/tasks.rs`:

```rust
use crate::models::Task;
use crate::queries::{self, TERMINAL_STATUS, WIP_STATUSES};
use crate::zenoh_store::ZenohStore;

pub const ENTERED_BY_USER: &str = "USER";

#[derive(Debug, Clone, PartialEq)]
pub enum TaskError {
    NotFound,
}

fn history_key(now: &str) -> String {
    now.replace(':', "-")
}

pub async fn create_task(
    store: &dyn ZenohStore,
    project_id: &str,
    task_id: &str,
    criteria: &str,
    now: &str,
) -> Task {
    let base = format!("projects/{project_id}/tasks/{task_id}");
    store.put(&format!("{base}/status"), "PENDING").await;
    store.put(&format!("{base}/time_entered"), now).await;
    store.put(&format!("{base}/entered_by"), ENTERED_BY_USER).await;
    if !criteria.is_empty() {
        store.put(&format!("{base}/acceptance_criteria"), criteria).await;
    }

    let history_value = serde_json::json!({
        "timestamp": now,
        "from_status": "NONE",
        "to_status": "PENDING",
        "note": "Task created via web UI",
    })
    .to_string();
    store
        .put(&format!("{base}/history/{}", history_key(now)), &history_value)
        .await;

    queries::fetch_task(store, project_id, task_id)
        .await
        .unwrap_or_else(|| Task::new(task_id))
}

pub async fn update_status(
    store: &dyn ZenohStore,
    project_id: &str,
    task_id: &str,
    status: &str,
    note: &str,
    now: &str,
) -> Result<Task, TaskError> {
    let base = format!("projects/{project_id}/tasks/{task_id}");
    let old_status = queries::fetch_status(store, project_id, task_id).await;
    if old_status == "UNKNOWN" {
        return Err(TaskError::NotFound);
    }

    let new_status = status.to_uppercase();
    store.put(&format!("{base}/status"), &new_status).await;

    let is_wip = |s: &str| WIP_STATUSES.contains(&s);
    if is_wip(&new_status) && !is_wip(&old_status) {
        store.put(&format!("{base}/time_accepted"), now).await;
    } else if new_status == TERMINAL_STATUS {
        store.put(&format!("{base}/time_completed"), now).await;
    }

    let history_value = serde_json::json!({
        "timestamp": now,
        "from_status": old_status,
        "to_status": new_status,
        "note": note,
    })
    .to_string();
    store
        .put(&format!("{base}/history/{}", history_key(now)), &history_value)
        .await;

    queries::fetch_task(store, project_id, task_id)
        .await
        .ok_or(TaskError::NotFound)
}

pub async fn edit_criteria(
    store: &dyn ZenohStore,
    project_id: &str,
    task_id: &str,
    criteria: &str,
    now: &str,
) -> Result<Task, TaskError> {
    let base = format!("projects/{project_id}/tasks/{task_id}");
    let status = queries::fetch_status(store, project_id, task_id).await;
    if status == "UNKNOWN" {
        return Err(TaskError::NotFound);
    }

    store.put(&format!("{base}/acceptance_criteria"), criteria).await;

    let history_value = serde_json::json!({
        "timestamp": now,
        "from_status": status,
        "to_status": status,
        "note": "criteria updated",
    })
    .to_string();
    store
        .put(&format!("{base}/history/{}", history_key(now)), &history_value)
        .await;

    queries::fetch_task(store, project_id, task_id)
        .await
        .ok_or(TaskError::NotFound)
}

pub async fn delete_task(store: &dyn ZenohStore, project_id: &str, task_id: &str) {
    store
        .delete(&format!("projects/{project_id}/tasks/{task_id}/**"))
        .await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::zenoh_store::fake::FakeStore;

    #[tokio::test]
    async fn create_task_writes_expected_fields_and_history() {
        let store = FakeStore::new();
        let task = create_task(&store, "p1", "t1", "Given X", "2026-08-01T00:00:00+00:00").await;

        assert_eq!(task.status, "PENDING");
        assert_eq!(task.entered_by.as_deref(), Some("USER"));
        assert_eq!(task.time_entered.as_deref(), Some("2026-08-01T00:00:00+00:00"));
        assert_eq!(task.acceptance_criteria.as_deref(), Some("Given X"));
        assert_eq!(task.history.len(), 1);
        assert_eq!(task.history[0].to_status, "PENDING");

        let put_calls = store.put_calls.lock().unwrap();
        assert!(put_calls.contains(&("projects/p1/tasks/t1/entered_by".to_string(), "USER".to_string())));
    }

    #[tokio::test]
    async fn create_task_without_criteria_leaves_it_unset() {
        let store = FakeStore::new();
        let task = create_task(&store, "p1", "t1", "", "2026-08-01T00:00:00+00:00").await;
        assert!(task.acceptance_criteria.is_none());
    }

    #[tokio::test]
    async fn update_status_to_wip_sets_time_accepted() {
        let store = FakeStore::new().seed("projects/p1/tasks/t1/status", "PENDING");
        let task = update_status(&store, "p1", "t1", "in_progress", "starting", "2026-08-01T01:00:00+00:00")
            .await
            .unwrap();
        assert_eq!(task.status, "IN_PROGRESS");
        assert_eq!(task.time_accepted.as_deref(), Some("2026-08-01T01:00:00+00:00"));
        assert!(task.time_completed.is_none());
    }

    #[tokio::test]
    async fn update_status_to_completed_sets_time_completed() {
        let store = FakeStore::new().seed("projects/p1/tasks/t1/status", "IN_PROGRESS");
        let task = update_status(&store, "p1", "t1", "completed", "", "2026-08-01T02:00:00+00:00")
            .await
            .unwrap();
        assert_eq!(task.status, "COMPLETED");
        assert_eq!(task.time_completed.as_deref(), Some("2026-08-01T02:00:00+00:00"));
    }

    #[tokio::test]
    async fn update_status_missing_task_returns_not_found() {
        let store = FakeStore::new();
        let result = update_status(&store, "p1", "missing", "completed", "", "2026-08-01T00:00:00+00:00").await;
        assert_eq!(result, Err(TaskError::NotFound));
    }

    #[tokio::test]
    async fn edit_criteria_updates_value_and_appends_history() {
        let store = FakeStore::new()
            .seed("projects/p1/tasks/t1/status", "PENDING")
            .seed("projects/p1/tasks/t1/acceptance_criteria", "old");
        let task = edit_criteria(&store, "p1", "t1", "new criteria", "2026-08-01T03:00:00+00:00")
            .await
            .unwrap();
        assert_eq!(task.acceptance_criteria.as_deref(), Some("new criteria"));
        assert!(task.history.iter().any(|h| h.note == "criteria updated"));
    }

    #[tokio::test]
    async fn edit_criteria_missing_task_returns_not_found() {
        let store = FakeStore::new();
        let result = edit_criteria(&store, "p1", "missing", "x", "2026-08-01T00:00:00+00:00").await;
        assert_eq!(result, Err(TaskError::NotFound));
    }

    #[tokio::test]
    async fn delete_task_removes_all_keys_under_the_task_prefix() {
        let store = FakeStore::new()
            .seed("projects/p1/tasks/t1/status", "PENDING")
            .seed("projects/p1/tasks/t1/time_entered", "now");
        delete_task(&store, "p1", "t1").await;
        assert!(queries::fetch_task(&store, "p1", "t1").await.is_none());
        assert_eq!(store.delete_calls.lock().unwrap().as_slice(), ["projects/p1/tasks/t1/**".to_string()]);
    }
}
```

- [ ] **Step 2: Register the module**

In `web/src/lib.rs`, add `pub mod tasks;`.

- [ ] **Step 3: Run the tests**

Run: `cd web && cargo test tasks::`
Expected: all 8 tests `ok`.

- [ ] **Step 4: Commit**

```bash
git add web/src/tasks.rs web/src/lib.rs
git commit -m "feat(web): add tasks module (create/update_status/edit_criteria/delete_task)"
```

---

### Task 7: All-projects dashboard (template + handler)

**Files:**
- Create: `web/src/render.rs`
- Create: `web/src/handlers/mod.rs`
- Create: `web/src/handlers/dashboard.rs`
- Create: `web/templates/base.html`
- Create: `web/templates/dashboard.html`
- Modify: `web/src/lib.rs` (add `pub mod handlers;`, `pub mod render;`, extend `app()` with `GET /`)

**Interfaces:**
- Consumes: `AppState` (Task 1), `queries::{fetch_all_projects, ProjectSummary}` (Task 4)
- Produces: `struct HtmlTemplate<T>(pub T)` implementing `IntoResponse` for any `T: askama::Template`, in `render.rs`
- Produces: `handlers::dashboard::show(State<AppState>) -> HtmlTemplate<DashboardTemplate>`

- [ ] **Step 1: Write `render.rs`**

```rust
use askama::Template;
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Response};

pub struct HtmlTemplate<T>(pub T);

impl<T: Template> IntoResponse for HtmlTemplate<T> {
    fn into_response(self) -> Response {
        match self.0.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (StatusCode::INTERNAL_SERVER_ERROR, format!("template error: {err}")).into_response(),
        }
    }
}
```

- [ ] **Step 2: Vendor htmx as a placeholder base URL and write `base.html`**

`web/templates/base.html` references `/static/htmx.min.js` and `/static/style.css`, which don't exist until Task 14 — that's fine, this template compiles and renders regardless of whether those routes exist yet (they're just `<link>`/`<script>` tags, not askama includes).

```html
<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>{% block title %}Zenoh Tasks{% endblock %}</title>
    <link rel="stylesheet" href="/static/style.css">
    <script src="/static/htmx.min.js"></script>
</head>
<body>
<main class="container">
    <nav>
        <ul><li><strong><a href="/">Zenoh Tasks</a></strong></li></ul>
    </nav>
    {% block content %}{% endblock %}
</main>
</body>
</html>
```

- [ ] **Step 3: Write `dashboard.html`**

```html
{% extends "base.html" %}
{% block title %}All Projects — Zenoh Tasks{% endblock %}
{% block content %}
<h1>Projects</h1>
{% if projects.is_empty() %}
<p>No projects yet.</p>
{% else %}
<table>
    <thead>
        <tr><th>Project</th><th>Total</th><th>Incomplete</th><th>WIP</th></tr>
    </thead>
    <tbody>
        {% for p in projects %}
        <tr>
            <td><a href="/projects/{{ p.id }}">{{ p.id }}</a></td>
            <td>{{ p.total }}</td>
            <td>{{ p.incomplete }}</td>
            <td>{{ p.wip }}</td>
        </tr>
        {% endfor %}
    </tbody>
</table>
{% endif %}
{% endblock %}
```

- [ ] **Step 4: Write the failing test and the handler**

Create `web/src/handlers/dashboard.rs`:

```rust
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
```

Create `web/src/handlers/mod.rs`:

```rust
pub mod dashboard;
```

- [ ] **Step 5: Register modules and wire the route**

In `web/src/lib.rs`:
1. Add `pub mod handlers;` and `pub mod render;` alongside the other `pub mod` lines.
2. Replace the `app()` function with:

```rust
pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/", get(handlers::dashboard::show))
        .route("/healthz", get(|| async { "ok" }))
        .with_state(state)
}
```

- [ ] **Step 6: Run the tests**

Run: `cd web && cargo test`
Expected: all prior tests plus `handlers::dashboard::tests::dashboard_lists_projects_with_counts` pass.

- [ ] **Step 7: Commit**

```bash
git add web/src/render.rs web/src/handlers web/src/lib.rs web/templates
git commit -m "feat(web): add all-projects dashboard"
```

---

### Task 8: Per-project dashboard (list + filter)

**Files:**
- Create: `web/src/handlers/project.rs`
- Create: `web/templates/project.html`
- Create: `web/templates/task_row.html`
- Modify: `web/src/handlers/mod.rs` (add `pub mod project;`)
- Modify: `web/src/lib.rs` (extend `app()` with `GET /projects/{id}`)

**Interfaces:**
- Consumes: `queries::{fetch_all_tasks, TERMINAL_STATUS, WIP_STATUSES}` (Task 4), `Task` (Task 3)
- Produces: `handlers::project::show(State<AppState>, Path<String>, Query<FilterQuery>) -> HtmlTemplate<ProjectTemplate>`
- Produces (in `handlers/task.rs`, created next task, but the template is shared starting now): `TaskRowTemplate { pub project_id: String, pub task: Task }` — defined here in `project.rs` for now since `handlers/task.rs` doesn't exist yet; Task 10 will move it there when task-specific handlers are introduced. To keep this task self-contained, define it here.

- [ ] **Step 1: Write `task_row.html`**

This is both a fragment included by `project.html`'s table loop and, from Task 9 onward, a standalone htmx-swap response (its root is a single `<tr>`).

```html
<tr id="task-{{ task.id }}">
    <td><a href="/projects/{{ project_id }}/tasks/{{ task.id }}">{{ task.id }}</a></td>
    <td>{{ task.status }}</td>
    <td>{{ task.entered_by.as_deref().unwrap_or("-") }}</td>
    <td>{{ task.time_entered.as_deref().unwrap_or("-") }}</td>
    <td>
        <form hx-post="/projects/{{ project_id }}/tasks/{{ task.id }}/status"
              hx-target="#task-{{ task.id }}" hx-swap="outerHTML" style="display:inline">
            <select name="status">
                <option value="PENDING" {% if task.status == "PENDING" %}selected{% endif %}>PENDING</option>
                <option value="IN_PROGRESS" {% if task.status == "IN_PROGRESS" %}selected{% endif %}>IN_PROGRESS</option>
                <option value="COMPLETED" {% if task.status == "COMPLETED" %}selected{% endif %}>COMPLETED</option>
            </select>
            <button type="submit">Update</button>
        </form>
        <form hx-post="/projects/{{ project_id }}/tasks/{{ task.id }}/criteria"
              hx-target="#task-{{ task.id }}" hx-swap="outerHTML" style="display:inline">
            <input type="text" name="criteria" value="{{ task.acceptance_criteria.as_deref().unwrap_or(\"\") }}"
                   placeholder="Acceptance criteria">
            <button type="submit">Save</button>
        </form>
        <button hx-delete="/projects/{{ project_id }}/tasks/{{ task.id }}"
                hx-target="#task-{{ task.id }}" hx-swap="delete"
                hx-confirm="Delete task {{ task.id }}?">Delete</button>
    </td>
</tr>
```

- [ ] **Step 2: Write `project.html`**

```html
{% extends "base.html" %}
{% block title %}{{ project_id }} — Zenoh Tasks{% endblock %}
{% block content %}
<h1>{{ project_id }}</h1>

<nav>
    <ul>
        <li><a href="/projects/{{ project_id }}?filter=all">All</a></li>
        <li><a href="/projects/{{ project_id }}?filter=incomplete">Incomplete</a></li>
        <li><a href="/projects/{{ project_id }}?filter=wip">WIP</a></li>
    </ul>
</nav>

<form hx-post="/projects/{{ project_id }}/tasks" hx-target="#task-list" hx-swap="afterbegin">
    <fieldset role="group">
        <input type="text" name="task_id" placeholder="Task ID" required>
        <input type="text" name="criteria" placeholder="Acceptance criteria (optional)">
        <button type="submit">Create task</button>
    </fieldset>
</form>

<table>
    <thead>
        <tr><th>ID</th><th>Status</th><th>Entered by</th><th>Entered</th><th>Actions</th></tr>
    </thead>
    <tbody id="task-list">
        {% for task in tasks %}
        {% include "task_row.html" %}
        {% endfor %}
    </tbody>
</table>
{% endblock %}
```

- [ ] **Step 3: Write the failing test and the handler**

Create `web/src/handlers/project.rs`:

```rust
use askama::Template;
use axum::extract::{Path, Query, State};
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

#[derive(Template)]
#[template(path = "task_row.html")]
pub struct TaskRowTemplate {
    pub project_id: String,
    pub task: Task,
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
) -> HtmlTemplate<ProjectTemplate> {
    let all_tasks = queries::fetch_all_tasks(state.store.as_ref(), &project_id).await;
    let mut tasks: Vec<Task> = all_tasks.into_values().filter(|t| matches_filter(t, &query.filter)).collect();
    tasks.sort_by(|a, b| a.id.cmp(&b.id));

    HtmlTemplate(ProjectTemplate { project_id, tasks })
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
}
```

Modify `web/src/handlers/mod.rs` to add `pub mod project;`.

- [ ] **Step 4: Wire the route**

In `web/src/lib.rs`'s `app()`, add `.route("/projects/{id}", get(handlers::project::show))`.

- [ ] **Step 5: Run the tests**

Run: `cd web && cargo test`
Expected: all prior tests plus the two new `handlers::project::tests` pass.

- [ ] **Step 6: Commit**

```bash
git add web/src/handlers/project.rs web/src/handlers/mod.rs web/src/lib.rs web/templates/project.html web/templates/task_row.html
git commit -m "feat(web): add per-project dashboard with status filter"
```

---

### Task 9: Create-task handler

**Files:**
- Modify: `web/src/handlers/project.rs` (add `create`)
- Modify: `web/src/lib.rs` (extend `app()` with `POST /projects/{id}/tasks`)

**Interfaces:**
- Consumes: `tasks::create_task` (Task 6), `TaskRowTemplate` (Task 8)
- Produces: `handlers::project::create(State<AppState>, Path<String>, Form<CreateTaskForm>) -> HtmlTemplate<TaskRowTemplate>`
- Produces: `fn iso_now() -> String` in `web/src/lib.rs` (the one place that reads the real clock; every business-logic function below it takes `now` as a parameter instead)

- [ ] **Step 1: Add `iso_now` to `lib.rs`**

In `web/src/lib.rs`, add near the top (after the `AppState` definition):

```rust
pub fn iso_now() -> String {
    chrono::Utc::now().to_rfc3339()
}
```

- [ ] **Step 2: Write the failing test and the handler**

Append to `web/src/handlers/project.rs`:

```rust
use axum::Form;

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
) -> HtmlTemplate<TaskRowTemplate> {
    let now = crate::iso_now();
    let task = crate::tasks::create_task(state.store.as_ref(), &project_id, &form.task_id, &form.criteria, &now).await;
    HtmlTemplate(TaskRowTemplate { project_id, task })
}
```

Add to the `#[cfg(test)] mod tests` block in the same file:

```rust
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
```

- [ ] **Step 3: Wire the route**

In `web/src/lib.rs`'s `app()`, add `.route("/projects/{id}/tasks", post(handlers::project::create))`. Add `post` to the `use axum::routing::{get, post};` import if it isn't there yet.

- [ ] **Step 4: Run the tests**

Run: `cd web && cargo test`
Expected: all prior tests plus `create_task_adds_row_and_persists_fields` pass.

- [ ] **Step 5: Commit**

```bash
git add web/src/handlers/project.rs web/src/lib.rs
git commit -m "feat(web): add create-task handler"
```

---

### Task 10: Update-status handler

**Files:**
- Create: `web/src/handlers/task.rs`
- Modify: `web/src/handlers/project.rs` (remove `TaskRowTemplate` — moved to `task.rs`)
- Modify: `web/src/handlers/mod.rs` (add `pub mod task;`)
- Modify: `web/src/lib.rs` (extend `app()` with `POST /projects/{id}/tasks/{task_id}/status`)

**Interfaces:**
- Consumes: `tasks::{update_status, TaskError}` (Task 6)
- Produces (moved here from `project.rs`): `TaskRowTemplate { pub project_id: String, pub task: Task }`
- Produces: `handlers::task::update_status(State<AppState>, Path<(String, String)>, Form<UpdateStatusForm>) -> Result<HtmlTemplate<TaskRowTemplate>, StatusCode>`

- [ ] **Step 1: Move `TaskRowTemplate` out of `project.rs`**

In `web/src/handlers/project.rs`, delete the `TaskRowTemplate` struct definition (it moves to `task.rs` below). Change every reference to `TaskRowTemplate` in `project.rs` (the `create` handler's return type and body) to `crate::handlers::task::TaskRowTemplate`.

- [ ] **Step 2: Write the failing test and the handler**

Create `web/src/handlers/task.rs`:

```rust
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
}
```

Modify `web/src/handlers/mod.rs` to add `pub mod task;`.

- [ ] **Step 3: Wire the route**

In `web/src/lib.rs`'s `app()`, add `.route("/projects/{id}/tasks/{task_id}/status", post(handlers::task::update_status))`.

- [ ] **Step 4: Run the tests**

Run: `cd web && cargo test`
Expected: all prior tests still pass (note `project.rs`'s own `create_task_adds_row_and_persists_fields` test must still compile after the `TaskRowTemplate` move — it references it via `HtmlTemplate<TaskRowTemplate>` return type inference only, not by name, so it should be unaffected) plus the two new `handlers::task::tests` pass.

- [ ] **Step 5: Commit**

```bash
git add web/src/handlers/task.rs web/src/handlers/project.rs web/src/handlers/mod.rs web/src/lib.rs
git commit -m "feat(web): add update-status handler"
```

---

### Task 11: Edit-criteria handler

**Files:**
- Modify: `web/src/handlers/task.rs` (add `edit_criteria`)
- Modify: `web/src/lib.rs` (extend `app()` with `POST /projects/{id}/tasks/{task_id}/criteria`)

**Interfaces:**
- Consumes: `tasks::edit_criteria` (Task 6), `TaskRowTemplate` (Task 10)
- Produces: `handlers::task::edit_criteria(State<AppState>, Path<(String, String)>, Form<EditCriteriaForm>) -> Result<HtmlTemplate<TaskRowTemplate>, StatusCode>`

- [ ] **Step 1: Write the failing test and the handler**

Append to `web/src/handlers/task.rs`:

```rust
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
```

Add to the `#[cfg(test)] mod tests` block in the same file:

```rust
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
```

- [ ] **Step 2: Wire the route**

In `web/src/lib.rs`'s `app()`, add `.route("/projects/{id}/tasks/{task_id}/criteria", post(handlers::task::edit_criteria))`.

- [ ] **Step 3: Run the tests**

Run: `cd web && cargo test`
Expected: all prior tests plus the two new tests pass.

- [ ] **Step 4: Commit**

```bash
git add web/src/handlers/task.rs web/src/lib.rs
git commit -m "feat(web): add edit-criteria handler"
```

---

### Task 12: Delete-task handler

**Files:**
- Modify: `web/src/handlers/task.rs` (add `delete`)
- Modify: `web/src/lib.rs` (extend `app()` with `DELETE /projects/{id}/tasks/{task_id}`)

**Interfaces:**
- Consumes: `tasks::delete_task` (Task 6)
- Produces: `handlers::task::delete(State<AppState>, Path<(String, String)>) -> StatusCode`

- [ ] **Step 1: Write the failing test and the handler**

Append to `web/src/handlers/task.rs`:

```rust
pub async fn delete(
    State(state): State<AppState>,
    Path((project_id, task_id)): Path<(String, String)>,
) -> StatusCode {
    crate::tasks::delete_task(state.store.as_ref(), &project_id, &task_id).await;
    StatusCode::OK
}
```

Add to the `#[cfg(test)] mod tests` block:

```rust
    #[tokio::test]
    async fn delete_task_removes_it() {
        let store = Arc::new(FakeStore::new().seed("projects/p1/tasks/t1/status", "PENDING"));
        let state = AppState { store: store.clone() as Arc<dyn ZenohStore> };

        let response = app(state)
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/projects/p1/tasks/t1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let delete_calls = store.delete_calls.lock().unwrap();
        assert_eq!(delete_calls.as_slice(), ["projects/p1/tasks/t1/**".to_string()]);
    }
```

- [ ] **Step 2: Wire the route**

In `web/src/lib.rs`'s `app()`, add a **new, standalone** route line: `.route("/projects/{id}/tasks/{task_id}", axum::routing::delete(handlers::task::delete))`. (Task 13 will change this line to also handle `GET` on the same path — don't try to combine them yet, since `handlers::task::show` doesn't exist until then.)

- [ ] **Step 3: Run the tests**

Run: `cd web && cargo test`
Expected: all prior tests plus `delete_task_removes_it` pass.

- [ ] **Step 4: Commit**

```bash
git add web/src/handlers/task.rs web/src/lib.rs
git commit -m "feat(web): add delete-task handler"
```

---

### Task 13: Task detail page

**Files:**
- Modify: `web/src/handlers/task.rs` (add `show` + `TaskDetailTemplate`)
- Create: `web/templates/task_detail.html`
- Modify: `web/src/lib.rs` (combine `GET`+`DELETE` on `/projects/{id}/tasks/{task_id}`)

**Interfaces:**
- Consumes: `queries::fetch_task` (Task 4)
- Produces: `TaskDetailTemplate { pub project_id: String, pub task: Task }`
- Produces: `handlers::task::show(State<AppState>, Path<(String, String)>) -> Result<HtmlTemplate<TaskDetailTemplate>, StatusCode>`

- [ ] **Step 1: Write `task_detail.html`**

```html
{% extends "base.html" %}
{% block title %}{{ task.id }} — {{ project_id }}{% endblock %}
{% block content %}
<h1>{{ task.id }}</h1>
<p><a href="/projects/{{ project_id }}">&larr; back to {{ project_id }}</a></p>

<table>
    <tbody>
        <tr><th>Status</th><td>{{ task.status }}</td></tr>
        <tr><th>Entered by</th><td>{{ task.entered_by.as_deref().unwrap_or("-") }}</td></tr>
        <tr><th>Time entered</th><td>{{ task.time_entered.as_deref().unwrap_or("-") }}</td></tr>
        <tr><th>Time accepted</th><td>{{ task.time_accepted.as_deref().unwrap_or("-") }}</td></tr>
        <tr><th>Time completed</th><td>{{ task.time_completed.as_deref().unwrap_or("-") }}</td></tr>
        <tr><th>Acceptance criteria</th><td>{{ task.acceptance_criteria.as_deref().unwrap_or("-") }}</td></tr>
    </tbody>
</table>

<h2>History</h2>
{% if task.history.is_empty() %}
<p>No history entries.</p>
{% else %}
<table>
    <thead><tr><th>Timestamp</th><th>From</th><th>To</th><th>Note</th></tr></thead>
    <tbody>
        {% for entry in task.history %}
        <tr>
            <td>{{ entry.timestamp }}</td>
            <td>{{ entry.from_status }}</td>
            <td>{{ entry.to_status }}</td>
            <td>{{ entry.note }}</td>
        </tr>
        {% endfor %}
    </tbody>
</table>
{% endif %}
{% endblock %}
```

- [ ] **Step 2: Write the failing test and the handler**

Append to `web/src/handlers/task.rs`:

```rust
#[derive(Template)]
#[template(path = "task_detail.html")]
pub struct TaskDetailTemplate {
    pub project_id: String,
    pub task: Task,
}

pub async fn show(
    State(state): State<AppState>,
    Path((project_id, task_id)): Path<(String, String)>,
) -> Result<HtmlTemplate<TaskDetailTemplate>, StatusCode> {
    match crate::queries::fetch_task(state.store.as_ref(), &project_id, &task_id).await {
        Some(task) => Ok(HtmlTemplate(TaskDetailTemplate { project_id, task })),
        None => Err(StatusCode::NOT_FOUND),
    }
}
```

Add to the `#[cfg(test)] mod tests` block:

```rust
    #[tokio::test]
    async fn task_detail_shows_history() {
        let store = FakeStore::new().seed("projects/p1/tasks/t1/status", "COMPLETED").seed(
            "projects/p1/tasks/t1/history/2026-08-01T00-00-00",
            r#"{"timestamp":"2026-08-01T00:00:00","from_status":"NONE","to_status":"PENDING","note":"created"}"#,
        );
        let state = AppState { store: Arc::new(store) as Arc<dyn ZenohStore> };

        let response = app(state)
            .oneshot(Request::builder().uri("/projects/p1/tasks/t1").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains("created"));
    }

    #[tokio::test]
    async fn task_detail_missing_returns_404() {
        let store = FakeStore::new();
        let state = AppState { store: Arc::new(store) as Arc<dyn ZenohStore> };

        let response = app(state)
            .oneshot(Request::builder().uri("/projects/p1/tasks/missing").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
```

This test module will now need `http_body_util::BodyExt` imported too (used by `task_detail_shows_history`) — check the existing `use` list at the top of the `#[cfg(test)] mod tests` block in `task.rs` and add `use http_body_util::BodyExt;` if it isn't already imported from an earlier task in this file.

- [ ] **Step 3: Wire the route**

In `web/src/lib.rs`, change the line added in Task 12 from:

```rust
.route("/projects/{id}/tasks/{task_id}", axum::routing::delete(handlers::task::delete))
```

to:

```rust
.route("/projects/{id}/tasks/{task_id}", get(handlers::task::show).delete(handlers::task::delete))
```

- [ ] **Step 4: Run the tests**

Run: `cd web && cargo test`
Expected: all prior tests plus the two new `task_detail` tests pass.

- [ ] **Step 5: Commit**

```bash
git add web/src/handlers/task.rs web/src/lib.rs web/templates/task_detail.html
git commit -m "feat(web): add task detail page"
```

---

### Task 14: Static assets (vendored CSS + htmx)

**Files:**
- Create: `web/static/pico.min.css` (downloaded, not hand-written)
- Create: `web/static/htmx.min.js` (downloaded, not hand-written)
- Create: `web/src/handlers/static_assets.rs`
- Modify: `web/src/handlers/mod.rs` (add `pub mod static_assets;`)
- Modify: `web/src/lib.rs` (extend `app()` with the two `/static/*` routes)

**Interfaces:**
- Produces: `handlers::static_assets::style_css() -> impl IntoResponse`
- Produces: `handlers::static_assets::htmx_js() -> impl IntoResponse`

- [ ] **Step 1: Vendor the two files**

Run from the repo root:

```bash
mkdir -p web/static
curl -sL "https://raw.githubusercontent.com/picocss/pico/main/css/pico.min.css" -o web/static/pico.min.css
curl -sL "https://raw.githubusercontent.com/bigskysoftware/htmx/master/dist/htmx.min.js" -o web/static/htmx.min.js
```

Verify both downloaded successfully (non-trivial size, not an HTML error page):

```bash
wc -l web/static/pico.min.css web/static/htmx.min.js
```

Expected: both files have substantial content (pico.min.css is a large single-line minified CSS file; htmx.min.js similarly a large minified JS file — a `wc -l` count of 1-2 lines each is normal for minified files, just confirm they're non-empty, e.g. `wc -c` shows several KB+).

- [ ] **Step 2: Write the failing test and the handlers**

Create `web/src/handlers/static_assets.rs`:

```rust
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
```

Modify `web/src/handlers/mod.rs` to add `pub mod static_assets;`.

- [ ] **Step 3: Wire the routes**

In `web/src/lib.rs`'s `app()`, add:

```rust
.route("/static/style.css", get(handlers::static_assets::style_css))
.route("/static/htmx.min.js", get(handlers::static_assets::htmx_js))
```

- [ ] **Step 4: Run the tests**

Run: `cd web && cargo test`
Expected: all prior tests plus the two new `static_assets` tests pass.

- [ ] **Step 5: Commit**

```bash
git add web/static web/src/handlers/static_assets.rs web/src/handlers/mod.rs web/src/lib.rs
git commit -m "feat(web): vendor and serve Pico.css and htmx"
```

---

### Task 15: Real startup binary (`main.rs`)

**Files:**
- Create: `web/src/main.rs`

**Interfaces:**
- Consumes: everything produced so far — `app`, `AppState`, `iso_now` (lib.rs); `resolve_endpoint`, `open_session`, `RealZenohStore` (Task 5)

This task has no automated test — it's a thin binary entrypoint (the same pragmatic choice `ztask/cli.py`'s `if __name__ == "__main__": app()` makes). It's verified manually against the router container you already have working from earlier sessions.

- [ ] **Step 1: Write `main.rs`**

```rust
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

    let store: Arc<dyn ZenohStore> = Arc::new(RealZenohStore::new(session));
    let state = AppState { store };

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    tracing::info!("listening on 0.0.0.0:8080");
    axum::serve(listener, app(state)).await.unwrap();
}
```

- [ ] **Step 2: Build**

Run: `cd web && cargo build`
Expected: compiles cleanly (this is the first time the `[[bin]]` target — auto-detected from `src/main.rs` — gets built; if it fails to compile, the error is almost certainly a mismatch between what `lib.rs` exports as `pub` and what `main.rs` imports here — check `pub mod zenoh_client;`, `pub mod zenoh_store;`, and that `AppState`/`app`/`iso_now` are all `pub` in `lib.rs`).

- [ ] **Step 3: Manually verify against a real router**

If a router container isn't already running, start one (adjust the port/image name if you're using a different tag than prior sessions):

```bash
container run -d --name ztask-web-manual-check -p 17461:7447 ztask-router:integration-test
```

Then run the web binary against it:

```bash
cd web
ZTASK_ZENOH_ENDPOINT=tcp/localhost:17461 cargo run
```

In another terminal, verify it serves real data:

```bash
curl -s http://localhost:8080/healthz
curl -s http://localhost:8080/ | grep -o '<h1>.*</h1>'
```

Expected: `ok` from `/healthz`; `<h1>Projects</h1>` from `/` (an empty dashboard is fine if no tasks exist yet — the point is it starts, connects, and renders without erroring).

Stop both the binary (Ctrl-C) and the container (`container rm -f ztask-web-manual-check`) when done.

- [ ] **Step 4: Commit**

```bash
git add web/src/main.rs
git commit -m "feat(web): add main.rs startup binary"
```

---

### Task 16: `docker/web/Dockerfile`

**Files:**
- Create: `docker/web/Dockerfile`

**Interfaces:**
- Consumes: the `web/` crate built in Tasks 1-15 (must have a committed `web/Cargo.lock` — it will, since Task 1's commit included it)

- [ ] **Step 1: Write the Dockerfile**

```dockerfile
# syntax=docker/dockerfile:1

FROM rust:1-bookworm AS build

WORKDIR /src
COPY web/Cargo.toml web/Cargo.lock ./
COPY web/src ./src
COPY web/templates ./templates
COPY web/static ./static

RUN cargo build --release

FROM debian:bookworm-slim AS runtime

RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=build /src/target/release/ztask-web /usr/local/bin/ztask-web

EXPOSE 8080

ENTRYPOINT ["ztask-web"]
```

- [ ] **Step 2: Build and verify against a real router**

From the repo root, build the image:

```bash
container build -f docker/web/Dockerfile -t ztask-web:local .
```

Expected: builds successfully (this compiles the whole crate fresh inside the container — expect a few minutes, similar in kind to the router's Rust build, though much smaller since there's no C library or plugin dance).

Verify it runs and connects to a router. First ensure the `ztask-net` network and a router container exist (reuse `scripts/up.sh`'s router-only portion, or the manual container from Task 15 if it's still running on that network):

```bash
container network create ztask-net 2>/dev/null || true
container rm -f ztask-web-dockertest zenoh-router-dockertest 2>/dev/null || true
container run -d --name zenoh-router-dockertest --network ztask-net ztask-router:integration-test
container run -d --name ztask-web-dockertest --network ztask-net -e ZTASK_ZENOH_ENDPOINT=tcp/zenoh-router-dockertest:7447 -p 18080:8080 ztask-web:local
```

Wait a couple seconds, then:

```bash
curl -s http://localhost:18080/healthz
```

Expected: `ok`.

Clean up:

```bash
container rm -f ztask-web-dockertest zenoh-router-dockertest
```

- [ ] **Step 3: Commit**

```bash
git add docker/web/Dockerfile
git commit -m "feat(web): add web container Dockerfile"
```

---

### Task 17: Extend `scripts/up.sh` to bring up both containers

**Files:**
- Modify: `scripts/up.sh`

**Interfaces:**
- Consumes: `docker/router/Dockerfile` (existing), `docker/web/Dockerfile` (Task 16)

- [ ] **Step 1: Modify the script**

Replace the full contents of `scripts/up.sh` with:

```bash
#!/usr/bin/env bash
set -euo pipefail

RUNTIME="${ZTASK_CONTAINER_RUNTIME:-docker}"
NETWORK="ztask-net"
ROUTER_IMAGE="ztask-router:local"
WEB_IMAGE="ztask-web:local"

if ! "$RUNTIME" network inspect "$NETWORK" >/dev/null 2>&1; then
  "$RUNTIME" network create "$NETWORK"
fi

"$RUNTIME" build -f docker/router/Dockerfile -t "$ROUTER_IMAGE" .

"$RUNTIME" run --rm -d \
  --name zenoh-router \
  --network "$NETWORK" \
  -p 7447:7447 \
  -v ztask-data:/data \
  "$ROUTER_IMAGE"

"$RUNTIME" build -f docker/web/Dockerfile -t "$WEB_IMAGE" .

"$RUNTIME" run --rm -d \
  --name ztask-web \
  --network "$NETWORK" \
  -e ZTASK_ZENOH_ENDPOINT=tcp/zenoh-router:7447 \
  -p 8080:8080 \
  "$WEB_IMAGE"

echo "Router running as 'zenoh-router' on network '$NETWORK', published on localhost:7447."
echo "Web UI running as 'ztask-web', published on http://localhost:8080"
echo "Local CLI: export ZTASK_ZENOH_ENDPOINT=tcp/localhost:7447"
echo "In-network agent containers: export ZTASK_ZENOH_ENDPOINT=tcp/zenoh-router:7447 (--network $NETWORK)"
```

- [ ] **Step 2: Run and verify**

```bash
container rm -f zenoh-router ztask-web 2>/dev/null || true
./scripts/up.sh
sleep 2
curl -s http://localhost:8080/healthz
curl -s http://localhost:7447 >/dev/null; echo "router port reachable: $?"
```

Expected: `ok` from the healthz check; exit code `0` printed for the router port check.

Clean up: `container rm -f zenoh-router ztask-web`.

- [ ] **Step 3: Commit**

```bash
git add scripts/up.sh
git commit -m "feat(web): bring up the web container alongside the router in up.sh"
```

---

### Task 18: Opt-in integration test against a real router

**Files:**
- Create: `web/tests/web_integration.rs`

**Interfaces:**
- Consumes: `app`, `AppState` (lib.rs), `open_session`, `RealZenohStore` (Task 5)

Mirrors `tests/integration/test_cli_integration.py` and its `conftest.py` router fixture — builds and runs the real router container, waits for it to accept connections, then drives the axum app **in-process** (via `tower::ServiceExt::oneshot`, no HTTP server needed) with a `RealZenohStore` pointed at that container, asserting on real Garry-backed round trips. Gated behind `#[ignore]`, matching the Python suite's opt-in `-m integration` marker.

- [ ] **Step 1: Write the test file**

Create `web/tests/web_integration.rs`:

```rust
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
    let store: Arc<dyn ZenohStore> = Arc::new(RealZenohStore::new(session));
    app(AppState { store })
}

#[tokio::test]
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

#[tokio::test]
#[ignore]
async fn update_status_persists_and_appears_in_project_list() {
    let _guard = start_router();
    let router = build_app().await;

    router
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
```

- [ ] **Step 2: Run it**

Run: `cd web && cargo test --test web_integration -- --ignored --test-threads=1`

`--test-threads=1` is required: both tests build/start a container with the same fixed name and port, so running them concurrently would race. This mirrors the Python suite's `scope="session"` fixture sharing one container — here each test manages its own, so serial execution is what keeps them from colliding.

Expected: both tests `ok` (each takes a while — it's building the router image from source, same as the Python integration fixture does).

- [ ] **Step 3: Commit**

```bash
git add web/tests/web_integration.rs
git commit -m "test(web): add opt-in integration test against a real router"
```

---

## Self-Review Notes

- **Spec coverage:** all-projects dashboard (Task 7), per-project dashboard + filter (Task 8), create (Task 9), update-status (Task 10), edit-criteria (Task 11), delete (Task 12), task detail/history (Task 13), static assets (Task 14), `ZTASK_ZENOH_ENDPOINT` convention (Task 5), `ztask-net` deployment (Tasks 16-17), unit + opt-in integration testing strategy (Tasks 1-14, 18) — every section of the design spec has a corresponding task.
- **Placeholder scan:** no TBD/TODO; every step has literal code or literal shell commands, not descriptions of code.
- **Type consistency:** `ZenohStore::{get,put,delete}` signatures are identical from Task 1 through every consumer; `Task`/`HistoryEntry`/`ProjectSummary` fields are used with the same names in `queries.rs`, `tasks.rs`, and every template; `TaskRowTemplate`'s single definition is created in Task 8 and explicitly relocated (not duplicated) to `task.rs` in Task 10, with `project.rs`'s callers updated in the same task.
- **Route collision handled explicitly:** Tasks 12 and 13 both touch the `/projects/{id}/tasks/{task_id}` route line in `lib.rs`; Task 12 registers it `DELETE`-only, Task 13's Step 3 gives the exact before/after text to merge in `GET`, rather than leaving two conflicting `.route()` calls on the same path (which would panic at startup).
