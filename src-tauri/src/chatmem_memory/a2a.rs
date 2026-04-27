use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
pub struct AgentCard {
    pub name: String,
    pub description: String,
    pub skills: Vec<String>,
    pub local_first: bool,
    pub surfaces: Vec<String>,
}

impl AgentCard {
    pub fn chatmem_default() -> Self {
        Self {
            name: "ChatMem".to_string(),
            description: "local-first control plane for approved repository memory, hybrid retrieval, generated wiki projections, runs, artifacts, and checkpoints"
                .to_string(),
            skills: vec![
                "memory".to_string(),
                "auto-extraction".to_string(),
                "conflict-review".to_string(),
                "entity-graph".to_string(),
                "hybrid-search".to_string(),
                "wiki".to_string(),
                "handoff".to_string(),
                "runs".to_string(),
                "artifacts".to_string(),
                "checkpoint".to_string(),
                "webdav-sync".to_string(),
            ],
            local_first: true,
            surfaces: vec![
                "desktop".to_string(),
                "mcp".to_string(),
                "a2a-lite".to_string(),
            ],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AgentCard;

    #[test]
    fn agent_card_describes_chatmem_as_control_plane() {
        let card = AgentCard::chatmem_default();

        assert_eq!(card.name, "ChatMem");
        assert!(card.skills.contains(&"handoff".to_string()));
        assert!(card.skills.contains(&"checkpoint".to_string()));
        assert!(card.skills.contains(&"auto-extraction".to_string()));
        assert!(card.skills.contains(&"conflict-review".to_string()));
        assert!(card.skills.contains(&"entity-graph".to_string()));
        assert!(card.skills.contains(&"hybrid-search".to_string()));
        assert!(card.skills.contains(&"wiki".to_string()));
        assert!(card.skills.contains(&"webdav-sync".to_string()));
        assert!(card.description.contains("local-first control plane"));
        assert!(card.local_first);
        assert!(card.surfaces.contains(&"mcp".to_string()));
    }
}
