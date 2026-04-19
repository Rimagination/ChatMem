pub fn derive_goal(goal_hint: Option<&str>, latest_summary: Option<&str>) -> String {
    goal_hint
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .or_else(|| latest_summary.filter(|value| !value.trim().is_empty()).map(ToString::to_string))
        .unwrap_or_else(|| "Continue repository work".to_string())
}

pub fn summarize_done_item(latest_summary: Option<&str>) -> Vec<String> {
    latest_summary
        .filter(|value| !value.trim().is_empty())
        .map(|value| vec![format!("Reviewed latest conversation: {value}")])
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{derive_goal, summarize_done_item};

    #[test]
    fn handoff_packet_prefers_recent_commands_and_next_steps() {
        let goal = derive_goal(Some("Finish the MCP search tool"), Some("Previous summary"));
        let done = summarize_done_item(Some("Schema migrated"));

        assert_eq!(goal, "Finish the MCP search tool");
        assert_eq!(done.len(), 1);
    }
}
