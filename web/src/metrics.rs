use std::collections::HashMap;

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
