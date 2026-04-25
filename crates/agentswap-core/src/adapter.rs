use crate::types::{AgentKind, Conversation, ConversationSummary};
use anyhow::Result;
use std::path::PathBuf;

/// Trait that each agent adapter must implement to support reading and writing
/// conversations in the Universal Conversation Format.
pub trait AgentAdapter {
    /// Returns true if the agent's data directory exists and is accessible.
    fn is_available(&self) -> bool;

    /// Lists all conversations found in the agent's data directory.
    fn list_conversations(&self) -> Result<Vec<ConversationSummary>>;

    /// Reads a single conversation by its identifier.
    fn read_conversation(&self, id: &str) -> Result<Conversation>;

    /// Writes a conversation into the agent's native format, returning the new id.
    fn write_conversation(&self, conv: &Conversation) -> Result<String>;

    /// Moves a conversation's local files to the system Trash and removes it from the agent index.
    fn delete_conversation(&self, id: &str) -> Result<()>;

    /// Renders a conversation as a human-readable prompt string.
    fn render_prompt(&self, conv: &Conversation) -> Result<String>;

    /// Returns which agent kind this adapter handles.
    fn agent_kind(&self) -> AgentKind;

    /// Returns a human-readable display name for this adapter.
    fn display_name(&self) -> &str;

    /// Returns the path to the agent's data directory.
    fn data_dir(&self) -> PathBuf;
}
