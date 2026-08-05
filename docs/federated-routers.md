# Federated ztask Routers — Multi-Tenant LLM Isolation

How to run multiple ztask routers with project-scoped data replication so that LLM agents in Docker Compose setups can only access their assigned projects.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│  Primary Router (zenohd + Garry)                                    │
│  Storage: projects/**  (key_expr)                                   │
│  Listen:  tcp/0.0.0.0:7447                                          │
│                                                                     │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐               │
│  │ project-a    │  │ project-b    │  │ project-c    │               │
│  │ tasks/...    │  │ tasks/...    │  │ tasks/...    │               │
│  └──────────────┘  └──────────────┘  └──────────────┘               │
└─────────────────────────────────────────────────────────────────────┘
        │                    │                    │
        │ (agent router      │ (agent router      │ (agent router
        │  connects to       │  connects to       │  connects to
        │  primary; queries  │  primary; queries  │  primary; queries
        │  routed to Garry)  │  routed to Garry)  │  routed to Garry)
        ▼                    ▼                    ▼
┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│ Router A     │    │ Router B     │    │ Router C     │
│ Listen: 7448 │    │ Listen: 7449 │    │ Listen: 7450 │
│ scouting off │    │ scouting off │    │ scouting off │
│              │    │              │    │              │
│ ┌──────────┐ │    │ ┌──────────┐ │    │ ┌──────────┐ │
│ │LLM Agent │ │    │ │LLM Agent │ │    │ │LLM Agent │ │
│ │ (docker) │ │    │ │ (docker) │ │    │ │ (docker) │ │
│ └──────────┘ │    │ └──────────┘ │    │ └──────────┘ │
└──────────────┘    └──────────────┘    └──────────────┘
```

Each LLM agent's router connects only to the primary (which owns the
Garry storage). The agent cannot reach sibling agent routers
(scouting/gossip disabled, separate Docker networks where shown).

## Zenoh Scouting and Routing

Zenoh routers/peers discover each other via scouting (multicast UDP by
default on `224.0.0.224:7446`) and via gossip once peers are connected.
When multiple routers run on the same network with multicast scouting
enabled, they will auto-connect according to their `autoconnect`
settings. To isolate agents by forcing them to connect only to the
primary router:

1. **Primary router** — stores all projects (`key_expr: "projects/**"`),
   listens on `tcp/0.0.0.0:7447`. May keep multicast scouting enabled on
   a controlled network, or be configured with explicit `connect`/`listen`
   endpoints only.
2. **Agent routers** — connect to the primary via `connect.endpoints`,
   listen on a per-agent port for their LLM agent, and **disable both
   multicast scouting and gossip** so they do not auto-discover sibling
   agent routers.
3. **LLM agents** — connect only to their assigned agent router's
   `ZTASK_ZENOH_ENDPOINT`.

See the [Zenoh configuration
manual](https://zenoh.io/docs/manual/configuration/) for the full
`scouting.multicast` and `scouting.gossip` options.

## Docker Compose Setup

### Directory Structure

```
docker/
  router/
    Dockerfile
    config.json5
  router-agent/
    Dockerfile
    config-agent.json5.template
  docker-compose.yml
  docker-compose.agent.yml
```

### Primary Router Config

`docker/router/config.json5` (mirrors the format used in this repo — see
the actual file and [Zenoh's storage manager plugin
docs](https://zenoh.io/docs/manual/plugin-storage-manager/)):

```json5
{
  plugins_loading: {
    enabled: true
  },
  plugins: {
    storage_manager: {
      volumes: {
        garry: {
          backend: "garry"
        }
      },
      storages: {
        projects: {
          key_expr: "projects/**",
          volume: {
            id: "garry",
            db_path: "/data",
            pool_size: 256,
            max_record_size: 1048576,
            max_versions: 64,
            compression: "lz4"
          }
        }
      }
    }
  }
}
```

### Agent Router Config Template

`docker/router-agent/config-agent.json5.template`:

```json5
{
  // Connect to primary router (no auto-discovery of siblings)
  connect: {
    endpoints: ["tcp/primary-router:7447"]
  },
  // Listen on a different port for the agent
  listen: {
    endpoints: ["tcp/0.0.0.0:AGENT_PORT"]
  },
  scouting: {
    multicast: {
      enabled: false  // disable multicast auto-discovery
    },
    gossip: {
      enabled: false  // disable gossip-based auto-discovery
    }
  }
}
```

### Docker Compose

`docker/docker-compose.yml`:

```yaml
version: "3.8"

services:
  # Primary router — stores all projects
  primary-router:
    build:
      context: ../docker/router
    ports:
      - "7447:7447"
    volumes:
      - primary-data:/data
    networks:
      - ztask-net

  # Agent router for project-a
  router-project-a:
    build:
      context: ../docker/router-agent
    ports:
      - "7448:7448"
    environment:
      - AGENT_PORT=7448
      - PRIMARY_ENDPOINT=tcp/primary-router:7447
      - PROJECT_ID=project-a
    depends_on:
      - primary-router
    networks:
      - ztask-net

  # Agent router for project-b
  router-project-b:
    build:
      context: ../docker/router-agent
    ports:
      - "7449:7449"
    environment:
      - AGENT_PORT=7449
      - PRIMARY_ENDPOINT=tcp/primary-router:7447
      - PROJECT_ID=project-b
    depends_on:
      - primary-router
    networks:
      - ztask-net

  # LLM agent for project-a
  llm-agent-project-a:
    build:
      context: ../docker/agent
    environment:
      - ZTASK_ZENOH_ENDPOINT=tcp/router-project-a:7448
    depends_on:
      - router-project-a
    networks:
      - ztask-net

  # LLM agent for project-b
  llm-agent-project-b:
    build:
      context: ../docker/agent
    environment:
      - ZTASK_ZENOH_ENDPOINT=tcp/router-project-b:7449
    depends_on:
      - router-project-b
    networks:
      - ztask-net

volumes:
  primary-data:

networks:
  ztask-net:
    driver: bridge
```

## Agent Router Dockerfile

`docker/router-agent/Dockerfile`:

```dockerfile
FROM debian:bookworm-slim

# Install zenohd (assumes pre-built binary or build from source)
COPY zenohd /usr/local/bin/
COPY entrypoint.sh /usr/local/bin/

RUN chmod +x /usr/local/bin/entrypoint.sh

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
```

`docker/router-agent/entrypoint.sh`:

```bash
#!/bin/bash
set -e

# Generate config from environment variables
cat > /tmp/config.json5 << EOF
{
  connect: {
    endpoints: ["${PRIMARY_ENDPOINT}"]
  },
  listen: {
    endpoints: ["tcp/0.0.0.0:${AGENT_PORT}"]
  },
  scouting: {
    multicast: {
      enabled: false
    },
    gossip: {
      enabled: false
    }
  }
}
EOF

exec zenohd -c /tmp/config.json5
```

## Key Expression Routing

Zenoh routes based on key expressions and the declarations made by
clients (interest-based routing): a publication is forwarded only to
routers/peers that have declared a matching subscriber or queryable.
To isolate agents, the practical levers are storage scoping, ACL, and
network isolation, summarised below.

### Option 1: Storage-Level Isolation

The primary router stores all projects. Agent routers don't configure
a storage; they only forward requests to the primary. The agent's
`ztask` queries (`projects/<project>/**`) are routed to the primary's
Garry storage, which returns only the matching keys.

**Limitation:** This is not a security boundary — Zenoh's
interest-based routing will forward matching publications to any
router that has declared a matching subscriber, so a misconfigured
agent router that subscribes to `projects/**` would still receive
other projects' updates. Use this only for *organisational*
isolation, not for *adversarial* isolation.

### Option 2: Access Control via Config

Zenoh 1.0+ supports an access-control list (ACL) plugin that filters
messages by key expression, message type, and flow. The config schema
requires `rules`, `subjects`, and `policies` sections — a single
flat rule list is not accepted. See the
[Access Control manual](https://zenoh.io/docs/manual/access-control/)
for the full schema.

```json5
{
  access_control: {
    enabled: true,
    default_permission: "deny",  // deny everything not explicitly allowed

    rules: [
      {
        id: "allow-project-a",
        permission: "allow",
        flows: ["ingress", "egress"],
        messages: [
          "put", "delete", "declare_subscriber",
          "query", "reply", "declare_queryable"
        ],
        key_exprs: ["projects/project-a/**"]
      },
      {
        id: "deny-project-b",
        permission: "deny",
        flows: ["ingress", "egress"],
        messages: [
          "put", "delete", "declare_subscriber",
          "query", "reply", "declare_queryable"
        ],
        key_exprs: ["projects/project-b/**"]
      }
    ],

    subjects: [
      // An empty subject is a wildcard that matches any connecting Zenoh instance.
      // For tighter isolation, constrain this by `interfaces`, `cert_common_names`,
      // `usernames`, `link_protocols`, or `zids` (see the ACL manual).
      { id: "any-agent" }
    ],

    policies: [
      {
        rules: ["allow-project-a", "deny-project-b"],
        subjects: ["any-agent"]
      }
    ]
  }
}
```

**Limitations:**
- ACL is not a substitute for network isolation — it filters Zenoh
  messages, but a malicious peer on the same Zenoh network can still
  attempt to connect and send messages that get filtered (not blocked
  at the transport level).
- ACL config cannot be updated at runtime; it requires a router restart.
- ACL decisions can have a measurable performance impact; prefer
  `default_permission: "deny"` with a small number of `allow` rules
  (or the opposite) to minimise the rules evaluated per message.
- A `deny` on `declare_subscriber` will also suppress the routing of
  matching publications to that agent, but the agent can still issue
  `query`/`put` against other key expressions unless those are also
  denied.

### Option 3: Separate Networks

Run each agent router on its own Docker network, connected only to the primary:

```yaml
networks:
  agent-a-net:
    driver: bridge
    internal: true  # No external access
  agent-b-net:
    driver: bridge
    internal: true
  primary-net:
    driver: bridge
```

The primary router connects to all networks. Each agent router connects only to `primary-net` and its own `agent-*-net`.

## Data Replication

To replicate data from primary to agent routers, use Zenoh's queryable storage or pub/sub:

### Approach 1: Queryable Storage

Agent routers don't store data locally. When the agent queries, the request routes to the primary:

```
Agent → Agent Router → Primary Router → Garry Storage
Agent ← Agent Router ← Primary Router ← Garry Storage
```

**Pros:** No data duplication, always consistent
**Cons:** Network dependency, higher latency

### Approach 2: Cached Replication

Agent routers cache recent queries locally (config snippet below uses
the same `volumes`/`storages` shape as the primary router config above):

```json5
{
  // Agent router config
  plugins_loading: {
    enabled: true
  },
  plugins: {
    storage_manager: {
      volumes: {
        garry: {
          backend: "garry"
        }
      },
      storages: {
        "project-a-cache": {
          key_expr: "projects/project-a/**",
          volume: {
            id: "garry",
            db_path: "/data/cache",
            pool_size: 64,
            max_record_size: 1048576,
            max_versions: 1,
            compression: "lz4"
          }
        }
      }
    }
  }
}
```

**Pros:** Lower latency, works offline
**Cons:** Data duplication, eventual consistency

### Approach 3: Pub/Sub Replication

Primary publishes changes, agent routers subscribe:

```python
# Primary router publishes changes
session.put("projects/project-a/tasks/t1/status", "COMPLETED")

# Agent router subscribes
def on_change(key, value):
    local_session.put(key, value)

session.subscribe("projects/project-a/**", on_change)
```

**Pros:** Real-time updates, selective replication
**Cons:** More complex, requires custom code

## Isolation Verification

To verify agent isolation:

### Test 1: Cross-Project Read

```bash
# From agent-a container
ztask get task-from-project-b --project project-b
# Expected: Error or empty (isolated)

ztask get task-from-project-a --project project-a
# Expected: Success (accessible)
```

### Test 2: Cross-Project Write

```bash
# From agent-a container
ztask create malicious-task --project project-b --criteria "test"
# Expected: Error or no effect (isolated)
```

### Test 3: Network Isolation

```bash
# From agent-a container, try to talk to router-project-b's Zenoh port.
# Zenoh's TCP listener does not serve a /healthz endpoint by default —
# this is a raw-TCP reachability probe, not an HTTP one.
nc -zv router-project-b 7449
# Expected: Connection refused or timeout (different Docker network)

# To query a router's status over HTTP, enable the REST plugin
# (--rest-http-port=8000) and GET @/local/router. The default zenohd
# build used in this repo does not enable the REST plugin.
```

## Multi-Project Orchestrator

For orchestrating across multiple isolated projects:

```
┌─────────────────────────────────────────────────────────────────────┐
│  Orchestrator (outside agent networks)                              │
│  Connects to: primary-router:7447                                   │
│                                                                     │
│  1. List all projects                                               │
│  2. For each project, spawn agent in isolated network               │
│  3. Monitor progress via primary router                             │
│  4. Collect results when agents complete                            │
└─────────────────────────────────────────────────────────────────────┘
```

The orchestrator connects to the primary router (not agent routers) to:
- Create projects and tasks
- Monitor progress across all projects
- Collect results when agents complete
- Never directly access agent routers

## Security Considerations

### What This Prevents

- Agent A reading Agent B's tasks
- Agent A writing to Agent B's project
- Agent A modifying the primary router's storage directly

### What This Does NOT Prevent

- Agent A accessing the primary router's network (if configured)
- Agent A reading its own project's full history
- Agent A consuming excessive resources

### Recommendations

1. **Use separate Docker networks** for each agent
2. **Disable multicast scouting** on agent routers
3. **Use internal networks** where possible
4. **Monitor agent resource usage** via Docker
5. **Use read-only volumes** for agent config files

## Example: Three Isolated Agents

```bash
# Start the stack
docker compose up -d primary-router

# Start agent for project-a
PROJECT_ID=project-a docker compose -f docker-compose.agent.yml up -d

# Start agent for project-b
PROJECT_ID=project-b docker compose -f docker-compose.agent.yml up -d

# Verify isolation
docker exec llm-agent-project-a ztask list --project project-b
# Expected: [] (empty or error)

docker exec llm-agent-project-a ztask list --project project-a
# Expected: [task1, task2, ...]
```

## Limitations

1. **ACL version sensitivity** — the ACL config schema shown above is
   the Zenoh 1.0+ format; older Zenoh 0.11 configs used a different
   flat-rule schema (see the [0.11 ACL
   RFC](https://github.com/eclipse-zenoh/roadmap/blob/ca841fe219890bf73289089b520271d70ded89b6/rfcs/ALL/Access%20Control%20Rules.md))
2. **Network overhead** — each agent router adds a hop and therefore latency
3. **Storage overhead** — cached replication duplicates data on each agent router
4. **Complexity** — more moving parts to manage (one router per project + primary)
5. **Debugging** — harder to trace issues across routers; use the admin space
   (`@/<router-id>/router`) and the REST plugin for observability
6. **Routing is not a security boundary** — interest-based routing will
   forward to any peer that declares the matching interest; for true
   isolation, combine network separation with ACL, not routing alone

## References

- [Zenoh Documentation](https://zenoh.io/docs/getting-started/first-app/) — overview, getting started
- [Zenoh Configuration](https://zenoh.io/docs/manual/configuration/) — `connect`/`listen`/`scouting` options used above
- [Zenoh Abstractions](https://zenoh.io/docs/manual/abstractions/) — key expressions, selectors, storages
- [Zenoh Storage Manager Plugin](https://zenoh.io/docs/manual/plugin-storage-manager/) — `volumes`/`storages` config schema
- [Zenoh Access Control](https://zenoh.io/docs/manual/access-control/) — ACL `rules`/`subjects`/`policies` schema
- [Zenoh DEFAULT_CONFIG.json5](https://github.com/eclipse-zenoh/zenoh/blob/main/DEFAULT_CONFIG.json5) — canonical reference config
- [Docker Networking](https://docs.docker.com/network/) — bridge networks, `internal: true`
- [Garry Storage Backend](https://github.com/Argylelabcoat/Garry) — the storage backend used in this repo
- `docker/router/config.json5` — this repo's primary router config (the source of truth for the snippet above)
