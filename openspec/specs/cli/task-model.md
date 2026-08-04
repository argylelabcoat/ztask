# Task Model

The core data model for tasks in the Zenoh keyspace. Shared by the Python CLI and Rust web UI.

## Current Model

**Python** (`ztask/models.py`):

```python
@dataclass
class Task:
    id: str
    status: str = "UNKNOWN"              # PENDING | IN_PROGRESS | WIP | RUNNING | COMPLETED | UNKNOWN
    time_entered: Optional[str] = None
    time_accepted: Optional[str] = None
    time_completed: Optional[str] = None
    acceptance_criteria: Optional[str] = None
    entered_by: Optional[str] = None     # LLM | USER
    history: List[dict] = field(default_factory=list)
```

**Rust** (`web/src/models.rs`):

```rust
pub struct HistoryEntry {
    pub timestamp: String,
    pub from_status: String,
    pub to_status: String,
    pub note: String,  // defaults to ""
}

pub struct Task {
    pub id: String,
    pub status: String,
    pub time_entered: Option<String>,
    pub time_accepted: Option<String>,
    pub time_completed: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub entered_by: Option<String>,
    pub history: Vec<HistoryEntry>,
}
```

## Extended Model (SDD→TDD)

```python
@dataclass
class Task:
    # --- existing ---
    id: str
    status: str = "UNKNOWN"
    time_entered: Optional[str] = None
    time_accepted: Optional[str] = None
    time_completed: Optional[str] = None
    acceptance_criteria: Optional[str] = None
    entered_by: Optional[str] = None
    history: List[dict] = field(default_factory=list)

    # --- SDD fields ---
    spec: Optional[str] = None                    # full spec / design notes
    depends_on: List[str] = field(default_factory=list)   # task IDs that must complete first
    blocks: List[str] = field(default_factory=list)       # task IDs waiting on this one

    # --- TDD fields ---
    test_files: List[str] = field(default_factory=list)           # paths to test files
    implementation_files: List[str] = field(default_factory=list) # paths to source files
    tdd_phase: Optional[str] = None               # "red" | "green" | "refactor" | None
    test_command: Optional[str] = None            # e.g. "poetry run pytest tests/test_auth.py"
    verification_command: Optional[str] = None    # full acceptance check

    # --- execution metadata ---
    failure_reason: Optional[str] = None          # top-level reason for last failure
    attempt_count: int = 0                        # number of sub-agent attempts
```

**Rust** (`web/src/models.rs`):

```rust
pub struct Task {
    // --- existing ---
    pub id: String,
    pub status: String,
    pub time_entered: Option<String>,
    pub time_accepted: Option<String>,
    pub time_completed: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub entered_by: Option<String>,
    pub history: Vec<HistoryEntry>,

    // --- SDD fields ---
    pub spec: Option<String>,
    pub depends_on: Vec<String>,
    pub blocks: Vec<String>,

    // --- TDD fields ---
    pub test_files: Vec<String>,
    pub implementation_files: Vec<String>,
    pub tdd_phase: Option<String>,
    pub test_command: Option<String>,
    pub verification_command: Option<String>,

    // --- execution metadata ---
    pub failure_reason: Option<String>,
    pub attempt_count: u32,
}
```

## Field Specifications

### `spec` (Optional[str])

Full specification text, distinct from `acceptance_criteria`. While `acceptance_criteria` is a concise Gherkin-style pass/fail check, `spec` holds richer design context: rationale, constraints, edge cases, API contracts.

### `depends_on` (List[str])

Task IDs that must reach `COMPLETED` before this task can start. Used for topological ordering. Circular dependencies are an error detected at task creation time.

### `blocks` (List[str])

Inverse of `depends_on`. Populated at ingestion time. Used for reporting: "task X blocks 3 other tasks."

### `test_files` (List[str])

Paths (relative to project root) to test files belonging to this task. A resuming sub-agent checks these first to determine TDD phase.

### `implementation_files` (List[str])

Paths to source files this task creates or modifies. Helps scope work and understand blast radius.

### `tdd_phase` (Optional[str])

Tracks where in the TDD cycle this task currently sits:

| Value | Meaning | Next action |
|-------|---------|-------------|
| `None` | Not started or non-code task | Write tests (→ `red`) or execute directly |
| `"red"` | Tests written, expected to fail | Implement code (→ `green`) |
| `"green"` | Tests passing | Refactor (→ `refactor`) or finalize |
| `"refactor"` | Refactoring in progress | Run tests again, finalize |

Stored as a top-level Zenoh key, not buried in history, because the orchestrator needs it for resumability.

### `test_command` (Optional[str])

The exact command to run this task's tests. Defaults to project-level convention if unset.

### `verification_command` (Optional[str])

The full acceptance check, distinct from `test_command`. Tests verify implementation; verification confirms acceptance criteria are met end-to-end. If unset, `test_command` is used for both.

### `failure_reason` (Optional[str])

Top-level field for the last failure reason. Updated whenever status transitions to `PENDING` from a WIP state.

### `attempt_count` (int)

Number of sub-agent attempts. Incremented each time a task is claimed (`→ IN_PROGRESS`).

## Status Lifecycle

```
                 ┌──────────────┐
                 │   PENDING    │◄─────────────────────┐
                 └──────┬───────┘                      │
                        │ claim                        │
                        ▼                              │
                 ┌──────────────┐                      │
           ┌────▶│ IN_PROGRESS  │────┐                 │
           │     └──────────────┘    │                 │
           │           │             │                 │
     re-enter          │             │ fail            │
    (refactor)         │             │                 │
           │           ▼             ▼                 │
           │     ┌──────────┐  ┌──────────┐            │
           └─────│ COMPLETED│  │ PENDING  │────────────┘
                 └──────────┘  └──────────┘
```

TDD phase is orthogonal to task status: a task can be `IN_PROGRESS` with `tdd_phase = "red"` or `IN_PROGRESS` with `tdd_phase = "green"`.

## Zenoh Key Schema

```
projects/<project_id>/tasks/<task_id>/status
projects/<project_id>/tasks/<task_id>/time_entered
projects/<project_id>/tasks/<task_id>/time_accepted
projects/<project_id>/tasks/<task_id>/time_completed
projects/<project_id>/tasks/<task_id>/acceptance_criteria
projects/<project_id>/tasks/<task_id>/entered_by
projects/<project_id>/tasks/<task_id>/history/<iso-timestamp>

# SDD fields
projects/<project_id>/tasks/<task_id>/spec
projects/<project_id>/tasks/<task_id>/depends_on         # JSON array
projects/<project_id>/tasks/<task_id>/blocks              # JSON array

# TDD fields
projects/<project_id>/tasks/<task_id>/test_files          # JSON array
projects/<project_id>/tasks/<task_id>/implementation_files # JSON array
projects/<project_id>/tasks/<task_id>/tdd_phase
projects/<project_id>/tasks/<task_id>/test_command
projects/<project_id>/tasks/<task_id>/verification_command

# Execution metadata
projects/<project_id>/tasks/<task_id>/failure_reason
projects/<project_id>/tasks/<task_id>/attempt_count
```

List fields are stored as JSON arrays in a single key, not sub-keys.

## Backward Compatibility

All new fields are optional. Existing tasks with only the original fields continue to work. No migration needed.
