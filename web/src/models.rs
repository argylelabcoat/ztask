use serde::Deserialize;

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct HistoryEntry {
    pub timestamp: String,
    pub from_status: String,
    pub to_status: String,
    #[serde(default)]
    pub note: String,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    pub id: String,
    pub status: String,
    pub time_entered: Option<String>,
    pub time_accepted: Option<String>,
    pub time_completed: Option<String>,
    pub acceptance_criteria: Option<String>,
    pub entered_by: Option<String>,
    pub history: Vec<HistoryEntry>,
    // SDD fields
    pub spec: Option<String>,
    pub depends_on: Vec<String>,
    pub blocks: Vec<String>,
    // TDD fields
    pub test_files: Vec<String>,
    pub implementation_files: Vec<String>,
    pub tdd_phase: Option<String>,
    pub test_command: Option<String>,
    pub verification_command: Option<String>,
    // Execution metadata
    pub failure_reason: Option<String>,
    pub attempt_count: u32,
}

impl Task {
    pub fn new(id: impl Into<String>) -> Self {
        Task {
            id: id.into(),
            status: "UNKNOWN".to_string(),
            time_entered: None,
            time_accepted: None,
            time_completed: None,
            acceptance_criteria: None,
            entered_by: None,
            history: Vec::new(),
            // SDD fields
            spec: None,
            depends_on: Vec::new(),
            blocks: Vec::new(),
            // TDD fields
            test_files: Vec::new(),
            implementation_files: Vec::new(),
            tdd_phase: None,
            test_command: None,
            verification_command: None,
            // Execution metadata
            failure_reason: None,
            attempt_count: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_task_has_unknown_status_and_empty_history() {
        let task = Task::new("t1");
        assert_eq!(task.id, "t1");
        assert_eq!(task.status, "UNKNOWN");
        assert!(task.time_entered.is_none());
        assert!(task.history.is_empty());
    }

    #[test]
    fn new_task_initializes_sdd_tdd_fields() {
        let task = Task::new("t1");
        assert!(task.spec.is_none());
        assert!(task.depends_on.is_empty());
        assert!(task.blocks.is_empty());
        assert!(task.test_files.is_empty());
        assert!(task.implementation_files.is_empty());
        assert!(task.tdd_phase.is_none());
        assert!(task.test_command.is_none());
        assert!(task.verification_command.is_none());
        assert!(task.failure_reason.is_none());
        assert_eq!(task.attempt_count, 0);
    }

    #[test]
    fn history_entry_deserializes_from_json_with_default_note() {
        let entry: HistoryEntry =
            serde_json::from_str(r#"{"timestamp":"t","from_status":"NONE","to_status":"PENDING"}"#).unwrap();
        assert_eq!(entry.timestamp, "t");
        assert_eq!(entry.from_status, "NONE");
        assert_eq!(entry.to_status, "PENDING");
        assert_eq!(entry.note, "");
    }
}
