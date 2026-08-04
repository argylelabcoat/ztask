# ztask vs GNU Task (Taskwarrior)

A comparison of the Zenoh-backed task tracker for LLM agents vs the traditional CLI task manager.

## Overview

| Feature | ztask | GNU Task (Taskwarrior) |
|---------|-------|------------------------|
| **Primary audience** | LLM agents & developers | Humans |
| **Storage** | Zenoh keyspace (distributed) | Local SQLite/file |
| **Architecture** | Client-server (zenohd router) | Single-user local |
| **Data model** | Hierarchical key-values | Flat task records |
| **Collaboration** | Multi-agent, multi-user | Single user |
| **Web UI** | Rust (axum + htmx) | None (separate projects) |
| **Language** | Python + Rust | C++ |

## Data Model

### ztask

```
projects/<project_id>/tasks/<task_id>/
  status                    # PENDING, IN_PROGRESS, WIP, RUNNING, COMPLETED
  acceptance_criteria       # Gherkin-style criteria
  spec                      # Full specification
  depends_on                # JSON array of task IDs
  blocks                    # JSON array of task IDs
  test_files                # JSON array of paths
  implementation_files      # JSON array of paths
  tdd_phase                 # red, green, refactor
  test_command              # Shell command
  verification_command      # Shell command
  failure_reason            # Last failure reason
  attempt_count             # Number of attempts
  history                   # JSON array of transitions
  time_entered              # ISO timestamp
  time_accepted             # ISO timestamp
  time_completed            # ISO timestamp
  entered_by                # LLM or USER
```

### GNU Task

```
uuid          # Unique identifier
description   # Task description
status        # pending, completed, deleted, waiting
project       # Project name
priority      # H, M, L (or none)
tags          # List of tags
depends       # List of dependency UUIDs
due           # Due date
scheduled     # Scheduled start date
wait          # Wait until date
entry          # Creation date
end           # Completion date
modified      # Last modification date
annotations   # List of {date, description}
urgency       # Calculated urgency score
```

## Command Comparison

### Creating Tasks

**ztask:**
```bash
ztask create auth-login --project myapp \
  --criteria "Given valid creds, When POST /login, Then return JWT" \
  --depends-on db-migrations \
  --test-files tests/test_auth.py \
  --impl-files src/auth.py \
  --test-command "pytest tests/test_auth.py"
```

**GNU Task:**
```bash
task add project:myapp +auth "Implement login endpoint"
task 1 modify depends:2
```

### Listing Tasks

**ztask:**
```bash
ztask list --project myapp                    # all tasks
ztask list --project myapp --filter incomplete # not completed
ztask list --project myapp --filter wip       # in progress
```

**GNU Task:**
```bash
task list                    # pending tasks
task list project:myapp      # filter by project
task list +auth              # filter by tag
task all                     # all tasks
```

### Updating Status

**ztask:**
```bash
ztask update-status auth-login IN_PROGRESS --project myapp --note "Starting implementation"
ztask update-status auth-login COMPLETED --project myapp --note "All tests pass"
```

**GNU Task:**
```bash
task 1 start                 # mark as started
task 1 done                  # mark as completed
task 1 annotate "Starting implementation"
```

### Viewing Task Details

**ztask:**
```bash
ztask get auth-login --project myapp
```

**GNU Task:**
```bash
task 1 info
task 1 show
```

## Unique Features

### ztask Only

| Feature | Description |
|---------|-------------|
| **Acceptance criteria** | Gherkin-style criteria for TDD/BDD |
| **Spec field** | Full specification context |
| **Dependency tracking** | `depends_on` and `blocks` with cycle detection |
| **TDD phase tracking** | red/green/refactor lifecycle |
| **Test/impl file tracking** | Knows which files belong to a task |
| **Test/verify commands** | How to run tests and verify |
| **Failure tracking** | `failure_reason` and `attempt_count` |
| **Multi-agent support** | Concurrent agents with status locking |
| **Web UI** | Rust axum + htmx admin interface |
| **OpenSpec integration** | Spec-driven development workflow |
| **BDD feature files** | Auto-generated from acceptance criteria |

### GNU Task Only

| Feature | Description |
|---------|-------------|
| **Urgency scoring** | Calculated urgency based on multiple factors |
| **Tags** | Flexible tagging system |
| **Priority levels** | H/M/L priority |
| **Due dates** | Date-based scheduling |
| **Recurring tasks** | Automatic task recurrence |
| **Reports** | Built-in burndown, history, summary reports |
| **Hooks** | Pre/post command hooks |
| **Sync** | Task server for multi-device sync |
| **Contexts** | Saved filters for different work modes |
| **Macros** | Custom command aliases |
| **Templates** | Task templates for repeated patterns |
| **Calendar** | Calendar view of tasks |
| **Burndown charts** | Visual progress tracking |

