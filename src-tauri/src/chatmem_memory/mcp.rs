use rmcp::{
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::ErrorData as McpError,
    tool, tool_handler, Json, ServerHandler,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    checkpoints::{CheckpointRecord, CreateCheckpointInput},
    models::{
        BuildHandoffPacketInput, CreateMemoryCandidateInput, CreateMemoryCandidateResult,
        CreateMemoryMergeProposalInput, CreateMemoryMergeProposalResult, EmbeddingRebuildReport,
        EntityGraphPayload, GetProjectContextInput, GetRepoMemoryInput, HistoryConversationPayload,
        ListMemoryCandidatesInput, ListMemoryCandidatesPayload, ListWikiPagesPayload,
        LocalHistoryImportReport, MemoryConflictResponse, ProjectContextPayload,
        ReadHistoryConversationInput, RepoAliasResponse, RepoMemoryHealthResponse,
        RepoMemoryPayload, RepoScanReport, SearchHistoryPayload, SearchRepoHistoryInput,
    },
    runs::{self, ArtifactRecord, RunRecord},
    search,
    store::MemoryStore,
    sync,
};

fn internal_error(message: impl Into<String>) -> McpError {
    McpError::internal_error(message.into(), None)
}

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(test)]
static RUN_SYNC_CALLS: AtomicUsize = AtomicUsize::new(0);

fn sync_repo_state_before_run_queries(repo_root: &str) {
    #[cfg(test)]
    RUN_SYNC_CALLS.fetch_add(1, Ordering::SeqCst);

    if let Ok(app_store) = MemoryStore::open_app() {
        let _ = sync::sync_repo_conversations(&app_store, repo_root);
    }
}

#[cfg(test)]
pub(crate) fn reset_run_sync_call_count() {
    RUN_SYNC_CALLS.store(0, Ordering::SeqCst);
}

