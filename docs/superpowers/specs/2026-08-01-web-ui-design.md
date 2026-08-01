# Zenoh Task Tracker — Web UI Design

## Purpose

A human-facing web UI for administering tasks across projects, complementing
the existing `ztask` CLI (which stays LLM/developer-facing). Provides two
dashboards — all-projects and per-project — plus the ability to create,
update, edit, and delete tasks through a browser instead of a shell.

This was explicitly out of scope in the original CLI design
(`2026-07-31-zenoh-task-tracker-design.md`); this spec picks it up.

## Audience & Trust Model

Single trusted operator, trusted network — no auth in v1 (matches the CLI's
existing "no access control" trust model). Auth can be layered on later if
the audience grows.

## Architecture

A new Rust binary crate at `zenoh-tasks/web/` (package `ztask-web`):

- **axum** for HTTP routing/handlers
- **askama** for templates — compile-time checked, embedded into the binary
  (no template files to ship or mount separately)
- **zenoh** (official Rust crate) for storage access — the same bindings
  `zenohd` and `zenoh-backend-garry` already use, talking to the router
  directly rather than shelling out to the Python CLI
- **htmx** on the frontend for interactivity (partial-page swaps on
  create/update/delete), server returns HTML fragments for these actions
- A single vendored classless CSS file (Pico.css) embedded via
  `include_str!` and served at `/static/style.css` — no JS build step

Connects to the router via the `ZTASK_ZENOH_ENDPOINT` env var — the exact
same convention the Python CLI (`ztask/zenoh_client.py`) already uses —
defaulting to `tcp/zenoh-router:7447` when containerized, `tcp/localhost:7447`
for local dev. The zenoh `Session` is opened once at startup and held in
shared axum state; failure to open it is a fail-fast startup error (matches
the CLI's behavior, no retry loop).

### Deployment

A new `docker/web/Dockerfile` — a multi-stage Rust build (builder + slim
runtime), simpler than the router's since there's no C library to compile.
Joins the same `ztask-net` bridge network `scripts/up.sh` already creates for
the router, so it reaches it the same way any agent container would.
`scripts/up.sh` (or a sibling script) is extended to build/run it alongside
the router.

```
┌─────────────────────┐        ┌──────────────────────────┐
│ ztask-web container │  tcp   │ zenoh-router container    │
│  axum + askama      │───────▶│  zenohd + garry backend   │
│  :8080 (browser)     │        │  storage: projects/**     │
└─────────────────────┘        └──────────────────────────┘
```

## Data Model & Key Schema

No schema changes. Reuses the exact keys the CLI already writes:

```
projects/<project_id>/tasks/<task_id>/status
projects/<project_id>/tasks/<task_id>/time_entered
projects/<project_id>/tasks/<task_id>/time_accepted
projects/<project_id>/tasks/<task_id>/time_completed
projects/<project_id>/tasks/<task_id>/acceptance_criteria
projects/<project_id>/tasks/<task_id>/entered_by
projects/<project_id>/tasks/<task_id>/history/<iso-timestamp>
```

Two new write capabilities beyond the CLI:

- **Edit acceptance criteria**: `put(.../acceptance_criteria, new_value)`,
  plus a `history/<ts>` entry (`from_status`/`to_status` both the task's
  current status, `note: "criteria updated"`) so edits show up in the audit
  trail the same way status transitions already do.
- **Delete task**: a single `delete("projects/<id>/tasks/<task_id>/**")`.
  Verified empirically against the real router/Garry backend during
  brainstorming (created a task, wildcard-deleted it, confirmed a subsequent
  `get` returns not-found) — zenoh's storage_manager replays the wildcard
  delete against every key it knows about for that prefix, so no per-key
  iteration is needed.

**Project discovery** (no project registry exists — projects are just
implicit key prefixes): query `projects/*/tasks/*/status`, one hit per task,
group by project ID, and bucket each project's tasks by status (total /
incomplete / WIP) using the same classification the CLI's `list --filter`
already uses (`TERMINAL_STATUS = "COMPLETED"`,
`WIP_STATUSES = {"IN_PROGRESS", "WIP", "RUNNING"}`), mirrored as Rust
constants.

