# Project Creation + Dashboard Sorting Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let a human create a new project (with its first task) from the dashboard, and let them sort the dashboard's project table by name, total, incomplete, wip, or last-activity.

**Architecture:** No new key namespace — "creating a project" is creating the first task under a project ID that has no existing tasks, reusing `tasks::create_task`. `queries::ProjectSummary` gains a `last_activity` field computed from a second wildcard query over task history entries. Sorting is a plain in-memory `sort_by` applied after `fetch_all_projects` returns, driven by `?sort=&dir=` query params on `GET /`. The dashboard handler grows a `create` function for `POST /projects` that validates, creates, and redirects (303) on success, or re-renders the dashboard with an inline error and preserved form values on failure (400/409) — a plain HTML form submit, not htmx, since the target project ID is user input.

**Tech Stack:** Rust, axum 0.8, askama 0.16, existing `ZenohStore` trait + `FakeStore` test double (see `web/src/zenoh_store.rs`).

## Global Constraints

- No new zenoh key prefixes — reuse `projects/<id>/tasks/<id>/...` exactly as `tasks::create_task` already writes it.
- No project marker/empty-project support — a project's existence stays fully derived from its tasks.
- Sort state lives only in the URL query string (`?sort=&dir=`) — no cookies, no local storage, no server-side session.
- `POST /projects` rejects (409) if the project ID already has any tasks; it is create-only, not an alternate path to `POST /projects/{id}/tasks`.
- Unknown/missing `sort` or `dir` values fall back to `name`/`asc` silently — never a 400 for that.
- Follow existing patterns in `web/src/queries.rs`, `web/src/handlers/dashboard.rs`, and `web/src/handlers/project.rs` for validation (`crate::is_valid_id`), template rendering (`HtmlTemplate`), and test style (`FakeStore` + `app(state).oneshot(...)`).

---

### Task 1: Add `last_activity` to `ProjectSummary`

**Files:**
- Modify: `web/src/queries.rs`

**Interfaces:**
- Produces: `ProjectSummary { id: String, total: usize, incomplete: usize, wip: usize, last_activity: Option<String> }` (adds `last_activity` to the existing struct) — later tasks (2, 3) depend on this field existing.
- Consumes: existing `ZenohStore::get`, `TERMINAL_STATUS`, `WIP_STATUSES` (already in this file).

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `web/src/queries.rs`:

```rust
    #[tokio::test]
    async fn fetch_all_projects_computes_last_activity_from_latest_history_entry() {
        let store = FakeStore::new()
            .seed("projects/p1/tasks/t1/status", "PENDING")
            .seed(
                "projects/p1/tasks/t1/history/2026-07-31T00-00-00",
                r#"{"timestamp":"2026-07-31T00:00:00+00:00","from_status":"NONE","to_status":"PENDING","note":""}"#,
            )
            .seed(
                "projects/p1/tasks/t1/history/2026-08-02T00-00-00",
                r#"{"timestamp":"2026-08-02T00:00:00+00:00","from_status":"PENDING","to_status":"IN_PROGRESS","note":""}"#,
            )
            .seed("projects/p2/tasks/t1/status", "PENDING");

        let projects = fetch_all_projects(&store).await;

        let p1 = projects.iter().find(|p| p.id == "p1").unwrap();
        assert_eq!(p1.last_activity.as_deref(), Some("2026-08-02T00:00:00+00:00"));

        let p2 = projects.iter().find(|p| p.id == "p2").unwrap();
        assert_eq!(p2.last_activity, None);
    }
```

Also update the existing `fetch_all_projects_groups_and_counts_by_status` test's assertions, since `ProjectSummary` is gaining a field:

```rust
        assert_eq!(
            projects[0],
            ProjectSummary { id: "p1".to_string(), total: 2, incomplete: 1, wip: 0, last_activity: None }
        );
        assert_eq!(
            projects[1],
            ProjectSummary { id: "p2".to_string(), total: 1, incomplete: 1, wip: 1, last_activity: None }
        );
```

- [ ] **Step 2: Run tests to verify the new test fails and the codebase fails to compile**

Run: `cd web && cargo test --lib queries:: -- --nocapture`
Expected: compile error — `last_activity` is not a field of `ProjectSummary` yet.

- [ ] **Step 3: Add the field and compute it**

