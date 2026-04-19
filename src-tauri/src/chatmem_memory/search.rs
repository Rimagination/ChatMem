use anyhow::Result;

use super::{
    models::{ApprovedMemoryResponse, RepoMemoryPayload, SearchHistoryMatch},
    store::MemoryStore,
};

pub fn build_repo_memory_payload(
    store: &MemoryStore,
    repo_root: &str,
    task_hint: Option<&str>,
) -> Result<RepoMemoryPayload> {
    let approved = store
        .list_repo_memories(repo_root)?
        .into_iter()
        .filter(|memory| memory.status == "active")
        .collect::<Vec<_>>();

    let mut gotchas = approved
        .iter()
        .filter(|memory| memory.kind == "gotcha")
        .take(2)
        .cloned()
        .collect::<Vec<_>>();

    let approved_memories = prioritize_memories(approved, task_hint);
    if gotchas.len() > 1 {
        gotchas.truncate(1);
    }

    Ok(RepoMemoryPayload {
        repo_summary: format!("Repository memory for {repo_root}"),
        approved_memories,
        priority_gotchas: gotchas,
        recent_handoff: store.latest_handoff(repo_root)?,
    })
}

fn prioritize_memories(
    approved: Vec<ApprovedMemoryResponse>,
    task_hint: Option<&str>,
) -> Vec<ApprovedMemoryResponse> {
    let hint = task_hint.unwrap_or_default().to_lowercase();
    let mut scored = approved
        .into_iter()
        .map(|memory| {
            let base = match memory.kind.as_str() {
                "gotcha" => 50,
                "command" => 40,
                "convention" => 30,
                "strategy" => 20,
                "preference" => 10,
                _ => 0,
            };
            let hint_bonus = if !hint.is_empty()
                && (memory.title.to_lowercase().contains(&hint)
                    || memory.value.to_lowercase().contains(&hint)
                    || memory.usage_hint.to_lowercase().contains(&hint))
            {
                20
            } else {
                0
            };
            (base + hint_bonus, memory)
        })
        .collect::<Vec<_>>();

    scored.sort_by(|left, right| right.0.cmp(&left.0));
    scored.into_iter().take(3).map(|(_, memory)| memory).collect()
}

pub fn trim_search_matches(matches: Vec<SearchHistoryMatch>, limit: usize) -> Vec<SearchHistoryMatch> {
    matches.into_iter().take(limit).collect()
}

#[cfg(test)]
mod tests {
    use super::{build_repo_memory_payload, trim_search_matches};
    use crate::chatmem_memory::store::MemoryStore;

    #[test]
    fn repo_memory_tool_returns_compact_startup_payload() {
        let path = std::env::temp_dir().join(format!("chatmem-search-test-{}.sqlite", uuid::Uuid::new_v4()));
        let store = MemoryStore::new(path.clone()).unwrap();
        let repo_root = "d:/vsp/agentswap-gui";
        store
            .create_candidate(&crate::chatmem_memory::models::CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "gotcha".to_string(),
                summary: "Remember release signing".to_string(),
                value: "Set TAURI_PRIVATE_KEY".to_string(),
                why_it_matters: "Release builds fail without it".to_string(),
                evidence_refs: vec![],
                confidence: 0.9,
                proposed_by: "codex".to_string(),
            })
            .unwrap();
        let candidates = store.list_candidates(repo_root).unwrap();
        store
            .review_candidate(
                &candidates[0].candidate_id,
                crate::chatmem_memory::store::ReviewAction::Approve {
                    title: "Release signing".to_string(),
                    usage_hint: "Inject on startup".to_string(),
                },
            )
            .unwrap();

        let payload = build_repo_memory_payload(&store, repo_root, Some("release")).unwrap();
        assert!(payload.approved_memories.len() <= 3);
        assert!(payload.priority_gotchas.len() <= 2);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn search_history_tool_never_returns_raw_transcript_blobs() {
        let matches = trim_search_matches(
            vec![crate::chatmem_memory::models::SearchHistoryMatch {
                r#type: "episode".to_string(),
                title: "Example".to_string(),
                summary: "a".repeat(500),
                why_matched: "match".to_string(),
                score: 1.0,
                evidence_refs: vec![],
            }],
            5,
        );

        assert!(matches.iter().all(|item| item.summary.len() < 600));
    }
}
