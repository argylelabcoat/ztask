# Web UI (ztask-web)

Rust (axum + askama + htmx) admin web UI for humans.

## Overview

A web UI that talks to the Zenoh router directly over the official zenoh Rust SDK — no CLI shell-out. Provides an all-projects dashboard and a per-project dashboard with inline create/update-status/edit-criteria/delete.

Defaults task creation to `entered_by: USER`.

## Crate Structure

```
web/
  Cargo.toml
  src/
    main.rs            # entry point, binds on 0.0.0.0:8080
    lib.rs             # app() router, AppState, helpers
    models.rs          # Task, HistoryEntry structs
    queries.rs         # fetch_all_tasks, fetch_task, fetch_all_projects
    tasks.rs           # create_task, update_status, edit_criteria, delete_task
    render.rs          # HtmlTemplate wrapper
    zenoh_client.rs    # session management, RealZenohStore
    zenoh_store.rs     # ZenohStore trait + FakeStore for tests
    handlers/
      mod.rs
      dashboard.rs     # GET / — all-projects dashboard
      project.rs       # GET /projects/{id}, POST /projects/{id}/tasks
      task.rs          # GET/DELETE /projects/{id}/tasks/{task_id}, POST .../status, POST .../criteria
      static_assets.rs # GET /static/style.css, /static/htmx.min.js
  templates/
    base.html
    dashboard.html
    project.html
    task_row.html
    task_detail.html
  tests/
    web_integration.rs # integration tests (real router container)
```

## Components

### `zenoh_store.rs`

Abstraction trait for Zenoh operations:

```rust
#[async_trait]
pub trait ZenohStore: Send + Sync {
    async fn get(&self, key_expr: &str) -> Vec<(String, String)>;
    async fn put(&self, key_expr: &str, value: &str);
    async fn delete(&self, key_expr: &str);
}
```

`FakeStore` for unit tests — in-memory BTreeMap with wildcard matching (`*` and `**`).

### `zenoh_client.rs`

Endpoint resolution from `ZTASK_ZENOH_ENDPOINT` env var (default: `tcp/localhost:7447`). `RealZenohStore` implements `ZenohStore` using a real Zenoh session.

### `models.rs`

```rust
pub struct HistoryEntry {
    pub timestamp: String,
    pub from_status: String,
    pub to_status: String,
    pub note: String,  // defaults to ""
}

pub struct Task {
    pub id: String,
    pub status: String,
    pub time_entered: Option<String>,
    pub time_accepted: Option<String>,
    pub time_completed: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub entered_by: Option<String>,
    pub history: Vec<HistoryEntry>,
    // SDD→TDD fields (when implemented)
    pub spec: Option<String>,
    pub depends_on: Vec<String>,
    pub blocks: Vec<String>,
    pub test_files: Vec<String>,
    pub implementation_files: Vec<String>,
    pub tdd_phase: Option<String>,
    pub test_command: Option<String>,
    pub verification_command: Option<String>,
    pub failure_reason: Option<String>,
    pub attempt_count: u32,
}
```

### `queries.rs`

- `fetch_all_tasks(store, project_id)` → `HashMap<String, Task>`
  - Queries `projects/{project_id}/tasks/**`
  - Groups fields by task ID via `apply_field()`

- `fetch_task(store, project_id, task_id)` → `Option<Task>`
  - Queries `projects/{project_id}/tasks/{task_id}/**`

- `fetch_all_projects(store)` → `Vec<ProjectSummary>`
  - Queries `projects/*/tasks/*/status`
  - Groups by project ID, counts total/incomplete/wip

`apply_field()` maps Zenoh key suffixes to Task fields:
- `"status"` → `task.status`
- `"time_entered"` → `task.time_entered`
- `"history/*"` → parse JSON, push to `task.history`
- etc.

### `tasks.rs`

Business logic for task mutations:

- `create_task(store, project_id, task_id, criteria, now)` → `Task`
  - Writes status=PENDING, time_entered, entered_by=USER
  - Writes acceptance_criteria if non-empty
  - Appends history entry

