# ztask vs GNU GNATS

A factual comparison of CLI commands, data models, and feature mapping.

## Data Model

### ztask

Hierarchical key-value pairs stored in Zenoh:

```
projects/<project_id>/tasks/<task_id>/
  status                    # PENDING, IN_PROGRESS, WIP, RUNNING, COMPLETED, UNKNOWN
  acceptance_criteria       # string
  spec                      # string
  depends_on                # JSON array of task ID strings
  blocks                    # JSON array of task ID strings
  test_files                # JSON array of path strings
  implementation_files      # JSON array of path strings
  tdd_phase                 # "red", "green", "refactor", or empty
  test_command              # string
  verification_command      # string
  failure_reason            # string
  attempt_count             # integer as string
  history                   # JSON array of {timestamp, from_status, to_status, note}
  time_entered              # ISO 8601 timestamp
  time_accepted             # ISO 8601 timestamp
  time_completed            # ISO 8601 timestamp
  entered_by                # "LLM" or "USER"
```

### GNU GNATS

Structured text records in flat files:

```
>Number:          # auto-incrementing integer
>Category:        # string (e.g., "bash", "gcc", "libc")
>Severity:        # "critical", "serious", "non-critical"
>Priority:        # "high", "medium", "low"
>Responsible:     # string (person or team)
>State:           # "open", "analyzed", "suspended", "feedback", "closed"
>Confidential:    # "yes" or "no"
>Submitter-Id:    # string
>Arrival-Date:    # date string
>Originator:      # string (full name)
>Release:         # string (software version)
>Organization:    # string
>Environment:     # string (free-text)
>Description:     # string (free-text)
>How-To-Repeat:   # string (free-text)
>Fix:             # string (free-text)
```

## CLI Commands

### Creating

**ztask:**
```bash
ztask create <task-id> --project <id> [--criteria "..."] [--spec "..."] \
  [--depends-on "id1,id2"] [--test-files "file1,file2"] \
  [--impl-files "file1,file2"] [--test-command "..."] [--verify-command "..."] \
  [--entered-by llm|user]
```

**GNATS:**
```bash
# Via email
echo ">Category: bash\n>Severity: serious\n>Description: ..." | mail bugs@project.org

# Via edit-pr
edit-pr -a <number>
```

### Querying/Listing

**ztask:**
```bash
ztask list --project <id> [--filter all|incomplete|wip]
ztask get <task-id> --project <id>
```

**GNATS:**
```bash
query-pr [--category "..."] [--state "..."] [--responsible "..."] [--full <number>]
```

### Updating

**ztask:**
```bash
ztask update-status <task-id> <status> --project <id> [--note "..."]
```

**GNATS:**
```bash
# Via email reply
echo ">Number: 42\n>State: analyzed\n>Fix: ..." | mail bugs@project.org

# Via edit-pr
edit-pr 42
```

### Deleting

**ztask:**
```bash
# No delete command; tasks are marked COMPLETED
```

**GNATS:**
```bash
# PRs are closed via state change, not deleted
```

## Feature Mapping

| ztask feature | GNATS equivalent | Notes |
|---------------|------------------|-------|
| `status` | `>State:` | ztask has 6 states; GNATS has 5 |
| `acceptance_criteria` | (none) | ztask-specific for TDD/BDD |
| `spec` | (none) | ztask-specific for spec-driven dev |
| `depends_on` | (none) | ztask has dependency tracking |
| `blocks` | (none) | ztask has dependency tracking |
| `test_files` | (none) | ztask tracks test file paths |
| `implementation_files` | (none) | ztask tracks implementation files |
| `tdd_phase` | (none) | ztask tracks TDD lifecycle |
| `test_command` | (none) | ztask stores test commands |
| `verification_command` | (none) | ztask stores verification commands |
| `failure_reason` | (none) | ztask tracks failure reasons |
| `attempt_count` | (none) | ztask tracks retry attempts |
| `history` | (none) | ztask logs all transitions |
| `time_entered` | `>Arrival-Date:` | Both track creation time |
| `time_accepted` | (none) | ztask tracks when work starts |
| `time_completed` | (none) | ztask tracks completion time |
| `entered_by` | `>Submitter-Id:` | ztask: LLM/USER; GNATS: string |
| (none) | `>Category:` | GNATS categorizes by component |
| (none) | `>Severity:` | GNATS has severity levels |
| (none) | `>Priority:` | GNATS has priority levels |
| (none) | `>Responsible:` | GNATS assigns responsibility |
| (none) | `>Confidential:` | GNATS supports private PRs |
| (none) | `>Environment:` | GNATS captures system info |
| (none) | `>How-To-Repeat:` | GNATS stores reproduction steps |

