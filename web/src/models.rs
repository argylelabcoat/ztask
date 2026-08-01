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
    fn history_entry_deserializes_from_json_with_default_note() {
        let entry: HistoryEntry =
            serde_json::from_str(r#"{"timestamp":"t","from_status":"NONE","to_status":"PENDING"}"#).unwrap();
        assert_eq!(entry.timestamp, "t");
        assert_eq!(entry.from_status, "NONE");
        assert_eq!(entry.to_status, "PENDING");
        assert_eq!(entry.note, "");
    }
}
