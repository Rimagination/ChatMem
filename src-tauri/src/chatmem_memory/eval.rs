use anyhow::Result;
use serde::{Deserialize, Serialize};

use super::store::MemoryStore;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalBenchmarkCase {
    pub name: String,
    pub query: String,
    pub expected_title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalBenchmarkCaseResult {
    pub name: String,
    pub query: String,
    pub expected_title: String,
    pub matched: bool,
    pub rank: Option<usize>,
    pub top_titles: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RetrievalBenchmarkReport {
    pub total: usize,
    pub passed: usize,
    pub recall_at_k: f64,
    pub results: Vec<RetrievalBenchmarkCaseResult>,
}

pub fn evaluate_search_history(
    store: &MemoryStore,
    repo_root: &str,
    cases: &[RetrievalBenchmarkCase],
    limit: usize,
) -> Result<RetrievalBenchmarkReport> {
    let mut results = Vec::new();
    for case in cases {
        let matches = store.search_history(repo_root, &case.query, limit)?;
        let top_titles = matches
            .iter()
            .map(|item| item.title.clone())
            .collect::<Vec<_>>();
        let rank = top_titles
            .iter()
            .position(|title| title == &case.expected_title)
            .map(|index| index + 1);

        results.push(RetrievalBenchmarkCaseResult {
            name: case.name.clone(),
            query: case.query.clone(),
            expected_title: case.expected_title.clone(),
            matched: rank.is_some(),
            rank,
            top_titles,
        });
    }

    let total = results.len();
    let passed = results.iter().filter(|result| result.matched).count();
    let recall_at_k = if total == 0 {
        0.0
    } else {
        passed as f64 / total as f64
    };

    Ok(RetrievalBenchmarkReport {
        total,
        passed,
        recall_at_k,
        results,
    })
}

#[cfg(test)]
mod tests {
    use super::{evaluate_search_history, RetrievalBenchmarkCase};
    use crate::chatmem_memory::{
        models::CreateMemoryCandidateInput,
        store::{MemoryStore, ReviewAction},
    };

    fn new_store() -> MemoryStore {
        let path =
            std::env::temp_dir().join(format!("chatmem-eval-test-{}.sqlite", uuid::Uuid::new_v4()));
        MemoryStore::new(path).unwrap()
    }

    fn approve_memory(store: &MemoryStore, repo_root: &str, title: &str, value: &str) {
        let candidate_id = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "strategy".to_string(),
                summary: title.to_string(),
                value: value.to_string(),
                why_it_matters: "Used by retrieval benchmark fixtures.".to_string(),
                evidence_refs: vec![],
                confidence: 0.9,
                proposed_by: "codex".to_string(),
            })
            .unwrap();

        store
            .review_candidate(
                &candidate_id,
                ReviewAction::Approve {
                    title: title.to_string(),
                    usage_hint: "Benchmark fixture memory.".to_string(),
                },
            )
            .unwrap();
    }

    #[test]
    fn retrieval_benchmark_reports_recall_at_k_for_hybrid_search() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";
        approve_memory(
            &store,
            repo_root,
            "Remote snapshot flow",
            "WebDAV sync uploads manifest snapshots to the configured remote path.",
        );
        approve_memory(
            &store,
            repo_root,
            "Updater signing key",
            "Tauri updater signing requires TAURI_PRIVATE_KEY before packaging installers.",
        );

        let report = evaluate_search_history(
            &store,
            repo_root,
            &[
                RetrievalBenchmarkCase {
                    name: "semantic cloud backup query".to_string(),
                    query: "cloud drive backup".to_string(),
                    expected_title: "Remote snapshot flow".to_string(),
                },
                RetrievalBenchmarkCase {
                    name: "release signing query".to_string(),
                    query: "release package signature".to_string(),
                    expected_title: "Updater signing key".to_string(),
                },
            ],
            3,
        )
        .unwrap();

        assert_eq!(report.total, 2);
        assert_eq!(report.passed, 2);
        assert_eq!(report.recall_at_k, 1.0);
    }
}
