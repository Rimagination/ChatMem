use agentswap_claude::ClaudeAdapter;
use agentswap_codex::CodexAdapter;
use agentswap_core::{adapter::AgentAdapter, types::Conversation};
use agentswap_gemini::GeminiAdapter;
use rusqlite::Connection;
use std::collections::BTreeSet;
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
    let normalized_repo = crate::chatmem_memory::repo_identity::canonical_repo_root(repo_root);
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
            if !summary_project_matches_repo(agent, &summary.project_dir, &normalized_repo) {
                continue;
            }

            let mut conversation = adapter.read_conversation(&summary.id)?;
            if agent == "gemini"
                && crate::chatmem_memory::repo_identity::normalize_repo_root(&conversation.project_dir)
                    != normalized_repo
            {
                conversation.project_dir = normalized_repo.clone();
            }
            sync_conversation_into_store(store, agent, &conversation)?;
            synced += 1;
        }
    }

    Ok(synced)
}

pub(crate) fn summary_project_matches_repo(
    agent: &str,
    project_dir: &str,
    repo_root: &str,
) -> bool {
    let normalized_repo = crate::chatmem_memory::repo_identity::normalize_repo_root(repo_root);
    if crate::chatmem_memory::repo_identity::normalize_repo_root(project_dir) == normalized_repo {
        return true;
    }

    if agent != "gemini" {
        return false;
    }

    let Some(project_hash) = project_dir.strip_prefix("gemini:") else {
        return false;
    };

    gemini_repo_hash_candidates(repo_root, &normalized_repo).contains(project_hash)
}

fn gemini_repo_hash_candidates(repo_root: &str, normalized_repo: &str) -> BTreeSet<String> {
    let mut variants = BTreeSet::new();
    let trimmed = repo_root.trim().trim_end_matches(['\\', '/']);
    if !trimmed.is_empty() {
        variants.insert(trimmed.to_string());
    }
    variants.insert(normalized_repo.to_string());
    variants.insert(normalized_repo.replace('/', "\\"));

    if normalized_repo.len() >= 2 && normalized_repo.as_bytes()[1] == b':' {
        let drive = normalized_repo.chars().next().unwrap();
        let rest = &normalized_repo[1..];
        variants.insert(format!("{}{}", drive.to_ascii_uppercase(), rest));
        variants.insert(format!("{}{}", drive.to_ascii_lowercase(), rest));
        variants.insert(format!("{}{}", drive.to_ascii_uppercase(), rest).replace('/', "\\"));
        variants.insert(format!("{}{}", drive.to_ascii_lowercase(), rest).replace('/', "\\"));
    }

    variants
        .into_iter()
        .map(|variant| GeminiAdapter::project_hash_for_path(&variant))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::summary_project_matches_repo;
    use agentswap_gemini::GeminiAdapter;

    #[test]
    fn gemini_hash_project_dir_matches_requested_repo_root() {
        let repo_root = "D:/VSP/agentswap-gui";
        let gemini_project_dir = format!(
            "gemini:{}",
            GeminiAdapter::project_hash_for_path("d:/vsp/agentswap-gui")
        );

        assert!(summary_project_matches_repo(
            "gemini",
            &gemini_project_dir,
            repo_root
        ));
    }
}
