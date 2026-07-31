# Zenoh Task Tracker — Design

## Purpose

A CLI tool (`ztask`) that lets LLM agents and developers create, query, and
update tasks stored in a shared Zenoh keyspace. Tasks live as hierarchical
Zenoh key-values, persisted durably by a `zenohd` router running the
[zenoh-backend-garry](https://github.com/Argylelabcoat/zenoh-backend-garry)
storage backend (a Garry embedded KV store plugin).

Agents run in their own Docker containers and `pip install` this package to
manage their own task state; they talk to a shared router container over
Docker networking. A companion human-facing UI and Claude skills are planned
separately and are out of scope here.

## Architecture

Two independently built artifacts in this repo:

1. **Router container** (`docker/router/`) — a multi-stage Dockerfile that
   builds Garry (cmake), `zenohd`, and `zenoh-backend-garry` from source
   (no prebuilt image exists for this backend), then ships a slim runtime
   image running `zenohd -c config.json5` with a `garry` volume/storage
   declared on `projects/**`. Data persists to a mounted volume.
2. **`ztask` Python package** (`ztask/`) — a Poetry-managed, pip-installable
   CLI (typer) that opens a Zenoh session against the router and issues
   `list`/`get`/`create`/`update-status` commands. Not containerized itself
   in this repo — it's meant to be installed inside whatever container an
   LLM agent runs in, and connects to the router over the network.

Local dev: `scripts/up.sh` builds and runs the router container on a named
Docker bridge network (`ztask-net`); the CLI is run locally (or in any
container joined to that network) against it.

```
┌─────────────────────┐        ┌──────────────────────────┐
│ agent container(s)  │  tcp   │ zenoh-router container    │
│  pip install ztask  │───────▶│  zenohd + garry backend   │
│  ztask create ...   │        │  storage: projects/**     │
└─────────────────────┘        │  db_path: /data (volume)  │
                                └──────────────────────────┘
```

## Key Schema

Unchanged from the existing prototype script:

```
projects/<project_id>/tasks/<task_id>/status
projects/<project_id>/tasks/<task_id>/time_entered
projects/<project_id>/tasks/<task_id>/time_accepted
projects/<project_id>/tasks/<task_id>/time_completed
projects/<project_id>/tasks/<task_id>/acceptance_criteria
projects/<project_id>/tasks/<task_id>/history/<iso-timestamp>
```

`history/<iso-timestamp>` values are JSON: `{timestamp, from_status,
to_status, note}`.

## Components

- **`ztask/zenoh_client.py`** — session handling. Endpoint comes from
  `ZTASK_ZENOH_ENDPOINT` env var, defaulting to `tcp/localhost:7447` (local
  dev) — agent containers override it to `tcp/zenoh-router:7447` (Docker
  network) via env. No reliance on multicast scouting, since containers
  don't reliably see each other via multicast.
- **`ztask/models.py`** — a `Task` dataclass replacing the loose dict
  construction in the prototype, used by both fetch and display paths.
- **`ztask/cli.py`** — the typer app; same four commands as the prototype
  (`list`, `get`, `create`, `update-status`), with these fixes over the
  prototype:
  - `get_task` currently fetches *all* tasks under the project and filters
    client-side to find one. Changed to query
    `projects/<project_id>/tasks/<task_id>/**` directly.
  - `update_status`'s old-status lookup (`list(session.get(...))[0]`)
    assumes a non-empty, ok reply; add explicit handling for a missing key
    (treat as `UNKNOWN` without indexing into an empty list).
  - Status comparisons normalized to uppercase consistently (already mostly
    done in the prototype; applied uniformly).
- **`pyproject.toml`** — Poetry-managed, with `typer` and `eclipse-zenoh`
  (the official Python Zenoh bindings on PyPI) as dependencies, `ztask`
  console-script entry point.

## Router Container

`docker/router/Dockerfile`:
- Build stage: Rust toolchain + cmake, builds Garry from source
  (`cmake -S . -B build && cmake --build build && cmake --install build`),
  sets `PKG_CONFIG_PATH`, then `cargo build --release` for `zenohd` and
  `zenoh-backend-garry`.
- Runtime stage: slim base image, copies the `zenohd` binary, the compiled
  `libzenoh_backend_garry` plugin, and `config.json5`. Entrypoint:
  `zenohd -c /config.json5`.

`docker/router/config.json5`:
```json5
{
  plugins: {
    storage_manager: {
      volumes: [{
        name: "garry",
        backend: "garry",
        storages: [{
          name: "projects",
          key_expr: "projects/**",
          volume: {
            db_path: "/data/zenoh-garry",
            pool_size: 256,
            max_record_size: 1048576,
            max_versions: 64,
            compression: "lz4"
          }
        }]
      }]
    }
  }
}
```

`/data` is a mounted volume for persistence across container restarts.

## Testing

- **Unit** (`tests/unit/`) — mock `zenoh.Session`/replies via
  `unittest.mock`; cover key construction, status-filtering logic
  (`list`), the `get_task` not-found path, and the status-transition
  timestamp logic in `update-status`. No network or container required;
  these run in normal `poetry run pytest`.
- **Integration** (`tests/integration/`, marked
  `@pytest.mark.integration`) — a session-scoped fixture that builds and
  runs the real router container (via `container`/`docker`), waits for it
  to accept connections, and runs CLI commands against it end-to-end,
  asserting on actual garry-persisted state. Skipped by default unless a
  container runtime is available; run explicitly via
  `poetry run pytest -m integration`.

## Out of Scope

- Human-facing UI.
- Companion Claude skills for agents.
- Multi-router / clustering / auth — single router, single garry volume,
  no access control, matching the current single-project prototype's
  trust model.
