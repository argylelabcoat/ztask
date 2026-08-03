# Zenoh Task Tracker

A task tracker for LLM agents and developers, backed by a [Zenoh](https://zenoh.io/)
keyspace with durable storage. Tasks live as hierarchical key-values under
`projects/<project_id>/tasks/<task_id>/...`, persisted by a `zenohd` router
running the [Garry](https://github.com/Argylelabcoat/Garry) embedded KV
storage backend.

Three pieces:

- **`ztask`** — a Python CLI for LLM agents and developers to create, query,
  and update tasks. Defaults task creation to `entered_by: LLM`.
- **`ztask-web`** — a Rust (axum + htmx) admin web UI for humans: an
  all-projects dashboard and a per-project dashboard with inline
  create/update-status/edit-criteria/delete. Defaults task creation to
  `entered_by: USER`.
- **router** — a `zenohd` + Garry container that both talk to.

## Quick start

Requires a container runtime (`docker`, or `container` on macOS —
`scripts/up.sh` uses `docker` by default; set `ZTASK_CONTAINER_RUNTIME` to
override).

```bash
./scripts/up.sh
```

Brings up the router (`localhost:7447`) and the web UI
(`http://localhost:8080`) on a shared `ztask-net` network.

For the CLI, install and point it at the router:

```bash
poetry install
export ZTASK_ZENOH_ENDPOINT=tcp/localhost:7447
poetry run ztask create --project demo task-1 --criteria "Given X, When Y, Then Z"
poetry run ztask list --project demo
```

Agent containers joined to `ztask-net` use `tcp/zenoh-router:7447` instead.

## `ztask` CLI

```
ztask list --project <id> [--filter all|incomplete|wip]
ztask get <task-id> --project <id>
ztask create <task-id> --project <id> [--criteria "..."] [--entered-by llm|user]
ztask update-status <task-id> <status> --project <id> [--note "..."]
```

Every status change and task creation appends an entry to the task's
history log. See `docs/superpowers/specs/2026-07-31-zenoh-task-tracker-design.md`
for the full design.

## Web UI

Source at `web/` (crate `ztask-web`). Talks to the router directly over the
official zenoh Rust SDK — no CLI shell-out. See
`docs/superpowers/specs/2026-08-01-web-ui-design.md` for the design and
`docs/superpowers/plans/2026-08-01-web-ui-implementation.md` for the
implementation plan.

Run it locally against an already-running router:

```bash
cd web
ZTASK_ZENOH_ENDPOINT=tcp/localhost:7447 cargo run
```

## Development

```bash
# Python (CLI)
poetry install
poetry run pytest                    # unit tests
poetry run pytest -m integration     # + real router container (slow)

# Rust (web UI)
cd web
cargo test                                                # unit tests
cargo test --test web_integration -- --ignored --test-threads=1  # + real router container (slow)
```

## Layout

```
ztask/            Python CLI package
web/               Rust web UI crate (axum + askama + htmx)
docker/router/     zenohd + Garry router image
docker/web/        web UI image
scripts/up.sh      brings up router + web UI on a shared network
tests/             CLI unit + integration tests
docs/superpowers/  design specs and implementation plans
```
