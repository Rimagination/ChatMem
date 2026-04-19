use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceRef {
    pub evidence_id: Option<String>,
    pub conversation_id: Option<String>,
    pub message_id: Option<String>,
    pub tool_call_id: Option<String>,
    pub file_change_id: Option<String>,
    pub excerpt: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApprovedMemoryResponse {
    pub memory_id: String,
    pub kind: String,
    pub title: String,
    pub value: String,
    pub usage_hint: String,
    pub status: String,
    pub last_verified_at: Option<String>,
    pub selected_because: Option<String>,
    pub evidence_refs: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct MemoryCandidateResponse {
    pub candidate_id: String,
    pub kind: String,
    pub summary: String,
    pub value: String,
    pub why_it_matters: String,
    pub confidence: f64,
    pub proposed_by: String,
    pub status: String,
    pub created_at: String,
    pub evidence_refs: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct EpisodeResponse {
    pub episode_id: String,
    pub title: String,
    pub summary: String,
    pub outcome: String,
    pub created_at: String,
    pub source_conversation_id: String,
    pub evidence_refs: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HandoffPacketResponse {
    pub handoff_id: String,
    pub repo_root: String,
    pub from_agent: String,
    pub to_agent: String,
    pub current_goal: String,
    pub done_items: Vec<String>,
    pub next_items: Vec<String>,
    pub key_files: Vec<String>,
    pub useful_commands: Vec<String>,
    pub related_memories: Vec<ApprovedMemoryResponse>,
    pub related_episodes: Vec<EpisodeResponse>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchHistoryMatch {
    pub r#type: String,
    pub title: String,
    pub summary: String,
    pub why_matched: String,
    pub score: f64,
    pub evidence_refs: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchHistoryPayload {
    pub matches: Vec<SearchHistoryMatch>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListMemoryCandidatesPayload {
    pub candidates: Vec<MemoryCandidateResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RepoMemoryPayload {
    pub repo_summary: String,
    pub approved_memories: Vec<ApprovedMemoryResponse>,
    pub priority_gotchas: Vec<ApprovedMemoryResponse>,
    pub recent_handoff: Option<HandoffPacketResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetRepoMemoryInput {
    pub repo_root: String,
    pub agent: String,
    pub task_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SearchRepoHistoryInput {
    pub repo_root: String,
    pub query: String,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateMemoryCandidateInput {
    pub repo_root: String,
    pub kind: String,
    pub summary: String,
    pub value: String,
    pub why_it_matters: String,
    pub evidence_refs: Vec<EvidenceRef>,
    pub confidence: f64,
    pub proposed_by: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct CreateMemoryCandidateResult {
    pub candidate_id: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ListMemoryCandidatesInput {
    pub repo_root: String,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct BuildHandoffPacketInput {
    pub repo_root: String,
    pub from_agent: String,
    pub to_agent: String,
    pub goal_hint: Option<String>,
}