## Storage Architecture

**ztask:**
- Zenoh router (`zenohd`) with Garry storage backend
- Hierarchical key-value pairs
- Distributed access via Zenoh protocol
- Persistent volume for data

**GNATS:**
- Flat files in `/var/gnats/db/<category>/`
- One file per PR, numbered sequentially
- Index file for fast lookups
- Direct filesystem access

## Access Protocol

**ztask:**
- CLI connects via Zenoh protocol (`tcp/localhost:7447`)
- Web UI connects via HTTP (`localhost:8080`)
- Remote access via Zenoh routing

**GNATS:**
- CLI reads/writes flat files directly
- Email submission via SMTP
- `gnatsd` daemon processes email
- Gnatsweb provides HTTP interface

## Concurrency

**ztask:**
- Multiple agents can operate simultaneously
- Status locking via Zenoh's eventual consistency
- First agent to set IN_PROGRESS wins

**GNATS:**
- File-level locking for writes
- Single-user editing at a time
- Email queue handles concurrent submissions

## Output Formats

**ztask:**
- JSON (all commands)
- Human-readable (web UI)

**GNATS:**
- Structured text (query-pr)
- Email (notifications)
- Gnatsweb (HTML)

## Environment Variables

**ztask:**
```
ZTASK_ZENOH_ENDPOINT    # default: tcp/localhost:7447
```

**GNATS:**
```
GNATS_ADDR              # gnatsd address
GNATS_ADMIN             # admin email
GNATS_DEFAULT_CATEGORY  # default category
GNATS_DEFAULT_SEVERITY  # default severity
GNATS_DEFAULT_PRIORITY  # default priority
```

## Feature Parity Gaps

### ztask has, GNATS lacks

1. **Dependency tracking** — `depends_on`/`blocks` with cycle detection
2. **TDD phase tracking** — red/green/refactor lifecycle
3. **Acceptance criteria** — Gherkin-style criteria
4. **Spec field** — full specification context
5. **Test/impl file tracking** — which files belong to a task
6. **Test/verify commands** — how to run tests
7. **Failure tracking** — `failure_reason` and `attempt_count`
8. **Multi-agent orchestration** — sub-agent spawning
9. **Web UI** — built-in Rust web interface
10. **OpenSpec integration** — spec-driven workflow

### GNATS has, ztask lacks

1. **Category system** — hierarchical component categorization
2. **Severity levels** — critical/serious/non-critical
3. **Priority levels** — high/medium/low
4. **Confidential PRs** — private bug reports
5. **Responsible assignment** — automatic routing by category
6. **Email workflow** — native email submission/notification
7. **Environment capture** — system information field
8. **Reproduction steps** — how-to-repeat field
9. **Emacs integration** — gnats.el
10. **CVS/SVN hooks** — version control integration

## Mapping ztask to GNATS

To replicate ztask features in GNATS:

| ztask | GNATS approach |
|-------|----------------|
| `depends_on` | Use `>Description:` to list dependencies manually |
| `tdd_phase` | Use `>State:` or custom annotations |
| `acceptance_criteria` | Include in `>Description:` |
| `test_files` | Include in `>Description:` or `>How-To-Repeat:` |
| `failure_reason` | Use `>Fix:` field or annotations |
| `attempt_count` | Track manually via annotations |
| `history` | GNATS logs all email changes automatically |
| Web UI | Use Gnatsweb (separate project) |

## Mapping GNATS to ztask

To replicate GNATS features in ztask:

| GNATS | ztask approach |
|-------|----------------|
| `>Category:` | Use project ID or task ID prefix |
| `>Severity:` | Use `acceptance_criteria` or `spec` to indicate severity |
| `>Priority:` | Not supported; use task ordering |
| `>Responsible:` | Use `entered_by` or task naming convention |
| `>Confidential:` | Not supported; use separate project |
| `>Environment:` | Include in `spec` field |
| `>How-To-Repeat:` | Include in `acceptance_criteria` (Gherkin format) |
| Email workflow | Use CLI or web UI directly |
