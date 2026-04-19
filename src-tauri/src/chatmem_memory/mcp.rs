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
        RepoMemoryPayload, SearchHistoryPayload, SearchRepoHistoryInput,
    },
    runs::{self, ArtifactRecord, RunRecord},
    search,
    store::MemoryStore,
    sync,
};

fn internal_error(message: impl Into<String>) -> McpError {
    McpError::internal_error(message.into(), None)
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct RepoRootInput {
    pub repo_root: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
struct ResumeFromCheckpointInput {
    pub checkpoint_id: String,
    pub from_agent: String,
    pub to_agent: String,
    pub goal_hint: Option<String>,
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
    }

    pub fn debug_tool_names(&self) -> Vec<String> {
        Self::tool_names()
            .iter()
            .map(|tool_name| (*tool_name).to_string())
            .collect()
    }

    fn tool_names() -> &'static [&'static str] {
        &[
            "get_repo_memory",
            "search_repo_history",
            "create_memory_candidate",
            "create_checkpoint",
            "list_memory_candidates",
            "build_handoff_packet",
            "list_active_runs",
            "list_run_artifacts",
            "resume_from_checkpoint",
        ]
    }

    #[tool(name = "get_repo_memory", description = "Return compact repository startup memory for an agent")]
    async fn get_repo_memory(
        &self,
        Parameters(input): Parameters<GetRepoMemoryInput>,
    ) -> Result<Json<RepoMemoryPayload>, McpError> {
        let _ = sync::sync_repo_conversations(&self.store, &input.repo_root);
        search::build_repo_memory_payload(&self.store, &input.repo_root, input.task_hint.as_deref())
            .map(Json)
            .map_err(|error| internal_error(error.to_string()))
    }

    #[tool(name = "search_repo_history", description = "Search prior repository work and memory")]
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
        runs::list_artifacts(&input.repo_root)
            .map(|artifacts| Json(ListRunArtifactsPayload { artifacts }))
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
                &input.from_agent,
                &input.to_agent,
                input.goal_hint.as_deref(),
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
    use super::ChatMemMcpService;
    use crate::chatmem_memory::{
        checkpoints::CreateCheckpointInput,
        models::{BuildHandoffPacketInput, ListMemoryCandidatesPayload},
        store::MemoryStore,
    };
    use rmcp::{Json, handler::server::wrapper::Parameters};
    use schemars::schema_for;

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
    fn debug_tool_names_retains_v1_tools_and_adds_v2_tools() {
        let service = ChatMemMcpService::new(new_store());
        let names = service.debug_tool_names();

        for expected in [
            "get_repo_memory",
            "search_repo_history",
            "create_memory_candidate",
            "list_memory_candidates",
            "build_handoff_packet",
            "list_active_runs",
            "list_run_artifacts",
            "create_checkpoint",
            "resume_from_checkpoint",
        ] {
            assert!(
                names.iter().any(|name| name == expected),
                "missing tool name: {expected}"
            );
        }
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
}
