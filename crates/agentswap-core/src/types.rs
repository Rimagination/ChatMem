use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use uuid::Uuid;

/// The kind of coding agent a conversation originates from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AgentKind {
    Claude,
    Codex,
    Gemini,
}

/// The role of a participant in a conversation turn.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Role {
    User,
    Assistant,
    System,
}

/// The outcome status of a tool invocation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ToolStatus {
    Success,
    Error,
}

/// The type of change applied to a file.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ChangeType {
    Created,
    Modified,
    Deleted,
}

/// A single tool invocation within a conversation message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub name: String,
    pub input: Value,
    pub output: Option<String>,
    pub status: ToolStatus,
}

/// A record of a file modification made during a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    pub path: String,
    pub change_type: ChangeType,
    pub timestamp: DateTime<Utc>,
    pub message_id: Uuid,
}

/// A single message (turn) within a conversation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub role: Role,
    pub content: String,
    pub tool_calls: Vec<ToolCall>,
    pub metadata: HashMap<String, Value>,
}

/// A full conversation in the Universal Conversation Format (UCF).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub source_agent: AgentKind,
    pub project_dir: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub summary: Option<String>,
    pub messages: Vec<Message>,
    pub file_changes: Vec<FileChange>,
}

impl Conversation {
    /// Returns a human-readable name for the source agent.
    pub fn source_agent_name(&self) -> &str {
        match self.source_agent {
            AgentKind::Claude => "Claude Code",
            AgentKind::Codex => "Codex CLI",
            AgentKind::Gemini => "Gemini CLI",
        }
    }
}

/// A lightweight summary of a conversation for listing purposes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationSummary {
    pub id: String,
    pub source_agent: AgentKind,
    pub project_dir: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub summary: Option<String>,
    pub message_count: usize,
    pub file_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;
    use uuid::Uuid;

    #[test]
    fn test_conversation_roundtrip_json() {
        let now = Utc::now();
        let msg_id = Uuid::new_v4();

        let conv = Conversation {
            id: "test-conv-001".to_string(),
            source_agent: AgentKind::Claude,
            project_dir: "/tmp/project".to_string(),
            created_at: now,
            updated_at: now,
            summary: Some("Test conversation".to_string()),
            messages: vec![Message {
                id: msg_id,
                timestamp: now,
                role: Role::User,
                content: "Hello, world!".to_string(),
                tool_calls: vec![ToolCall {
                    name: "read_file".to_string(),
                    input: json!({"path": "src/main.rs"}),
                    output: Some("fn main() {}".to_string()),
                    status: ToolStatus::Success,
                }],
                metadata: HashMap::from([("key".to_string(), json!("value"))]),
            }],
            file_changes: vec![FileChange {
                path: "src/main.rs".to_string(),
                change_type: ChangeType::Modified,
                timestamp: now,
                message_id: msg_id,
            }],
        };

        let json_str = serde_json::to_string_pretty(&conv).expect("serialize");
        let deserialized: Conversation = serde_json::from_str(&json_str).expect("deserialize");

        assert_eq!(deserialized.id, conv.id);
        assert_eq!(deserialized.source_agent, conv.source_agent);
        assert_eq!(deserialized.project_dir, conv.project_dir);
        assert_eq!(deserialized.summary, conv.summary);
        assert_eq!(deserialized.messages.len(), 1);
        assert_eq!(deserialized.messages[0].id, msg_id);
        assert_eq!(deserialized.messages[0].role, Role::User);
        assert_eq!(deserialized.messages[0].content, "Hello, world!");
        assert_eq!(deserialized.messages[0].tool_calls.len(), 1);
        assert_eq!(deserialized.messages[0].tool_calls[0].name, "read_file");
        assert_eq!(
            deserialized.messages[0].tool_calls[0].status,
            ToolStatus::Success
        );
        assert_eq!(deserialized.file_changes.len(), 1);
        assert_eq!(
            deserialized.file_changes[0].change_type,
            ChangeType::Modified
        );
        assert_eq!(deserialized.source_agent_name(), "Claude Code");
    }

    #[test]
    fn test_conversation_summary() {
        let now = Utc::now();

        let summary = ConversationSummary {
            id: "sum-001".to_string(),
            source_agent: AgentKind::Gemini,
            project_dir: "/home/user/project".to_string(),
            created_at: now,
            updated_at: now,
            summary: Some("A Gemini session".to_string()),
            message_count: 42,
            file_count: 7,
        };

        assert_eq!(summary.id, "sum-001");
        assert_eq!(summary.source_agent, AgentKind::Gemini);
        assert_eq!(summary.project_dir, "/home/user/project");
        assert_eq!(summary.summary, Some("A Gemini session".to_string()));
        assert_eq!(summary.message_count, 42);
        assert_eq!(summary.file_count, 7);
    }
}
