# ztask vs GNU GNATS

A comparison of the Zenoh-backed task tracker for LLM agents vs the classic GNU bug tracking system.

## Overview

| Feature | ztask | GNU GNATS |
|---------|-------|-----------|
| **Primary audience** | LLM agents & developers | Software projects (bug tracking) |
| **Storage** | Zenoh keyspace (distributed) | Flat files + index |
| **Architecture** | Client-server (zenohd router) | Email-based + CLI |
| **Data model** | Hierarchical key-values | Structured PR (Problem Reports) |
| **Collaboration** | Multi-agent, multi-user | Email-driven workflow |
| **Web UI** | Rust (axum + htmx) | Gnatsweb (separate) |
| **Language** | Python + Rust | C |
| **First release** | 2026 | 1992 |
| **Philosophy** | Agent-native, TDD-driven | Email-centric, minimal |

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

### GNU GNATS

```
>Number:          # Auto-incrementing PR number
>Category:        # Component category (e.g., "bash", "gcc")
>Severity:        # critical, serious, non-critical
>Priority:        # high, medium, low
>Responsible:     # Person responsible
>State:           # open, analyzed, suspended, feedback, closed
>Confidential:    # yes/no
>Submitter-Id:    # Submitter identifier
>Arrival-Date:    # When PR was filed
>Originator:      # Full name of submitter
>Release:         # Software release version
>Organization:    # Submitter's organization
>Environment:     # System environment description
>Description:     # Full problem description
>How-To-Repeat:   # Steps to reproduce
>Fix:             # Fix description (if available
```

## Command Comparison

### Creating Tasks/PRs

**ztask:**
```bash
ztask create auth-login --project myapp \
  --criteria "Given valid creds, When POST /login, Then return JWT" \
  --depends-on db-migrations \
  --test-files tests/test_auth.py \
  --impl-files src/auth.py \
  --test-command "pytest tests/test_auth.py"
```

**GNATS:**
```bash
# Via email
mail bugs@project.org << EOF
>From: developer@example.org
>Category: auth
>Severity: serious
>Priority: medium
>Responsible: dev-team
>State: open
>Class: sw-bug
>Submitter-Id: net
>Release: 1.0
>Environment: Linux x86_64
>Description: Login endpoint returns 500 on valid credentials
>How-To-Repeat: POST /login with valid JWT
>Fix: (none)
EOF

# Via CLI
edit-pr -a 42
```

### Listing Tasks/PRs

**ztask:**
```bash
ztask list --project myapp                    # all tasks
ztask list --project myapp --filter incomplete # not completed
ztask list --project myapp --filter wip       # in progress
```

**GNATS:**
```bash
query-pr --category auth                      # filter by category
query-pr --state open                         # filter by state
query-pr --responsible dev-team               # filter by responsible
query-pr --full 42                            # full PR details
```

### Updating Status

**ztask:**
```bash
ztask update-status auth-login IN_PROGRESS --project myapp --note "Starting implementation"
ztask update-status auth-login COMPLETED --project myapp --note "All tests pass"
```

**GNATS:**
```bash
# Via email reply
mail bugs@project.org << EOF
>From: developer@example.org
>Number: 42
>State: analyzed
>Responsible: dev-team
>Fix: Implemented JWT validation
EOF

# Via CLI
edit-pr 42
```

### Viewing Details

**ztask:**
```bash
ztask get auth-login --project myapp
```

**GNATS:**
```bash
query-pr --full 42
cat /var/gnats/db/category/42
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
| **Orchestrator** | Automated sub-agent orchestration |
| **Spec management** | Merge, organize, archive specs |

### GNATS Only

| Feature | Description |
|---------|-------------|
| **Email-based workflow** | Submit/update PRs via email |
| **Mail interfaces** | `gnatsd` daemon for mail processing |
| **Audit trail** | Immutable email-based history |
| **Category system** | Hierarchical component categories |
| **Severity levels** | Critical/serious/non-critical |
| **Confidential PRs** | Private bug reports |
| **Responsible assignment** | Automatic routing by category |
| **Query language** | Structured query syntax |
| **Emacs integration** | `gnats.el` for Emacs users |
| **CVS/SVN integration** | Version control hooks |
| **Auto-reply** | Automatic acknowledgment emails |
| **PR database** | Flat-file database with index |

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

### GNATS

```
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  User           │────▶│  Email Client   │────▶│  SMTP Server    │
│                 │     │                 │     │                 │
└─────────────────┘     └─────────────────┘     └─────────────────┘
                               │                        │
                               ▼                        ▼
                        ┌─────────────────┐     ┌─────────────────┐
                        │  gnatsd daemon  │     │  GNATS Database │
│  (mail handler) │     │  (flat files)   │
                        └─────────────────┘     └─────────────────┘
