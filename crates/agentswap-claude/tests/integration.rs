//! Integration tests for the Claude Code adapter.
//!
//! These tests use real Claude session data from `~/.claude/projects/` when available.
//! Tests are skipped if Claude is not installed or no session files exist.

use agentswap_claude::ClaudeAdapter;
use agentswap_core::adapter::AgentAdapter;
use agentswap_core::types::*;

fn skip_if_claude_not_installed() -> Option<ClaudeAdapter> {
    let adapter = ClaudeAdapter::new();
    if adapter.is_available() {
        Some(adapter)
    } else {
        eprintln!("Skipping: Claude Code not installed (~/.claude/projects/ not found)");
        None
    }
}

/// Find a conversation that actually has messages (skip empty/in-progress sessions).
fn find_conversation_with_messages(
    adapter: &ClaudeAdapter,
) -> Option<agentswap_core::types::Conversation> {
    let convos = adapter.list_conversations().ok()?;
    for summary in &convos {
        if summary.message_count == 0 {
            continue;
        }
        if let Ok(conv) = adapter.read_conversation(&summary.id) {
            if !conv.messages.is_empty() {
                return Some(conv);
            }
        }
    }
    None
}

#[test]
fn test_real_is_available() {
    let adapter = ClaudeAdapter::new();
    // Just verify it doesn't panic
    let _available = adapter.is_available();
}

#[test]
fn test_real_list_conversations() {
    let adapter = match skip_if_claude_not_installed() {
        Some(a) => a,
        None => return,
    };

    let convos = adapter.list_conversations().unwrap();
    assert!(
        !convos.is_empty(),
        "Expected at least one conversation in ~/.claude/projects/"
    );

    // Verify all entries have sensible fields
    for conv in &convos {
        assert!(!conv.id.is_empty(), "Conversation id should not be empty");
        assert_eq!(conv.source_agent, AgentKind::Claude);
        assert!(
            !conv.project_dir.is_empty(),
            "project_dir should not be empty"
        );
    }

    // Verify sorted by most recent first
    for window in convos.windows(2) {
        assert!(
            window[0].updated_at >= window[1].updated_at,
            "Conversations should be sorted by most recent first"
        );
    }

    eprintln!(
        "Found {} Claude conversations across projects",
        convos.len()
    );
}

#[test]
fn test_real_read_conversation_with_messages() {
    let adapter = match skip_if_claude_not_installed() {
        Some(a) => a,
        None => return,
    };

    let conv = match find_conversation_with_messages(&adapter) {
        Some(c) => c,
        None => {
            eprintln!("Skipping: no conversations with messages found");
            return;
        }
    };

    assert_eq!(conv.source_agent, AgentKind::Claude);
    assert!(
        !conv.messages.is_empty(),
        "Conversation should have messages"
    );

    // Verify the first message is from a user
    assert_eq!(
        conv.messages[0].role,
        Role::User,
        "First message should be from user"
    );

    eprintln!(
        "Read conversation '{}': {} messages, {} file changes",
        conv.summary.as_deref().unwrap_or(&conv.id),
        conv.messages.len(),
        conv.file_changes.len()
    );
}

#[test]
fn test_real_render_prompt() {
    let adapter = match skip_if_claude_not_installed() {
        Some(a) => a,
        None => return,
    };

    let conv = match find_conversation_with_messages(&adapter) {
        Some(c) => c,
        None => {
            eprintln!("Skipping: no conversations with messages found");
            return;
        }
    };

    let rendered = adapter.render_prompt(&conv).unwrap();

    assert!(rendered.contains("# Conversation:"), "Should have header");
    assert!(
        rendered.contains("**Source:** Claude Code"),
        "Should have source"
    );
    assert!(rendered.contains("## User"), "Should have user message");

    eprintln!("Rendered prompt: {} bytes", rendered.len());
}

#[test]
fn test_real_data_dir() {
    let adapter = ClaudeAdapter::new();
    let data_dir = adapter.data_dir();
    // Should end with .claude/projects
    let data_dir_str = data_dir.to_string_lossy();
    assert!(
        data_dir_str.ends_with(".claude/projects"),
        "data_dir should end with .claude/projects, got: {}",
        data_dir_str
    );
}

#[test]
fn test_real_agent_metadata() {
    let adapter = ClaudeAdapter::new();
    assert_eq!(adapter.agent_kind(), AgentKind::Claude);
    assert_eq!(adapter.display_name(), "Claude Code");
}
