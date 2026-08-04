# Delete Column + Per-Project Metrics Dashboard Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the per-task Delete button into its own column, and add a per-project metrics dashboard (`GET /projects/{id}/metrics`) showing a status-breakdown donut chart, a stuck/churning task list, a completions-per-day velocity chart, a status transition-count heatmap, and a per-task timing breakdown.

**Architecture:** `web/src/metrics.rs` is pure computation — functions over an already-fetched `HashMap<String, Task>` (plus thresholds and a fixed `now`), no `ZenohStore` dependency, unit-tested with hand-built fixtures. `web/src/queries.rs` gains `fetch_thresholds` (the one store-touching piece — reads two per-project config keys, falling back to fixed defaults). `web/src/handlers/metrics.rs` wires fetch → compute → render. All charts (donut, velocity bars, transition-matrix heatmap) are computed server-side into ready-to-render values (SVG dasharray/dashoffset, CSS height percentages, CSS background-color strings) so the askama template stays free of arithmetic — no charting library, no JS.

**Tech Stack:** Rust, axum 0.8, askama 0.16, chrono (already a dependency), existing `ZenohStore` trait + `FakeStore` test double.

## Global Constraints

- Delete button moves to its own trailing column in the per-project task table; Update/Save stay grouped in "Actions".
- Threshold config keys are read-only in v1, never written by this feature: `projects/{id}/config/stuck_threshold_hours` (default `2.0`), `projects/{id}/config/churn_transition_count` (default `4`). Missing or unparseable values silently fall back to the default — never an error.
- Status breakdown buckets: `completed` (`status == COMPLETED`), `wip` (`status` in `WIP_STATUSES`), `open` (everything else).
- A task is evaluated for stuck/churning only while its status is non-terminal (`!= COMPLETED`). **Stuck**: hours since its most recent history entry exceeds `stuck_threshold_hours`. **Churning**: its history has at least `churn_transition_count` entries. A task may be both.
- Velocity covers the full project history (earliest history entry through today), one entry per calendar day, zero-filled, counting `to_status == COMPLETED` transitions per day.
- Transition matrix axes are the distinct statuses actually observed in the project's history — built dynamically, never hardcoded to a fixed status list (tolerates `WIP_STATUSES` synonyms like `WIP`/`RUNNING`).
- No charting library, no CDN calls — donut chart is hand-computed SVG, velocity chart is CSS divs, heatmap is a shaded `<table>`. All presentation-ready values (dasharray, height percentages, cell colors) are computed in Rust, not in the askama template.
- `metrics.rs` itself must not depend on `ZenohStore` — keep it pure and directly unit-testable.
- Follow existing patterns: `crate::is_valid_id` for path validation, `HtmlTemplate` for rendering, `FakeStore` + `app(state).oneshot(...)` for handler tests, `#[derive(Debug, Clone, PartialEq)]` on plain data structs (matches `ProjectSummary`'s existing style).

---

### Task 1: Move Delete into its own column

**Files:**
- Modify: `web/templates/task_row.html`
- Modify: `web/templates/project.html`
- Modify: `web/src/handlers/project.rs` (test only)

**Interfaces:** None — template-only change, no Rust signatures affected.

- [ ] **Step 1: Write the failing test**

Add to the `#[cfg(test)] mod tests` block in `web/src/handlers/project.rs`:

```rust
    #[tokio::test]
    async fn create_task_response_has_separate_delete_column() {
        let store = Arc::new(FakeStore::new());
        let state = AppState { store: store.clone() as Arc<dyn crate::zenoh_store::ZenohStore> };

        let response = app(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/projects/p1/tasks")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from("task_id=t1&criteria="))
                    .unwrap(),
            )
            .await
            .unwrap();

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(html.matches("<td>").count(), 6);
    }
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cd web && cargo test --lib handlers::project:: create_task_response_has_separate_delete_column`
Expected: FAIL — the current `task_row.html` renders 5 `<td>` cells (ID, Status, Entered by, Entered, Actions), not 6.

- [ ] **Step 3: Split the Delete button into its own column**

Replace the full contents of `web/templates/task_row.html` with:

```html
<tr id="task-{{ task.id }}">
    <td><a href="/projects/{{ project_id }}/tasks/{{ task.id }}">{{ task.id }}</a></td>
    <td>{{ task.status }}</td>
    <td>{{ task.entered_by.as_deref().unwrap_or("-") }}</td>
    <td>{{ task.time_entered.as_deref().unwrap_or("-") }}</td>
    <td>
        <div style="display:flex; flex-wrap:wrap; gap:0.5rem; align-items:center">
            <form hx-post="/projects/{{ project_id }}/tasks/{{ task.id }}/status"
                  hx-target="#task-{{ task.id }}" hx-swap="outerHTML"
                  style="display:flex; gap:0.25rem; margin:0">
                <select name="status" style="width:auto; margin:0">
                    <option value="PENDING" {% if task.status == "PENDING" %}selected{% endif %}>PENDING</option>
                    <option value="IN_PROGRESS" {% if task.status == "IN_PROGRESS" %}selected{% endif %}>IN_PROGRESS</option>
                    <option value="COMPLETED" {% if task.status == "COMPLETED" %}selected{% endif %}>COMPLETED</option>
                </select>
                <button type="submit" style="width:auto; margin:0">Update</button>
            </form>
            <form hx-post="/projects/{{ project_id }}/tasks/{{ task.id }}/criteria"
                  hx-target="#task-{{ task.id }}" hx-swap="outerHTML"
                  style="display:flex; gap:0.25rem; margin:0">
                <input type="text" name="criteria" value='{{ task.acceptance_criteria.as_deref().unwrap_or("") }}'
                       placeholder="Acceptance criteria" style="width:12rem; margin:0">
                <button type="submit" style="width:auto; margin:0">Save</button>
            </form>
        </div>
    </td>
    <td>
        <button hx-delete="/projects/{{ project_id }}/tasks/{{ task.id }}"
                hx-target="#task-{{ task.id }}" hx-swap="delete"
                hx-confirm="Delete task {{ task.id }}?"
                style="width:auto; margin:0">Delete</button>
    </td>
</tr>
```

In `web/templates/project.html`, update the header row:

```html
        <tr><th>ID</th><th>Status</th><th>Entered by</th><th>Entered</th><th>Actions</th><th>Delete</th></tr>
```

(replaces the existing `<tr><th>ID</th><th>Status</th><th>Entered by</th><th>Entered</th><th>Actions</th></tr>` line)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd web && cargo test --lib`
Expected: all tests pass, including the new one (36+1 pre-existing plus the new test — no other test asserts an exact `<td>` count, so no other test should need updating).

- [ ] **Step 5: Commit**

```bash
git add web/templates/task_row.html web/templates/project.html web/src/handlers/project.rs
git commit -m "feat(web): move Delete into its own column, separate from Update/Save"
```

---

### Task 2: metrics.rs foundations — Thresholds, status breakdown, donut chart

**Files:**
- Create: `web/src/metrics.rs`
- Modify: `web/src/lib.rs` (register the module)

**Interfaces:**
- Produces: `pub struct Thresholds { pub stuck_hours: f64, pub churn_count: usize }` with `impl Default` (`stuck_hours: 2.0, churn_count: 4`), deriving `Debug, Clone, Copy, PartialEq`.
- Produces: `pub struct StatusBreakdown { pub completed: usize, pub wip: usize, pub open: usize }` (`Debug, Clone, Copy, PartialEq`) and `pub fn compute_status_breakdown(tasks: &HashMap<String, Task>) -> StatusBreakdown`.
- Produces: `pub struct DonutSegment { pub label: &'static str, pub color: &'static str, pub count: usize, pub dasharray: String, pub dashoffset: String }` (`Debug, Clone, PartialEq`) and `pub fn compute_donut_segments(breakdown: &StatusBreakdown) -> Vec<DonutSegment>` — empty `Vec` when all counts are zero.
- Produces (private, used by later tasks in this file): `fn parse_timestamp(value: &str) -> Option<chrono::DateTime<chrono::Utc>>` (RFC3339 parse) and `fn format_duration(duration: chrono::Duration) -> String` (`"{d}d {h}h"` if ≥1 day, else `"{h}h {m}m"` if ≥1 hour, else `"{m}m"`).
- Consumes: `crate::models::Task`, `crate::queries::{TERMINAL_STATUS, WIP_STATUSES}` (both already `pub`).

- [ ] **Step 1: Write the failing tests**

Create `web/src/metrics.rs` with just this content first (types + a `#[cfg(test)]` module referencing functions that don't exist yet):

```rust
use std::collections::HashMap;

use crate::models::Task;

#[cfg(test)]
mod tests {
    use super::*;

    fn task_with_status(id: &str, status: &str) -> Task {
        let mut task = Task::new(id);
        task.status = status.to_string();
        task
    }

    #[test]
    fn thresholds_default_matches_documented_values() {
        let thresholds = Thresholds::default();
        assert_eq!(thresholds.stuck_hours, 2.0);
        assert_eq!(thresholds.churn_count, 4);
    }

    #[test]
    fn compute_status_breakdown_counts_each_bucket() {
        let mut tasks = HashMap::new();
        tasks.insert("t1".to_string(), task_with_status("t1", "PENDING"));
        tasks.insert("t2".to_string(), task_with_status("t2", "IN_PROGRESS"));
        tasks.insert("t3".to_string(), task_with_status("t3", "COMPLETED"));
        tasks.insert("t4".to_string(), task_with_status("t4", "WIP"));

        let breakdown = compute_status_breakdown(&tasks);

        assert_eq!(breakdown, StatusBreakdown { completed: 1, wip: 2, open: 1 });
    }

    #[test]
    fn compute_donut_segments_splits_by_bucket() {
        let breakdown = StatusBreakdown { completed: 1, wip: 1, open: 2 };
        let segments = compute_donut_segments(&breakdown);

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0].label, "Completed");
        assert_eq!(segments[0].count, 1);
        assert_eq!(segments[1].label, "WIP");
        assert_eq!(segments[1].count, 1);
        assert_eq!(segments[2].label, "Open");
        assert_eq!(segments[2].count, 2);
    }

    #[test]
    fn compute_donut_segments_empty_when_no_tasks() {
        let breakdown = StatusBreakdown { completed: 0, wip: 0, open: 0 };
        assert!(compute_donut_segments(&breakdown).is_empty());
    }

    #[test]
    fn compute_donut_segments_dasharray_parts_sum_to_circumference() {
        let breakdown = StatusBreakdown { completed: 1, wip: 1, open: 2 };
        let segments = compute_donut_segments(&breakdown);
        let circumference = 2.0 * std::f64::consts::PI * 40.0;

        for seg in &segments {
            let parts: Vec<f64> = seg.dasharray.split_whitespace().map(|p| p.parse().unwrap()).collect();
            assert_eq!(parts.len(), 2);
            assert!((parts[0] + parts[1] - circumference).abs() < 0.01);
        }
    }

    #[test]
    fn parse_timestamp_parses_rfc3339_and_rejects_garbage() {
        assert!(parse_timestamp("2026-08-01T00:00:00+00:00").is_some());
        assert!(parse_timestamp("not-a-date").is_none());
    }

    #[test]
    fn format_duration_formats_minutes_hours_and_days() {
        assert_eq!(format_duration(chrono::Duration::minutes(45)), "45m");
        assert_eq!(format_duration(chrono::Duration::minutes(135)), "2h 15m");
        assert_eq!(format_duration(chrono::Duration::hours(27)), "1d 3h");
    }
}
```

- [ ] **Step 2: Run tests to verify they fail to compile**

Run: `cd web && cargo test --lib metrics::`
Expected: compile errors — `Thresholds`, `StatusBreakdown`, `compute_status_breakdown`, `DonutSegment`, `compute_donut_segments`, `parse_timestamp`, `format_duration` don't exist yet. (You'll also need to register the module in `lib.rs` before this compiles at all — do that now as part of getting to a clean "fails for the right reason" state, then implement.)

