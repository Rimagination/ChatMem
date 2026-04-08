#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tauri::command;
use walkdir::WalkDir;

// Import AgentSwap adapters
use agentswap_claude::ClaudeAdapter;
use agentswap_codex::CodexAdapter;
use agentswap_gemini::GeminiAdapter;
use agentswap_core::adapter::AgentAdapter;
use agentswap_core::types::{Conversation, ConversationSummary, AgentKind};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConversationSummaryResponse {
    id: String,
    source_agent: String,
    project_dir: String,
    created_at: String,
    updated_at: String,
    summary: Option<String>,
    message_count: usize,
    file_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConversationResponse {
    id: String,
    source_agent: String,
    project_dir: String,
    created_at: String,
    updated_at: String,
    summary: Option<String>,
    storage_path: Option<String>,
    resume_command: Option<String>,
    messages: Vec<MessageResponse>,
    file_changes: Vec<FileChangeResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MessageResponse {
    id: String,
    timestamp: String,
    role: String,
    content: String,
    tool_calls: Vec<ToolCallResponse>,
    metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolCallResponse {
    name: String,
    input: serde_json::Value,
    output: Option<String>,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileChangeResponse {
    path: String,
    change_type: String,
    timestamp: String,
    message_id: String,
}

fn get_adapter(agent: &str) -> Result<Box<dyn AgentAdapter>, String> {
    match agent {
        "claude" => Ok(Box::new(ClaudeAdapter::new())),
        "codex" => Ok(Box::new(CodexAdapter::new())),
        "gemini" => Ok(Box::new(GeminiAdapter::new())),
        _ => Err(format!("Unknown agent: {}", agent)),
    }
}

fn agent_key(agent: &AgentKind) -> &'static str {
    match agent {
        AgentKind::Claude => "claude",
        AgentKind::Codex => "codex",
        AgentKind::Gemini => "gemini",
    }
}

fn build_resume_command(agent: &str, id: &str) -> Option<String> {
    match agent {
        "claude" => Some(format!("claude --resume {}", id)),
        "codex" => Some(format!("codex resume {}", id)),
        "gemini" => Some(format!("gemini --resume {}", id)),
        _ => None,
    }
}

fn resolve_claude_storage_path(id: &str) -> Option<String> {
    let projects_dir = dirs::home_dir()?.join(".claude").join("projects");
    let filename = format!("{}.jsonl", id);

    WalkDir::new(projects_dir)
        .min_depth(2)
        .max_depth(2)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .find(|entry| entry.file_name().to_string_lossy() == filename)
        .map(|entry| entry.path().display().to_string())
}

fn resolve_codex_storage_path(id: &str) -> Option<String> {
    let db_path = dirs::home_dir()?.join(".codex").join("state_5.sqlite");
    let conn = Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    let mut stmt = conn
        .prepare("SELECT rollout_path FROM threads WHERE id = ?1")
        .ok()?;
    stmt.query_row([id], |row| row.get::<_, String>(0)).ok()
}

fn resolve_gemini_storage_path(id: &str) -> Option<String> {
    let tmp_dir = dirs::home_dir()?.join(".gemini").join("tmp");

    WalkDir::new(tmp_dir)
        .min_depth(3)
        .max_depth(3)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .find_map(|entry| {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                return None;
            }

            let parent_name = path
                .parent()
                .and_then(|parent| parent.file_name())
                .and_then(|name| name.to_str())?;
            if parent_name != "chats" {
                return None;
            }

            let file_name = path.file_name()?.to_string_lossy();
            let direct_match = file_name == format!("session-{}.json", id) || file_name == format!("{}.json", id);
            if direct_match {
                return Some(path.display().to_string());
            }

            let data = std::fs::read(path).ok()?;
            let parsed = serde_json::from_slice::<serde_json::Value>(&data).ok()?;
            let session_id = parsed.get("sessionId").and_then(|value| value.as_str())?;
            if session_id == id {
                Some(path.display().to_string())
            } else {
                None
            }
        })
}

fn resolve_storage_path(agent: &str, id: &str) -> Option<String> {
    match agent {
        "claude" => resolve_claude_storage_path(id),
        "codex" => resolve_codex_storage_path(id),
        "gemini" => resolve_gemini_storage_path(id),
        _ => None,
    }
}

fn contains_query(haystack: &str, query: &str) -> bool {
    haystack.to_lowercase().contains(query)
}

fn summary_matches_query(summary: &ConversationSummary, query: &str) -> bool {
    contains_query(&summary.id, query)
        || contains_query(&summary.project_dir, query)
        || summary
            .summary
            .as_deref()
            .map(|text| contains_query(text, query))
            .unwrap_or(false)
}

fn conversation_matches_query(conversation: &Conversation, query: &str) -> bool {
    if contains_query(&conversation.id, query)
        || contains_query(&conversation.project_dir, query)
        || conversation
            .summary
            .as_deref()
            .map(|text| contains_query(text, query))
            .unwrap_or(false)
    {
        return true;
    }

    if conversation
        .messages
        .iter()
        .any(|message| contains_query(&message.content, query))
    {
        return true;
    }

    if conversation.file_changes.iter().any(|change| contains_query(&change.path, query)) {
        return true;
    }

    conversation.messages.iter().any(|message| {
        message.tool_calls.iter().any(|tool_call| {
            contains_query(&tool_call.name, query)
                || contains_query(&tool_call.input.to_string(), query)
                || tool_call
                    .output
                    .as_deref()
                    .map(|output| contains_query(output, query))
                    .unwrap_or(false)
        })
    })
}

fn convert_summary(summary: ConversationSummary) -> ConversationSummaryResponse {
    ConversationSummaryResponse {
        id: summary.id,
        source_agent: agent_key(&summary.source_agent).to_string(),
        project_dir: summary.project_dir,
        created_at: summary.created_at.to_rfc3339(),
        updated_at: summary.updated_at.to_rfc3339(),
        summary: summary.summary,
        message_count: summary.message_count,
        file_count: summary.file_count,
    }
}

fn convert_conversation(
    conv: Conversation,
    storage_path: Option<String>,
    resume_command: Option<String>,
) -> ConversationResponse {
    ConversationResponse {
        id: conv.id,
        source_agent: agent_key(&conv.source_agent).to_string(),
        project_dir: conv.project_dir,
        created_at: conv.created_at.to_rfc3339(),
        updated_at: conv.updated_at.to_rfc3339(),
        summary: conv.summary,
        storage_path,
        resume_command,
        messages: conv
            .messages
            .into_iter()
            .map(|m| MessageResponse {
                id: m.id.to_string(),
                timestamp: m.timestamp.to_rfc3339(),
                role: match m.role {
                    agentswap_core::types::Role::User => "user".to_string(),
                    agentswap_core::types::Role::Assistant => "assistant".to_string(),
                    agentswap_core::types::Role::System => "system".to_string(),
                },
                content: m.content,
                tool_calls: m
                    .tool_calls
                    .into_iter()
                    .map(|tc| ToolCallResponse {
                        name: tc.name,
                        input: tc.input,
                        output: tc.output,
                        status: match tc.status {
                            agentswap_core::types::ToolStatus::Success => "success".to_string(),
                            agentswap_core::types::ToolStatus::Error => "error".to_string(),
                        },
                    })
                    .collect(),
                metadata: serde_json::to_value(m.metadata).unwrap_or(serde_json::Value::Null),
            })
            .collect(),
        file_changes: conv
            .file_changes
            .into_iter()
            .map(|fc| FileChangeResponse {
                path: fc.path,
                change_type: match fc.change_type {
                    agentswap_core::types::ChangeType::Created => "created".to_string(),
                    agentswap_core::types::ChangeType::Modified => "modified".to_string(),
                    agentswap_core::types::ChangeType::Deleted => "deleted".to_string(),
                },
                timestamp: fc.timestamp.to_rfc3339(),
                message_id: fc.message_id.to_string(),
            })
            .collect(),
    }
}

#[command]
async fn list_conversations(agent: String) -> Result<Vec<ConversationSummaryResponse>, String> {
    let adapter = get_adapter(&agent)?;
    
    if !adapter.is_available() {
        return Ok(vec![]);
    }

    let conversations = adapter
        .list_conversations()
        .map_err(|e| e.to_string())?;

    Ok(conversations.into_iter().map(convert_summary).collect())
}

#[command]
async fn search_conversations(agent: String, query: String) -> Result<Vec<ConversationSummaryResponse>, String> {
    let trimmed_query = query.trim();
    if trimmed_query.is_empty() {
        return list_conversations(agent).await;
    }

    let adapter = get_adapter(&agent)?;
    
    if !adapter.is_available() {
        return Ok(vec![]);
    }

    let normalized_query = trimmed_query.to_lowercase();
    let summaries = adapter
        .list_conversations()
        .map_err(|e| e.to_string())?;

    let mut matches = Vec::new();

    for summary in summaries {
        if summary_matches_query(&summary, &normalized_query) {
            matches.push(convert_summary(summary));
            continue;
        }

        let conversation = adapter
            .read_conversation(&summary.id)
            .map_err(|e| e.to_string())?;

        if conversation_matches_query(&conversation, &normalized_query) {
            matches.push(convert_summary(summary));
        }
    }

    Ok(matches)
}

#[command]
async fn read_conversation(agent: String, id: String) -> Result<ConversationResponse, String> {
    let adapter = get_adapter(&agent)?;
    let conversation = adapter.read_conversation(&id).map_err(|e| e.to_string())?;
    let storage_path = resolve_storage_path(&agent, &id);
    let resume_command = build_resume_command(&agent, &id);
    Ok(convert_conversation(conversation, storage_path, resume_command))
}

#[command]
async fn migrate_conversation(
    source: String,
    target: String,
    id: String,
    mode: String,  // "copy" or "cut"
) -> Result<String, String> {
    let source_adapter = get_adapter(&source)?;
    let target_adapter = get_adapter(&target)?;

    // Read from source
    let conversation = source_adapter.read_conversation(&id).map_err(|e| e.to_string())?;

    // Write to target
    let new_id = target_adapter
        .write_conversation(&conversation)
        .map_err(|e| e.to_string())?;

    // If cut mode, delete from source
    if mode == "cut" {
        source_adapter.delete_conversation(&id).map_err(|e| e.to_string())?;
    }

    Ok(new_id)
}

#[command]
async fn delete_conversation(agent: String, id: String) -> Result<(), String> {
    let adapter = get_adapter(&agent)?;
    adapter.delete_conversation(&id).map_err(|e| e.to_string())?;
    Ok(())
}

#[command]
async fn check_agent_available(agent: String) -> Result<bool, String> {
    let adapter = get_adapter(&agent)?;
    Ok(adapter.is_available())
}

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            list_conversations,
            search_conversations,
            read_conversation,
            migrate_conversation,
            delete_conversation,
            check_agent_available,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{build_resume_command, conversation_matches_query, AgentKind, Conversation};
    use agentswap_core::types::{Message, Role, ToolCall, ToolStatus};
    use chrono::Utc;
    use serde_json::json;
    use std::collections::HashMap;
    use uuid::Uuid;

    #[test]
    fn builds_resume_command_for_codex() {
        assert_eq!(
            build_resume_command("codex", "conv-001"),
            Some("codex resume conv-001".to_string())
        );
    }

    #[test]
    fn returns_none_for_unknown_agent_resume_command() {
        assert_eq!(build_resume_command("unknown", "conv-001"), None);
    }

    #[test]
    fn full_text_search_matches_message_content() {
        let now = Utc::now();
        let message_id = Uuid::new_v4();

        let conversation = Conversation {
            id: "conv-002".to_string(),
            source_agent: AgentKind::Claude,
            project_dir: "D:/VSP/service".to_string(),
            created_at: now,
            updated_at: now,
            summary: Some("Memory investigation".to_string()),
            messages: vec![Message {
                id: message_id,
                timestamp: now,
                role: Role::Assistant,
                content: "问题根因是内存泄漏出现在缓存清理逻辑。".to_string(),
                tool_calls: vec![ToolCall {
                    name: "read_logs".to_string(),
                    input: json!({"path": "logs/app.log"}),
                    output: Some("found repeated allocation spikes".to_string()),
                    status: ToolStatus::Success,
                }],
                metadata: HashMap::new(),
            }],
            file_changes: vec![],
        };

        assert!(conversation_matches_query(&conversation, "内存泄漏"));
    }
}
