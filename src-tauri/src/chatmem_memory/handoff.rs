use super::models::{ApprovedMemoryResponse, EpisodeResponse, HandoffPacketResponse};

pub fn derive_goal(goal_hint: Option<&str>, latest_summary: Option<&str>) -> String {
    goal_hint
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .or_else(|| {
            latest_summary
                .filter(|value| !value.trim().is_empty())
                .map(ToString::to_string)
        })
        .unwrap_or_else(|| "继续当前仓库工作".to_string())
}

pub fn summarize_done_item(latest_summary: Option<&str>) -> Vec<String> {
    latest_summary
        .filter(|value| !value.trim().is_empty())
        .map(|value| vec![format!("已查看最近对话：{value}")])
        .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
pub fn build_handoff_packet(
    repo_root: &str,
    from_agent: &str,
    to_agent: &str,
    current_goal: String,
    done_items: Vec<String>,
    next_items: Vec<String>,
    key_files: Vec<String>,
    useful_commands: Vec<String>,
    related_memories: Vec<ApprovedMemoryResponse>,
    related_episodes: Vec<EpisodeResponse>,
    target_profile: Option<&str>,
) -> HandoffPacketResponse {
    HandoffPacketResponse {
        handoff_id: uuid::Uuid::new_v4().to_string(),
        repo_root: repo_root.to_string(),
        from_agent: from_agent.to_string(),
        to_agent: to_agent.to_string(),
        status: "draft".to_string(),
        checkpoint_id: None,
        target_profile: target_profile.map(ToString::to_string),
        compression_strategy: None,
        current_goal,
        done_items,
        next_items,
        key_files,
        useful_commands,
        related_memories,
        related_episodes,
        consumed_at: None,
        consumed_by: None,
        created_at: chrono::Utc::now().to_rfc3339(),
    }
}

#[cfg(test)]
mod tests {
    use super::{build_handoff_packet, derive_goal, summarize_done_item};

    #[test]
    fn handoff_packet_prefers_recent_commands_and_next_steps() {
        let goal = derive_goal(Some("Finish the MCP search tool"), Some("Previous summary"));
        let done = summarize_done_item(Some("Schema migrated"));

        assert_eq!(goal, "Finish the MCP search tool");
        assert_eq!(done.len(), 1);
    }

    #[test]
    fn handoff_builder_sets_default_lifecycle_fields() {
        let packet = build_handoff_packet(
            "d:/vsp/agentswap-gui",
            "codex",
            "claude",
            "Finish Task 1".to_string(),
            vec!["Schema updated".to_string()],
            vec!["Run targeted tests".to_string()],
            vec!["src-tauri/src/chatmem_memory/db.rs".to_string()],
            vec!["cargo test migrations_add_handoff_lifecycle_columns".to_string()],
            vec![],
            vec![],
            Some("claude_contextual"),
        );

        assert_eq!(packet.status, "draft");
        assert_eq!(packet.target_profile.as_deref(), Some("claude_contextual"));
        assert_eq!(packet.checkpoint_id, None);
        assert_eq!(packet.consumed_at, None);
    }
}
