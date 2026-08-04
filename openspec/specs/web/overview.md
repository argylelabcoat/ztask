# Web UI (ztask-web)

Rust (axum + askama + htmx) admin web UI for humans.

## Overview

A web UI that talks to the Zenoh router directly over the official zenoh Rust SDK — no CLI shell-out. Provides an all-projects dashboard — sortable, with inline project creation — a per-project dashboard with inline task creation and delete, and a per-project metrics dashboard.

In the per-project task table, Delete is its own trailing column, separate from the other actions. Editing a task (status, acceptance criteria, and a handful of the SDD/TDD fields) happens in a modal dialog opened from an "Edit" button — there is no inline per-field edit form in the row.

Defaults task creation to `entered_by: USER`.

A project has no standalone entity or marker key — its existence is fully derived from its tasks. Creating a project from the dashboard is creating the first task under a project ID that has no existing tasks.

## Non-goals (this pass)

- Syntax highlighting or linting for the acceptance-criteria editor (Gherkin/Cucumber-aware editing is a planned follow-up once the plain textarea editor exists).
- A pills/chip-style input for `depends_on`/`blocks` — v1 uses a plain comma-separated text field; chips are a planned follow-up.
- Editing the TDD-execution fields (`tdd_phase`, `test_files`, `implementation_files`, `test_command`, `verification_command`, `failure_reason`) through the web UI — these stay agent-managed and display-only in `task_detail.html`.

## Crate Structure