## Use Case Comparison

### ztask is better for:

| Use Case | Why |
|----------|-----|
| **LLM agent orchestration** | Designed for autonomous agents |
| **TDD/BDD workflows** | Built-in phase tracking and test integration |
| **Multi-agent collaboration** | Status locking, dependency resolution |
| **Spec-driven development** | OpenSpec integration, acceptance criteria |
| **Complex dependencies** | Cycle detection, topological sort |
| **Audit trails** | Full history with notes per transition |
| **Web-based monitoring** | Built-in web UI for humans |

### GNU Task is better for:

| Use Case | Why |
|----------|-----|
| **Personal task management** | Simple, fast, local |
| **Date-based scheduling** | Due dates, recurring tasks |
| **Quick capture** | Fast task creation with tags |
| **Offline work** | No server required |
| **Existing workflows** | Mature ecosystem, integrations |
| **Reporting** | Built-in reports and charts |
| **Multi-device sync** | Task server for synchronization |

## Architecture Comparison

### ztask

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  LLM Agent      │     │  ztask CLI      │     │  zenohd router  │
│  (sub-agent)    │────▶│  (Python)       │────▶│  + Garry        │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                               │                        │
                               ▼                        ▼
                        ┌─────────────────┐     ┌─────────────────┐
                        │  ztask-web      │     │  Storage        │
                        │  (Rust/htmx)    │     │  (persistent)   │
                        └─────────────────┘     └─────────────────┘
```

### GNU Task

```
┌─────────────────┐     ┌─────────────────┐
│  Human          │────▶│  task CLI       │
│                 │     │  (C++)          │
└─────────────────┘     └─────────────────┘
                               │
                               ▼
                        ┌─────────────────┐
                        │  Local Storage  │
                        │  (~/.task/)     │
                        └─────────────────┘
```

## Data Portability

### ztask

- **Export**: JSON via `ztask list --filter all`
- **Import**: `ztask create` with all fields
- **Zenoh protocol**: Native wire format
- **Web UI**: Human-readable via browser

### GNU Task

- **Export**: JSON, CSV, YAML, XML
- **Import**: JSON, CSV
- **Hook system**: Custom export formats
- **Task server**: Native sync protocol

## Performance

| Metric | ztask | GNU Task |
|--------|-------|----------|
| **Startup time** | ~100ms (Zenoh session) | ~10ms |
| **List 1000 tasks** | ~200ms (network) | ~50ms (local) |
| **Create task** | ~50ms (network) | ~5ms (local) |
| **Storage size** | Zenoh keyspace | SQLite/flat files |
| **Memory usage** | ~50MB (router) | ~5MB |

## Integration Ecosystem

### ztask

- **MiMoCode**: Native skill integration
- **Claude Code**: Skill-compatible
- **OpenAI Codex**: Skill-compatible
- **Custom agents**: CLI-based integration
- **Web UI**: Browser-based monitoring

### GNU Task

- **Shell**: Bash/Zsh integration
- **Editors**: Vim, Emacs plugins
- **CI/CD**: Jenkins, GitHub Actions
- **Mobile**: iOS, Android apps
- **Web**: Third-party web UIs
- **Calendar**: CalDAV sync
- **Email**: IMAP integration

## When to Choose

### Choose ztask when:

- Building LLM agent workflows
- Need TDD/BDD integration
- Multi-agent collaboration required
- Complex task dependencies
- Spec-driven development
- Need web UI for monitoring
- Want audit trails

### Choose GNU Task when:

- Personal task management
- Date-based scheduling important
- Offline-first workflow
- Existing Taskwarrior ecosystem
- Simple tag-based organization
- Need mobile apps
- Want built-in reports

## Hybrid Approach

You can use both together:

```bash
# Use ztask for agent-driven work
ztask create implement-auth --project myapp --criteria "..."

# Use GNU Task for personal tracking
task add "Review ztask implementation" +review +code

# Sync via export/import
ztask list --project myapp --filter all | jq '.[].description' | \
  xargs -I {} task add "{}" +ztask
```

## Summary

| Aspect | ztask | GNU Task |
|--------|-------|----------|
| **Best for** | LLM agents, TDD, collaboration | Personal tasks, scheduling |
| **Complexity** | Higher (server, agents) | Lower (local CLI) |
| **Learning curve** | Moderate | Low |
| **Maturity** | New | 15+ years |
| **Community** | Niche | Large |
| **Extensibility** | Skills, MCP | Hooks, scripts |

**ztask** is purpose-built for the LLM agent era — designed for autonomous sub-agents, TDD workflows, and spec-driven development. **GNU Task** is the battle-tested personal task manager that's been refined over 15+ years.

Choose based on your primary use case: agents → ztask, humans → GNU Task, both → hybrid.
