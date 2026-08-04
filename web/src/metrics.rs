use std::collections::{BTreeSet, HashMap};

use crate::models::Task;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::HistoryEntry;

    fn task_with_status(id: &str, status: &str) -> Task {
        let mut task = Task::new(id);
        task.status = status.to_string();
        task
    }

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
}