```

## Use Case Comparison

### ztask is better for:

| Use Case | Why |
|----------|-----|
| **LLM agent orchestration** | Designed for autonomous agents |
| **TDD/BDD workflows** | Built-in phase tracking and test integration |
| **Multi-agent collaboration** | Status locking, dependency resolution |
| **Spec-driven development** | OpenSpec integration, acceptance criteria |
| **Complex dependencies** | Cycle detection, topological sort |
| **Modern workflows** | REST-like CLI, JSON output |
| **Real-time monitoring** | Web UI with live updates |
| **Automated testing** | Test commands and verification |

### GNATS is better for:

| Use Case | Why |
|----------|-----|
| **Legacy projects** | 30+ years of maturity |
| **Email-centric teams** | Native email workflow |
| **Minimal infrastructure** | No server required |
| **Regulatory compliance** | Immutable audit trail |
| **Simple bug tracking** | Lightweight, focused |
| **Unix philosophy** | Small, composable tools |
| **Offline workflows** | Email queuing support |
| **Existing GNATS users** | Migration-free |

## Data Portability

### ztask

- **Export**: JSON via `ztask list --filter all`
- **Import**: `ztask create` with all fields
- **Zenoh protocol**: Native wire format
- **Web UI**: Human-readable via browser

### GNATS

- **Export**: Flat files, query output
- **Import**: Email-based submission
- **Database**: Human-readable text files
- **Backup**: File system copy

## Performance

| Metric | ztask | GNATS |
|--------|-------|-------|
| **Startup time** | ~100ms (Zenoh session) | ~10ms |
| **List 1000 PRs** | ~200ms (network) | ~500ms (file scan) |
| **Create task** | ~50ms (network) | ~200ms (email) |
| **Storage size** | Zenoh keyspace | Flat files |
| **Memory usage** | ~50MB (router) | ~1MB |
| **Concurrent users** | Unlimited (distributed) | Limited (file locks) |

## Integration Ecosystem

### ztask

- **MiMoCode**: Native skill integration
- **Claude Code**: Skill-compatible
- **OpenAI Codex**: Skill-compatible
- **Custom agents**: CLI-based integration
- **Web UI**: Browser-based monitoring
- **OpenSpec**: Spec-driven development

### GNATS

- **Emacs**: gnats.el
- **CVS/SVN**: Commit hooks
- **Email**: SMTP integration
- **Gnatsweb**: Web interface
- **Perl**: GNATS API
- **Shell**: CLI tools

## Workflow Comparison

### ztask: Agent-Driven Development

```
1. Write OpenSpec (specs/tasks/*.md)
2. Ingest: ztask-ingest myproject specs/
3. Orchestrate: /ztask-orchestrator myproject
4. Sub-agents execute tasks (TDD cycle)
5. Merge specs: /ztask-spec-merge
6. Archive: /ztask-spec-organize archive
```

### GNATS: Email-Based Bug Tracking

```
1. User reports bug via email
2. gnatsd processes email, creates PR
3. Developer queries PRs, assigns to self
4. Developer fixes bug, replies to email
5. PR updated via email
6. Tester verifies, closes PR via email
```

## Modernization Path

### GNATS → ztask Migration

For projects moving from GNATS to ztask:

```bash
# Export GNATS PRs
query-pr --format json > gnats-export.json

# Convert to ztask format
python convert-gnats-to-ztask.py gnats-export.json > ztask-import.json

# Import into ztask
ztask import --project myproject ztask-import.json
```

### ztask GNATS Compatibility

For projects needing both:

```bash
# Export ztask tasks to GNATS format
ztask list --project myapp --filter all | \
  python convert-ztask-to-gnats.py > gnats-import.txt

# Import into GNATS
cat gnats-import.txt | mail bugs@project.org
```

## When to Choose

### Choose ztask when:

- Building LLM agent workflows
- Need TDD/BDD integration
- Multi-agent collaboration required
- Complex task dependencies
- Spec-driven development
- Need web UI for monitoring
- Want automated orchestration

### Choose GNATS when:

- Legacy project maintenance
- Email-centric team workflow
- Minimal infrastructure budget
- Regulatory audit requirements
- Simple bug tracking needs
- Unix philosophy preferred
- Offline-first workflow

## Hybrid Approach

Use both for different purposes:

```bash
# ztask for agent-driven development
ztask create implement-auth --project myapp --criteria "..."

# GNATS for human-reported bugs
mail bugs@project.org << EOF
>Category: auth
>Severity: serious
>Description: Login fails on Safari
EOF

# Sync via export/import
ztask list --project myapp --filter all | \
  python sync-to-gnats.py | mail bugs@project.org
```

## Summary

| Aspect | ztask | GNU GNATS |
|--------|-------|-----------|
| **Best for** | LLM agents, TDD, automation | Legacy, email, minimal |
| **Complexity** | Higher (server, agents) | Lower (flat files) |
| **Learning curve** | Moderate | Low |
| **Maturity** | New (2026) | 30+ years (1992) |
| **Community** | Niche | Legacy |
| **Extensibility** | Skills, MCP | Email, scripts |
| **Philosophy** | Agent-native | Unix/email-centric |

**ztask** is purpose-built for the LLM agent era — designed for autonomous sub-agents, TDD workflows, and spec-driven development. **GNU GNATS** is the classic Unix bug tracker that's been refined over 30+ years with an email-centric philosophy.

Choose based on your primary use case: agents → ztask, legacy/email → GNATS, both → hybrid.
