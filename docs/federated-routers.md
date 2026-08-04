# Federated ztask Routers — Multi-Tenant LLM Isolation

How to run multiple ztask routers with project-scoped data replication so that LLM agents in Docker Compose setups can only access their assigned projects.

## Architecture

```
┌─────────────────────────────────────────────────────────────────────┐
│  Primary Router (zenohd + Garry)                                    │
│  Storage: projects/**                                               │
│  Port: 7447                                                         │
│                                                                     │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐              │
│  │ project-a    │  │ project-b    │  │ project-c    │              │
│  │ tasks/...    │  │ tasks/...    │  │ tasks/...    │              │
│  └──────────────┘  └──────────────┘  └──────────────┘              │
└─────────────────────────────────────────────────────────────────────┘
        │                    │                    │
        │ (replicate         │ (replicate         │ (replicate
        │  project-a/**)     │  project-b/**)     │  project-c/**)
        ▼                    ▼                    ▼
┌──────────────┐    ┌──────────────┐    ┌──────────────┐
│ Router A     │    │ Router B     │    │ Router C     │
│ Port: 7448   │    │ Port: 7449   │    │ Port: 7450   │
│              │    │              │    │              │
│ ┌──────────┐ │    │ ┌──────────┐ │    │ ┌──────────┐ │
│ │LLM Agent│ │    │ │LLM Agent│ │    │ │LLM Agent│ │
│ │ (docker)│ │    │ │ (docker)│ │    │ │ (docker)│ │
│ └──────────┘ │    │ └──────────┘ │    │ └──────────┘ │
└──────────────┘    └──────────────┘    └──────────────┘
```

Each LLM agent's router only contains data for its assigned project. The agent cannot read or write to other projects.

## Zenoh Scouting and Routing

Zenoh routers discover each other via scouting (multicast UDP by default). When multiple routers run on the same network, they automatically form a routing mesh. To isolate agents:

1. **Primary router** — stores all projects, connects to all agent routers
2. **Agent routers** — connect to primary, replicate only assigned project keys
3. **LLM agents** — connect only to their assigned agent router

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

### Agent Router Config Template

`docker/router-agent/config-agent.json5.template`:

```json5
{
  // Connect to primary router
  connect: {
    endpoints: ["tcp/primary-router:7447"]
  },
  // Listen on a different port for the agent
  listen: {
    endpoints: ["tcp/0.0.0.0:AGENT_PORT"]
  },
  // Only route specific key expressions
  scouting: {
    multicast: {
      enabled: false  // Disable auto-discovery
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
    }
  }
}
EOF

exec zenohd -c /tmp/config.json5
```

## Key Expression Routing

Zenoh routes based on key expressions. To isolate agents:

### Option 1: Storage-Level Isolation

The primary router stores all projects. Agent routers don't store data locally — they route requests to the primary. The agent can only access keys that match the routing rules.

**Limitation:** Agent router still sees all traffic, just doesn't store it.

### Option 2: Access Control via Config

Configure agent routers to only accept specific key expressions:

```json5
{
  // ... other config ...
  access_control: {
    enabled: true,
    rules: [
      {
        key_expr: "projects/project-a/**",
        permission: "allow",
        flow: "both"
      },
      {
        key_expr: "projects/project-b/**",
        permission: "deny",
        flow: "both"
      },
      {
        key_expr: "projects/project-c/**",
        permission: "deny",
        flow: "both"
      }
    ]
  }
}
```

**Limitation:** Zenoh's access control is experimental and may not be available in all versions.

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

Agent routers cache recent queries locally:

```json5
{
  // Agent router config
  plugins: {
    storage_manager: {
      volumes: [{
        name: "cache",
        backend: "garry",
        storages: [{
          name: "project-a-cache",
          key_expr: "projects/project-a/**",
          volume: {
            db_path: "/data/cache",
            pool_size: 64,
            max_record_size: 1048576,
            max_versions: 1,
            compression: "lz4"
          }
        }]
      }]
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
# From agent-a container
curl http://router-project-b:7449/healthz
# Expected: Connection refused (different network)
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

1. **Zenoh version** — access control features may be experimental
2. **Network overhead** — each agent router adds latency
3. **Storage overhead** — cached replication duplicates data
4. **Complexity** — more moving parts to manage
5. **Debugging** — harder to trace issues across routers

## References

- [Zenoh Documentation](https://zenoh.io/docs/)
- [Zenoh Access Control](https://zenoh.io/docs/manual/access-control/)
- [Docker Networking](https://docs.docker.com/network/)
- [Garry Storage Backend](https://github.com/Argylelabcoat/Garry)
