use agentswap_core::types::{ChangeType, Conversation, Role};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationChunk {
    pub chunk_id_suffix: String,
    pub chunk_type: String,
    pub title: String,
    pub body: String,
    pub message_ids: Vec<String>,
    pub ordinal: usize,
    pub token_estimate: usize,
}

pub fn build_conversation_chunks(conversation_id: &str, conversation: &Conversation) -> Vec<ConversationChunk> {
    let _ = conversation_id;

    let mut chunks = Vec::new();
    let mut ordinal = 0usize;

    for message in &conversation.messages {
        if message.content.trim().is_empty() {
            continue;
        }

        let chunk_type = match message.role {
            Role::User => "user_request",
            Role::Assistant => "assistant_summary",
            Role::System => "implementation_detail",
        }
        .to_string();

        let body = truncate_body(&message.content);
        chunks.push(ConversationChunk {
            chunk_id_suffix: format!("message:{}", message.id),
            chunk_type,
            title: message_title(&message.content, &message.role),
            body: body.clone(),
            message_ids: vec![message.id.to_string()],
            ordinal,
            token_estimate: body.chars().count().div_ceil(4).max(1),
        });
        ordinal += 1;
    }

    for (index, file_change) in conversation.file_changes.iter().enumerate() {
        let change_type = match file_change.change_type {
            ChangeType::Created => "created",
            ChangeType::Modified => "modified",
            ChangeType::Deleted => "deleted",
        };
        let body = format!(
            "path: {}\nchange_type: {}\ntimestamp: {}",
            file_change.path,
            change_type,
            file_change.timestamp.to_rfc3339()
        );
        chunks.push(ConversationChunk {
            chunk_id_suffix: format!("file:{index}"),
            chunk_type: "file_change".to_string(),
            title: file_change.path.clone(),
            body: body.clone(),
            message_ids: vec![file_change.message_id.to_string()],
            ordinal,
            token_estimate: body.chars().count().div_ceil(4).max(1),
        });
        ordinal += 1;
    }

    chunks
}

fn truncate_body(text: &str) -> String {
    const LIMIT: usize = 2400;
    let char_count = text.chars().count();
    if char_count <= LIMIT {
        return text.to_string();
    }

    let mut truncated = text.chars().take(LIMIT).collect::<String>();
    truncated.push_str("\n[truncated]");
    truncated
}

fn message_title(text: &str, role: &Role) -> String {
    let fallback = match role {
        Role::User => "User request",
        Role::Assistant => "Assistant summary",
        Role::System => "Implementation detail",
    };

    let first_line = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or(fallback);

    first_line.chars().take(96).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentswap_core::types::{AgentKind, ChangeType, Conversation, FileChange, Message, Role, ToolCall, ToolStatus};
    use chrono::Utc;
    use serde_json::json;
    use std::collections::HashMap;
    use uuid::Uuid;

    fn message(role: Role, content: &str) -> Message {
        Message {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            role,
            content: content.to_string(),
            tool_calls: vec![ToolCall {
                name: "noop".to_string(),
                input: json!({}),
                output: None,
                status: ToolStatus::Success,
            }],
            metadata: HashMap::new(),
        }
    }

    fn conversation(messages: Vec<Message>) -> Conversation {
        Conversation {
            id: "conv-chunks".to_string(),
            source_agent: AgentKind::Codex,
            project_dir: "D:/VSP/agentswap-gui".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            summary: None,
            messages,
            file_changes: vec![FileChange {
                path: "src-tauri/src/chatmem_memory/chunks.rs".to_string(),
                change_type: ChangeType::Modified,
                timestamp: Utc::now(),
                message_id: Uuid::new_v4(),
            }],
        }
    }

    #[test]
    fn chunks_include_late_conversation_messages() {
        let mut messages = Vec::new();
        for index in 0..16 {
            messages.push(message(Role::User, &format!("ordinary setup message {index}")));
        }
        messages.push(message(
            Role::Assistant,
            "Late important release signing detail: TAURI_PRIVATE_KEY must be configured.",
        ));

        let chunks = build_conversation_chunks("codex:conv-chunks", &conversation(messages));

        assert!(chunks
            .iter()
            .any(|chunk| chunk.body.contains("TAURI_PRIVATE_KEY must be configured")));
        assert!(chunks.iter().all(|chunk| !chunk.message_ids.is_empty()));
    }

    #[test]
    fn chunks_classify_user_requests_and_assistant_summaries() {
        let conv = conversation(vec![
            message(Role::User, "Can you fix the memory search recall bug?"),
            message(Role::Assistant, "Implemented chunk-level recall search for ChatMem."),
        ]);

        let chunks = build_conversation_chunks("codex:conv-chunks", &conv);

        assert!(chunks.iter().any(|chunk| chunk.chunk_type == "user_request"));
        assert!(chunks.iter().any(|chunk| chunk.chunk_type == "assistant_summary"));
        assert_eq!(chunks[0].ordinal, 0);
    }
}