## Routes & Pages

| Method | Path | Purpose |
|---|---|---|
| GET | `/` | All-projects dashboard: project table with task counts, links into each project |
| GET | `/projects/:id` | Per-project dashboard: filterable task table (all/incomplete/wip), create form, inline per-row actions |
| GET | `/projects/:id/tasks/:task_id` | Task detail: full criteria, all timestamps, `entered_by`, complete history log |
| POST | `/projects/:id/tasks` | Create task (htmx form submit → updated list/row fragment) |
| POST | `/projects/:id/tasks/:task_id/status` | Update status (htmx → updated row fragment) |
| POST | `/projects/:id/tasks/:task_id/criteria` | Edit acceptance criteria (htmx → updated fragment) |
| DELETE | `/projects/:id/tasks/:task_id` | Delete task (htmx `hx-delete` → row removed from DOM) |
| GET | `/static/style.css` | Vendored Pico.css |

Status update, edit-criteria, and delete are all available **inline on the
per-project task list**, not gated behind the detail page — the detail page
is for reading full history, not the only place to act on a task.

## Components

- `web/src/main.rs` — axum app wiring, tracing init, shared zenoh `Session`
  state, fail-fast startup.
- `web/src/zenoh_client.rs` — mirrors `ztask/zenoh_client.py`:
  `resolve_endpoint()` / `open_session()`, same env var.
- `web/src/queries.rs` — mirrors `ztask/queries.py`: `fetch_all_projects`,
  `fetch_all_tasks(project_id)`, `fetch_task(project_id, task_id)`,
  reimplemented against the zenoh Rust API.
- `web/src/models.rs` — `Task` struct mirroring the Python dataclass, serde
  for history JSON entries.
- `web/src/handlers/` — `dashboard.rs`, `project.rs`, `task.rs`, one module
  per resource.
- `web/templates/` — Askama templates: `base.html` (layout + nav),
  `dashboard.html`, `project.html`, `task_row.html` (htmx fragment reused
  across create/update/delete responses), `task_detail.html`.

The zenoh get/put/delete calls used by `queries.rs`/handlers are abstracted
behind a small trait so they can be swapped for an in-memory fake in unit
tests (see Testing).

## Error Handling

- **Startup**: zenoh session open failure → log and exit non-zero. No
  retries.
- **Missing task/project**: 404 response with a small "not found" template.
- **Put/delete/get failures mid-request**: 500 response with an inline error
  banner in the htmx response fragment (so the swap target shows the error
  in place), logged server-side. No retries in v1.

## Testing

- **Unit**: the zenoh-call trait is faked in-memory, mirroring
  `tests/unit/fakes.py`'s `FakeSession`/`FakeReply` pattern — covers
  `queries.rs` and handler logic (status classification, key construction,
  the wildcard-delete call, the criteria-edit history entry) without a
  network or container.
- **Integration**: reuses the router-container fixture pattern from
  `tests/integration/` — builds/runs `docker/router/Dockerfile`, points
  `ztask-web` at it, drives real HTTP requests (axum's test client or
  `reqwest`), asserts on rendered HTML and round-tripped zenoh state.
  Opt-in, not part of the default test run — consistent with how the Python
  integration tests are gated behind `pytest -m integration`.

## Out of Scope (v1)

- Auth (single trusted operator for now; can be layered on later)
- Live/real-time updates (dashboards refresh on page load/navigation only;
  no zenoh subscribe + SSE push)
- Editing or deleting individual history entries
- Bulk actions (multi-select delete/status-change)
- Explicit project create/delete as first-class operations — a project
  still just comes into existence via its first task, matching the current
  implicit-prefix model
