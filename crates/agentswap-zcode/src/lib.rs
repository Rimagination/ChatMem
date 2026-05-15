//! ZCode conversation adapters.
//!
//! ZCode keeps per-engine profile directories under
//! `~/.zcode/v2/acp-config`. The conversation-capable stores currently mirror
//! Claude and Codex layouts; Gemini/OpenCode are discovered as configured
//! engines and only become readable when a conversation store appears.

pub mod adapter;

pub use adapter::{
    discover_profiles, ZCodeAdapter, ZCodeClaudeAdapter, ZCodeCodexAdapter, ZCodeGeminiAdapter,
    ZCodeOpenCodeAdapter, ZCodeProfileStatus,
};
