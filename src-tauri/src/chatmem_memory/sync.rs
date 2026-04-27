use agentswap_claude::ClaudeAdapter;
use agentswap_codex::CodexAdapter;
use agentswap_core::{adapter::AgentAdapter, types::Conversation};
use agentswap_gemini::GeminiAdapter;
use agentswap_opencode::OpenCodeAdapter;
use rusqlite::Connection;
use std::collections::{BTreeSet, HashMap};
use walkdir::WalkDir;

use super::{
    models::{
        AgentConversationCount, LocalHistoryImportReport, ObservedProjectRootCount, RepoScanReport,
    },
    store::MemoryStore,
};

pub fn build_resume_command(agent: &str, id: &str) -> Option<String> {
    match agent {
        "claude" => Some(format!("claude --resume {}", id)),
        "codex" => Some(format!("codex resume {}", id)),
        "gemini" => Some(format!("gemini --resume {}", id)),
        "opencode" => Some(format!("opencode --session {}", id)),
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
    let conn =
        Connection::open_with_flags(db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
    let mut stmt = conn
        .prepare("SELECT rollout_path FROM threads WHERE id = ?1")
        .ok()?;
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

pub fn resolve_opencode_storage_path(id: &str) -> Option<String> {
    let adapter = OpenCodeAdapter::new();
    if !adapter.is_available() {
        return None;
    }
    adapter.read_conversation(id).ok()?;
    Some(adapter.db_path().display().to_string())
}

pub fn resolve_storage_path(agent: &str, id: &str) -> Option<String> {
    match agent {
        "claude" => resolve_claude_storage_path(id),
        "codex" => resolve_codex_storage_path(id),
        "gemini" => resolve_gemini_storage_path(id),
        "opencode" => resolve_opencode_storage_path(id),
        _ => None,
    }
}

fn get_adapter(agent: &str) -> Option<Box<dyn AgentAdapter>> {
    match agent {
        "claude" => Some(Box::new(ClaudeAdapter::new())),
        "codex" => Some(Box::new(CodexAdapter::new())),
        "gemini" => Some(Box::new(GeminiAdapter::new())),
        "opencode" => Some(Box::new(OpenCodeAdapter::new())),
        _ => None,
    }
}

fn is_digits(value: &str) -> bool {
    !value.is_empty() && value.chars().all(|character| character.is_ascii_digit())
}

fn is_codex_new_chat_leaf(leaf: &str) -> bool {
    let Some(suffix) = leaf.strip_prefix("new-chat") else {
        return false;
    };

    suffix.is_empty() || suffix.strip_prefix('-').map(is_digits).unwrap_or(false)
}

fn is_codex_flat_new_chat_leaf(leaf: &str) -> bool {
    if is_codex_new_chat_leaf(leaf) {
        return true;
    }

    if leaf.len() < "0000-00-00-new-chat".len() {
        return false;
    }

    let date_part = &leaf[..10];
    let separator_suffix = &leaf[10..];
    date_part.as_bytes().get(4) == Some(&b'-')
        && date_part.as_bytes().get(7) == Some(&b'-')
        && date_part
            .chars()
            .enumerate()
            .all(|(index, character)| index == 4 || index == 7 || character.is_ascii_digit())
        && separator_suffix
            .strip_prefix('-')
            .map(is_codex_new_chat_leaf)
            .unwrap_or(false)
}

pub(crate) fn is_root_project_placeholder(project_dir: &str) -> bool {
    let normalized = crate::chatmem_memory::repo_identity::normalize_repo_root(project_dir);
    normalized.is_empty()
        || (normalized.len() == 2
            && normalized.as_bytes()[0].is_ascii_alphabetic()
            && normalized.as_bytes()[1] == b':')
}

pub(crate) fn is_codex_generated_chat_project_dir(agent: &str, project_dir: &str) -> bool {
    if agent != "codex" {
        return false;
    }

    if is_root_project_placeholder(project_dir) {
        return true;
    }

    let normalized = crate::chatmem_memory::repo_identity::normalize_repo_root(project_dir);
    let marker = "/documents/codex/";
    let Some(marker_index) = normalized.rfind(marker) else {
        return false;
    };

    let relative_path = normalized[marker_index + marker.len()..].trim_matches('/');
    let segments = relative_path
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    match segments.as_slice() {
        [leaf] => is_codex_flat_new_chat_leaf(leaf),
        [date_folder, leaf] => {
            date_folder.len() == 10
                && date_folder.as_bytes().get(4) == Some(&b'-')
                && date_folder.as_bytes().get(7) == Some(&b'-')
                && date_folder.chars().enumerate().all(|(index, character)| {
                    index == 4 || index == 7 || character.is_ascii_digit()
                })
                && is_codex_new_chat_leaf(leaf)
        }
        _ => false,
    }
}

fn is_standalone_history_project(agent: &str, project_dir: &str) -> bool {
    is_root_project_placeholder(project_dir)
        || is_codex_generated_chat_project_dir(agent, project_dir)
}

fn importable_history_project_root(agent: &str, project_dir: &str) -> Option<String> {
    if is_standalone_history_project(agent, project_dir) {
        return Some(crate::chatmem_memory::repo_identity::GLOBAL_LOCAL_HISTORY_ROOT.to_string());
    }

    let canonical_project_root =
        crate::chatmem_memory::repo_identity::canonical_repo_root(project_dir);
    if canonical_project_root.is_empty() {
        None
    } else {
        Some(canonical_project_root)
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
    Ok(scan_repo_conversations(store, repo_root)?.linked_conversation_count)
}

pub fn import_all_local_history(store: &MemoryStore) -> anyhow::Result<LocalHistoryImportReport> {
    let mut adapters = Vec::new();
    for agent in ["claude", "codex", "gemini", "opencode"] {
        let Some(adapter) = get_adapter(agent) else {
            continue;
        };
        adapters.push((agent, adapter));
    }

    import_local_history_from_adapters(store, adapters)
}

pub(crate) fn import_local_history_from_adapters(
    store: &MemoryStore,
    adapters: Vec<(&str, Box<dyn AgentAdapter>)>,
) -> anyhow::Result<LocalHistoryImportReport> {
    let mut scanned = 0usize;
    let mut imported = 0usize;
    let mut skipped = 0usize;
    let mut source_agent_counts: HashMap<String, usize> = HashMap::new();
    let mut imported_project_root_counts: HashMap<(String, String), usize> = HashMap::new();
    let mut warnings = Vec::new();

    for (agent, adapter) in adapters {
        if !adapter.is_available() {
            continue;
        }

        let summaries = match adapter.list_conversations() {
            Ok(summaries) => summaries,
            Err(error) => {
                warnings.push(format!("Failed to list {agent} conversations: {error}"));
                continue;
            }
        };

        for summary in summaries {
            scanned += 1;
            let mut conversation = match adapter.read_conversation(&summary.id) {
                Ok(conversation) => conversation,
                Err(error) => {
                    skipped += 1;
                    warnings.push(format!(
                        "Failed to read {agent} conversation {}: {error}",
                        summary.id
                    ));
                    continue;
                }
            };

            let Some(canonical_project_root) =
                importable_history_project_root(agent, &conversation.project_dir)
            else {
                skipped += 1;
                warnings.push(format!(
                    "Skipped {agent} conversation {} because it has no project path.",
                    summary.id
                ));
                continue;
            };

            conversation.project_dir = canonical_project_root.clone();
            sync_conversation_into_store(store, agent, &conversation)?;
            imported += 1;
            *source_agent_counts.entry(agent.to_string()).or_insert(0) += 1;
            *imported_project_root_counts
                .entry((agent.to_string(), canonical_project_root))
                .or_insert(0) += 1;
        }
    }

    let mut source_agents = source_agent_counts
        .into_iter()
        .map(
            |(source_agent, conversation_count)| AgentConversationCount {
                source_agent,
                conversation_count,
            },
        )
        .collect::<Vec<_>>();
    source_agents.sort_by(|left, right| left.source_agent.cmp(&right.source_agent));

    let imported_project_roots = build_unmatched_project_roots(imported_project_root_counts);
    let indexed_repo_count = imported_project_roots
        .iter()
        .map(|root| root.project_root.as_str())
        .collect::<BTreeSet<_>>()
        .len();

    Ok(LocalHistoryImportReport {
        scanned_conversation_count: scanned,
        imported_conversation_count: imported,
        skipped_conversation_count: skipped,
        indexed_repo_count,
        source_agents,
        imported_project_roots,
        warnings,
        imported_at: chrono::Utc::now().to_rfc3339(),
    })
}

pub fn scan_repo_conversations(
    store: &MemoryStore,
    repo_root: &str,
) -> anyhow::Result<RepoScanReport> {
    let normalized_requested_repo =
        crate::chatmem_memory::repo_identity::normalize_repo_root(repo_root);
    let normalized_repo = crate::chatmem_memory::repo_identity::canonical_repo_root(repo_root);
    let repo_id = store.ensure_repo(&normalized_repo)?;
    store.upsert_repo_alias_for_repo_id(&repo_id, &normalized_requested_repo, "requested", 1.0)?;
    store.upsert_repo_alias_for_repo_id(&repo_id, &normalized_repo, "canonical", 1.0)?;
    let repo_match_roots = store.repo_match_roots_for_repo_id(&repo_id, &normalized_repo)?;
    let mut scanned = 0usize;
    let mut linked = 0usize;
    let mut skipped = 0usize;
    let mut source_agent_counts: HashMap<String, usize> = HashMap::new();
    let mut unmatched_project_root_counts: HashMap<(String, String), usize> = HashMap::new();

    for agent in ["claude", "codex", "gemini", "opencode"] {
        let Some(adapter) = get_adapter(agent) else {
            continue;
        };

        if !adapter.is_available() {
            continue;
        }

        let summaries = adapter.list_conversations()?;
        for summary in summaries {
            scanned += 1;
            if is_standalone_history_project(agent, &summary.project_dir) {
                let mut conversation = adapter.read_conversation(&summary.id)?;
                conversation.project_dir =
                    crate::chatmem_memory::repo_identity::GLOBAL_LOCAL_HISTORY_ROOT.to_string();
                sync_conversation_into_store(store, agent, &conversation)?;
                skipped += 1;
                continue;
            }

            if !summary_project_matches_repo_roots(agent, &summary.project_dir, &repo_match_roots) {
                skipped += 1;
                record_unmatched_project_root(
                    &mut unmatched_project_root_counts,
                    agent,
                    &summary.project_dir,
                );
                continue;
            }

            let mut conversation = adapter.read_conversation(&summary.id)?;
            let observed_project_root = crate::chatmem_memory::repo_identity::normalize_repo_root(
                &conversation.project_dir,
            );
            if observed_project_root != normalized_repo {
                conversation.project_dir = normalized_repo.clone();
            }
            sync_conversation_into_store(store, agent, &conversation)?;
            linked += 1;
            *source_agent_counts.entry(agent.to_string()).or_insert(0) += 1;

            if !observed_project_root.is_empty() && observed_project_root != normalized_repo {
                store.upsert_repo_alias_for_repo_id(
                    &repo_id,
                    &observed_project_root,
                    "observed",
                    0.72,
                )?;
            }
        }
    }

    let mut source_agents = source_agent_counts
        .into_iter()
        .map(
            |(source_agent, conversation_count)| AgentConversationCount {
                source_agent,
                conversation_count,
            },
        )
        .collect::<Vec<_>>();
    source_agents.sort_by(|left, right| left.source_agent.cmp(&right.source_agent));
    let unmatched_project_roots = build_unmatched_project_roots(unmatched_project_root_counts);

    let mut warnings = Vec::new();
    if linked == 0 && scanned > 0 {
        warnings.push(
            "ChatMem scanned local conversations but none matched this repo root; verify project paths or aliases."
                .to_string(),
        );
    }

    let report = RepoScanReport {
        repo_root: normalized_requested_repo,
        canonical_repo_root: normalized_repo,
        scanned_conversation_count: scanned,
        linked_conversation_count: linked,
        skipped_conversation_count: skipped,
        source_agents,
        unmatched_project_roots,
        warnings,
        scanned_at: chrono::Utc::now().to_rfc3339(),
    };
    store.record_repo_scan_report(&report)?;

    Ok(report)
}

pub(crate) fn record_unmatched_project_root(
    counts: &mut HashMap<(String, String), usize>,
    agent: &str,
    project_dir: &str,
) {
    let project_root = normalize_observed_project_root(project_dir);
    if project_root.is_empty() {
        return;
    }

    *counts.entry((agent.to_string(), project_root)).or_insert(0) += 1;
}

pub(crate) fn build_unmatched_project_roots(
    counts: HashMap<(String, String), usize>,
) -> Vec<ObservedProjectRootCount> {
    let mut roots = counts
        .into_iter()
        .map(
            |((source_agent, project_root), conversation_count)| ObservedProjectRootCount {
                source_agent,
                project_root,
                conversation_count,
            },
        )
        .collect::<Vec<_>>();

    roots.sort_by(|left, right| {
        right
            .conversation_count
            .cmp(&left.conversation_count)
            .then_with(|| left.project_root.cmp(&right.project_root))
            .then_with(|| left.source_agent.cmp(&right.source_agent))
    });
    roots.truncate(8);
    roots
}

fn normalize_observed_project_root(project_dir: &str) -> String {
    let trimmed = project_dir.trim();
    if trimmed.starts_with("gemini:") {
        return trimmed.to_string();
    }

    crate::chatmem_memory::repo_identity::normalize_repo_root(trimmed)
}

#[cfg(test)]
pub(crate) fn summary_project_matches_repo(
    agent: &str,
    project_dir: &str,
    repo_root: &str,
) -> bool {
    summary_project_matches_repo_roots(agent, project_dir, &[repo_root.to_string()])
}

pub(crate) fn summary_project_matches_repo_roots(
    agent: &str,
    project_dir: &str,
    repo_roots: &[String],
) -> bool {
    let normalized_project = crate::chatmem_memory::repo_identity::normalize_repo_root(project_dir);
    if repo_roots.iter().any(|repo_root| {
        crate::chatmem_memory::repo_identity::normalize_repo_root(repo_root) == normalized_project
    }) {
        return true;
    }

    if agent != "gemini" {
        return false;
    }

    let Some(project_hash) = project_dir.strip_prefix("gemini:") else {
        return false;
    };

    repo_roots.iter().any(|repo_root| {
        let normalized_repo = crate::chatmem_memory::repo_identity::normalize_repo_root(repo_root);
        gemini_repo_hash_candidates(repo_root, &normalized_repo).contains(project_hash)
    })
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
    use super::{
        build_unmatched_project_roots, import_local_history_from_adapters,
        is_codex_generated_chat_project_dir, is_root_project_placeholder,
        record_unmatched_project_root, summary_project_matches_repo,
        summary_project_matches_repo_roots,
    };
    use crate::chatmem_memory::store::MemoryStore;
    use agentswap_core::{
        adapter::AgentAdapter,
        types::{AgentKind, Conversation, ConversationSummary, Message, Role},
    };
    use agentswap_gemini::GeminiAdapter;
    use chrono::Utc;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use uuid::Uuid;

    struct FakeAdapter {
        kind: AgentKind,
        conversations: Vec<Conversation>,
    }

    impl FakeAdapter {
        fn new(kind: AgentKind, conversations: Vec<Conversation>) -> Self {
            Self {
                kind,
                conversations,
            }
        }
    }

    impl AgentAdapter for FakeAdapter {
        fn is_available(&self) -> bool {
            true
        }

        fn list_conversations(&self) -> anyhow::Result<Vec<ConversationSummary>> {
            Ok(self
                .conversations
                .iter()
                .map(|conversation| ConversationSummary {
                    id: conversation.id.clone(),
                    source_agent: conversation.source_agent.clone(),
                    project_dir: conversation.project_dir.clone(),
                    created_at: conversation.created_at,
                    updated_at: conversation.updated_at,
                    summary: conversation.summary.clone(),
                    message_count: conversation.messages.len(),
                    file_count: conversation.file_changes.len(),
                })
                .collect())
        }

        fn read_conversation(&self, id: &str) -> anyhow::Result<Conversation> {
            self.conversations
                .iter()
                .find(|conversation| conversation.id == id)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("conversation {id} not found"))
        }

        fn write_conversation(&self, _conv: &Conversation) -> anyhow::Result<String> {
            unimplemented!()
        }

        fn delete_conversation(&self, _id: &str) -> anyhow::Result<()> {
            unimplemented!()
        }

        fn render_prompt(&self, _conv: &Conversation) -> anyhow::Result<String> {
            Ok(String::new())
        }

        fn agent_kind(&self) -> AgentKind {
            self.kind.clone()
        }

        fn display_name(&self) -> &str {
            match self.kind {
                AgentKind::Claude => "Claude",
                AgentKind::Codex => "Codex",
                AgentKind::Gemini => "Gemini",
                AgentKind::OpenCode => "OpenCode",
            }
        }

        fn data_dir(&self) -> PathBuf {
            PathBuf::new()
        }
    }

    fn new_store() -> MemoryStore {
        let path =
            std::env::temp_dir().join(format!("chatmem-sync-test-{}.sqlite", uuid::Uuid::new_v4()));
        MemoryStore::new(path).unwrap()
    }

    fn fake_conversation(
        id: &str,
        source_agent: AgentKind,
        project_dir: &str,
        content: &str,
    ) -> Conversation {
        let now = Utc::now();
        Conversation {
            id: id.to_string(),
            source_agent,
            project_dir: project_dir.to_string(),
            created_at: now,
            updated_at: now,
            summary: Some(content.to_string()),
            messages: vec![Message {
                id: Uuid::new_v4(),
                timestamp: now,
                role: Role::User,
                content: content.to_string(),
                tool_calls: vec![],
                metadata: HashMap::new(),
            }],
            file_changes: vec![],
        }
    }

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

    #[test]
    fn unmatched_project_roots_are_grouped_for_scan_diagnostics() {
        let mut counts = HashMap::new();

        record_unmatched_project_root(&mut counts, "codex", "D:\\VSP\\bm.md");
        record_unmatched_project_root(&mut counts, "codex", "d:/vsp/bm.md/");
        record_unmatched_project_root(&mut counts, "claude", "D:\\VSP\\other");

        let roots = build_unmatched_project_roots(counts);

        assert_eq!(roots.len(), 2);
        assert_eq!(roots[0].source_agent, "codex");
        assert_eq!(roots[0].project_root, "d:/vsp/bm.md");
        assert_eq!(roots[0].conversation_count, 2);
        assert_eq!(roots[1].source_agent, "claude");
        assert_eq!(roots[1].project_root, "d:/vsp/other");
        assert_eq!(roots[1].conversation_count, 1);
    }

    #[test]
    fn manual_repo_alias_matches_project_root_during_scan() {
        let repo_roots = vec![
            "d:/vsp/agentswap-gui".to_string(),
            "d:/vsp/bm.md".to_string(),
        ];

        assert!(summary_project_matches_repo_roots(
            "codex",
            "D:\\VSP\\bm.md",
            &repo_roots,
        ));
    }

    #[test]
    fn codex_desktop_generated_chat_paths_are_standalone() {
        assert!(is_codex_generated_chat_project_dir(
            "codex",
            r"C:\Users\Liang\Documents\Codex\2026-04-25\new-chat"
        ));
        assert!(is_codex_generated_chat_project_dir(
            "codex",
            r"\\?\C:\Users\Liang\Documents\Codex\2026-04-25\new-chat-2"
        ));
        assert!(is_codex_generated_chat_project_dir(
            "codex",
            "C:/Users/Liang/Documents/Codex/2026-04-21-new-chat-3"
        ));
        assert!(is_codex_generated_chat_project_dir("codex", "C:"));
        assert!(is_codex_generated_chat_project_dir("codex", "C:/"));
        assert!(is_codex_generated_chat_project_dir("codex", "/"));
        assert!(is_root_project_placeholder("C:"));
        assert!(is_root_project_placeholder("/"));
        assert!(!is_codex_generated_chat_project_dir("codex", "D:/VSP"));
        assert!(!is_root_project_placeholder("D:/VSP"));
        assert!(!is_codex_generated_chat_project_dir(
            "claude",
            "C:/Users/Liang/Documents/Codex/2026-04-25/new-chat-2"
        ));
    }

    #[test]
    fn full_local_history_import_indexes_each_project_once() {
        let store = new_store();
        let codex_conversation = fake_conversation(
            "codex-easymd",
            AgentKind::Codex,
            "D:\\VSP\\bm.md",
            "讨论 EasyMD 的本地历史导入",
        );
        let claude_conversation = fake_conversation(
            "claude-vsp",
            AgentKind::Claude,
            "D:\\VSP\\agentswap-gui",
            "讨论 ChatMem 的本地历史卡片",
        );

        let report = import_local_history_from_adapters(
            &store,
            vec![
                (
                    "codex",
                    Box::new(FakeAdapter::new(AgentKind::Codex, vec![codex_conversation])),
                ),
                (
                    "claude",
                    Box::new(FakeAdapter::new(
                        AgentKind::Claude,
                        vec![claude_conversation],
                    )),
                ),
            ],
        )
        .unwrap();

        assert_eq!(report.scanned_conversation_count, 2);
        assert_eq!(report.imported_conversation_count, 2);
        assert_eq!(report.indexed_repo_count, 2);
        assert!(report
            .imported_project_roots
            .iter()
            .any(|root| { root.project_root == "d:/vsp/bm.md" && root.conversation_count == 1 }));

        let easymd_health = store.repo_memory_health("d:/vsp/bm.md").unwrap();
        assert_eq!(easymd_health.indexed_chunk_count, 1);
        assert_eq!(
            easymd_health.conversation_counts_by_agent[0].source_agent,
            "codex"
        );
    }

    #[test]
    fn full_local_history_import_indexes_codex_desktop_standalone_chats_as_global_history() {
        let store = new_store();
        let standalone_chat = fake_conversation(
            "codex-standalone",
            AgentKind::Codex,
            r"C:\Users\Liang\Documents\Codex\2026-04-25\new-chat-2",
            "临时新对话",
        );
        let root_chat =
            fake_conversation("codex-root-chat", AgentKind::Codex, "C:", "根目录临时对话");
        let project_chat = fake_conversation("codex-vsp", AgentKind::Codex, "D:/VSP", "项目对话");

        let report = import_local_history_from_adapters(
            &store,
            vec![(
                "codex",
                Box::new(FakeAdapter::new(
                    AgentKind::Codex,
                    vec![standalone_chat, root_chat, project_chat],
                )),
            )],
        )
        .unwrap();

        assert_eq!(report.scanned_conversation_count, 3);
        assert_eq!(report.imported_conversation_count, 3);
        assert_eq!(report.skipped_conversation_count, 0);
        assert_eq!(report.indexed_repo_count, 2);
        assert!(report.imported_project_roots.iter().any(|root| {
            root.project_root == crate::chatmem_memory::repo_identity::GLOBAL_LOCAL_HISTORY_ROOT
                && root.conversation_count == 2
        }));
        assert!(report
            .imported_project_roots
            .iter()
            .any(|root| root.project_root == "d:/vsp" && root.conversation_count == 1));
        assert!(!report
            .imported_project_roots
            .iter()
            .any(|root| root.project_root.contains("documents/codex")));
    }

    #[test]
    fn full_local_history_import_indexes_standalone_opencode_chats() {
        let store = new_store();
        let standalone_chat = fake_conversation(
            "opencode-qtx-sponge",
            AgentKind::OpenCode,
            "",
            "光头强与海绵宝宝的对决故事讲解",
        );

        let report = import_local_history_from_adapters(
            &store,
            vec![(
                "opencode",
                Box::new(FakeAdapter::new(AgentKind::OpenCode, vec![standalone_chat])),
            )],
        )
        .unwrap();

        assert_eq!(report.scanned_conversation_count, 1);
        assert_eq!(report.imported_conversation_count, 1);
        assert_eq!(report.skipped_conversation_count, 0);
        assert_eq!(report.indexed_repo_count, 1);

        let matches = store
            .search_history("d:/vsp", "光头强和海绵宝宝", 5)
            .unwrap();
        assert!(
            matches
                .iter()
                .any(|item| item.conversation_id.as_deref() == Some("opencode:opencode-qtx-sponge")),
            "expected standalone OpenCode history to be searchable from a repo, got {matches:#?}"
        );
    }
}
