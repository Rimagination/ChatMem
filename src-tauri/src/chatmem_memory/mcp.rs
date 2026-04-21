use rmcp::{
    Json, ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::ErrorData as McpError,
    tool, tool_handler,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{
    checkpoints::{CheckpointRecord, CreateCheckpointInput},
    models::{
        BuildHandoffPacketInput, CreateMemoryCandidateInput, CreateMemoryCandidateResult,
        GetRepoMemoryInput, ListMemoryCandidatesInput, ListMemoryCandidatesPayload,
        ListWikiPagesPayload, RepoMemoryPayload, SearchHistoryPayload, SearchRepoHistoryInput,
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
            .with_route((Self::search_repo_history_tool_attr(), Self::search_repo_history))
            .with_route((Self::create_memory_candidate_tool_attr(), Self::create_memory_candidate))
            .with_route((Self::create_checkpoint_tool_attr(), Self::create_checkpoint))
            .with_route((Self::list_memory_candidates_tool_attr(), Self::list_memory_candidates))
            .with_route((Self::build_handoff_packet_tool_attr(), Self::build_handoff_packet))
            .with_route((Self::list_active_runs_tool_attr(), Self::list_active_runs))
            .with_route((Self::list_run_artifacts_tool_attr(), Self::list_run_artifacts))
            .with_route((Self::resume_from_checkpoint_tool_attr(), Self::resume_from_checkpoint))
            .with_route((Self::list_repo_wiki_pages_tool_attr(), Self::list_repo_wiki_pages))
            .with_route((Self::rebuild_repo_wiki_tool_attr(), Self::rebuild_repo_wiki))
    }

    pub fn debug_tool_names(&self) -> Vec<String> {
        self.tool_router
            .list_all()
            .iter()
            .map(|tool| tool.name.to_string())
            .collect()
    }

    #[tool(name = "get_repo_memory", description = "Return compact approved repository startup memory for an agent")]
    async fn get_repo_memory(
        &self,
        Parameters(input): Parameters<GetRepoMemoryInput>,
    ) -> Result<Json<RepoMemoryPayload>, McpError> {
        let _ = sync::sync_repo_conversations(&self.store, &input.repo_root);
        search::build_repo_memory_payload(&self.store, &input.repo_root, input.task_hint.as_deref())
            .map(Json)
            .map_err(|error| internal_error(error.to_string()))
    }

    #[tool(name = "search_repo_history", description = "Search prior repository work, approved memory, and generated wiki projections")]
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

    #[tool(name = "create_memory_candidate", description = "Create a pending repository memory candidate")]
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

    #[tool(name = "create_checkpoint", description = "Freeze the current repo context into a resumable checkpoint")]
    async fn create_checkpoint(
        &self,
        Parameters(input): Parameters<CreateCheckpointInput>,
    ) -> Result<Json<CheckpointRecord>, McpError> {
        self.store
            .create_checkpoint(&input)
            .map(Json)
            .map_err(|error| internal_error(error.to_string()))
    }

    #[tool(name = "list_memory_candidates", description = "List pending or filtered repository memory candidates")]
    async fn list_memory_candidates(
        &self,
        Parameters(input): Parameters<ListMemoryCandidatesInput>,
    ) -> Result<Json<ListMemoryCandidatesPayload>, McpError> {
        self.store
            .list_candidates_with_status(&input.repo_root, input.status.as_deref())
            .map(|candidates| Json(ListMemoryCandidatesPayload { candidates }))
            .map_err(|error| internal_error(error.to_string()))
    }

    #[tool(name = "build_handoff_packet", description = "Build and save a repository handoff packet for agent switching")]
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

    #[tool(name = "list_active_runs", description = "List active repository runs that still need attention")]
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

    #[tool(name = "list_run_artifacts", description = "List artifacts produced by repository runs")]
    async fn list_run_artifacts(
        &self,
        Parameters(input): Parameters<RepoRootInput>,
    ) -> Result<Json<ListRunArtifactsPayload>, McpError> {
        sync_repo_state_before_run_queries(&input.repo_root);
        runs::list_artifacts(&input.repo_root)
            .map(|artifacts| Json(ListRunArtifactsPayload { artifacts }))
            .map_err(|error| internal_error(error.to_string()))
    }

    #[tool(name = "list_repo_wiki_pages", description = "List generated repository wiki projection pages; approved memory remains the source of truth")]
    async fn list_repo_wiki_pages(
        &self,
        Parameters(input): Parameters<RepoRootInput>,
    ) -> Result<Json<ListWikiPagesPayload>, McpError> {
        self.store
            .list_wiki_pages(&input.repo_root)
            .map(|pages| Json(ListWikiPagesPayload { pages }))
            .map_err(|error| internal_error(error.to_string()))
    }

    #[tool(name = "rebuild_repo_wiki", description = "Rebuild generated repository wiki projection pages from approved memory and episodes")]
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
        run_sync_call_count, reset_run_sync_call_count, ChatMemMcpService, RepoRootInput,
    };
    use crate::chatmem_memory::{
        checkpoints::CreateCheckpointInput,
        models::{BuildHandoffPacketInput, ListMemoryCandidatesPayload},
        store::MemoryStore,
    };
    use rmcp::{Json, handler::server::wrapper::Parameters};
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
            ChatMemMcpService::search_repo_history_tool_attr().name,
            ChatMemMcpService::create_memory_candidate_tool_attr().name,
            ChatMemMcpService::create_checkpoint_tool_attr().name,
            ChatMemMcpService::list_memory_candidates_tool_attr().name,
            ChatMemMcpService::build_handoff_packet_tool_attr().name,
            ChatMemMcpService::list_active_runs_tool_attr().name,
            ChatMemMcpService::list_run_artifacts_tool_attr().name,
            ChatMemMcpService::resume_from_checkpoint_tool_attr().name,
            ChatMemMcpService::list_repo_wiki_pages_tool_attr().name,
            ChatMemMcpService::rebuild_repo_wiki_tool_attr().name,
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
        assert_eq!(checkpoint.resume_command.as_deref(), Some("claude --resume conv-001"));
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
            .any(|item| item.contains("Checkpoint frozen from codex")));
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
        assert!(rebuilt.pages.iter().any(|page| page.slug == "project-overview"));

        let Json(listed) = service
            .list_repo_wiki_pages(Parameters(RepoRootInput { repo_root }))
            .await
            .unwrap();
        assert!(listed.pages.iter().any(|page| page.slug == "project-overview"));
    }
}
