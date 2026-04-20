use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

fn default_checkpoint_status() -> String {
    "active".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CheckpointRecord {
    pub checkpoint_id: String,
    pub repo_root: String,
    pub conversation_id: String,
    pub source_agent: String,
    #[serde(default = "default_checkpoint_status")]
    pub status: String,
    pub summary: String,
    #[serde(default)]
    pub resume_command: Option<String>,
    #[serde(default = "default_metadata_json")]
    pub metadata_json: String,
    #[serde(default)]
    pub handoff_id: Option<String>,
    pub created_at: String,
}

impl Default for CheckpointRecord {
    fn default() -> Self {
        Self {
            checkpoint_id: String::new(),
            repo_root: String::new(),
            conversation_id: String::new(),
            source_agent: String::new(),
            status: default_checkpoint_status(),
            summary: String::new(),
            resume_command: None,
            metadata_json: default_metadata_json(),
            handoff_id: None,
            created_at: String::new(),
        }
    }
}

fn default_metadata_json() -> String {
    "{}".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateCheckpointInput {
    pub repo_root: String,
    pub conversation_id: String,
    pub source_agent: String,
    pub summary: String,
    pub resume_command: Option<String>,
    pub metadata_json: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::CheckpointRecord;

    #[test]
    fn checkpoint_record_defaults_to_active_state() {
        let checkpoint = CheckpointRecord::default();

        assert_eq!(checkpoint.status, "active");
        assert_eq!(checkpoint.metadata_json, "{}");
        assert!(checkpoint.resume_command.is_none());
        assert!(checkpoint.handoff_id.is_none());
    }
}