#[cfg(test)]
pub(crate) fn run_sync_call_count() -> usize {
    RUN_SYNC_CALLS.load(Ordering::SeqCst)
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct RepoRootInput {
    pub repo_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct ResumeFromCheckpointInput {
    pub checkpoint_id: String,
    pub to_agent: String,
    pub target_profile: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct ListActiveRunsPayload {
    pub runs: Vec<RunRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct ListRunArtifactsPayload {
    pub artifacts: Vec<ArtifactRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct MergeRepoAliasInput {
    pub repo_root: String,
    pub alias_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct ListMemoryConflictsInput {
    pub repo_root: String,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct ListMemoryConflictsPayload {
    pub conflicts: Vec<MemoryConflictResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct ListEntityGraphInput {
    pub repo_root: String,
    pub limit: Option<usize>,
}

#[derive(Clone)]
pub struct ChatMemMcpService {
    store: MemoryStore,
    tool_router: ToolRouter<Self>,
}

impl ChatMemMcpService {
    pub fn new(store: MemoryStore) -> Self {
        Self {
            store,
            tool_router: Self::build_tool_router(),
        }
    }

    fn build_tool_router() -> ToolRouter<Self> {
        ToolRouter::new()
            .with_route((Self::get_repo_memory_tool_attr(), Self::get_repo_memory))
            .with_route((
                Self::get_project_context_tool_attr(),
                Self::get_project_context,
            ))
            .with_route((
                Self::get_repo_memory_health_tool_attr(),
                Self::get_repo_memory_health,
            ))
            .with_route((
                Self::import_all_local_history_tool_attr(),
                Self::import_all_local_history,
            ))
            .with_route((
                Self::scan_repo_conversations_tool_attr(),
                Self::scan_repo_conversations,
            ))
            .with_route((Self::merge_repo_alias_tool_attr(), Self::merge_repo_alias))
            .with_route((
                Self::search_repo_history_tool_attr(),
                Self::search_repo_history,
            ))
            .with_route((
                Self::read_history_conversation_tool_attr(),
                Self::read_history_conversation,
            ))
            .with_route((
                Self::create_memory_candidate_tool_attr(),
                Self::create_memory_candidate,
            ))
            .with_route((
                Self::propose_memory_merge_tool_attr(),
                Self::propose_memory_merge,
            ))
            .with_route((Self::create_checkpoint_tool_attr(), Self::create_checkpoint))
            .with_route((
                Self::list_memory_candidates_tool_attr(),
                Self::list_memory_candidates,
            ))
            .with_route((
                Self::build_handoff_packet_tool_attr(),
                Self::build_handoff_packet,
            ))
            .with_route((Self::list_active_runs_tool_attr(), Self::list_active_runs))
            .with_route((
                Self::list_run_artifacts_tool_attr(),
                Self::list_run_artifacts,
            ))
            .with_route((
                Self::resume_from_checkpoint_tool_attr(),
                Self::resume_from_checkpoint,
            ))
            .with_route((
                Self::list_repo_wiki_pages_tool_attr(),
                Self::list_repo_wiki_pages,
            ))
            .with_route((Self::rebuild_repo_wiki_tool_attr(), Self::rebuild_repo_wiki))
            .with_route((
                Self::rebuild_repo_embeddings_tool_attr(),
                Self::rebuild_repo_embeddings,
            ))
            .with_route((
                Self::list_memory_conflicts_tool_attr(),
                Self::list_memory_conflicts,
            ))
            .with_route((Self::list_entity_graph_tool_attr(), Self::list_entity_graph))
    }

    pub fn debug_tool_names(&self) -> Vec<String> {
        self.tool_router
            .list_all()
            .iter()
            .map(|tool| tool.name.to_string())
            .collect()
    }

    #[tool(
        name = "get_repo_memory",
        description = "Return compact approved startup rules for an agent"
    )]
    async fn get_repo_memory(
        &self,
        Parameters(input): Parameters<GetRepoMemoryInput>,
    ) -> Result<Json<RepoMemoryPayload>, McpError> {
        let _ = sync::sync_repo_conversations(&self.store, &input.repo_root);
        search::build_repo_memory_payload(&self.store, &input.repo_root, input.task_hint.as_deref())
            .map(Json)
            .map_err(|error| internal_error(error.to_string()))
    }

    #[tool(
        name = "get_project_context",
        description = "Token-efficient first step for project recall/continuation: return approved startup rules, recent handoff, diagnostics, and small local-history evidence. Prefer limit=3 before broader search. If relevant_history has matches, name the source agent/conversation and ask whether to call read_history_conversation; do not ask the user to redescribe the topic first."
    )]
    async fn get_project_context(
        &self,
        Parameters(input): Parameters<GetProjectContextInput>,
    ) -> Result<Json<ProjectContextPayload>, McpError> {
        let _ = sync::sync_repo_conversations(&self.store, &input.repo_root);
        self.store
            .get_project_context(
                &input.repo_root,
                &input.query,
                input.intent.as_deref(),
                input.limit,
            )
            .map(Json)
            .map_err(|error| internal_error(error.to_string()))
    }

    #[tool(
        name = "get_repo_memory_health",
        description = "Return local-history diagnostics, pending startup-rule candidates, and ancestor-root drift"
    )]
    async fn get_repo_memory_health(
        &self,
        Parameters(input): Parameters<RepoRootInput>,
    ) -> Result<Json<RepoMemoryHealthResponse>, McpError> {
        let _ = sync::sync_repo_conversations(&self.store, &input.repo_root);
        self.store
            .repo_memory_health(&input.repo_root)
            .map(Json)
            .map_err(|error| internal_error(error.to_string()))
    }

    #[tool(
        name = "import_all_local_history",
        description = "Import all available Claude, Codex, Gemini, and OpenCode local conversations into the local history index. Use this after first install, after changing history locations, or when recall misses because history has not been imported."
    )]
    async fn import_all_local_history(&self) -> Result<Json<LocalHistoryImportReport>, McpError> {
        sync::import_all_local_history(&self.store)
            .map(Json)
            .map_err(|error| internal_error(error.to_string()))
    }

    #[tool(
        name = "scan_repo_conversations",
        description = "Scan local conversations for one repository, link matching history, and return unmatched project roots that may need alias merging."
    )]
    async fn scan_repo_conversations(
        &self,
        Parameters(input): Parameters<RepoRootInput>,
    ) -> Result<Json<RepoScanReport>, McpError> {
        sync::scan_repo_conversations(&self.store, &input.repo_root)
            .map(Json)
            .map_err(|error| internal_error(error.to_string()))
    }

    #[tool(
        name = "merge_repo_alias",
        description = "Add a project path alias to the current repository so future scans can link conversations stored under an old cwd, file cwd, or generated project path."
    )]
    async fn merge_repo_alias(
        &self,
        Parameters(input): Parameters<MergeRepoAliasInput>,
    ) -> Result<Json<RepoAliasResponse>, McpError> {
        self.store
            .merge_repo_alias(&input.repo_root, &input.alias_root)
            .map(Json)
            .map_err(|error| internal_error(error.to_string()))
    }

    #[tool(
        name = "search_repo_history",
        description = "Second-stage targeted hybrid keyword/vector search over indexed local history, approved startup rules, and generated wiki projections. Start with limit<=3; when matches appear, list source_agent/conversation and ask whether to call read_history_conversation. Do not treat unapproved history hits as missing context or ask the user to redescribe before offering to read them."
    )]
    async fn search_repo_history(
        &self,
        Parameters(input): Parameters<SearchRepoHistoryInput>,
    ) -> Result<Json<SearchHistoryPayload>, McpError> {
        let _ = sync::sync_repo_conversations(&self.store, &input.repo_root);
        let limit = input.limit.unwrap_or(5);
        let matches = self
            .store
            .search_history(&input.repo_root, &input.query, limit)
            .map_err(|error| internal_error(error.to_string()))?;
        Ok(Json(SearchHistoryPayload {
            matches: search::trim_search_matches(matches, limit),
        }))
    }

    #[tool(
        name = "read_history_conversation",
        description = "Read a compact message window from an indexed local conversation after get_project_context/search_repo_history returns a conversation_id. Use this when the user wants to recall a found conversation; pass message_id from evidence_refs when available, or query for a low-token focused window."
    )]
    async fn read_history_conversation(
        &self,
        Parameters(input): Parameters<ReadHistoryConversationInput>,
    ) -> Result<Json<HistoryConversationPayload>, McpError> {
        let _ = sync::sync_repo_conversations(&self.store, &input.repo_root);
        self.store
            .read_history_conversation(
                &input.repo_root,
                &input.conversation_id,
                input.message_id.as_deref(),
                input.query.as_deref(),
                input.limit,
            )
            .map(Json)
            .map_err(|error| internal_error(error.to_string()))
    }

    #[tool(
        name = "create_memory_candidate",
        description = "Create a pending startup-rule candidate. For Chinese-speaking users, write prose fields in Chinese while preserving exact commands, paths, function names, config keys, model names, and tool names."
    )]
    async fn create_memory_candidate(
        &self,
        Parameters(input): Parameters<CreateMemoryCandidateInput>,
    ) -> Result<Json<CreateMemoryCandidateResult>, McpError> {
        let candidate_id = self
            .store
            .create_candidate(&input)
            .map_err(|error| internal_error(error.to_string()))?;

        Ok(Json(CreateMemoryCandidateResult {
            candidate_id,
            status: "pending_review".to_string(),
        }))
    }

    #[tool(
        name = "propose_memory_merge",
        description = "Create an agent-authored merge rewrite proposal for human review; does not approve or update memory. For Chinese-speaking users, write prose fields in Chinese while preserving exact technical tokens."
    )]
    async fn propose_memory_merge(
        &self,
        Parameters(input): Parameters<CreateMemoryMergeProposalInput>,
    ) -> Result<Json<CreateMemoryMergeProposalResult>, McpError> {
        let proposal_id = self
            .store
            .propose_memory_merge(&input)
            .map_err(|error| internal_error(error.to_string()))?;

        Ok(Json(CreateMemoryMergeProposalResult {
            proposal_id,
            status: "pending_review".to_string(),
        }))
    }

    #[tool(
        name = "create_checkpoint",
        description = "Freeze the current repo context into a resumable checkpoint. For Chinese-speaking users, write checkpoint summaries in Chinese while preserving exact technical tokens."
    )]
    async fn create_checkpoint(
        &self,
        Parameters(input): Parameters<CreateCheckpointInput>,
    ) -> Result<Json<CheckpointRecord>, McpError> {
        self.store
            .create_checkpoint(&input)
            .map(Json)
            .map_err(|error| internal_error(error.to_string()))
    }

    #[tool(
        name = "list_memory_candidates",
        description = "List pending or filtered startup-rule candidates"
    )]
    async fn list_memory_candidates(
        &self,
        Parameters(input): Parameters<ListMemoryCandidatesInput>,
    ) -> Result<Json<ListMemoryCandidatesPayload>, McpError> {
        self.store
            .list_candidates_with_status(&input.repo_root, input.status.as_deref())
            .map(|candidates| Json(ListMemoryCandidatesPayload { candidates }))
            .map_err(|error| internal_error(error.to_string()))
    }

    #[tool(
        name = "list_memory_conflicts",
        description = "List open or filtered memory candidate conflicts that need review"
    )]
    async fn list_memory_conflicts(
        &self,
        Parameters(input): Parameters<ListMemoryConflictsInput>,
    ) -> Result<Json<ListMemoryConflictsPayload>, McpError> {
        self.store
            .list_memory_conflicts(&input.repo_root, input.status.as_deref())
            .map(|conflicts| Json(ListMemoryConflictsPayload { conflicts }))
            .map_err(|error| internal_error(error.to_string()))
    }

    #[tool(
        name = "build_handoff_packet",
        description = "Build and save a repository handoff packet for agent switching. For Chinese-speaking users, write goals and handoff prose in Chinese while preserving exact technical tokens."
    )]
    async fn build_handoff_packet(
        &self,
        Parameters(input): Parameters<BuildHandoffPacketInput>,
    ) -> Result<Json<super::models::HandoffPacketResponse>, McpError> {
        let _ = sync::sync_repo_conversations(&self.store, &input.repo_root);
        self.store
            .build_and_store_handoff_for_target_profile(
                &input.repo_root,
                &input.from_agent,
                &input.to_agent,
                input.goal_hint.as_deref(),
                input.target_profile.as_deref(),
            )
            .map(Json)
            .map_err(|error| internal_error(error.to_string()))
    }

    #[tool(
        name = "list_active_runs",
        description = "List active repository runs that still need attention"
    )]
    async fn list_active_runs(
        &self,
        Parameters(input): Parameters<RepoRootInput>,
    ) -> Result<Json<ListActiveRunsPayload>, McpError> {
        sync_repo_state_before_run_queries(&input.repo_root);
        runs::list_runs(&input.repo_root)
            .map(|runs| {
                Json(ListActiveRunsPayload {
                    runs: runs
                        .into_iter()
                        .filter(|run| run.status != "completed")
                        .collect(),
                })
            })
            .map_err(|error| internal_error(error.to_string()))
    }

    #[tool(
        name = "list_run_artifacts",
        description = "List artifacts produced by repository runs"
    )]
    async fn list_run_artifacts(
        &self,
        Parameters(input): Parameters<RepoRootInput>,
    ) -> Result<Json<ListRunArtifactsPayload>, McpError> {
        sync_repo_state_before_run_queries(&input.repo_root);
        runs::list_artifacts(&input.repo_root)
            .map(|artifacts| Json(ListRunArtifactsPayload { artifacts }))
            .map_err(|error| internal_error(error.to_string()))
    }

    #[tool(
        name = "list_repo_wiki_pages",
        description = "List generated repository wiki projection pages; approved memory remains the source of truth"
    )]
    async fn list_repo_wiki_pages(
        &self,
        Parameters(input): Parameters<RepoRootInput>,
    ) -> Result<Json<ListWikiPagesPayload>, McpError> {
        self.store
            .list_wiki_pages(&input.repo_root)
            .map(|pages| Json(ListWikiPagesPayload { pages }))
            .map_err(|error| internal_error(error.to_string()))
    }

    #[tool(
        name = "rebuild_repo_wiki",
        description = "Rebuild generated repository wiki projection pages from approved memory and episodes"
    )]
    async fn rebuild_repo_wiki(
        &self,
        Parameters(input): Parameters<RepoRootInput>,
    ) -> Result<Json<ListWikiPagesPayload>, McpError> {
        let _ = sync::sync_repo_conversations(&self.store, &input.repo_root);
        self.store
            .rebuild_repo_wiki(&input.repo_root)
            .map(|pages| Json(ListWikiPagesPayload { pages }))
            .map_err(|error| internal_error(error.to_string()))
    }

    #[tool(
        name = "rebuild_repo_embeddings",
        description = "Rebuild repository vector embeddings using the configured provider, keeping local hash fallback vectors"
    )]
    async fn rebuild_repo_embeddings(
        &self,
        Parameters(input): Parameters<RepoRootInput>,
    ) -> Result<Json<EmbeddingRebuildReport>, McpError> {
        let _ = sync::sync_repo_conversations(&self.store, &input.repo_root);
        self.store
            .rebuild_repo_embeddings(&input.repo_root)
            .map(Json)
            .map_err(|error| internal_error(error.to_string()))
    }

    #[tool(
        name = "list_entity_graph",
        description = "List lightweight repository entity graph nodes and links extracted from memory/search documents"
    )]
    async fn list_entity_graph(
        &self,
        Parameters(input): Parameters<ListEntityGraphInput>,
    ) -> Result<Json<EntityGraphPayload>, McpError> {
        let limit = input.limit.unwrap_or(25);
        self.store
            .list_entity_graph(&input.repo_root, limit)
            .map(Json)
            .map_err(|error| internal_error(error.to_string()))
    }

    #[tool(
        name = "resume_from_checkpoint",
        description = "Resume repository work by promoting a checkpoint into a handoff packet"
    )]
    async fn resume_from_checkpoint(
        &self,
        Parameters(input): Parameters<ResumeFromCheckpointInput>,
    ) -> Result<Json<super::models::HandoffPacketResponse>, McpError> {
        self.store
            .build_and_store_handoff_from_checkpoint(
                &input.checkpoint_id,
                "",
                &input.to_agent,
                None,
                input.target_profile.as_deref(),
            )
            .map(Json)
            .map_err(|error| internal_error(error.to_string()))
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for ChatMemMcpService {}

#[cfg(test)]
mod tests {
    use super::{
        reset_run_sync_call_count, run_sync_call_count, ChatMemMcpService, MergeRepoAliasInput,
        RepoRootInput,
    };
    use crate::chatmem_memory::{
        checkpoints::CreateCheckpointInput,
        models::{
            BuildHandoffPacketInput, CreateMemoryCandidateInput, CreateMemoryMergeProposalInput,
            ListMemoryCandidatesInput, ListMemoryCandidatesPayload,
        },
        store::{MemoryStore, ReviewAction},
    };
    use rmcp::{handler::server::wrapper::Parameters, Json};
    use schemars::schema_for;
    use std::collections::BTreeSet;

    fn new_store() -> MemoryStore {
        let path =
            std::env::temp_dir().join(format!("chatmem-mcp-test-{}.sqlite", uuid::Uuid::new_v4()));
        MemoryStore::new(path).unwrap()
    }

    #[test]
    fn service_initializes_without_panicking() {
        let store = new_store();
        let result = std::panic::catch_unwind(|| ChatMemMcpService::new(store));
        assert!(result.is_ok(), "ChatMemMcpService::new should not panic");
    }

    #[test]
    fn list_memory_candidates_payload_schema_has_object_root() {
        let schema = schema_for!(ListMemoryCandidatesPayload);
        let schema_json = serde_json::to_value(&schema).unwrap();

        assert_eq!(
            schema_json.get("type").and_then(|value| value.as_str()),
            Some("object")
        );
        assert!(schema_json
            .get("properties")
            .and_then(|value| value.get("candidates"))
            .is_some());
    }

    #[test]
    fn resume_from_checkpoint_tool_schema_only_exposes_effective_inputs() {
        let schema_json = serde_json::to_value(
            ChatMemMcpService::resume_from_checkpoint_tool_attr().input_schema,
        )
        .unwrap();

        let properties = schema_json
            .get("properties")
            .and_then(|value| value.as_object())
            .unwrap();

        assert!(properties.contains_key("checkpoint_id"));
        assert!(properties.contains_key("to_agent"));
        assert!(properties.contains_key("target_profile"));
        assert!(!properties.contains_key("from_agent"));
        assert!(!properties.contains_key("goal_hint"));
    }

    #[test]
    fn debug_tool_names_reflect_actual_router_registrations() {
        let service = ChatMemMcpService::new(new_store());
        let names = service
            .debug_tool_names()
            .into_iter()
            .collect::<BTreeSet<_>>();
        let expected_names = [
            ChatMemMcpService::get_repo_memory_tool_attr().name,
            ChatMemMcpService::get_project_context_tool_attr().name,
            ChatMemMcpService::get_repo_memory_health_tool_attr().name,
            ChatMemMcpService::import_all_local_history_tool_attr().name,
            ChatMemMcpService::scan_repo_conversations_tool_attr().name,
            ChatMemMcpService::merge_repo_alias_tool_attr().name,
            ChatMemMcpService::search_repo_history_tool_attr().name,
            ChatMemMcpService::read_history_conversation_tool_attr().name,
            ChatMemMcpService::create_memory_candidate_tool_attr().name,
            ChatMemMcpService::propose_memory_merge_tool_attr().name,
            ChatMemMcpService::create_checkpoint_tool_attr().name,
            ChatMemMcpService::list_memory_candidates_tool_attr().name,
            ChatMemMcpService::build_handoff_packet_tool_attr().name,
            ChatMemMcpService::list_active_runs_tool_attr().name,
            ChatMemMcpService::list_run_artifacts_tool_attr().name,
            ChatMemMcpService::resume_from_checkpoint_tool_attr().name,
            ChatMemMcpService::list_repo_wiki_pages_tool_attr().name,
            ChatMemMcpService::rebuild_repo_wiki_tool_attr().name,
            ChatMemMcpService::rebuild_repo_embeddings_tool_attr().name,
            ChatMemMcpService::list_memory_conflicts_tool_attr().name,
            ChatMemMcpService::list_entity_graph_tool_attr().name,
        ]
        .into_iter()
        .map(|name| name.to_string())
        .collect::<BTreeSet<_>>();

        for expected in &expected_names {
            assert!(
                service.tool_router.has_route(expected),
                "missing router registration: {expected}"
            );
        }

        assert_eq!(names, expected_names);
    }

    #[test]
    fn recall_tool_descriptions_require_history_followup_before_redescription() {
        let project_context_description = ChatMemMcpService::get_project_context_tool_attr()
            .description
            .expect("get_project_context should have a description");
        let search_description = ChatMemMcpService::search_repo_history_tool_attr()
            .description
            .expect("search_repo_history should have a description");
        let read_description = ChatMemMcpService::read_history_conversation_tool_attr()
            .description
            .expect("read_history_conversation should have a description");

        assert!(project_context_description.contains("source agent/conversation"));
        assert!(project_context_description.contains("redescribe"));
        assert!(search_description.contains("read_history_conversation"));
        assert!(search_description.contains("unapproved history hits"));
        assert!(read_description.contains("compact message window"));
    }

    #[tokio::test]
    async fn merge_repo_alias_tool_records_manual_alias() {
        let service = ChatMemMcpService::new(new_store());
        let Json(alias) = service
            .merge_repo_alias(Parameters(MergeRepoAliasInput {
                repo_root: "d:/vsp/agentswap-gui".to_string(),
                alias_root: "d:/vsp/easymd".to_string(),
            }))
            .await
            .unwrap();

        assert_eq!(alias.alias_root, "d:/vsp/easymd");
        assert_eq!(alias.alias_kind, "manual");
        assert_eq!(alias.confidence, 1.0);

        let Json(health) = service
            .get_repo_memory_health(Parameters(RepoRootInput {
                repo_root: "d:/vsp/agentswap-gui".to_string(),
            }))
            .await
            .unwrap();

        assert!(health
            .repo_aliases
            .iter()
            .any(|item| item.alias_root == "d:/vsp/easymd" && item.alias_kind == "manual"));
    }

    #[tokio::test]
    async fn propose_memory_merge_tool_stores_agent_authored_rewrite_for_review() {
        let service = ChatMemMcpService::new(new_store());
        let repo_root = "d:/vsp/agentswap-gui".to_string();
        let approved_candidate_id = service
            .store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.clone(),
                kind: "command".to_string(),
                summary: "Run tests before merge".to_string(),
                value: "npm run test:run".to_string(),
                why_it_matters: "Primary verification command".to_string(),
                evidence_refs: vec![],
                confidence: 0.95,
                proposed_by: "codex".to_string(),
            })
            .unwrap();
        service
            .store
            .review_candidate(
                &approved_candidate_id,
                ReviewAction::Approve {
                    title: "Primary verification".to_string(),
                    usage_hint: "Use before merge".to_string(),
                },
            )
            .unwrap();
        let memory_id = service.store.list_repo_memories(&repo_root).unwrap()[0]
            .memory_id
            .clone();
        let candidate_id = service
            .store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.clone(),
                kind: "command".to_string(),
                summary: "Run tests before release".to_string(),
                value: "npm run test:run -- --runInBand".to_string(),
                why_it_matters: "Use the serial variant before release packaging".to_string(),
                evidence_refs: vec![],
                confidence: 0.82,
                proposed_by: "claude".to_string(),
            })
            .unwrap();

        let Json(result) = service
            .propose_memory_merge(Parameters(CreateMemoryMergeProposalInput {
                repo_root: repo_root.clone(),
                candidate_id: candidate_id.clone(),
                target_memory_id: memory_id,
                proposed_title: "Primary verification".to_string(),
                proposed_value:
                    "npm run test:run\n\nBefore packaging, use npm run test:run -- --runInBand."
                        .to_string(),
                proposed_usage_hint:
                    "Use before merge; prefer the serial variant before release packaging."
                        .to_string(),
                risk_note: Some(
                    "Agent-authored rewrite; review wording before approval.".to_string(),
                ),
                proposed_by: "codex".to_string(),
                evidence_refs: vec![],
            }))
            .await
            .unwrap();

        assert_eq!(result.status, "pending_review");
        let Json(payload) = service
            .list_memory_candidates(Parameters(ListMemoryCandidatesInput {
                repo_root,
                status: Some("pending_review".to_string()),
            }))
            .await
            .unwrap();
        let proposal = payload
            .candidates
            .into_iter()
            .find(|candidate| candidate.candidate_id == candidate_id)
            .unwrap()
            .merge_suggestion
            .unwrap();

        assert_eq!(
            proposal.proposal_id.as_deref(),
            Some(result.proposal_id.as_str())
        );
        assert_eq!(proposal.proposed_by.as_deref(), Some("codex"));
    }

    #[tokio::test]
    async fn build_handoff_packet_forwards_target_profile() {
        let service = ChatMemMcpService::new(new_store());

        let Json(packet) = service
            .build_handoff_packet(Parameters(BuildHandoffPacketInput {
                repo_root: "d:/vsp/agentswap-gui".to_string(),
                from_agent: "codex".to_string(),
                to_agent: "claude".to_string(),
                goal_hint: Some("Wrap schema changes".to_string()),
                target_profile: Some("claude_contextual".to_string()),
            }))
            .await
            .unwrap();

        assert_eq!(packet.target_profile.as_deref(), Some("claude_contextual"));
    }

    #[tokio::test]
    async fn create_checkpoint_returns_an_active_checkpoint_record() {
        let service = ChatMemMcpService::new(new_store());

        let Json(checkpoint) = service
            .create_checkpoint(Parameters(CreateCheckpointInput {
                repo_root: "d:/vsp/agentswap-gui".to_string(),
                conversation_id: "claude:conv-001".to_string(),
                source_agent: "claude".to_string(),
                summary: "Freeze the current debugging state".to_string(),
                resume_command: Some("claude --resume conv-001".to_string()),
                metadata_json: None,
            }))
            .await
            .unwrap();

        assert_eq!(checkpoint.status, "active");
        assert_eq!(
            checkpoint.resume_command.as_deref(),
            Some("claude --resume conv-001")
        );
    }

    #[tokio::test]
    async fn resume_from_checkpoint_uses_checkpoint_provenance_and_goal() {
        let service = ChatMemMcpService::new(new_store());

        let Json(checkpoint) = service
            .create_checkpoint(Parameters(CreateCheckpointInput {
                repo_root: "d:/vsp/agentswap-gui".to_string(),
                conversation_id: "codex:conv-777".to_string(),
                source_agent: "codex".to_string(),
                summary: "Checkpoint-owned goal".to_string(),
                resume_command: Some("codex resume conv-777".to_string()),
                metadata_json: None,
            }))
            .await
            .unwrap();

        let Json(packet) = service
            .resume_from_checkpoint(Parameters(super::ResumeFromCheckpointInput {
                checkpoint_id: checkpoint.checkpoint_id,
                to_agent: "gemini".to_string(),
                target_profile: Some("gemini_research".to_string()),
            }))
            .await
            .unwrap();

        assert_eq!(packet.from_agent, "codex");
        assert_eq!(packet.to_agent, "gemini");
        assert_eq!(packet.current_goal, "Checkpoint-owned goal");
        assert!(packet
            .done_items
            .iter()
            .any(|item| item.contains("已从 codex checkpoint 固化上下文")));
    }

    #[tokio::test]
    async fn list_active_runs_and_artifacts_sync_repo_state_before_reading_local_store() {
        reset_run_sync_call_count();
        let repo_root = "d:/vsp/agentswap-gui";
        let service = ChatMemMcpService::new(new_store());

        let Json(runs) = service
            .list_active_runs(Parameters(RepoRootInput {
                repo_root: repo_root.to_string(),
            }))
            .await
            .unwrap();

        let Json(artifacts) = service
            .list_run_artifacts(Parameters(RepoRootInput {
                repo_root: repo_root.to_string(),
            }))
            .await
            .unwrap();

        assert!(runs.runs.is_empty());
        assert!(artifacts.artifacts.is_empty());
        assert_eq!(run_sync_call_count(), 2);
    }

    #[tokio::test]
    async fn wiki_tools_rebuild_and_return_repo_pages() {
        let service = ChatMemMcpService::new(new_store());
        let repo_root = "d:/vsp/agentswap-gui".to_string();

        let Json(rebuilt) = service
            .rebuild_repo_wiki(Parameters(RepoRootInput {
                repo_root: repo_root.clone(),
            }))
            .await
            .unwrap();
        assert!(rebuilt
            .pages
            .iter()
            .any(|page| page.slug == "project-overview"));

        let Json(listed) = service
            .list_repo_wiki_pages(Parameters(RepoRootInput { repo_root }))
            .await
            .unwrap();
        assert!(listed
            .pages
            .iter()
            .any(|page| page.slug == "project-overview"));
    }
}
