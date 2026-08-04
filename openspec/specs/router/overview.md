# Router (zenohd + Garry)

Zenoh router with Garry embedded KV storage backend.

## Overview

A multi-stage Docker container that builds and runs `zenohd` with the `zenoh-backend-garry` storage plugin. Persists task data to a mounted volume.

## Container Structure

```
docker/router/
  Dockerfile         # multi-stage build
  config.json5       # zenohd configuration
```

## Dockerfile

### Build Stage

- Base: Rust toolchain + cmake
- Builds Garry from source (`cmake -S . -B build && cmake --build build && cmake --install build`)
- Sets `PKG_CONFIG_PATH`
- `cargo build --release` for `zenohd` and `zenoh-backend-garry`

### Runtime Stage

- Slim base image
- Copies: `zenohd` binary, `libzenoh_backend_garry` plugin, `config.json5`
- Entrypoint: `zenohd -c /config.json5`

## Configuration

`config.json5`:

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

### Storage Configuration

| Setting | Value | Description |
|---------|-------|-------------|
| `key_expr` | `projects/**` | Captures all task data under the `projects/` keyspace |
| `db_path` | `/data/zenoh-garry` | Path inside the container for persistent storage |
| `pool_size` | `256` | Connection pool size |
| `max_record_size` | `1048576` | Max 1MB per record |
| `max_versions` | `64` | Keep up to 64 versions per key |
| `compression` | `lz4` | LZ4 compression for stored data |

## Volume

`/data` is a mounted volume for persistence across container restarts. On Docker, this is a named volume. On Apple's `container` CLI, it's a bind mount.

## Networking

The router listens on port 7447 (Zenoh protocol). Both the Python CLI and Rust web UI connect to this port.

- **Local dev:** `tcp/localhost:7447`
- **Docker network:** `tcp/zenoh-router:7447` (container name resolution)

## Startup

`scripts/up.sh` brings up the full stack:

1. Creates a Docker bridge network (`ztask-net`)
2. Builds and runs the `zenohd` + Garry router container on port 7447
3. Resolves the router's container IP (needed for Apple's `container` CLI which lacks sibling DNS)
4. Builds and runs the `ztask-web` Rust web UI on port 8080

Environment variable: `ZTASK_CONTAINER_RUNTIME` (defaults to `docker`, can be set to `container` for Apple's container CLI).

## Key Schema

All task data lives under `projects/<project_id>/tasks/<task_id>/...`. See `../task-model.md` for the full key schema.

## Garry Backend

[Garry](https://github.com/Argylelabcoat/Garry) is an embedded KV store. The `zenoh-backend-garry` plugin enables zenohd to persist key-value data to a Garry database. Data is stored as hierarchical keys with LZ4 compression.

Key properties:
- Embedded (no separate database process)
- Hierarchical key support (matches Zenoh's key expressions)
- Versioned (keeps up to `max_versions` per key)
- Compressed (LZ4)

## Container Runtime

The project supports two container runtimes:
- **Docker** — standard, widely available
- **Apple `container`** — macOS-native, used via `ZTASK_CONTAINER_RUNTIME=container`

`scripts/up.sh` handles both, with IP resolution workarounds for Apple's container CLI which lacks sibling DNS resolution.