```
web/
  Cargo.toml
  src/
    main.rs            # entry point, binds on 0.0.0.0:8080
    lib.rs             # app() router, AppState, helpers
    models.rs          # Task, HistoryEntry structs
    queries.rs         # fetch_all_tasks, fetch_task, fetch_all_projects, SortKey/SortDir/sort_projects
    tasks.rs           # create_task, edit_task, delete_task
    metrics.rs         # pure computation: status breakdown, stuck/churn detection, velocity, transition matrix
    render.rs          # HtmlTemplate wrapper
    zenoh_client.rs    # session management, RealZenohStore
    zenoh_store.rs     # ZenohStore trait + FakeStore for tests
    handlers/
      mod.rs
      dashboard.rs     # GET / (sortable), POST /projects — all-projects dashboard + project creation
      project.rs       # GET /projects/{id}, POST /projects/{id}/tasks
      task.rs          # GET/DELETE /projects/{id}/tasks/{task_id}, GET/POST .../edit
      metrics.rs       # GET /projects/{id}/metrics — per-project metrics dashboard
      static_assets.rs # GET /static/style.css, /static/htmx.min.js, /static/line-numbers.js
  templates/
    base.html          # includes the shared <dialog id="edit-modal">
    dashboard.html
    project.html
    task_row.html
    task_detail.html
    task_edit.html     # edit-modal form fragment (GET .../edit response)
    metrics.html
  static/
    line-numbers.js    # hand-rolled line-number gutter for the criteria textarea (no library)
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
  - Queries `projects/*/tasks/*/status`, groups by project ID, counts total/incomplete/wip
  - Queries `projects/*/tasks/*/history/*` to fold in `last_activity` (max history timestamp per project)
  - Returns projects sorted by ID ascending (the default; callers apply `sort_projects` for other orderings)

`apply_field()` maps Zenoh key suffixes to Task fields:
- `"status"` → `task.status`
- `"time_entered"` → `task.time_entered`
- `"history/*"` → parse JSON, push to `task.history`
- etc.

```rust
pub struct ProjectSummary {
    pub id: String,
    pub total: usize,
    pub incomplete: usize,
    pub wip: usize,
    pub last_activity: Option<String>,  // RFC3339, max history timestamp across the project's tasks
}
```

**Sorting** — `SortKey` (`Name | Total | Incomplete | Wip | Activity`) and `SortDir` (`Asc | Desc`), each with `parse(&str)` (unknown/empty input silently falls back to `Name`/`Asc`) and `as_str()`; `SortDir::flip()` toggles direction. `sort_projects(projects, key, dir)` sorts a `&mut [ProjectSummary]` in place. Sort state lives only in the `GET /` query string (`?sort=&dir=`) — no cookies, no session.

### `tasks.rs`

Business logic for task mutations:

- `create_task(store, project_id, task_id, criteria, now)` → `Task`
  - Writes status=PENDING, time_entered, entered_by=USER
  - Writes acceptance_criteria if non-empty
  - Appends history entry

- `edit_task(store, project_id, task_id, status, criteria, spec, depends_on, blocks, note, now)` → `Result<Task, TaskError>` — the single write path behind the edit modal, replacing the former separate `update_status`/`edit_criteria` functions
  - Reads current task (404 if missing)
  - Updates status; sets `time_accepted` (WIP transition) or `time_completed` (COMPLETED transition) using the same rules the old `update_status` used
  - Updates `acceptance_criteria`, `spec` (always writes the given value, including empty-to-clear, matching the former `edit_criteria`'s behavior)
  - Updates `depends_on`/`blocks`: caller passes already-split `Vec<String>`, written back as a comma-joined string — `queries.rs`'s `apply_field` already falls back to comma-split parsing when a `depends_on`/`blocks` value isn't valid JSON, so no new read-path parsing is needed
  - Appends one history entry ("edited via modal") regardless of how many fields actually changed

- `delete_task(store, project_id, task_id)`
  - Deletes `projects/{project_id}/tasks/{task_id}/**`

### `metrics.rs`

Pure computation over an already-fetched `HashMap<String, Task>` and a fixed `now: DateTime<Utc>` — no `ZenohStore` dependency, so it's unit-tested directly with hand-built `Task`/`HistoryEntry` fixtures.

**Thresholds** — read (never written) by the metrics handler from per-project config keys, falling back to fixed defaults if missing or unparseable:

| Key | Default | Meaning |
|-----|---------|---------|
| `projects/{id}/config/stuck_threshold_hours` | `2.0` | Hours since a task's last history entry before it's flagged stuck |
| `projects/{id}/config/churn_transition_count` | `4` | History-entry count at or above which a task is flagged churning |

There is no UI to edit these in v1 — set them directly via `ztask` or a zenoh client if the defaults don't fit a project.

**Status breakdown** — three buckets, same categorization `ProjectSummary` already implies:
- `completed`: `status == COMPLETED`
- `wip`: `status` in `WIP_STATUSES`
- `open`: everything else (PENDING and any other non-terminal, non-WIP status)

Rendered as an SVG donut chart (hand-computed `stroke-dasharray`/`stroke-dashoffset` per bucket around a shared circle — no charting library) with a count legend.

**Stuck / churning detection** — per task, evaluated only while `status` is non-terminal (`!= COMPLETED`):
- **stuck**: hours since the task's most recent history entry exceeds `stuck_threshold_hours`
- **churning**: the task's history has at least `churn_transition_count` entries

A task can be both. Flagged tasks surface in a dedicated list on the metrics page.

**Velocity** — one entry per calendar day from the project's earliest history entry to today (zero-filled for days with no completions), counting `to_status == COMPLETED` transitions per day. Rendered as a horizontally-scrollable CSS bar chart (div width/height proportional to count).

**Transition matrix** — axes are the distinct statuses actually observed in the project's history (built dynamically, not hardcoded — tolerates `WIP_STATUSES` synonyms like `WIP`/`RUNNING`). Cell = count of `from_status → to_status` transitions across all the project's tasks. Rendered as a table with cell background-color intensity (`rgba` alpha scaled to count) — a hot off-diagonal cell (e.g. `IN_PROGRESS → PENDING`) visualizes churn at a glance.

**Timing table** — per task: queued duration (`time_entered → time_accepted`), work duration (`time_accepted → time_completed`, or → `now` if still open), current-status duration, transition count, and the stuck/churning flags.

### `handlers/`

HTTP handlers using axum extractors:

**`dashboard.rs`** — `GET /`, `POST /projects`
- `show()`: parses `?sort=&dir=` query params, fetches all projects, sorts them via `sort_projects`, renders `dashboard.html` — sort-toggle link hrefs and the active column/direction are computed server-side and passed to the template. Each row links directly to that project's `/projects/{id}/metrics` page, alongside the link to `/projects/{id}` itself.
- `build_dashboard()`: shared by `show()` and `create()`'s error paths — fetches, sorts, and assembles the template's data, including an optional inline form error and the submitted form values (for re-populating the create-project form on failure)
- `create()`: creates a new project by creating its first task —
  1. validates `project_id`/`task_id` via `is_valid_id` → `400` (re-renders dashboard, form values preserved, inline error) if either fails
  2. checks `fetch_all_tasks(project_id)` is empty → `409` (same re-render) if the project already has tasks — this route is create-only, not an alternate path to `POST /projects/{id}/tasks`
  3. otherwise calls `tasks::create_task` (no new key namespace — identical to the per-project create-task flow) and responds `303 See Other` with `Location: /projects/{project_id}`
  - Plain HTML form submit (not htmx): the target URL depends on user-entered input (the new project ID), which htmx's static `hx-post` can't express

**`project.rs`** — `GET /projects/{id}`, `POST /projects/{id}/tasks`
- `show()`: fetches all tasks, applies filter (all/incomplete/wip), sorts by ID, renders `project.html`
- `create()`: validates IDs, creates task, returns `task_row.html` fragment (htmx)

**`task.rs`** — `GET/DELETE /projects/{id}/tasks/{task_id}`, `GET/POST .../edit`
- `show()`: fetches single task, renders `task_detail.html`
- `edit_form()`: validates, fetches the task (404 if missing), renders `task_edit.html` — the modal's form fragment, pre-filled with the task's current editable field values (`depends_on`/`blocks` joined with `, ` for display in their text inputs)
- `edit()`: validates, parses the combined form (splitting `depends_on`/`blocks` on commas, trimming, dropping empties), calls `tasks::edit_task`, returns the updated `task_row.html` fragment — the client-side `hx-on::after-request` on the form closes the dialog after a successful swap
- `delete()`: validates, deletes task, returns 200

**`metrics.rs`** — `GET /projects/{id}/metrics`
- `show()`: fetches all tasks, reads the two threshold config keys (falling back to defaults), runs the `metrics.rs` computations (status breakdown, stuck/churning list, velocity, transition matrix, timing table), renders `metrics.html`

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
| GET | `/` | `dashboard::show` | All-projects dashboard, supports `?sort=name\|total\|incomplete\|wip\|activity&dir=asc\|desc` |
| POST | `/projects` | `dashboard::create` | Create a project (+ its first task); `303` to `/projects/{id}` on success, `400`/`409` re-renders dashboard on failure |
| GET | `/projects/{id}` | `project::show` | Per-project task list |
| POST | `/projects/{id}/tasks` | `project::create` | Create task (form) |
| GET | `/projects/{id}/tasks/{task_id}` | `task::show` | Task detail |
| GET | `/projects/{id}/tasks/{task_id}/edit` | `task::edit_form` | Edit-modal form fragment (htmx) |
| POST | `/projects/{id}/tasks/{task_id}/edit` | `task::edit` | Combined save (status, criteria, spec, depends_on, blocks); returns updated `task_row.html` fragment |
| DELETE | `/projects/{id}/tasks/{task_id}` | `task::delete` | Delete task |
| GET | `/projects/{id}/metrics` | `metrics::show` | Per-project metrics dashboard |
| GET | `/static/style.css` | `static_assets::style_css` | Stylesheet |
| GET | `/static/htmx.min.js` | `static_assets::htmx_js` | htmx library |
| GET | `/static/line-numbers.js` | `static_assets::line_numbers_js` | Criteria-textarea line-number gutter script |
| GET | `/healthz` | inline | Health check |

## HTMX Integration

The web UI uses htmx for inline interactions:
- Task creation returns a `task_row.html` fragment that gets inserted into the task list
- The edit modal's save returns an updated `task_row.html` fragment that replaces the existing row
- Deletes remove the row from the DOM

**Edit modal** — a single `<dialog id="edit-modal">` lives in `base.html`, present but empty on every page. The row's (or `task_detail.html`'s) "Edit" button does `hx-get` to `GET /projects/{id}/tasks/{task_id}/edit`, swaps the response into the dialog body, and opens it via `hx-on::after-request="document.getElementById('edit-modal').showModal()"` — plain htmx plus the standard `<dialog>` element, no new JS library. The form inside submits `POST` to the same path; on success the response replaces the task row (existing `hx-target`/`hx-swap="outerHTML"` pattern) and `hx-on::after-request="if(event.detail.successful) this.closest('dialog').close()"` closes the dialog. The criteria `<textarea>`'s line-number gutter is driven by `static/line-numbers.js`, a small hand-rolled script (not a library) loaded like `htmx.min.js`.

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