In `web/src/queries.rs`, update the struct:

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectSummary {
    pub id: String,
    pub total: usize,
    pub incomplete: usize,
    pub wip: usize,
    pub last_activity: Option<String>,
}
```

Update `fetch_all_projects` to initialize the new field and fold in history timestamps:

```rust
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
            last_activity: None,
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

    for (key, value) in store.get("projects/*/tasks/*/history/*").await {
        let parts: Vec<&str> = key.split('/').collect();
        let Some(project_id) = parts.get(1) else { continue };
        let Some(summary) = summaries.get_mut(*project_id) else { continue };
        let Some(timestamp) = serde_json::from_str::<serde_json::Value>(&value)
            .ok()
            .and_then(|json| json.get("timestamp").and_then(|t| t.as_str()).map(str::to_string))
        else {
            continue;
        };
        let should_update = match &summary.last_activity {
            Some(existing) => timestamp.as_str() > existing.as_str(),
            None => true,
        };
        if should_update {
            summary.last_activity = Some(timestamp);
        }
    }

    let mut result: Vec<ProjectSummary> = summaries.into_values().collect();
    result.sort_by(|a, b| a.id.cmp(&b.id));
    result
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd web && cargo test --lib queries::`
Expected: all tests in `queries.rs` pass, including the two touched/added above.

- [ ] **Step 5: Commit**

```bash
git add web/src/queries.rs
git commit -m "feat(web): compute last_activity per project from task history"
```

---

### Task 2: Add `SortKey` / `SortDir` and `sort_projects`

**Files:**
- Modify: `web/src/queries.rs`

**Interfaces:**
- Consumes: `ProjectSummary` from Task 1 (needs `id: String, total: usize, incomplete: usize, wip: usize, last_activity: Option<String>`).
- Produces: `pub enum SortKey { Name, Total, Incomplete, Wip, Activity }` with `SortKey::parse(&str) -> SortKey` and `SortKey::as_str(self) -> &'static str`; `pub enum SortDir { Asc, Desc }` with `SortDir::parse(&str) -> SortDir`, `SortDir::as_str(self) -> &'static str`, `SortDir::flip(self) -> SortDir`; `pub fn sort_projects(projects: &mut Vec<ProjectSummary>, key: SortKey, dir: SortDir)`. Task 3 depends on all of these exact names.

- [ ] **Step 1: Write the failing tests**

Add to `web/src/queries.rs`'s test module:

```rust
    fn sample_projects() -> Vec<ProjectSummary> {
        vec![
            ProjectSummary { id: "b".to_string(), total: 5, incomplete: 1, wip: 2, last_activity: Some("2026-08-01T00:00:00+00:00".to_string()) },
            ProjectSummary { id: "a".to_string(), total: 2, incomplete: 3, wip: 0, last_activity: Some("2026-08-03T00:00:00+00:00".to_string()) },
        ]
    }

    #[test]
    fn sort_key_parse_defaults_to_name_for_unknown_values() {
        assert_eq!(SortKey::parse("bogus"), SortKey::Name);
        assert_eq!(SortKey::parse("total"), SortKey::Total);
        assert_eq!(SortKey::parse("incomplete"), SortKey::Incomplete);
        assert_eq!(SortKey::parse("wip"), SortKey::Wip);
        assert_eq!(SortKey::parse("activity"), SortKey::Activity);
    }

    #[test]
    fn sort_dir_parse_defaults_to_asc_for_unknown_values() {
        assert_eq!(SortDir::parse("bogus"), SortDir::Asc);
        assert_eq!(SortDir::parse("desc"), SortDir::Desc);
        assert_eq!(SortDir::parse("asc"), SortDir::Asc);
    }

    #[test]
    fn sort_dir_flip_toggles() {
        assert_eq!(SortDir::Asc.flip(), SortDir::Desc);
        assert_eq!(SortDir::Desc.flip(), SortDir::Asc);
    }

    #[test]
    fn sort_projects_by_name_ascending() {
        let mut projects = sample_projects();
        sort_projects(&mut projects, SortKey::Name, SortDir::Asc);
        assert_eq!(projects.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
    }

    #[test]
    fn sort_projects_by_total_descending() {
        let mut projects = sample_projects();
        sort_projects(&mut projects, SortKey::Total, SortDir::Desc);
        assert_eq!(projects.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), vec!["b", "a"]);
    }

    #[test]
    fn sort_projects_by_incomplete_ascending() {
        let mut projects = sample_projects();
        sort_projects(&mut projects, SortKey::Incomplete, SortDir::Asc);
        assert_eq!(projects.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), vec!["b", "a"]);
    }

    #[test]
    fn sort_projects_by_wip_descending() {
        let mut projects = sample_projects();
        sort_projects(&mut projects, SortKey::Wip, SortDir::Desc);
        assert_eq!(projects.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), vec!["b", "a"]);
    }

    #[test]
    fn sort_projects_by_activity_descending_puts_most_recent_first() {
        let mut projects = sample_projects();
        sort_projects(&mut projects, SortKey::Activity, SortDir::Desc);
        assert_eq!(projects.iter().map(|p| p.id.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd web && cargo test --lib queries::`
Expected: compile error — `SortKey`, `SortDir`, `sort_projects` don't exist yet.

- [ ] **Step 3: Implement the types and function**

Add to `web/src/queries.rs` (anywhere below the imports, above `#[cfg(test)]`):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortKey {
    Name,
    Total,
    Incomplete,
    Wip,
    Activity,
}

impl SortKey {
    pub fn parse(s: &str) -> SortKey {
        match s {
            "total" => SortKey::Total,
            "incomplete" => SortKey::Incomplete,
            "wip" => SortKey::Wip,
            "activity" => SortKey::Activity,
            _ => SortKey::Name,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            SortKey::Name => "name",
            SortKey::Total => "total",
            SortKey::Incomplete => "incomplete",
            SortKey::Wip => "wip",
            SortKey::Activity => "activity",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDir {
    Asc,
    Desc,
}

impl SortDir {
    pub fn parse(s: &str) -> SortDir {
        if s == "desc" {
            SortDir::Desc
        } else {
            SortDir::Asc
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            SortDir::Asc => "asc",
            SortDir::Desc => "desc",
        }
    }

    pub fn flip(self) -> SortDir {
        match self {
            SortDir::Asc => SortDir::Desc,
            SortDir::Desc => SortDir::Asc,
        }
    }
}

pub fn sort_projects(projects: &mut [ProjectSummary], key: SortKey, dir: SortDir) {
    projects.sort_by(|a, b| {
        let ordering = match key {
            SortKey::Name => a.id.cmp(&b.id),
            SortKey::Total => a.total.cmp(&b.total),
            SortKey::Incomplete => a.incomplete.cmp(&b.incomplete),
            SortKey::Wip => a.wip.cmp(&b.wip),
            SortKey::Activity => a.last_activity.cmp(&b.last_activity),
        };
        if dir == SortDir::Desc {
            ordering.reverse()
        } else {
            ordering
        }
    });
}
```

(`sort_projects` takes `&mut [ProjectSummary]` rather than `&mut Vec<..>` — accepts a `&mut Vec` at call sites too, since `Vec` derefs to slice.)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd web && cargo test --lib queries::`
Expected: all pass.

- [ ] **Step 5: Commit**

```bash
git add web/src/queries.rs
git commit -m "feat(web): add SortKey/SortDir and sort_projects"
```

---

### Task 3: Wire dashboard sorting (handler + template)

**Files:**
- Modify: `web/src/handlers/dashboard.rs`
- Modify: `web/templates/dashboard.html`

**Interfaces:**
- Consumes: `queries::fetch_all_projects`, `queries::SortKey::{parse, as_str}`, `queries::SortDir::{parse, as_str, flip}`, `queries::sort_projects` (all from Tasks 1–2); `crate::render::HtmlTemplate`.
- Produces: `DashboardTemplate` gains fields `sort_name_href`, `sort_total_href`, `sort_incomplete_href`, `sort_wip_href`, `sort_activity_href`, `active_sort`, `active_dir` (all `String`) plus `form_error: Option<String>`, `form_project_id: String`, `form_task_id: String`, `form_criteria: String` (the last four are used by Task 4 too — add them now so the struct only changes shape once). Produces `pub(crate) async fn build_dashboard(state: &AppState, sort: queries::SortKey, dir: queries::SortDir, form_error: Option<String>, form_project_id: String, form_task_id: String, form_criteria: String) -> DashboardTemplate` — Task 4's `create` handler calls this directly.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `web/src/handlers/dashboard.rs`:

```rust
    #[tokio::test]
    async fn dashboard_sorts_by_total_descending_when_requested() {
        let store = FakeStore::new()
            .seed("projects/small/tasks/t1/status", "PENDING")
            .seed("projects/big/tasks/t1/status", "PENDING")
            .seed("projects/big/tasks/t2/status", "PENDING");
        let state = AppState { store: Arc::new(store) };

        let response = app(state)
            .oneshot(Request::builder().uri("/?sort=total&dir=desc").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8(body.to_vec()).unwrap();
        let big_pos = html.find("big").unwrap();
        let small_pos = html.find("small").unwrap();
        assert!(big_pos < small_pos, "expected 'big' (total=2) to appear before 'small' (total=1) when sorted by total desc");
    }

    #[tokio::test]
    async fn dashboard_falls_back_to_default_sort_for_unknown_sort_value() {
        let store = FakeStore::new()
            .seed("projects/p1/tasks/t1/status", "PENDING")
            .seed("projects/p2/tasks/t1/status", "PENDING");
        let state = AppState { store: Arc::new(store) };

        let response = app(state)
            .oneshot(Request::builder().uri("/?sort=nonsense").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd web && cargo test --lib handlers::dashboard::`
Expected: FAIL to compile — the test file references `sort=`/`dir=` query behavior that `DashboardTemplate`/`show` don't implement yet (the struct has no `sort_*_href`/`active_sort`/`active_dir` fields, and `show` ignores query params entirely).

- [ ] **Step 3: Implement**

Replace the full contents of `web/src/handlers/dashboard.rs` with:

```rust
use askama::Template;
use axum::extract::{Query, State};
use serde::Deserialize;

use crate::render::HtmlTemplate;
use crate::{queries, AppState};

#[derive(Template)]
#[template(path = "dashboard.html")]
pub struct DashboardTemplate {
    pub projects: Vec<queries::ProjectSummary>,
    pub sort_name_href: String,
    pub sort_total_href: String,
    pub sort_incomplete_href: String,
    pub sort_wip_href: String,
    pub sort_activity_href: String,
    pub active_sort: String,
    pub active_dir: String,
    pub form_error: Option<String>,
    pub form_project_id: String,
    pub form_task_id: String,
    pub form_criteria: String,
}

#[derive(Deserialize)]
pub struct SortQuery {
    #[serde(default)]
    sort: String,
    #[serde(default)]
    dir: String,
}

fn sort_link(col: queries::SortKey, current_sort: queries::SortKey, current_dir: queries::SortDir) -> String {
    let dir = if col == current_sort { current_dir.flip() } else { queries::SortDir::Asc };
    format!("/?sort={}&dir={}", col.as_str(), dir.as_str())
}

pub(crate) async fn build_dashboard(
    state: &AppState,
    sort: queries::SortKey,
    dir: queries::SortDir,
    form_error: Option<String>,
    form_project_id: String,
    form_task_id: String,
    form_criteria: String,
) -> DashboardTemplate {
    let mut projects = queries::fetch_all_projects(state.store.as_ref()).await;
    queries::sort_projects(&mut projects, sort, dir);

    DashboardTemplate {
        projects,
        sort_name_href: sort_link(queries::SortKey::Name, sort, dir),
        sort_total_href: sort_link(queries::SortKey::Total, sort, dir),
        sort_incomplete_href: sort_link(queries::SortKey::Incomplete, sort, dir),
        sort_wip_href: sort_link(queries::SortKey::Wip, sort, dir),
        sort_activity_href: sort_link(queries::SortKey::Activity, sort, dir),
        active_sort: sort.as_str().to_string(),
        active_dir: dir.as_str().to_string(),
        form_error,
        form_project_id,
        form_task_id,
        form_criteria,
    }
}

pub async fn show(State(state): State<AppState>, Query(query): Query<SortQuery>) -> HtmlTemplate<DashboardTemplate> {
    let sort = queries::SortKey::parse(&query.sort);
    let dir = queries::SortDir::parse(&query.dir);
    HtmlTemplate(build_dashboard(&state, sort, dir, None, String::new(), String::new(), String::new()).await)
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

    #[tokio::test]
    async fn dashboard_sorts_by_total_descending_when_requested() {
        let store = FakeStore::new()
            .seed("projects/small/tasks/t1/status", "PENDING")
            .seed("projects/big/tasks/t1/status", "PENDING")
            .seed("projects/big/tasks/t2/status", "PENDING");
        let state = AppState { store: Arc::new(store) };

        let response = app(state)
            .oneshot(Request::builder().uri("/?sort=total&dir=desc").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8(body.to_vec()).unwrap();
        let big_pos = html.find("big").unwrap();
        let small_pos = html.find("small").unwrap();
        assert!(big_pos < small_pos, "expected 'big' (total=2) to appear before 'small' (total=1) when sorted by total desc");
    }

    #[tokio::test]
    async fn dashboard_falls_back_to_default_sort_for_unknown_sort_value() {
        let store = FakeStore::new()
            .seed("projects/p1/tasks/t1/status", "PENDING")
            .seed("projects/p2/tasks/t1/status", "PENDING");
        let state = AppState { store: Arc::new(store) };

        let response = app(state)
            .oneshot(Request::builder().uri("/?sort=nonsense").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
```

Replace the full contents of `web/templates/dashboard.html` with:

```html
{% extends "base.html" %}
{% block title %}All Projects — Zenoh Tasks{% endblock %}
{% block content %}
<h1>Projects</h1>

{% if let Some(error) = form_error %}
<p style="color:#b00020">{{ error }}</p>
{% endif %}

<form method="post" action="/projects">
    <fieldset role="group">
        <input type="text" name="project_id" placeholder="Project ID" value="{{ form_project_id }}" required>
        <input type="text" name="task_id" placeholder="First task ID" value="{{ form_task_id }}" required>
        <input type="text" name="criteria" placeholder="Acceptance criteria (optional)" value="{{ form_criteria }}">
        <button type="submit">Create project</button>
    </fieldset>
</form>

{% if projects.is_empty() %}
<p>No projects yet.</p>
{% else %}
<table>
    <thead>
        <tr>
            <th><a href="{{ sort_name_href }}">Project{% if active_sort == "name" %} {% if active_dir == "asc" %}▲{% else %}▼{% endif %}{% endif %}</a></th>
            <th><a href="{{ sort_total_href }}">Total{% if active_sort == "total" %} {% if active_dir == "asc" %}▲{% else %}▼{% endif %}{% endif %}</a></th>
            <th><a href="{{ sort_incomplete_href }}">Incomplete{% if active_sort == "incomplete" %} {% if active_dir == "asc" %}▲{% else %}▼{% endif %}{% endif %}</a></th>
            <th><a href="{{ sort_wip_href }}">WIP{% if active_sort == "wip" %} {% if active_dir == "asc" %}▲{% else %}▼{% endif %}{% endif %}</a></th>
            <th><a href="{{ sort_activity_href }}">Activity{% if active_sort == "activity" %} {% if active_dir == "asc" %}▲{% else %}▼{% endif %}{% endif %}</a></th>
        </tr>
    </thead>
    <tbody>
        {% for p in projects %}
        <tr>
            <td><a href="/projects/{{ p.id }}">{{ p.id }}</a></td>
            <td>{{ p.total }}</td>
            <td>{{ p.incomplete }}</td>
            <td>{{ p.wip }}</td>
            <td>{{ p.last_activity.as_deref().unwrap_or("-") }}</td>
        </tr>
        {% endfor %}
    </tbody>
</table>
{% endif %}
{% endblock %}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd web && cargo test --lib handlers::dashboard::`
Expected: all pass, including the two added in Step 1.

- [ ] **Step 5: Run the full test suite to check for regressions**

Run: `cd web && cargo test --lib`
Expected: all pass (no other file references the old `DashboardTemplate` shape).

- [ ] **Step 6: Commit**

```bash
git add web/src/handlers/dashboard.rs web/templates/dashboard.html
git commit -m "feat(web): sortable dashboard columns via ?sort=&dir="
```

---

### Task 4: Add `POST /projects` create-project route

**Files:**
- Modify: `web/src/handlers/dashboard.rs`
- Modify: `web/src/lib.rs`

**Interfaces:**
- Consumes: `build_dashboard` (Task 3), `queries::fetch_all_tasks`, `crate::is_valid_id`, `crate::iso_now`, `crate::tasks::create_task` (all pre-existing except `build_dashboard`).
- Produces: `pub async fn create(State(state): State<AppState>, Form(form): Form<CreateProjectForm>) -> Response` in `web/src/handlers/dashboard.rs`, registered as `POST /projects` in `web/src/lib.rs`.

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `web/src/handlers/dashboard.rs` (extend the existing `use` list in that module with `use axum::http::HeaderValue;` if needed for header assertions, or just read the header via `.to_str()` as shown):

```rust
    #[tokio::test]
    async fn create_project_redirects_and_creates_first_task() {
        let store = Arc::new(FakeStore::new());
        let state = AppState { store: store.clone() as Arc<dyn crate::zenoh_store::ZenohStore> };

        let response = app(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("project_id=newproj&task_id=t1&criteria=Given+X"))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(response.headers().get("location").unwrap().to_str().unwrap(), "/projects/newproj");

        let put_calls = store.put_calls.lock().unwrap();
        assert!(put_calls.iter().any(|(k, v)| k == "projects/newproj/tasks/t1/status" && v == "PENDING"));
    }

    #[tokio::test]
    async fn create_project_rejects_invalid_project_id() {
        let store = FakeStore::new();
        let state = AppState { store: Arc::new(store) };

        let response = app(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("project_id=bad*id&task_id=t1&criteria="))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn create_project_rejects_project_that_already_has_tasks() {
        let store = FakeStore::new().seed("projects/existing/tasks/t1/status", "PENDING");
        let state = AppState { store: Arc::new(store) };

        let response = app(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("project_id=existing&task_id=t2&criteria="))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::CONFLICT);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd web && cargo test --lib handlers::dashboard::`
Expected: compile error — `POST /projects` isn't a registered route yet, `create` doesn't exist.

- [ ] **Step 3: Implement**

In `web/src/handlers/dashboard.rs`, add these imports to the top (merge with the existing `use` block):

```rust
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
```

Add below `show`:

```rust
#[derive(Deserialize)]
pub struct CreateProjectForm {
    project_id: String,
    task_id: String,
    #[serde(default)]
    criteria: String,
}

pub async fn create(State(state): State<AppState>, Form(form): Form<CreateProjectForm>) -> Response {
    if !crate::is_valid_id(&form.project_id) || !crate::is_valid_id(&form.task_id) {
        let template = build_dashboard(
            &state,
            queries::SortKey::Name,
            queries::SortDir::Asc,
            Some("Invalid project or task ID.".to_string()),
            form.project_id,
            form.task_id,
            form.criteria,
        )
        .await;
        return (StatusCode::BAD_REQUEST, HtmlTemplate(template)).into_response();
    }

    let existing = queries::fetch_all_tasks(state.store.as_ref(), &form.project_id).await;
    if !existing.is_empty() {
        let template = build_dashboard(
            &state,
            queries::SortKey::Name,
            queries::SortDir::Asc,
            Some(format!("Project '{}' already exists.", form.project_id)),
            form.project_id,
            form.task_id,
            form.criteria,
        )
        .await;
        return (StatusCode::CONFLICT, HtmlTemplate(template)).into_response();
    }

    let now = crate::iso_now();
    crate::tasks::create_task(state.store.as_ref(), &form.project_id, &form.task_id, &form.criteria, &now).await;
    Redirect::to(&format!("/projects/{}", form.project_id)).into_response()
}
```

In `web/src/lib.rs`, add the route (inside `app()`'s `Router::new()` chain, next to the existing `/projects/{id}` routes):

```rust
        .route("/projects", post(handlers::dashboard::create))
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd web && cargo test --lib handlers::dashboard::`
Expected: all pass, including the three added in Step 1.

- [ ] **Step 5: Run the full test suite to check for regressions**

Run: `cd web && cargo test --lib`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add web/src/handlers/dashboard.rs web/src/lib.rs
git commit -m "feat(web): add POST /projects to create a project + its first task"
```

---

### Task 5: Manual verification against a real router

**Files:** none (verification only)

- [ ] **Step 1: Build and run the stack**

Run: `./scripts/up.sh` (set `ZTASK_CONTAINER_RUNTIME=container` if using Apple's `container` CLI instead of Docker)

- [ ] **Step 2: Exercise the create-project flow**

Open `http://localhost:8080/`. Use the new "Create project" form to create a brand-new project with a first task and optional criteria. Confirm the browser lands on `/projects/<new-id>` and the task appears.

- [ ] **Step 3: Exercise validation errors**

Submit the create-project form again with the same project ID (now has a task) — confirm a 409-style inline error renders on the dashboard with the form values preserved. Submit with an invalid ID (e.g. containing `*`) — confirm a 400-style inline error.

- [ ] **Step 4: Exercise sorting**

With at least two projects of differing sizes, click each column header (Project, Total, Incomplete, WIP, Activity) and confirm the row order changes and the arrow indicator moves to the clicked column; click the same header twice and confirm direction flips.

- [ ] **Step 5: Clean up**

Run: `container rm -f zenoh-router ztask-web` (or `docker rm -f zenoh-router ztask-web`)

No commit for this task — it's manual verification only.
