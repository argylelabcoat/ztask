# OpenSpec

Specifications and implementation plans for ztask features.

## Philosophy

**Specs are source of truth.** They describe what exists and why. They evolve with the code — when you change behavior, update the spec. Specs are not timestamped artifacts; they are living documents.

**Plans are point-in-time.** They describe how to implement a specific change. Plans are dated because they capture a moment's approach to implementation.

## Directory Structure

```
openspec/
  README.md              # this file
  specs/
    agent-skills.md      # skill architecture and design
    cli/
      overview.md        # Python CLI package
      task-model.md      # core data model
    web/
      overview.md        # Rust web UI
    router/
      overview.md        # Zenoh + Garry container
  plans/                 # implementation plans (dated, point-in-time)
    README.md
```

## Component Specs

| Component | Directory | Description |
|-----------|-----------|-------------|
| CLI | `specs/cli/` | Python CLI package (`ztask`) — models, queries, CLI commands |
| Web | `specs/web/` | Rust web UI (`ztask-web`) — axum + askama + htmx |
| Router | `specs/router/` | Zenoh router + Garry storage backend |
| Skills | `specs/agent-skills.md` | Agent-agnostic skill architecture |
| Task Model | `specs/cli/task-model.md` | Data model (current + SDD→TDD extension) |

## Naming Conventions

- **Specs:** `<topic>.md` — no date prefix. Examples: `task-model.md`, `overview.md`
- **Plans:** `YYYY-MM-DD-<slug>.md` — date prefix. Examples: `2026-08-03-sdd-tdd-extension.md`

## Lifecycle

1. **Spec** is written before implementation. It defines the "what" and "why".
2. **Plan** is derived from the spec. It defines the "how" — task-by-task steps for agentic workers.
3. Tasks are created in Zenoh from the plan (`ztask create ... --criteria "..."`).
4. Orchestrator executes tasks via sub-agents.
5. When implementation changes behavior, the **spec is updated** to reflect reality.

## Relationship to `docs/superpowers/`

`docs/superpowers/` is the legacy location for specs and plans. New work goes here. Existing specs can be migrated over time.

## Writing Specs

Specs should answer:
- **What** does this component do?
- **Why** was it designed this way?
- **What are the interfaces?** (data models, CLI commands, APIs)
- **What are the constraints?** (backward compatibility, performance, security)
- **What is out of scope?** (explicit exclusions)

Specs should NOT contain:
- Implementation steps (that's a plan)
- Timestamps or dates (specs are living documents)
- "Current state" vs "future state" (just describe the state)

## Writing Plans

Plans should contain:
- Reference to the spec they implement
- File-by-file changes with checkbox steps
- Code snippets for new/modified interfaces
- Test requirements per task

Plans are dated because they capture a specific approach. A plan may be abandoned and a new one written — that's fine.