- `update_status(store, project_id, task_id, status, note, now)` → `Result<Task, TaskError>`
  - Reads current task (404 if missing)
  - Updates status
  - Sets time_accepted (WIP transition) or time_completed (COMPLETED transition)
  - Appends history entry

- `edit_criteria(store, project_id, task_id, criteria, now)` → `Result<Task, TaskError>`
  - Reads current task (404 if missing)
  - Updates acceptance_criteria
  - Appends history entry ("criteria updated")

- `delete_task(store, project_id, task_id)`
  - Deletes `projects/{project_id}/tasks/{task_id}/**`

### `handlers/`

HTTP handlers using axum extractors:

**`dashboard.rs`** — `GET /`
- Fetches all projects via `fetch_all_projects()`
- Renders `dashboard.html` template

**`project.rs`** — `GET /projects/{id}`, `POST /projects/{id}/tasks`
- `show()`: fetches all tasks, applies filter (all/incomplete/wip), sorts by ID, renders `project.html`
- `create()`: validates IDs, creates task, returns `task_row.html` fragment (htmx)

**`task.rs`** — `GET/DELETE /projects/{id}/tasks/{task_id}`, `POST .../status`, `POST .../criteria`
- `show()`: fetches single task, renders `task_detail.html`
- `update_status()`: validates, updates status, returns `task_row.html` fragment
- `edit_criteria()`: validates, updates criteria, returns `task_row.html` fragment
- `delete()`: validates, deletes task, returns 200

### `render.rs`

`HtmlTemplate<T>` wrapper that renders askama templates into HTML responses, or 500 on template error.

### `lib.rs`

- `AppState` — holds `Arc<dyn ZenohStore>`
- `iso_now()` — UTC timestamp helper
- `is_valid_id()` — rejects empty strings and wildcards (`*?#$/`)
- `app()` — builds the axum Router with all routes

## Routes

| Method | Path | Handler | Description |
|--------|------|---------|-------------|
| GET | `/` | `dashboard::show` | All-projects dashboard |
| GET | `/projects/{id}` | `project::show` | Per-project task list |
| POST | `/projects/{id}/tasks` | `project::create` | Create task (form) |
| GET | `/projects/{id}/tasks/{task_id}` | `task::show` | Task detail |
| POST | `/projects/{id}/tasks/{task_id}/status` | `task::update_status` | Update status (form) |
| POST | `/projects/{id}/tasks/{task_id}/criteria` | `task::edit_criteria` | Edit criteria (form) |
| DELETE | `/projects/{id}/tasks/{task_id}` | `task::delete` | Delete task |
| GET | `/static/style.css` | `static_assets::style_css` | Stylesheet |
| GET | `/static/htmx.min.js` | `static_assets::htmx_js` | htmx library |
| GET | `/healthz` | inline | Health check |

## HTMX Integration

The web UI uses htmx for inline interactions:
- Task creation returns a `task_row.html` fragment that gets inserted into the task list
- Status updates return an updated `task_row.html` fragment that replaces the existing row
- Criteria edits return an updated `task_row.html` fragment
- Deletes remove the row from the DOM

## Environment Variables

| Variable | Default | Description |
|----------|---------|-------------|
| `ZTASK_ZENOH_ENDPOINT` | `tcp/localhost:7447` | Zenoh router endpoint |

## Dependencies

Key crates: `axum`, `askama`, `tokio`, `zenoh`, `serde`, `serde_json`, `chrono`, `tracing`, `async-trait`

## Testing

- **Unit** — `FakeStore` enables testing handlers without network; tests in each module
- **Integration** (`tests/web_integration.rs`) — real router container, marked `#[ignore]`

## Startup

```bash
cd web
ZTASK_ZENOH_ENDPOINT=tcp/localhost:7447 cargo run
```

Listens on `0.0.0.0:8080`. Includes a 500ms settle delay after opening the Zenoh session to avoid cold-start routing races.
