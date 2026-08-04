# Design: Create Project + Sortable Dashboard

Date: 2026-08-03
Status: Approved

## Problem

The all-projects dashboard (`GET /`) only lists projects that already have at
least one task, since `ProjectSummary` rows are derived entirely from
`projects/*/tasks/*/status` keys — there is no explicit project entity in the
keyspace. There is also no way to create a project from the web UI (only
`ztask create` from the CLI, which always creates a task, or the per-project
page's inline task-create form, which requires the project to already exist
and be known). The dashboard table has no sorting — it is always ordered by
project ID ascending.

This spec adds:
1. A "create project" flow reachable from the dashboard.
2. Sortable dashboard columns (name, total, incomplete, wip, activity).

## Non-goals

- No standalone project entity/marker key. A project's existence remains
  fully derived from its tasks, exactly as today.
- No persistence of sort preference beyond the URL (no cookies/local storage).
- No pagination — out of scope, dashboards are assumed small enough for a
  single in-memory sort (consistent with the existing full-scan approach).

## Data model

Unchanged. "Creating a project" is creating the first task in a project
namespace that has no existing tasks — it reuses `tasks::create_task` exactly
as the existing per-project create-task flow does. No new key prefixes.

`queries::ProjectSummary` gains one field:

```rust
pub struct ProjectSummary {
    pub id: String,
    pub total: usize,
    pub incomplete: usize,
    pub wip: usize,
    pub last_activity: Option<String>, // RFC3339, max history timestamp across the project's tasks
}
```

`last_activity` is `None` only when a project has tasks with no history
entries at all, which should not happen in practice since `create_task`
always writes one — but the field stays `Option` to match the defensive style
already used for `Task`'s optional fields (e.g. `time_entered`), and to avoid
a panic if a project's data is ever inconsistent (e.g. manually edited via
the CLI/zenoh directly).

## Backend changes

### `queries::fetch_all_projects`

Add a second wildcard query, `projects/*/tasks/*/history/*`, run alongside
the existing `projects/*/tasks/*/status` query. For each returned key, parse
out the project ID (same `key.split('/')` approach already used) and fold in
the timestamp from the history entry's JSON value, keeping the max per
project. RFC3339 timestamps sort correctly as plain strings, so no parsing
into a `DateTime` is needed.

Add a `sort: SortKey` and `dir: SortDir` parameter (or a single combined
enum) so the function can sort before returning:

```rust
pub enum SortKey { Name, Total, Incomplete, Wip, Activity }
pub enum SortDir { Asc, Desc }
```

Unknown/missing values are the caller's problem to normalize (see handler
below) — `fetch_all_projects` always receives valid, defaulted values.

### `GET /` (dashboard)

New query params, both optional with defaults:

```
GET /?sort=<name|total|incomplete|wip|activity>&dir=<asc|desc>
```

Default: `sort=name`, `dir=asc` (today's implicit behavior — no visible
change if no query params are given). Unrecognized `sort`/`dir` values fall
back to the default rather than erroring — this is a display concern, not a
correctness one.

The template renders each column header as a link that sets `sort` to that
column and either keeps the current `dir` (if switching columns) or flips it
(if the column is already the active sort) — computed server-side in the
handler/template context, no client-side JS.

### `POST /projects` (new route)

```rust
#[derive(Deserialize)]
pub struct CreateProjectForm {
    project_id: String,
    task_id: String,
    #[serde(default)]
    criteria: String,
}
```

Handler behavior:
1. Validate `project_id` and `task_id` with the existing `crate::is_valid_id`
   — `400 Bad Request` if either fails.
2. Check whether the project already has any tasks via
   `queries::fetch_all_tasks(store, &project_id)` — if non-empty, return
   `409 Conflict` (this route is for *new* projects only; the per-project
   page's existing `POST /projects/{id}/tasks` remains the way to add tasks
   to an existing project).
3. Otherwise call `tasks::create_task` exactly as
   `handlers::project::create` does today.
4. On success, respond `303 See Other` with `Location: /projects/{project_id}`.

This is a plain HTML form submit (not htmx) because the target URL depends
on user input (the new project ID), and htmx's `hx-post` requires a static
target — a normal form POST + server-side redirect is the simplest correct
way to land the user on their newly created project's page. The form lives
inline at the top of the dashboard template, mirroring the existing
inline create-task form on the project page.

On `400`/`409`, the dashboard re-renders with the form's values preserved
and an inline error message above the form (same pattern as would be used
for any other in-page validation error in this codebase).

## Error handling summary

| Condition | Response |
|---|---|
| Invalid `project_id` or `task_id` (per `is_valid_id`) | `400`, form re-rendered with inline error |
| Project already has tasks | `409`, form re-rendered with inline error |
| Unknown `sort`/`dir` query value | Silently falls back to default (`name`/`asc`) |

## Testing

- `queries.rs`: unit tests for `last_activity` computation (max across
  multiple tasks/history entries, `None` when a project has no history) and
  for each `SortKey`/`SortDir` combination producing the expected row order.
- `handlers/dashboard.rs`: integration-style tests (via `FakeStore` + the
  full router, following the existing pattern in this file and in
  `handlers/project.rs`) for:
  - `POST /projects` success → `303` with the right `Location` header and
    the task's fields persisted (mirrors
    `create_task_adds_row_and_persists_fields` in `project.rs`).
  - `POST /projects` with a project ID that already has tasks → `409`, no
    additional writes.
  - `POST /projects` with an invalid ID → `400`.
  - `GET /?sort=total&dir=desc` (and similar) → rendered row order matches
    expected sort.
