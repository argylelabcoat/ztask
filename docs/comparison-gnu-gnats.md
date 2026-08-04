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

Structured text records (one file per PR) in a configurable database
directory (commonly `/var/gnats/`; see the `GNATSDB` environment variable
and the *Where GNATS lives* appendix of the GNATS manual). Each PR is a
specially formatted mail message with `>`-prefixed fields ([GNATS 4.2
manual](https://www.gnu.org/software/gnats/doc/gnats-4.2.0/gnats.html)):

```
>Number:          # auto-incrementing integer (added by GNATS on arrival)
>Category:        # enumerated-in-file (e.g., "bash", "gcc", "libc")
>Synopsis:        # one-line summary (also copied to the mail Subject:)
>Severity:        # "critical", "serious", "non-critical"
>Priority:        # "high", "medium", "low"
>Responsible:     # text (person or team, from the categories file)
>State:           # "open", "analyzed", "feedback", "suspended", "closed"
>Class:           # "sw-bug", "doc-bug", "change-request", "support",
                  #  "duplicate", "mistaken"
>Confidential:    # "yes" or "no"
>Submitter-Id:    # enumerated-in-file (e.g., "net" for unaffiliated)
>Arrival-Date:    # date (auto-filled by GNATS)
>Date-Required:   # date (added at the Support Site)
>Originator:      # text (real name)
>Organization:    # multitext
>Release:         # text (software version)
>Environment:     # multitext (free-text)
>Description:     # multitext (free-text)
>How-To-Repeat:   # multitext (free-text)
>Fix:             # multitext (free-text)
>Audit-Trail:     # multitext — auto-appended state/responsible changes
                  #  (State-Changed-From-To/-When/-Why,
                  #   Responsible-Changed-From-To/-When/-Why) and
                  #  related follow-up email
>Notify-List:     # comma-separated email addresses to notify on change
>Last-Modified:   # date (added during the PR's lifetime)
>Closed Date:     # date (added when the PR is closed)
>Unformatted:     # multitext — any random text outside the fields
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
# Submit a new PR via send-pr (the canonical client)
send-pr -f /tmp/my-pr.txt              # validate + submit a completed template
send-pr -s serious                     # open the editor with Severity pre-set
# Or via email: send plain mail to the support site's PR address; GNATS
# fills Subject -> Synopsis, From -> Submitter, body -> Description.
```

### Querying/Listing

**ztask:**
```bash
ztask list --project <id> [--filter all|incomplete|wip]
ztask get <task-id> --project <id>
```

**GNATS:**
```bash
# query-pr queries the database
query-pr [--category "..."] [--state "..."] [--responsible "..."] [--full <number>]
# See the GNATS 4.2 manual, "Querying the database" section, for the
# full set of options (regex matches, field-specific predicates, etc.)
```

### Updating

**ztask:**
```bash
ztask update-status <task-id> <status> --project <id> [--note "..."]
```

**GNATS:**
```bash
# State changes happen via edit-pr (it locks the PR, opens $EDITOR, and
# appends a State-Changed-* / Responsible-Changed-* entry to >Audit-Trail:)
edit-pr <number>

# Follow-up email is appended to the PR's >Audit-Trail: field, but only
# if the Subject: references the PR (e.g., "Re: category/123"). Email
# cannot directly change >State: — that requires edit-pr or a client
# talking to gnatsd.
```

### Deleting

**ztask:**
```bash
# No delete command in the CLI; tasks are marked COMPLETED.
# (The bundled web UI, ztask-web, exposes a delete_task endpoint that
#  removes all keys under projects/<id>/tasks/<task_id>/** .)
```

**GNATS:**
```bash
# PRs are closed via state change, not deleted
```

## Feature Mapping

| ztask feature | GNATS equivalent | Notes |
|---------------|------------------|-------|
| `status` | `>State:` | ztask uses 6 values: PENDING, IN_PROGRESS, WIP, RUNNING, COMPLETED, UNKNOWN; GNATS uses 5: open, analyzed, feedback, suspended, closed (customizable via the `states` file) |
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
| `history` | `>Audit-Trail:` | GNATS auto-records State/Responsible changes and follow-up email; ztask records every status transition |
| `time_entered` | `>Arrival-Date:` | Both track creation time |
| `time_accepted` | (none) | ztask tracks when work starts |
| `time_completed` | (none) | ztask tracks completion time |
| `entered_by` | `>Originator:` / `>Submitter-Id:` | ztask: "LLM" or "USER"; GNATS: `>Originator:` is the real name, `>Submitter-Id:` is a site-assigned code |
| (none) | `>Category:` | GNATS categorizes by component |
| (none) | `>Severity:` | GNATS has severity levels |
| (none) | `>Priority:` | GNATS has priority levels |
| (none) | `>Class:` | GNATS classifies PR type (sw-bug, doc-bug, ...) |
| (none) | `>Responsible:` | GNATS assigns responsibility |
| (none) | `>Confidential:` | GNATS supports private PRs |
| (none) | `>Environment:` | GNATS captures system info |
| (none) | `>How-To-Repeat:` | GNATS stores reproduction steps |
| (none) | `>Audit-Trail:` | GNATS auto-records state/responsible changes (ztask uses `history`) |
| (none) | `>Notify-List:` | GNATS supports per-PR notification lists |

## Storage Architecture

**ztask:**
- Zenoh router (`zenohd`) with Garry storage backend
- Hierarchical key-value pairs
- Distributed access via Zenoh protocol
- Persistent volume for data

**GNATS:**
- One text file per PR, named by PR number, under `<gnats-db>/<category>/`
  (database path is configurable; commonly `/var/gnats/` on Linux distros)
- Index file (`index`) per database for fast lookups
- PRs are also mailable: each PR is a specially formatted mail message
- Direct filesystem access for `edit-pr`/`query-pr`; remote access via `gnatsd`

## Access Protocol

**ztask:**
- CLI connects via Zenoh protocol (`tcp/localhost:7447`)
- Web UI connects via HTTP (`localhost:8080`)
- Remote access via Zenoh routing

**GNATS:**
- `send-pr`/`edit-pr`/`query-pr` can run against a local database (direct
  filesystem access) or a remote one via `gnatsd` (the GNATS network daemon,
  on port 1529 by default)
- Email submission via SMTP to the support site's PR address
- `gnatsd` daemon processes email and serves network clients
- Gnatsweb provides a CGI-based HTTP interface (separate project)

## Concurrency

**ztask:**
- Multiple agents can operate simultaneously against the same router
- No compare-and-set in the CLI: the last `update-status` write wins
  (Zenoh `put` is not atomic across writers), so agents must coordinate
  out-of-band to avoid clobbering each other's status
- Timestamps come from the Zenoh router's Hybrid Logical Clock, giving a
  per-key total order for history reconstruction

**GNATS:**
- File-level locking for writes
- Single-user editing at a time
- Email queue handles concurrent submissions

## Output Formats

**ztask:**
- JSON (all commands)
- Human-readable (web UI)

**GNATS:**
- Structured text (`query-pr`)
- Email (notifications + Audit-Trail follow-ups)
- Gnatsweb (HTML — separate project)
- TkGNATS (Tcl/Tk — separate project)

## Environment Variables

**ztask:**
```
ZTASK_ZENOH_ENDPOINT    # default: tcp/localhost:7447
```

**GNATS:**
```
GNATSDB              # local db name, or "server:port:databasename:username:password"
                     # for network access via gnatsd. Defaults to "default".
EDITOR               # editor invoked by send-pr / edit-pr (defaults to vi)
```

## Feature Parity Gaps

### ztask has, GNATS lacks

1. **Dependency tracking** — `depends_on`/`blocks` with cycle detection (in `ztask-ingest`)
2. **TDD phase tracking** — red/green/refactor lifecycle
3. **Acceptance criteria** — Gherkin-style criteria as a first-class field
4. **Spec field** — full specification context
5. **Test/impl file tracking** — which files belong to a task
6. **Test/verify commands** — how to run tests
7. **Failure tracking** — `failure_reason` and `attempt_count`
8. **OpenSpec integration** — spec-driven workflow (this repo's `openspec/` directory)
9. **First-class Rust web UI** — bundled `ztask-web` (axum + htmx) in this repo;
   GNATS web access is via the separate Gnatsweb CGI project

> Note: "Multi-agent orchestration" and "sub-agent spawning" are
> implemented in this repo's `.agents/skills/` (ztask-orchestrator,
> ztask-worker), not in the `ztask` tool itself — GNATS has no
> equivalent orchestration layer.

### GNATS has, ztask lacks

1. **Category system** — enumerated-in-file categorization by component
2. **Severity levels** — critical/serious/non-critical
3. **Priority levels** — high/medium/low
4. **Class** — sw-bug/doc-bug/change-request/support/duplicate/mistaken
5. **Confidential PRs** — private bug reports (`>Confidential:` field)
6. **Responsible assignment** — auto-routing by category via the
   `categories` file
7. **Email workflow** — native email submission (`send-pr`, plain email)
   and notification
8. **Audit-Trail** — auto-recorded state/responsible changes with
   reason annotations, plus follow-up email archiving
9. **Environment capture** — `>Environment:` system information field
10. **Reproduction steps** — `>How-To-Repeat:` field
11. **Emacs integration** — `send-pr` and `edit-pr` from within Emacs
    (GNATS user tools chapter, "The Emacs interface to GNATS")
12. **Multiple frontends** — Gnatsweb (CGI), TkGNATS (Tcl/Tk),
    gnatsperl (Perl API), GTK+ send-pr

## Mapping ztask to GNATS

To replicate ztask features in GNATS:

| ztask | GNATS approach |
|-------|----------------|
| `depends_on` | Use `>Description:` to list dependencies manually |
| `tdd_phase` | Use `>State:` or custom annotations in `>Description:` |
| `acceptance_criteria` | Include in `>Description:` |
| `test_files` | Include in `>Description:` or `>How-To-Repeat:` |
| `failure_reason` | Use `>Fix:` field or `>Audit-Trail:` annotations |
| `attempt_count` | Track manually via `>Audit-Trail:` entries |
| `history` | GNATS `>Audit-Trail:` auto-records state/responsible changes |
| Web UI | Use Gnatsweb (separate CGI project, not bundled with GNATS) |

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

## References

- [GNU GNATS project page](https://www.gnu.org/software/gnats/) — official site, current stable version 4.2.0 (Feb 2015)
- [GNATS 4.2.0 manual](https://www.gnu.org/software/gnats/doc/gnats-4.2.0/gnats.html) — data model, user tools, administration
- [Gnatsweb](https://www.gnu.org/software/gnatsweb/) — the official CGI web interface for GNATS (separate project)
- [Zenoh documentation](https://zenoh.io/docs/) — Zenoh abstractions and configuration
- `ztask/cli.py`, `ztask/models.py`, `ztask/queries.py`, `ztask/ingest.py` — this repo's CLI, data model, and ingest implementation
- `web/src/tasks.rs` — this repo's web UI task operations (including `delete_task`)