In `web/src/lib.rs`, add `pub mod metrics;` next to the other `pub mod` lines (after `pub mod handlers;` is fine, alphabetical isn't enforced elsewhere in this file — just add it to the existing list).

- [ ] **Step 3: Implement**

Add above the `#[cfg(test)]` block in `web/src/metrics.rs`:

```rust
use crate::queries::{TERMINAL_STATUS, WIP_STATUSES};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Thresholds {
    pub stuck_hours: f64,
    pub churn_count: usize,
}

impl Default for Thresholds {
    fn default() -> Self {
        Thresholds { stuck_hours: 2.0, churn_count: 4 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StatusBreakdown {
    pub completed: usize,
    pub wip: usize,
    pub open: usize,
}

pub fn compute_status_breakdown(tasks: &HashMap<String, Task>) -> StatusBreakdown {
    let mut breakdown = StatusBreakdown { completed: 0, wip: 0, open: 0 };
    for task in tasks.values() {
        let status = task.status.to_uppercase();
        if status == TERMINAL_STATUS {
            breakdown.completed += 1;
        } else if WIP_STATUSES.contains(&status.as_str()) {
            breakdown.wip += 1;
        } else {
            breakdown.open += 1;
        }
    }
    breakdown
}

#[derive(Debug, Clone, PartialEq)]
pub struct DonutSegment {
    pub label: &'static str,
    pub color: &'static str,
    pub count: usize,
    pub dasharray: String,
    pub dashoffset: String,
}

const DONUT_RADIUS: f64 = 40.0;
const DONUT_CIRCUMFERENCE: f64 = 2.0 * std::f64::consts::PI * DONUT_RADIUS;

pub fn compute_donut_segments(breakdown: &StatusBreakdown) -> Vec<DonutSegment> {
    let total = breakdown.completed + breakdown.wip + breakdown.open;
    if total == 0 {
        return Vec::new();
    }

    let buckets: [(&str, &str, usize); 3] = [
        ("Completed", "#2e7d32", breakdown.completed),
        ("WIP", "#f9a825", breakdown.wip),
        ("Open", "#757575", breakdown.open),
    ];

    let mut cumulative = 0.0;
    let mut segments = Vec::new();
    for (label, color, count) in buckets {
        let length = DONUT_CIRCUMFERENCE * (count as f64 / total as f64);
        segments.push(DonutSegment {
            label,
            color,
            count,
            dasharray: format!("{:.3} {:.3}", length, DONUT_CIRCUMFERENCE - length),
            dashoffset: format!("{:.3}", -cumulative),
        });
        cumulative += length;
    }
    segments
}

fn parse_timestamp(value: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(value).ok().map(|dt| dt.with_timezone(&chrono::Utc))
}

fn format_duration(duration: chrono::Duration) -> String {
    let total_minutes = duration.num_minutes().max(0);
    let days = total_minutes / (60 * 24);
    let hours = (total_minutes % (60 * 24)) / 60;
    let minutes = total_minutes % 60;
    if days > 0 {
        format!("{days}d {hours}h")
    } else if hours > 0 {
        format!("{hours}h {minutes}m")
    } else {
        format!("{minutes}m")
    }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd web && cargo test --lib metrics::`
Expected: all pass.

- [ ] **Step 5: Run the full suite to check for regressions**

Run: `cd web && cargo test --lib`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add web/src/metrics.rs web/src/lib.rs
git commit -m "feat(web): add metrics.rs foundations — thresholds, status breakdown, donut chart"
```

---

### Task 3: metrics.rs — per-task timing table (stuck/churning flags, durations)

**Files:**
- Modify: `web/src/metrics.rs`

**Interfaces:**
- Consumes: `Thresholds`, `parse_timestamp`, `format_duration` from Task 2 (same file). `crate::queries::{TERMINAL_STATUS, WIP_STATUSES}` already imported.
- Produces: `pub struct TaskTiming { pub id: String, pub status: String, pub queued_duration: Option<String>, pub work_duration: Option<String>, pub current_status_duration: String, pub transition_count: usize, pub stuck: bool, pub churning: bool }` (`Debug, Clone, PartialEq`) and `pub fn compute_timing_table(tasks: &HashMap<String, Task>, thresholds: &Thresholds, now: chrono::DateTime<chrono::Utc>) -> Vec<TaskTiming>` — sorted by `id` ascending. Task 7 depends on this exact signature and on `TaskTiming` being `Clone` (the handler filters a stuck/churning subset via `.cloned()`).

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `web/src/metrics.rs` (add `use crate::models::HistoryEntry;` to the module's existing `use super::*;` — `HistoryEntry` isn't imported yet at the top of the file, so add it inside the test module):

```rust
    use crate::models::HistoryEntry;

    fn history_entry(timestamp: &str, from: &str, to: &str) -> HistoryEntry {
        HistoryEntry { timestamp: timestamp.to_string(), from_status: from.to_string(), to_status: to.to_string(), note: String::new() }
    }

    #[test]
    fn compute_timing_table_computes_queued_and_work_duration_for_completed_task() {
        let mut task = Task::new("t1");
        task.status = "COMPLETED".to_string();
        task.time_entered = Some("2026-08-01T00:00:00+00:00".to_string());
        task.time_accepted = Some("2026-08-01T01:00:00+00:00".to_string());
        task.time_completed = Some("2026-08-01T04:00:00+00:00".to_string());
        task.history = vec![history_entry("2026-08-01T04:00:00+00:00", "IN_PROGRESS", "COMPLETED")];

        let mut tasks = HashMap::new();
        tasks.insert("t1".to_string(), task);

        let now = parse_timestamp("2026-08-01T05:00:00+00:00").unwrap();
        let timings = compute_timing_table(&tasks, &Thresholds::default(), now);

        assert_eq!(timings.len(), 1);
        assert_eq!(timings[0].queued_duration.as_deref(), Some("1h 0m"));
        assert_eq!(timings[0].work_duration.as_deref(), Some("3h 0m"));
        assert!(!timings[0].stuck);
        assert!(!timings[0].churning);
    }

    #[test]
    fn compute_timing_table_uses_now_for_open_task_work_duration() {
        let mut task = Task::new("t1");
        task.status = "IN_PROGRESS".to_string();
        task.time_entered = Some("2026-08-01T00:00:00+00:00".to_string());
        task.time_accepted = Some("2026-08-01T01:00:00+00:00".to_string());
        task.history = vec![history_entry("2026-08-01T01:00:00+00:00", "PENDING", "IN_PROGRESS")];

        let mut tasks = HashMap::new();
        tasks.insert("t1".to_string(), task);

        let now = parse_timestamp("2026-08-01T04:00:00+00:00").unwrap();
        let timings = compute_timing_table(&tasks, &Thresholds::default(), now);

        assert_eq!(timings[0].work_duration.as_deref(), Some("3h 0m"));
    }

    #[test]
    fn compute_timing_table_flags_stuck_when_over_threshold() {
        let mut task = Task::new("t1");
        task.status = "IN_PROGRESS".to_string();
        task.history = vec![history_entry("2026-08-01T00:00:00+00:00", "PENDING", "IN_PROGRESS")];

        let mut tasks = HashMap::new();
        tasks.insert("t1".to_string(), task);

        let now = parse_timestamp("2026-08-01T03:00:00+00:00").unwrap();
        let thresholds = Thresholds { stuck_hours: 2.0, churn_count: 100 };
        let timings = compute_timing_table(&tasks, &thresholds, now);

        assert!(timings[0].stuck);
        assert!(!timings[0].churning);
    }

    #[test]
    fn compute_timing_table_flags_churning_when_transition_count_meets_threshold() {
        let mut task = Task::new("t1");
        task.status = "IN_PROGRESS".to_string();
        task.history = vec![
            history_entry("2026-08-01T00:00:00+00:00", "NONE", "PENDING"),
            history_entry("2026-08-01T00:10:00+00:00", "PENDING", "IN_PROGRESS"),
            history_entry("2026-08-01T00:20:00+00:00", "IN_PROGRESS", "PENDING"),
            history_entry("2026-08-01T00:30:00+00:00", "PENDING", "IN_PROGRESS"),
        ];

        let mut tasks = HashMap::new();
        tasks.insert("t1".to_string(), task);

        let now = parse_timestamp("2026-08-01T00:31:00+00:00").unwrap();
        let thresholds = Thresholds { stuck_hours: 100.0, churn_count: 4 };
        let timings = compute_timing_table(&tasks, &thresholds, now);

        assert!(timings[0].churning);
        assert!(!timings[0].stuck);
    }

    #[test]
    fn compute_timing_table_never_flags_completed_tasks() {
        let mut task = Task::new("t1");
        task.status = "COMPLETED".to_string();
        task.history = vec![
            history_entry("2026-08-01T00:00:00+00:00", "NONE", "PENDING"),
            history_entry("2026-08-01T00:01:00+00:00", "PENDING", "IN_PROGRESS"),
            history_entry("2026-08-01T00:02:00+00:00", "IN_PROGRESS", "PENDING"),
            history_entry("2026-08-01T00:03:00+00:00", "PENDING", "COMPLETED"),
        ];

        let mut tasks = HashMap::new();
        tasks.insert("t1".to_string(), task);

        let now = parse_timestamp("2030-01-01T00:00:00+00:00").unwrap();
        let thresholds = Thresholds { stuck_hours: 1.0, churn_count: 4 };
        let timings = compute_timing_table(&tasks, &thresholds, now);

        assert!(!timings[0].stuck);
        assert!(!timings[0].churning);
    }

    #[test]
    fn compute_timing_table_sorts_by_id() {
        let mut tasks = HashMap::new();
        tasks.insert("b".to_string(), Task::new("b"));
        tasks.insert("a".to_string(), Task::new("a"));

        let now = parse_timestamp("2026-08-01T00:00:00+00:00").unwrap();
        let timings = compute_timing_table(&tasks, &Thresholds::default(), now);

        assert_eq!(timings.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(), vec!["a", "b"]);
    }
```

- [ ] **Step 2: Run tests to verify they fail to compile**

Run: `cd web && cargo test --lib metrics::`
Expected: compile error — `TaskTiming` and `compute_timing_table` don't exist yet.

- [ ] **Step 3: Implement**

Add above the `#[cfg(test)]` block in `web/src/metrics.rs` (after the `format_duration` function):

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct TaskTiming {
    pub id: String,
    pub status: String,
    pub queued_duration: Option<String>,
    pub work_duration: Option<String>,
    pub current_status_duration: String,
    pub transition_count: usize,
    pub stuck: bool,
    pub churning: bool,
}

pub fn compute_timing_table(
    tasks: &HashMap<String, Task>,
    thresholds: &Thresholds,
    now: chrono::DateTime<chrono::Utc>,
) -> Vec<TaskTiming> {
    let mut result: Vec<TaskTiming> = tasks
        .values()
        .map(|task| {
            let status = task.status.to_uppercase();
            let is_terminal = status == TERMINAL_STATUS;

            let entered = task.time_entered.as_deref().and_then(parse_timestamp);
            let accepted = task.time_accepted.as_deref().and_then(parse_timestamp);
            let completed = task.time_completed.as_deref().and_then(parse_timestamp);

            let queued_duration = match (entered, accepted) {
                (Some(e), Some(a)) => Some(format_duration(a - e)),
                _ => None,
            };

            let work_duration = match (accepted, completed, is_terminal) {
                (Some(a), Some(c), true) => Some(format_duration(c - a)),
                (Some(a), None, false) => Some(format_duration(now - a)),
                _ => None,
            };

            let last_change = task.history.iter().filter_map(|h| parse_timestamp(&h.timestamp)).max();
            let current_status_duration = match last_change {
                Some(t) => format_duration(now - t),
                None => "-".to_string(),
            };

            let transition_count = task.history.len();

            let hours_since_change = last_change.map(|t| (now - t).num_minutes() as f64 / 60.0);
            let stuck = !is_terminal && hours_since_change.map(|h| h > thresholds.stuck_hours).unwrap_or(false);
            let churning = !is_terminal && transition_count >= thresholds.churn_count;

            TaskTiming {
                id: task.id.clone(),
                status: task.status.clone(),
                queued_duration,
                work_duration,
                current_status_duration,
                transition_count,
                stuck,
                churning,
            }
        })
        .collect();

    result.sort_by(|a, b| a.id.cmp(&b.id));
    result
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd web && cargo test --lib metrics::`
Expected: all pass.

- [ ] **Step 5: Run the full suite to check for regressions**

Run: `cd web && cargo test --lib`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add web/src/metrics.rs
git commit -m "feat(web): compute per-task timing breakdown with stuck/churning flags"
```

---

### Task 4: metrics.rs — velocity (completions per day)

**Files:**
- Modify: `web/src/metrics.rs`

**Interfaces:**
- Consumes: `parse_timestamp` from Task 2, `TERMINAL_STATUS` (already imported).
- Produces: `pub struct VelocityPoint { pub date: String, pub completions: usize, pub height_pct: u32 }` (`Debug, Clone, PartialEq`) and `pub fn compute_velocity(tasks: &HashMap<String, Task>) -> Vec<VelocityPoint>` — `date` formatted `"%Y-%m-%d"`, one entry per calendar day from the earliest history timestamp across all tasks through today (inclusive), zero-filled, ordered chronologically. Empty `Vec` if no task has any history. `height_pct` is each day's completion count scaled to a 0–100 percentage of the maximum single-day count in the series (0 if the series has no completions at all). Task 7 depends on this exact signature.

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `web/src/metrics.rs`:

```rust
    #[test]
    fn compute_velocity_counts_completions_per_day_and_zero_fills() {
        let mut task = Task::new("t1");
        task.history = vec![
            history_entry("2026-08-01T00:00:00+00:00", "NONE", "PENDING"),
            history_entry("2026-08-03T00:00:00+00:00", "IN_PROGRESS", "COMPLETED"),
        ];

        let mut tasks = HashMap::new();
        tasks.insert("t1".to_string(), task);

        // "today" is real Utc::now() inside compute_velocity, so this only checks
        // the deterministic prefix of the series (from the earliest history entry
        // through the fixture's last entry) — later, real-time-dependent entries
        // aren't asserted.
        let velocity = compute_velocity(&tasks);

        assert!(velocity.len() >= 3);
        assert_eq!(velocity[0].date, "2026-08-01");
        assert_eq!(velocity[0].completions, 0);
        assert_eq!(velocity[0].height_pct, 0);
        assert_eq!(velocity[1].date, "2026-08-02");
        assert_eq!(velocity[1].completions, 0);
        assert_eq!(velocity[2].date, "2026-08-03");
        assert_eq!(velocity[2].completions, 1);
        assert_eq!(velocity[2].height_pct, 100);
    }

    #[test]
    fn compute_velocity_empty_when_no_history() {
        let tasks: HashMap<String, Task> = HashMap::new();
        assert!(compute_velocity(&tasks).is_empty());
    }
```

- [ ] **Step 2: Run tests to verify they fail to compile**

Run: `cd web && cargo test --lib metrics::`
Expected: compile error — `VelocityPoint` and `compute_velocity` don't exist yet.

- [ ] **Step 3: Implement**

Add above the `#[cfg(test)]` block in `web/src/metrics.rs` (after `compute_timing_table`):

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct VelocityPoint {
    pub date: String,
    pub completions: usize,
    pub height_pct: u32,
}

pub fn compute_velocity(tasks: &HashMap<String, Task>) -> Vec<VelocityPoint> {
    let mut earliest: Option<chrono::NaiveDate> = None;
    let mut completions_by_date: HashMap<chrono::NaiveDate, usize> = HashMap::new();

    for task in tasks.values() {
        for entry in &task.history {
            let Some(dt) = parse_timestamp(&entry.timestamp) else { continue };
            let date = dt.date_naive();
            earliest = Some(earliest.map_or(date, |e| e.min(date)));
            if entry.to_status.to_uppercase() == TERMINAL_STATUS {
                *completions_by_date.entry(date).or_insert(0) += 1;
            }
        }
    }

    let Some(start) = earliest else { return Vec::new() };
    let today = chrono::Utc::now().date_naive();

    let mut raw: Vec<(String, usize)> = Vec::new();
    let mut day = start;
    while day <= today {
        raw.push((day.format("%Y-%m-%d").to_string(), completions_by_date.get(&day).copied().unwrap_or(0)));
        day += chrono::Duration::days(1);
    }

    let max = raw.iter().map(|(_, c)| *c).max().unwrap_or(0);
    raw.into_iter()
        .map(|(date, completions)| {
            let height_pct = if max == 0 { 0 } else { ((completions as f64 / max as f64) * 100.0).round() as u32 };
            VelocityPoint { date, completions, height_pct }
        })
        .collect()
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd web && cargo test --lib metrics::`
Expected: all pass.

- [ ] **Step 5: Run the full suite to check for regressions**

Run: `cd web && cargo test --lib`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add web/src/metrics.rs
git commit -m "feat(web): compute daily completion velocity"
```

---

### Task 5: metrics.rs — transition matrix (heatmap)

**Files:**
- Modify: `web/src/metrics.rs`

**Interfaces:**
- Produces: `pub struct TransitionMatrix { pub statuses: Vec<String>, pub counts: Vec<Vec<usize>>, pub cell_styles: Vec<Vec<String>> }` (`Debug, Clone, PartialEq`) and `pub fn compute_transition_matrix(tasks: &HashMap<String, Task>) -> TransitionMatrix`. `statuses` is the sorted (alphabetical), deduplicated set of every `from_status`/`to_status` seen across all tasks' history. `counts[i][j]` is the number of `statuses[i] → statuses[j]` transitions. `cell_styles[i][j]` is a ready-to-use CSS `style` attribute value: `"background-color: transparent"` when `counts[i][j] == 0`, otherwise `"background-color: rgba(21, 101, 192, {alpha:.2})"` where `alpha = 0.15 + 0.65 * (count / max_count_in_matrix)`. Task 7 depends on this exact signature and field layout.

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `web/src/metrics.rs`:

```rust
    #[test]
    fn compute_transition_matrix_counts_transitions_across_tasks() {
        let mut t1 = Task::new("t1");
        t1.history = vec![
            history_entry("2026-08-01T00:00:00+00:00", "NONE", "PENDING"),
            history_entry("2026-08-01T00:10:00+00:00", "PENDING", "IN_PROGRESS"),
            history_entry("2026-08-01T00:20:00+00:00", "IN_PROGRESS", "PENDING"),
        ];
        let mut t2 = Task::new("t2");
        t2.history = vec![history_entry("2026-08-01T00:00:00+00:00", "PENDING", "IN_PROGRESS")];

        let mut tasks = HashMap::new();
        tasks.insert("t1".to_string(), t1);
        tasks.insert("t2".to_string(), t2);

        let matrix = compute_transition_matrix(&tasks);

        assert_eq!(matrix.statuses, vec!["IN_PROGRESS".to_string(), "NONE".to_string(), "PENDING".to_string()]);
        let idx = |s: &str| matrix.statuses.iter().position(|x| x == s).unwrap();
        assert_eq!(matrix.counts[idx("NONE")][idx("PENDING")], 1);
        assert_eq!(matrix.counts[idx("PENDING")][idx("IN_PROGRESS")], 2);
        assert_eq!(matrix.counts[idx("IN_PROGRESS")][idx("PENDING")], 1);

        assert_eq!(matrix.cell_styles.len(), matrix.statuses.len());
        assert_eq!(matrix.cell_styles[idx("NONE")][idx("NONE")], "background-color: transparent");
        assert!(matrix.cell_styles[idx("PENDING")][idx("IN_PROGRESS")].starts_with("background-color: rgba"));
    }

    #[test]
    fn compute_transition_matrix_empty_when_no_tasks() {
        let tasks: HashMap<String, Task> = HashMap::new();
        let matrix = compute_transition_matrix(&tasks);
        assert!(matrix.statuses.is_empty());
        assert!(matrix.counts.is_empty());
        assert!(matrix.cell_styles.is_empty());
    }
```

- [ ] **Step 2: Run tests to verify they fail to compile**

Run: `cd web && cargo test --lib metrics::`
Expected: compile error — `TransitionMatrix` and `compute_transition_matrix` don't exist yet.

- [ ] **Step 3: Implement**

Add above the `#[cfg(test)]` block in `web/src/metrics.rs` (after `compute_velocity`; add `use std::collections::BTreeSet;` to the file's top-level `use` statements):

```rust
#[derive(Debug, Clone, PartialEq)]
pub struct TransitionMatrix {
    pub statuses: Vec<String>,
    pub counts: Vec<Vec<usize>>,
    pub cell_styles: Vec<Vec<String>>,
}

pub fn compute_transition_matrix(tasks: &HashMap<String, Task>) -> TransitionMatrix {
    let mut status_set: BTreeSet<String> = BTreeSet::new();
    for task in tasks.values() {
        for entry in &task.history {
            status_set.insert(entry.from_status.clone());
            status_set.insert(entry.to_status.clone());
        }
    }
    let statuses: Vec<String> = status_set.into_iter().collect();
    let index: HashMap<&str, usize> = statuses.iter().enumerate().map(|(i, s)| (s.as_str(), i)).collect();

    let mut counts = vec![vec![0usize; statuses.len()]; statuses.len()];
    for task in tasks.values() {
        for entry in &task.history {
            if let (Some(&from_idx), Some(&to_idx)) =
                (index.get(entry.from_status.as_str()), index.get(entry.to_status.as_str()))
            {
                counts[from_idx][to_idx] += 1;
            }
        }
    }

    let max = counts.iter().flatten().copied().max().unwrap_or(0);
    let cell_styles: Vec<Vec<String>> = counts
        .iter()
        .map(|row| {
            row.iter()
                .map(|&count| {
                    if count == 0 || max == 0 {
                        "background-color: transparent".to_string()
                    } else {
                        let alpha = 0.15 + 0.65 * (count as f64 / max as f64);
                        format!("background-color: rgba(21, 101, 192, {alpha:.2})")
                    }
                })
                .collect()
        })
        .collect();

    TransitionMatrix { statuses, counts, cell_styles }
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd web && cargo test --lib metrics::`
Expected: all pass.

- [ ] **Step 5: Run the full suite to check for regressions**

Run: `cd web && cargo test --lib`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add web/src/metrics.rs
git commit -m "feat(web): compute status transition matrix for the churn heatmap"
```

---

### Task 6: `queries::fetch_thresholds`

**Files:**
- Modify: `web/src/queries.rs`

**Interfaces:**
- Consumes: `crate::metrics::Thresholds` (from Task 2), `ZenohStore::get` (existing trait method).
- Produces: `pub async fn fetch_thresholds(store: &dyn ZenohStore, project_id: &str) -> crate::metrics::Thresholds` — reads `projects/{project_id}/config/stuck_threshold_hours` (parsed as `f64`) and `projects/{project_id}/config/churn_transition_count` (parsed as `usize`); each independently falls back to `Thresholds::default()`'s corresponding field if the key is missing or its value fails to parse. Task 7 depends on this exact signature.

- [ ] **Step 1: Write the failing tests**

Add to the `#[cfg(test)] mod tests` block in `web/src/queries.rs`:

```rust
    #[tokio::test]
    async fn fetch_thresholds_returns_defaults_when_keys_missing() {
        let store = FakeStore::new();
        let thresholds = fetch_thresholds(&store, "p1").await;
        assert_eq!(thresholds, crate::metrics::Thresholds::default());
    }

    #[tokio::test]
    async fn fetch_thresholds_reads_seeded_values() {
        let store = FakeStore::new()
            .seed("projects/p1/config/stuck_threshold_hours", "6")
            .seed("projects/p1/config/churn_transition_count", "10");
        let thresholds = fetch_thresholds(&store, "p1").await;
        assert_eq!(thresholds.stuck_hours, 6.0);
        assert_eq!(thresholds.churn_count, 10);
    }

    #[tokio::test]
    async fn fetch_thresholds_falls_back_on_unparseable_values() {
        let store = FakeStore::new().seed("projects/p1/config/stuck_threshold_hours", "not-a-number");
        let thresholds = fetch_thresholds(&store, "p1").await;
        assert_eq!(thresholds.stuck_hours, crate::metrics::Thresholds::default().stuck_hours);
    }
```

- [ ] **Step 2: Run tests to verify they fail to compile**

Run: `cd web && cargo test --lib queries::`
Expected: compile error — `fetch_thresholds` doesn't exist yet.

- [ ] **Step 3: Implement**

Add to `web/src/queries.rs` (anywhere below `fetch_all_projects`, above `#[cfg(test)]`):

```rust
pub async fn fetch_thresholds(store: &dyn ZenohStore, project_id: &str) -> crate::metrics::Thresholds {
    let mut thresholds = crate::metrics::Thresholds::default();

    let stuck_key = format!("projects/{project_id}/config/stuck_threshold_hours");
    if let Some((_, value)) = store.get(&stuck_key).await.into_iter().next() {
        if let Ok(parsed) = value.parse::<f64>() {
            thresholds.stuck_hours = parsed;
        }
    }

    let churn_key = format!("projects/{project_id}/config/churn_transition_count");
    if let Some((_, value)) = store.get(&churn_key).await.into_iter().next() {
        if let Ok(parsed) = value.parse::<usize>() {
            thresholds.churn_count = parsed;
        }
    }

    thresholds
}
```

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd web && cargo test --lib queries::`
Expected: all pass.

- [ ] **Step 5: Run the full suite to check for regressions**

Run: `cd web && cargo test --lib`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add web/src/queries.rs
git commit -m "feat(web): read per-project stuck/churn thresholds with defaults"
```

---

### Task 7: Wire the metrics page (handler, template, route, nav link)

**Files:**
- Create: `web/src/handlers/metrics.rs`
- Create: `web/templates/metrics.html`
- Modify: `web/src/handlers/mod.rs`
- Modify: `web/src/lib.rs`
- Modify: `web/templates/project.html`

**Interfaces:**
- Consumes: `queries::fetch_all_tasks`, `queries::fetch_thresholds` (Task 6); `metrics::{compute_status_breakdown, compute_donut_segments, compute_timing_table, compute_velocity, compute_transition_matrix, DonutSegment, VelocityPoint, TransitionMatrix, TaskTiming}` (Tasks 2–5); `crate::is_valid_id`; `crate::render::HtmlTemplate`.
- Produces: `pub async fn show(State(state): State<AppState>, Path(project_id): Path<String>) -> Result<HtmlTemplate<MetricsTemplate>, StatusCode>` in `web/src/handlers/metrics.rs`, registered as `GET /projects/{id}/metrics` in `web/src/lib.rs`.

- [ ] **Step 1: Write the failing tests**

Create `web/src/handlers/metrics.rs` with just the template struct and a `#[cfg(test)]` module (no `show` function yet):

```rust
use askama::Template;
use axum::extract::{Path, State};
use axum::http::StatusCode;

use crate::render::HtmlTemplate;
use crate::{metrics, queries, AppState};

#[derive(Template)]
#[template(path = "metrics.html")]
pub struct MetricsTemplate {
    pub project_id: String,
    pub donut_segments: Vec<metrics::DonutSegment>,
    pub total_tasks: usize,
    pub velocity: Vec<metrics::VelocityPoint>,
    pub matrix: metrics::TransitionMatrix,
    pub timings: Vec<metrics::TaskTiming>,
    pub flagged: Vec<metrics::TaskTiming>,
}

#[cfg(test)]
mod tests {
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use http_body_util::BodyExt;
    use std::sync::Arc;
    use tower::ServiceExt;

    use crate::zenoh_store::fake::FakeStore;
    use crate::{app, AppState};

    #[tokio::test]
    async fn metrics_page_renders_for_project_with_tasks() {
        let store = FakeStore::new()
            .seed("projects/p1/tasks/t1/status", "COMPLETED")
            .seed(
                "projects/p1/tasks/t1/history/2026-08-01T00-00-00",
                r#"{"timestamp":"2026-08-01T00:00:00+00:00","from_status":"NONE","to_status":"PENDING","note":""}"#,
            )
            .seed(
                "projects/p1/tasks/t1/history/2026-08-01T01-00-00",
                r#"{"timestamp":"2026-08-01T01:00:00+00:00","from_status":"PENDING","to_status":"COMPLETED","note":""}"#,
            );
        let state = AppState { store: Arc::new(store) };

        let response = app(state)
            .oneshot(Request::builder().uri("/projects/p1/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains("t1"));
        assert!(html.contains("Completed"));
    }

    #[tokio::test]
    async fn metrics_page_rejects_invalid_project_id() {
        let store = FakeStore::new();
        let state = AppState { store: Arc::new(store) };

        let response = app(state)
            .oneshot(Request::builder().uri("/projects/*/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn metrics_page_handles_project_with_no_tasks() {
        let store = FakeStore::new();
        let state = AppState { store: Arc::new(store) };

        let response = app(state)
            .oneshot(Request::builder().uri("/projects/empty/metrics").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }
}
```

Register the module and a placeholder route so the crate compiles enough to run the tests (and fail for the right reason): in `web/src/handlers/mod.rs`, add `pub mod metrics;` to the existing list. In `web/src/lib.rs`, add this route to the `Router::new()` chain (next to the other `/projects/{id}/...` routes):

```rust
        .route("/projects/{id}/metrics", get(handlers::metrics::show))
```

- [ ] **Step 2: Run tests to verify they fail to compile**

Run: `cd web && cargo test --lib handlers::metrics::`
Expected: compile error — `handlers::metrics::show` doesn't exist yet (and `web/templates/metrics.html` doesn't exist yet either, which would separately fail the `#[derive(Template)]` macro once `show` is added — both are fixed together in Step 3).

- [ ] **Step 3: Implement**

Add to `web/src/handlers/metrics.rs` (below `MetricsTemplate`, above `#[cfg(test)]`):

```rust
pub async fn show(
    State(state): State<AppState>,
    Path(project_id): Path<String>,
) -> Result<HtmlTemplate<MetricsTemplate>, StatusCode> {
    if !crate::is_valid_id(&project_id) {
        return Err(StatusCode::BAD_REQUEST);
    }

    let tasks = queries::fetch_all_tasks(state.store.as_ref(), &project_id).await;
    let thresholds = queries::fetch_thresholds(state.store.as_ref(), &project_id).await;
    let now = chrono::Utc::now();

    let breakdown = metrics::compute_status_breakdown(&tasks);
    let donut_segments = metrics::compute_donut_segments(&breakdown);
    let velocity = metrics::compute_velocity(&tasks);
    let matrix = metrics::compute_transition_matrix(&tasks);
    let timings = metrics::compute_timing_table(&tasks, &thresholds, now);
    let flagged: Vec<metrics::TaskTiming> = timings.iter().filter(|t| t.stuck || t.churning).cloned().collect();

    Ok(HtmlTemplate(MetricsTemplate {
        project_id,
        donut_segments,
        total_tasks: tasks.len(),
        velocity,
        matrix,
        timings,
        flagged,
    }))
}
```

Create `web/templates/metrics.html`:

```html
{% extends "base.html" %}
{% block title %}{{ project_id }} metrics — Zenoh Tasks{% endblock %}
{% block content %}
<h1>{{ project_id }} — Metrics</h1>

<nav>
    <ul>
        <li><a href="/projects/{{ project_id }}">Back to tasks</a></li>
    </ul>
</nav>

<h2>Status breakdown</h2>
{% if donut_segments.is_empty() %}
<p>No tasks yet.</p>
{% else %}
<div style="display:flex; align-items:center; gap:1.5rem">
    <svg viewBox="0 0 100 100" width="160" height="160">
        <circle cx="50" cy="50" r="40" fill="none" stroke="#e0e0e0" stroke-width="16" />
        {% for seg in donut_segments %}
        <circle cx="50" cy="50" r="40" fill="none" stroke="{{ seg.color }}" stroke-width="16"
                stroke-dasharray="{{ seg.dasharray }}" stroke-dashoffset="{{ seg.dashoffset }}"
                transform="rotate(-90 50 50)" />
        {% endfor %}
    </svg>
    <ul>
        {% for seg in donut_segments %}
        <li><span style="color:{{ seg.color }}">&#9679;</span> {{ seg.label }}: {{ seg.count }}</li>
        {% endfor %}
    </ul>
</div>
{% endif %}

<h2>Stuck / churning tasks</h2>
{% if flagged.is_empty() %}
<p>None flagged.</p>
{% else %}
<table>
    <thead><tr><th>ID</th><th>Status</th><th>Stuck</th><th>Churning</th><th>Current status duration</th><th>Transitions</th></tr></thead>
    <tbody>
        {% for t in flagged %}
        <tr>
            <td><a href="/projects/{{ project_id }}/tasks/{{ t.id }}">{{ t.id }}</a></td>
            <td>{{ t.status }}</td>
            <td>{% if t.stuck %}yes{% else %}-{% endif %}</td>
            <td>{% if t.churning %}yes{% else %}-{% endif %}</td>
            <td>{{ t.current_status_duration }}</td>
            <td>{{ t.transition_count }}</td>
        </tr>
        {% endfor %}
    </tbody>
</table>
{% endif %}

<h2>Velocity (completions per day)</h2>
{% if velocity.is_empty() %}
<p>No history yet.</p>
{% else %}
<div style="display:flex; align-items:flex-end; gap:2px; overflow-x:auto; height:120px; border:1px solid #ddd; padding:0.5rem">
    {% for point in velocity %}
    <div title="{{ point.date }}: {{ point.completions }}" style="width:10px; flex-shrink:0; background:#1565c0; height:{{ point.height_pct }}%"></div>
    {% endfor %}
</div>
{% endif %}

<h2>Transition matrix</h2>
{% if matrix.statuses.is_empty() %}
<p>No history yet.</p>
{% else %}
<table>
    <thead>
        <tr>
            <th>from \ to</th>
            {% for status in matrix.statuses %}
            <th>{{ status }}</th>
            {% endfor %}
        </tr>
    </thead>
    <tbody>
        {% for (row_idx, row) in matrix.counts.iter().enumerate() %}
        <tr>
            <th>{{ matrix.statuses[row_idx] }}</th>
            {% for (col_idx, count) in row.iter().enumerate() %}
            <td style="{{ matrix.cell_styles[row_idx][col_idx] }}">{{ count }}</td>
            {% endfor %}
        </tr>
        {% endfor %}
    </tbody>
</table>
{% endif %}

<h2>Task timing</h2>
{% if timings.is_empty() %}
<p>No tasks yet.</p>
{% else %}
<table>
    <thead><tr><th>ID</th><th>Status</th><th>Queued</th><th>Work</th><th>Current status</th><th>Transitions</th></tr></thead>
    <tbody>
        {% for t in timings %}
        <tr>
            <td><a href="/projects/{{ project_id }}/tasks/{{ t.id }}">{{ t.id }}</a></td>
            <td>{{ t.status }}</td>
            <td>{{ t.queued_duration.as_deref().unwrap_or("-") }}</td>
            <td>{{ t.work_duration.as_deref().unwrap_or("-") }}</td>
            <td>{{ t.current_status_duration }}</td>
            <td>{{ t.transition_count }}</td>
        </tr>
        {% endfor %}
    </tbody>
</table>
{% endif %}
{% endblock %}
```

In `web/templates/project.html`, add a "Metrics" link to the nav:

```html
<nav>
    <ul>
        <li><a href="/projects/{{ project_id }}?filter=all">All</a></li>
        <li><a href="/projects/{{ project_id }}?filter=incomplete">Incomplete</a></li>
        <li><a href="/projects/{{ project_id }}?filter=wip">WIP</a></li>
        <li><a href="/projects/{{ project_id }}/metrics">Metrics</a></li>
    </ul>
</nav>
```

(replaces the existing 3-item nav list)

- [ ] **Step 4: Run tests to verify they pass**

Run: `cd web && cargo test --lib handlers::metrics::`
Expected: all pass.

- [ ] **Step 5: Run the full suite to check for regressions**

Run: `cd web && cargo test --lib`
Expected: all pass.

- [ ] **Step 6: Commit**

```bash
git add web/src/handlers/metrics.rs web/templates/metrics.html web/src/handlers/mod.rs web/src/lib.rs web/templates/project.html
git commit -m "feat(web): add per-project metrics dashboard (GET /projects/{id}/metrics)"
```

---

### Task 8: Manual verification against a real router

**Files:** none (verification only)

- [ ] **Step 1: Build and run the stack**

Run: `./scripts/up.sh` (set `ZTASK_CONTAINER_RUNTIME=container` if using Apple's `container` CLI instead of Docker)

- [ ] **Step 2: Seed a project with varied task states**

Create a project with several tasks in different statuses (PENDING, IN_PROGRESS, COMPLETED), and update at least one task's status multiple times (e.g. IN_PROGRESS → PENDING → IN_PROGRESS) to produce churn signal and transition-matrix data.

- [ ] **Step 3: Verify the delete column**

Open the project page. Confirm Delete renders in its own column, separate from the Update/Save controls, and still works (deletes the row on confirm).

- [ ] **Step 4: Verify the metrics page**

Click "Metrics" from the project nav. Confirm:
- The donut chart renders and its legend counts match the project's actual status breakdown
- Any task with several rapid status changes appears in the stuck/churning list
- The velocity chart shows at least one day with a bar (if any task was completed)
- The transition matrix shows nonzero cells matching the status changes made in Step 2
- The timing table lists every task with sensible queued/work/current-status durations

- [ ] **Step 5: Verify an empty project doesn't error**

Create a brand-new project (one task, no status changes) and visit its metrics page — confirm it renders without error (donut/velocity/matrix sections show their "no data yet" fallback text as appropriate).

- [ ] **Step 6: Clean up**

Run: `container rm -f zenoh-router ztask-web` (or `docker rm -f zenoh-router ztask-web`)

No commit for this task — it's manual verification only.
