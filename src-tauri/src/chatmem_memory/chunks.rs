use agentswap_core::types::{ChangeType, Conversation, Role};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

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

pub fn build_conversation_chunks(
    conversation_id: &str,
    conversation: &Conversation,
) -> Vec<ConversationChunk> {
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

    for file_change in conversation.file_changes.iter() {
        let change_type = match file_change.change_type {
            ChangeType::Created => "created",
            ChangeType::Modified => "modified",
            ChangeType::Deleted => "deleted",
        };
        let suffix = stable_file_change_suffix(
            conversation_id,
            &file_change.path,
            change_type,
            &file_change.message_id.to_string(),
        );
        let body = format!(
            "path: {}\nchange_type: {}\ntimestamp: {}",
            file_change.path,
            change_type,
            file_change.timestamp.to_rfc3339()
        );
        chunks.push(ConversationChunk {
            chunk_id_suffix: format!("file:{suffix}"),
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

fn stable_file_change_suffix(
    conversation_id: &str,
    path: &str,
    change_type: &str,
    message_id: &str,
) -> String {
    let key = format!("{conversation_id}|{path}|{change_type}|{message_id}");
    Uuid::new_v5(&Uuid::NAMESPACE_URL, key.as_bytes()).to_string()
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
    use agentswap_core::types::{
        AgentKind, ChangeType, Conversation, FileChange, Message, Role, ToolCall, ToolStatus,
    };
    use chrono::{TimeZone, Utc};
    use serde_json::json;
    use std::collections::HashMap;
    use uuid::Uuid;

    const CONVERSATION_ID: &str = "codex:conv-chunks";
    const PROJECT_DIR: &str = "D:/VSP/agentswap-gui";

    fn ts(
        year: i32,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
    ) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(year, month, day, hour, minute, second)
            .single()
            .expect("valid timestamp")
    }

    fn message(id: Uuid, role: Role, content: &str) -> Message {
        Message {
            id,
            timestamp: ts(2026, 4, 23, 10, 0, 0),
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

    fn conversation(messages: Vec<Message>, file_changes: Vec<FileChange>) -> Conversation {
        Conversation {
            id: "conv-chunks".to_string(),
            source_agent: AgentKind::Codex,
            project_dir: PROJECT_DIR.to_string(),
            created_at: ts(2026, 4, 23, 9, 0, 0),
            updated_at: ts(2026, 4, 23, 11, 0, 0),
            summary: None,
            messages,
            file_changes,
        }
    }

    #[test]
    fn chunks_include_late_conversation_messages() {
        let mut messages = Vec::new();
        for index in 0..16 {
            messages.push(message(
                Uuid::from_u128(0x1000_0000_0000_0000_0000_0000_0000_0000 + index as u128),
                Role::User,
                &format!("ordinary setup message {index}"),
            ));
        }
        messages.push(message(
            Uuid::from_u128(0x2000_0000_0000_0000_0000_0000_0000_0001),
            Role::Assistant,
            "Late important release signing detail: TAURI_PRIVATE_KEY must be configured.",
        ));

        let chunks = build_conversation_chunks(CONVERSATION_ID, &conversation(messages, vec![]));

        assert_eq!(chunks.len(), 17);
        assert_eq!(chunks[16].chunk_type, "assistant_summary");
        assert_eq!(
            chunks[16].chunk_id_suffix,
            "message:20000000-0000-0000-0000-000000000001"
        );
        assert_eq!(chunks[16].ordinal, 16);
        assert_eq!(
            chunks[16].message_ids,
            vec!["20000000-0000-0000-0000-000000000001"]
        );
        assert_eq!(
            chunks[16].body,
            "Late important release signing detail: TAURI_PRIVATE_KEY must be configured."
        );
    }

    #[test]
    fn chunks_classify_roles_and_skip_empty_messages() {
        let conv = conversation(
            vec![
                message(
                    Uuid::from_u128(0x3000_0000_0000_0000_0000_0000_0000_0001),
                    Role::System,
                    "  implementation detail from system  ",
                ),
                message(
                    Uuid::from_u128(0x3000_0000_0000_0000_0000_0000_0000_0002),
                    Role::User,
                    "Can you fix the memory search recall bug?",
                ),
                message(
                    Uuid::from_u128(0x3000_0000_0000_0000_0000_0000_0000_0003),
                    Role::Assistant,
                    "Implemented chunk-level recall search for ChatMem.",
                ),
                message(
                    Uuid::from_u128(0x3000_0000_0000_0000_0000_0000_0000_0004),
                    Role::User,
                    "   ",
                ),
            ],
            vec![],
        );

        let chunks = build_conversation_chunks(CONVERSATION_ID, &conv);

        assert_eq!(chunks.len(), 3);
        assert_eq!(chunks[0].chunk_type, "implementation_detail");
        assert_eq!(chunks[0].ordinal, 0);
        assert_eq!(
            chunks[0].chunk_id_suffix,
            "message:30000000-0000-0000-0000-000000000001"
        );
        assert_eq!(
            chunks[0].message_ids,
            vec!["30000000-0000-0000-0000-000000000001"]
        );
        assert_eq!(chunks[1].chunk_type, "user_request");
        assert_eq!(chunks[2].chunk_type, "assistant_summary");
        assert!(!chunks.iter().any(|chunk| chunk.body.trim().is_empty()));
    }

    #[test]
    fn chunks_truncate_long_message_body_and_title_deterministically() {
        let long_content = format!(
            "Leading line for title that is intentionally long to exceed ninety-six characters by a visible margin.\n{}",
            "x".repeat(2500)
        );
        let conv = conversation(
            vec![message(
                Uuid::from_u128(0x4000_0000_0000_0000_0000_0000_0000_0001),
                Role::Assistant,
                &long_content,
            )],
            vec![],
        );

        let chunks = build_conversation_chunks(CONVERSATION_ID, &conv);

        assert_eq!(chunks.len(), 1);
        assert_eq!(
            chunks[0].title,
            "Leading line for title that is intentionally long to exceed ninety-six characters by a visible m"
        );
        assert!(chunks[0].body.ends_with("\n[truncated]"));
        assert_eq!(chunks[0].body.chars().count(), 2412);
        assert_eq!(
            chunks[0].token_estimate,
            chunks[0].body.chars().count().div_ceil(4).max(1)
        );
    }

    #[test]
    fn file_change_suffix_is_stable_across_order_changes() {
        let change_a = FileChange {
            path: "src-tauri/src/chatmem_memory/chunks.rs".to_string(),
            change_type: ChangeType::Modified,
            timestamp: ts(2026, 4, 23, 8, 0, 0),
            message_id: Uuid::from_u128(0x5000_0000_0000_0000_0000_0000_0000_0001),
        };
        let change_b = FileChange {
            path: "src-tauri/src/chatmem_memory/mod.rs".to_string(),
            change_type: ChangeType::Created,
            timestamp: ts(2026, 4, 23, 8, 5, 0),
            message_id: Uuid::from_u128(0x5000_0000_0000_0000_0000_0000_0000_0002),
        };

        let forward = build_conversation_chunks(
            CONVERSATION_ID,
            &conversation(vec![], vec![change_a.clone(), change_b.clone()]),
        );
        let reversed = build_conversation_chunks(
            CONVERSATION_ID,
            &conversation(vec![], vec![change_b, change_a.clone()]),
        );

        let forward_a = forward
            .iter()
            .find(|chunk| chunk.title.ends_with("chunks.rs"))
            .unwrap();
        let reversed_a = reversed
            .iter()
            .find(|chunk| chunk.title.ends_with("chunks.rs"))
            .unwrap();
        assert_eq!(forward_a.chunk_id_suffix, reversed_a.chunk_id_suffix);
        assert_eq!(
            forward_a.chunk_id_suffix,
            "file:bb834231-c437-57d3-a36b-db6cea50759d"
        );

        let forward_b = forward
            .iter()
            .find(|chunk| chunk.title.ends_with("mod.rs"))
            .unwrap();
        let reversed_b = reversed
            .iter()
            .find(|chunk| chunk.title.ends_with("mod.rs"))
            .unwrap();
        assert_eq!(forward_b.chunk_id_suffix, reversed_b.chunk_id_suffix);
        assert_eq!(
            forward_b.chunk_id_suffix,
            "file:78dc2860-58c0-59c6-87a9-9637fd2e0aa6"
        );
        assert_eq!(forward.len(), 2);
        assert_eq!(reversed.len(), 2);
    }
}
