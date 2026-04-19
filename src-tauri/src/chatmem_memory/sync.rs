use agentswap_claude::ClaudeAdapter;
use agentswap_codex::CodexAdapter;
use agentswap_core::{adapter::AgentAdapter, types::Conversation};
use agentswap_gemini::GeminiAdapter;
use rusqlite::Connection;
use walkdir::WalkDir;

use super::store::MemoryStore;

pub fn build_resume_command(agent: &str, id: &str) -> Option<String> {
    match agent {
        "claude" => Some(format!("claude --resume {}", id)),
        "codex" => Some(format!("codex resume {}", id)),
        "gemini" => Some(format!("gemini --resume {}", id)),
        _ => None,
    }
}

pub fn resolve_claude_storage_path(id: &str) -> Option<String> {
    let projects_dir = dirs::home_dir()?.join(".claude").join("projects");
    let filename = format!("{id}.jsonl");

    WalkDir::new(projects_dir)
        .min_depth(2)
        .max_depth(2)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .find(|entry| entry.file_name().to_string_lossy() == filename)
        .map(|entry| entry.path().display().to_string())
}

pub fn resolve_codex_storage_path(id: &str) -> Option<String> {
    let db_path = dirs::home_dir()?.join(".codex").join("state_5.sqlite");
    let conn = Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    let mut stmt = conn.prepare("SELECT rollout_path FROM threads WHERE id = ?1").ok()?;
    stmt.query_row([id], |row| row.get::<_, String>(0)).ok()
}

pub fn resolve_gemini_storage_path(id: &str) -> Option<String> {
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
            if file_name == format!("session-{id}.json") || file_name == format!("{id}.json") {
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

pub fn resolve_storage_path(agent: &str, id: &str) -> Option<String> {
    match agent {
        "claude" => resolve_claude_storage_path(id),
        "codex" => resolve_codex_storage_path(id),
        "gemini" => resolve_gemini_storage_path(id),
        _ => None,
    }
}

fn get_adapter(agent: &str) -> Option<Box<dyn AgentAdapter>> {
    match agent {
        "claude" => Some(Box::new(ClaudeAdapter::new())),
        "codex" => Some(Box::new(CodexAdapter::new())),
        "gemini" => Some(Box::new(GeminiAdapter::new())),
        _ => None,
    }
}

pub fn sync_conversation_into_store(
    store: &MemoryStore,
    agent: &str,
    conversation: &Conversation,
) -> anyhow::Result<String> {
    let storage_path = resolve_storage_path(agent, &conversation.id);
    store.upsert_conversation_snapshot(agent, conversation, storage_path)
}

pub fn sync_repo_conversations(store: &MemoryStore, repo_root: &str) -> anyhow::Result<usize> {
    let normalized_repo = crate::chatmem_memory::repo_identity::normalize_repo_root(repo_root);
    let mut synced = 0usize;

    for agent in ["claude", "codex", "gemini"] {
        let Some(adapter) = get_adapter(agent) else {
            continue;
        };

        if !adapter.is_available() {
            continue;
        }

        let summaries = adapter.list_conversations()?;
        for summary in summaries {
            if crate::chatmem_memory::repo_identity::normalize_repo_root(&summary.project_dir)
                != normalized_repo
            {
                continue;
            }

            let conversation = adapter.read_conversation(&summary.id)?;
            sync_conversation_into_store(store, agent, &conversation)?;
            synced += 1;
        }
    }

    Ok(synced)
}
