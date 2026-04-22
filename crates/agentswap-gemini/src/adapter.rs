use std::collections::HashMap;
use std::fs;
use std::io::Write as IoWrite;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use uuid::Uuid;
use walkdir::WalkDir;

use agentswap_core::adapter::AgentAdapter;
use agentswap_core::tool_mapping::map_tool;
use agentswap_core::types::*;

use crate::parser::*;

/// Adapter for reading and writing Gemini CLI session files.
pub struct GeminiAdapter {
    /// The root directory containing Gemini tmp data, typically `~/.gemini/tmp`.
    tmp_dir: PathBuf,
}

impl Default for GeminiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl GeminiAdapter {
    /// Create a new GeminiAdapter using the default `~/.gemini/tmp` directory.
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        Self {
            tmp_dir: home.join(".gemini").join("tmp"),
        }
    }

    /// Create a GeminiAdapter with a custom tmp directory (for testing).
    #[allow(dead_code)]
    pub fn with_tmp_dir(tmp_dir: PathBuf) -> Self {
        Self { tmp_dir }
    }

    pub fn project_hash_for_path(project_dir: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(project_dir.as_bytes());
        hex::encode(hasher.finalize())
    }

    /// Find the JSON session file for a given session ID.
    fn find_session_file(&self, session_id: &str) -> Result<PathBuf> {
        // Walk ~/.gemini/tmp/*/chats/session-*.json looking for matching sessionId.
        // The filename might encode the session ID, or we may need to open each file.
        // Strategy: first try to find by filename pattern, then fall back to parsing.
        for entry in WalkDir::new(&self.tmp_dir)
            .min_depth(3)
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            // Check if the parent directory is "chats"
            let parent_name = path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if parent_name != "chats" {
                continue;
            }

            // Try to match by reading the file and checking sessionId
            if let Ok(data) = fs::read(path) {
                if let Ok(session) = parse_session(&data) {
                    if session.session_id == session_id {
                        return Ok(path.to_path_buf());
                    }
                }
            }
        }

        anyhow::bail!("Session file not found for id: {}", session_id)
    }

    /// Quick-parse a session file to extract summary metadata without full conversion.
    fn quick_parse_metadata(&self, path: &Path) -> Result<ConversationSummary> {
        let data = fs::read(path)
            .with_context(|| format!("Failed to read session file: {}", path.display()))?;
        let session = parse_session(&data)
            .with_context(|| format!("Failed to parse session file: {}", path.display()))?;

        // Derive project directory from the project-hash directory name
        let project_hash = path
            .parent() // chats/
            .and_then(|p| p.parent()) // <project-hash>/
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        let project_dir = session
            .project_path
            .clone()
            .or_else(|| session.cwd.clone())
            .unwrap_or_else(|| format!("gemini:{}", project_hash));

        let start_time = session
            .start_time
            .parse::<DateTime<Utc>>()
            .unwrap_or_else(|_| Utc::now());
        let last_updated = session
            .last_updated
            .parse::<DateTime<Utc>>()
            .unwrap_or_else(|_| Utc::now());

        let mut message_count = 0;
        let mut first_user_message: Option<String> = None;

        // Count unique file paths from tool calls
        let mut file_paths: std::collections::HashSet<String> = std::collections::HashSet::new();
        for msg in &session.messages {
            if msg.message_type == "user" || msg.message_type == "gemini" {
                message_count += 1;
            }
            if msg.message_type == "user" && first_user_message.is_none() {
                if let Some(content) = &msg.content {
                    if !content.is_empty() {
                        let truncated = truncate_str(content, 100);
                        first_user_message = Some(truncated);
                    }
                }
            }
            for tc in &msg.tool_calls {
                if let Some(args) = &tc.args {
                    if let Some(fp) = extract_file_path_from_args(&tc.name, args) {
                        file_paths.insert(fp);
                    }
                }
            }
        }

        Ok(ConversationSummary {
            id: session.session_id,
            source_agent: AgentKind::Gemini,
            project_dir,
            created_at: start_time,
            updated_at: last_updated,
            summary: session.summary.or(first_user_message),
            message_count,
            file_count: file_paths.len(),
        })
    }
}

/// Truncate a string to at most `max_chars` characters, appending "..." if truncated.
fn truncate_str(s: &str, max_chars: usize) -> String {
    let mut chars = s.chars();
    let truncated: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        format!("{}...", truncated)
    } else {
        truncated
    }
}

/// Try to extract a file path from tool call args for file-modifying tools.
fn extract_file_path_from_args(tool_name: &str, args: &Value) -> Option<String> {
    // Common Gemini CLI tool names that modify files
    let file_keys = ["file_path", "path", "filePath", "filename"];
    let is_file_tool = tool_name.contains("write")
        || tool_name.contains("Write")
        || tool_name.contains("edit")
        || tool_name.contains("Edit")
        || tool_name.contains("create")
        || tool_name.contains("Create");

    if is_file_tool {
        for key in &file_keys {
            if let Some(v) = args.get(key) {
                if let Some(s) = v.as_str() {
                    return Some(s.to_string());
                }
            }
        }
    }
    None
}

/// Convert a Gemini session into a UCF Conversation.
fn convert_session(session: &GeminiSession, project_dir: &str) -> Result<Conversation> {
    let start_time = session
        .start_time
        .parse::<DateTime<Utc>>()
        .unwrap_or_else(|_| Utc::now());
    let last_updated = session
        .last_updated
        .parse::<DateTime<Utc>>()
        .unwrap_or_else(|_| Utc::now());

    let mut messages: Vec<Message> = Vec::new();
    let mut file_changes: Vec<FileChange> = Vec::new();
    let mut file_paths_seen: std::collections::HashSet<String> = std::collections::HashSet::new();

    for gemini_msg in &session.messages {
        let ts = gemini_msg
            .timestamp
            .as_ref()
            .and_then(|t| t.parse::<DateTime<Utc>>().ok())
            .unwrap_or(start_time);

        let msg_id = gemini_msg
            .id
            .as_ref()
            .and_then(|id| Uuid::parse_str(id).ok())
            .unwrap_or_else(Uuid::new_v4);

        match gemini_msg.message_type.as_str() {
            "user" => {
                messages.push(Message {
                    id: msg_id,
                    timestamp: ts,
                    role: Role::User,
                    content: gemini_msg.content.clone().unwrap_or_default(),
                    tool_calls: Vec::new(),
                    metadata: HashMap::new(),
                });
            }
            "gemini" => {
                let mut metadata: HashMap<String, Value> = HashMap::new();

                // Preserve model info
                if let Some(model) = &gemini_msg.model {
                    metadata.insert("model".to_string(), json!(model));
                }

                // Preserve thoughts
                if !gemini_msg.thoughts.is_empty() {
                    let thoughts: Vec<Value> = gemini_msg
                        .thoughts
                        .iter()
                        .map(|t| {
                            json!({
                                "subject": t.subject,
                                "description": t.description,
                                "timestamp": t.timestamp,
                            })
                        })
                        .collect();
                    metadata.insert("thoughts".to_string(), json!(thoughts));
                }

                // Preserve token usage
                if let Some(tokens) = &gemini_msg.tokens {
                    metadata.insert(
                        "tokens".to_string(),
                        json!({
                            "input": tokens.input,
                            "output": tokens.output,
                            "cached": tokens.cached,
                            "thoughts": tokens.thoughts,
                            "tool": tokens.tool,
                            "total": tokens.total,
                        }),
                    );
                }

                // Convert tool calls
                let mut tool_calls: Vec<ToolCall> = Vec::new();
                for tc in &gemini_msg.tool_calls {
                    let status = match tc.status.as_deref() {
                        Some("success") => ToolStatus::Success,
                        _ => ToolStatus::Error,
                    };

                    let output = extract_tool_output(tc);
                    let input = tc.args.clone().unwrap_or(json!({}));

                    // Track file changes
                    if let Some(args) = &tc.args {
                        if let Some(fp) = extract_file_path_from_args(&tc.name, args) {
                            let change_type = if file_paths_seen.contains(&fp) {
                                ChangeType::Modified
                            } else {
                                file_paths_seen.insert(fp.clone());
                                ChangeType::Created
                            };
                            file_changes.push(FileChange {
                                path: fp,
                                change_type,
                                timestamp: ts,
                                message_id: msg_id,
                            });
                        }
                    }

                    tool_calls.push(ToolCall {
                        name: tc.name.clone(),
                        input,
                        output,
                        status,
                    });
                }

                messages.push(Message {
                    id: msg_id,
                    timestamp: ts,
                    role: Role::Assistant,
                    content: gemini_msg.content.clone().unwrap_or_default(),
                    tool_calls,
                    metadata,
                });
            }
            "info" | "error" => {
                messages.push(Message {
                    id: msg_id,
                    timestamp: ts,
                    role: Role::System,
                    content: gemini_msg.content.clone().unwrap_or_default(),
                    tool_calls: Vec::new(),
                    metadata: HashMap::from([(
                        "system_type".to_string(),
                        json!(gemini_msg.message_type),
                    )]),
                });
            }
            // Skip unknown message types
            _ => {}
        }
    }

    Ok(Conversation {
        id: session.session_id.clone(),
        source_agent: AgentKind::Gemini,
        project_dir: project_dir.to_string(),
        created_at: start_time,
        updated_at: last_updated,
        summary: session.summary.clone(),
        messages,
        file_changes,
    })
}

impl AgentAdapter for GeminiAdapter {
    fn is_available(&self) -> bool {
        self.tmp_dir.exists() && self.tmp_dir.is_dir()
    }

    fn list_conversations(&self) -> Result<Vec<ConversationSummary>> {
        if !self.is_available() {
            return Ok(Vec::new());
        }

        let mut summaries = Vec::new();

        // Walk ~/.gemini/tmp/*/chats/session-*.json
        for entry in WalkDir::new(&self.tmp_dir)
            .min_depth(3)
            .max_depth(3)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }

            // Check if the parent directory is "chats"
            let parent_name = path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("");
            if parent_name != "chats" {
                continue;
            }

            match self.quick_parse_metadata(path) {
                Ok(summary) => summaries.push(summary),
                Err(e) => {
                    eprintln!(
                        "Warning: failed to parse session file {}: {}",
                        path.display(),
                        e
                    );
                }
            }
        }

        // Sort by most recent first
        summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(summaries)
    }

    fn read_conversation(&self, id: &str) -> Result<Conversation> {
        let path = self.find_session_file(id)?;
        let data = fs::read(&path)
            .with_context(|| format!("Failed to read session file: {}", path.display()))?;
        let session = parse_session(&data)
            .with_context(|| format!("Failed to parse session file: {}", path.display()))?;

        // Derive project directory from the project-hash directory name
        let project_hash = path
            .parent() // chats/
            .and_then(|p| p.parent()) // <project-hash>/
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let project_dir = session
            .project_path
            .clone()
            .or_else(|| session.cwd.clone())
            .unwrap_or_else(|| format!("gemini:{}", project_hash));

        convert_session(&session, &project_dir)
    }

    fn write_conversation(&self, conv: &Conversation) -> Result<String> {
        // Generate a new session UUID
        let session_id = Uuid::new_v4().to_string();

        // Derive project hash from project_dir.
        // If it already has "gemini:" prefix, strip it (it's already a hash).
        // Otherwise compute SHA-256 of the path, matching Gemini CLI's algorithm.
        let project_hash = if let Some(stripped) = conv.project_dir.strip_prefix("gemini:") {
            stripped.to_string()
        } else {
            Self::project_hash_for_path(&conv.project_dir)
        };

        // Create the chats directory
        let chats_dir = self.tmp_dir.join(&project_hash).join("chats");
        fs::create_dir_all(&chats_dir).with_context(|| {
            format!("Failed to create chats directory: {}", chats_dir.display())
        })?;

        // Build the session JSON
        let start_time = conv
            .created_at
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let last_updated = conv
            .updated_at
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

        let mut gemini_messages: Vec<Value> = Vec::new();

        for msg in &conv.messages {
            let msg_id = Uuid::new_v4().to_string();
            let ts = msg
                .timestamp
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

            match msg.role {
                Role::User => {
                    gemini_messages.push(json!({
                        "id": msg_id,
                        "timestamp": ts,
                        "type": "user",
                        "content": msg.content
                    }));
                }
                Role::Assistant => {
                    // Build tool calls array (with tool name mapping)
                    let mut tool_calls: Vec<Value> = Vec::new();
                    for tc in &msg.tool_calls {
                        let mapped =
                            map_tool(&conv.source_agent, &AgentKind::Gemini, &tc.name, &tc.input);
                        let tc_id = Uuid::new_v4().to_string();
                        let status = match tc.status {
                            ToolStatus::Success => "success",
                            ToolStatus::Error => "error",
                        };

                        // Build result array
                        let mut result = Vec::new();
                        if let Some(output) = &tc.output {
                            let response_key = match tc.status {
                                ToolStatus::Success => "output",
                                ToolStatus::Error => "error",
                            };
                            result.push(json!({
                                "functionResponse": {
                                    "id": tc_id,
                                    "name": mapped.name,
                                    "response": { response_key: output }
                                }
                            }));
                        }

                        tool_calls.push(json!({
                            "id": tc_id,
                            "name": mapped.name,
                            "args": mapped.input,
                            "result": result,
                            "status": status
                        }));
                    }

                    // Build thoughts array
                    let mut thoughts: Vec<Value> = Vec::new();
                    if let Some(thought_meta) = msg.metadata.get("thoughts") {
                        if let Some(arr) = thought_meta.as_array() {
                            for t in arr {
                                thoughts.push(json!({
                                    "subject": t.get("subject").and_then(|v| v.as_str()).unwrap_or(""),
                                    "description": t.get("description").and_then(|v| v.as_str()).unwrap_or(""),
                                    "timestamp": t.get("timestamp").and_then(|v| v.as_str()).unwrap_or(&ts)
                                }));
                            }
                        }
                    }

                    // Extract model
                    let model = msg
                        .metadata
                        .get("model")
                        .and_then(|v| v.as_str())
                        .unwrap_or("imported")
                        .to_string();

                    let mut gemini_msg = json!({
                        "id": msg_id,
                        "timestamp": ts,
                        "type": "gemini",
                        "content": msg.content,
                        "model": model,
                        "thoughts": thoughts,
                        "toolCalls": tool_calls
                    });

                    // Add token info if present
                    if let Some(tokens) = msg.metadata.get("tokens") {
                        gemini_msg
                            .as_object_mut()
                            .unwrap()
                            .insert("tokens".to_string(), tokens.clone());
                    }

                    gemini_messages.push(gemini_msg);
                }
                Role::System => {
                    // Map system messages to "info" type
                    let sys_type = msg
                        .metadata
                        .get("system_type")
                        .and_then(|v| v.as_str())
                        .unwrap_or("info");
                    gemini_messages.push(json!({
                        "id": msg_id,
                        "timestamp": ts,
                        "type": sys_type,
                        "content": msg.content
                    }));
                }
            }
        }

        let session_json = json!({
            "sessionId": session_id,
            "projectHash": project_hash,
            "projectPath": conv.project_dir,
            "startTime": start_time,
            "lastUpdated": last_updated,
            "summary": conv.summary,
            "messages": gemini_messages
        });

        // Write the session file
        let file_path = chats_dir.join(format!("session-{}.json", session_id));
        let mut file = fs::File::create(&file_path)
            .with_context(|| format!("Failed to create session file: {}", file_path.display()))?;
        write!(file, "{}", serde_json::to_string_pretty(&session_json)?)?;

        Ok(session_id)
    }

    fn render_prompt(&self, conv: &Conversation) -> Result<String> {
        let mut output = String::new();

        // Header
        output.push_str(&format!(
            "# Conversation: {}\n\n",
            conv.summary.as_deref().unwrap_or(&conv.id)
        ));
        output.push_str(&format!("**Source:** {}\n", conv.source_agent_name()));
        output.push_str(&format!("**Project:** {}\n", conv.project_dir));
        output.push_str(&format!(
            "**Date:** {}\n\n",
            conv.created_at.format("%Y-%m-%d %H:%M UTC")
        ));
        output.push_str("---\n\n");

        // Messages
        for msg in &conv.messages {
            let role_label = match msg.role {
                Role::User => "User",
                Role::Assistant => "Assistant",
                Role::System => "System",
            };

            output.push_str(&format!("## {}\n\n", role_label));

            // Include thoughts if present (Gemini-specific)
            if let Some(thoughts) = msg.metadata.get("thoughts") {
                if let Some(arr) = thoughts.as_array() {
                    for thought in arr {
                        let subject = thought
                            .get("subject")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Thought");
                        let desc = thought
                            .get("description")
                            .and_then(|v| v.as_str())
                            .unwrap_or("");
                        output.push_str(&format!("<thinking subject=\"{}\">\n", subject));
                        output.push_str(desc);
                        output.push_str("\n</thinking>\n\n");
                    }
                }
            }

            if !msg.content.is_empty() {
                output.push_str(&msg.content);
                output.push_str("\n\n");
            }

            // Tool calls
            for tc in &msg.tool_calls {
                output.push_str(&format!("**Tool: {}**\n", tc.name));
                output.push_str(&format!(
                    "```json\n{}\n```\n",
                    serde_json::to_string_pretty(&tc.input).unwrap_or_default()
                ));
                if let Some(out) = &tc.output {
                    let truncated = truncate_str(out, 500);
                    output.push_str(&format!("Output: {}\n", truncated));
                }
                output.push('\n');
            }

            output.push_str("---\n\n");
        }

        // File changes summary
        if !conv.file_changes.is_empty() {
            output.push_str("## Files Changed\n\n");
            for fc in &conv.file_changes {
                let action = match fc.change_type {
                    ChangeType::Created => "created",
                    ChangeType::Modified => "modified",
                    ChangeType::Deleted => "deleted",
                };
                output.push_str(&format!("- `{}` ({})\n", fc.path, action));
            }
            output.push('\n');
        }

        Ok(output)
    }

    fn agent_kind(&self) -> AgentKind {
        AgentKind::Gemini
    }

    fn display_name(&self) -> &str {
        "Gemini CLI"
    }

    fn data_dir(&self) -> PathBuf {
        self.tmp_dir.clone()
    }

    fn delete_conversation(&self, id: &str) -> Result<()> {
        let path = self.find_session_file(id)?;
        fs::remove_file(&path)
            .with_context(|| format!("Failed to delete session file: {}", path.display()))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as IoWrite;
    use tempfile::TempDir;

    /// Helper: create a session JSON file under the expected directory layout.
    /// Layout: <tmp_dir>/<project_hash>/chats/<filename>.json
    fn create_test_session(
        tmp_dir: &Path,
        project_hash: &str,
        filename: &str,
        content: &Value,
    ) -> PathBuf {
        let chats_dir = tmp_dir.join(project_hash).join("chats");
        fs::create_dir_all(&chats_dir).unwrap();
        let file_path = chats_dir.join(format!("{}.json", filename));
        let mut file = fs::File::create(&file_path).unwrap();
        write!(file, "{}", serde_json::to_string_pretty(content).unwrap()).unwrap();
        file_path
    }

    fn sample_session_json(session_id: &str, start: &str, updated: &str) -> Value {
        json!({
            "sessionId": session_id,
            "projectHash": "abc123hash",
            "startTime": start,
            "lastUpdated": updated,
            "messages": [
                {
                    "id": "11111111-1111-1111-1111-111111111111",
                    "timestamp": start,
                    "type": "user",
                    "content": "Hello Gemini"
                },
                {
                    "id": "22222222-2222-2222-2222-222222222222",
                    "timestamp": updated,
                    "type": "gemini",
                    "content": "Hi there!",
                    "model": "gemini-2.5-pro",
                    "thoughts": [],
                    "tokens": {
                        "input": 10,
                        "output": 20,
                        "cached": 0,
                        "thoughts": 0,
                        "tool": 0,
                        "total": 30
                    },
                    "toolCalls": []
                }
            ]
        })
    }

    #[test]
    fn test_is_available_with_existing_dir() {
        let tmp = TempDir::new().unwrap();
        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        assert!(adapter.is_available());
    }

    #[test]
    fn test_is_available_with_missing_dir() {
        let adapter = GeminiAdapter::with_tmp_dir(PathBuf::from("/nonexistent/gemini/tmp"));
        assert!(!adapter.is_available());
    }

    #[test]
    fn test_list_conversations_empty() {
        let tmp = TempDir::new().unwrap();
        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let convos = adapter.list_conversations().unwrap();
        assert!(convos.is_empty());
    }

    #[test]
    fn test_list_conversations_finds_sessions() {
        let tmp = TempDir::new().unwrap();
        let session = sample_session_json(
            "session-001",
            "2026-03-04T10:00:00.000Z",
            "2026-03-04T10:05:00.000Z",
        );
        create_test_session(tmp.path(), "projhash1", "session-001", &session);

        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let convos = adapter.list_conversations().unwrap();

        assert_eq!(convos.len(), 1);
        assert_eq!(convos[0].id, "session-001");
        assert_eq!(convos[0].source_agent, AgentKind::Gemini);
        assert_eq!(convos[0].message_count, 2);
    }

    #[test]
    fn test_list_conversations_sorted_by_recent() {
        let tmp = TempDir::new().unwrap();

        let old_session = sample_session_json(
            "session-old",
            "2026-01-01T10:00:00.000Z",
            "2026-01-01T10:05:00.000Z",
        );
        create_test_session(tmp.path(), "projhash1", "session-old", &old_session);

        let new_session = sample_session_json(
            "session-new",
            "2026-03-04T10:00:00.000Z",
            "2026-03-04T10:05:00.000Z",
        );
        create_test_session(tmp.path(), "projhash1", "session-new", &new_session);

        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let convos = adapter.list_conversations().unwrap();

        assert_eq!(convos.len(), 2);
        assert_eq!(convos[0].id, "session-new");
        assert_eq!(convos[1].id, "session-old");
    }

    #[test]
    fn test_read_simple_conversation() {
        let tmp = TempDir::new().unwrap();
        let session = sample_session_json(
            "session-001",
            "2026-03-04T10:00:00.000Z",
            "2026-03-04T10:05:00.000Z",
        );
        create_test_session(tmp.path(), "projhash1", "session-001", &session);

        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let conv = adapter.read_conversation("session-001").unwrap();

        assert_eq!(conv.id, "session-001");
        assert_eq!(conv.source_agent, AgentKind::Gemini);
        assert_eq!(conv.messages.len(), 2);
        assert_eq!(conv.messages[0].role, Role::User);
        assert_eq!(conv.messages[0].content, "Hello Gemini");
        assert_eq!(conv.messages[1].role, Role::Assistant);
        assert_eq!(conv.messages[1].content, "Hi there!");
    }

    #[test]
    fn test_read_conversation_with_tool_calls() {
        let tmp = TempDir::new().unwrap();
        let session = json!({
            "sessionId": "session-tools",
            "projectHash": "hash1",
            "startTime": "2026-03-04T10:00:00.000Z",
            "lastUpdated": "2026-03-04T10:05:00.000Z",
            "messages": [
                {
                    "id": "11111111-1111-1111-1111-111111111111",
                    "timestamp": "2026-03-04T10:00:00.000Z",
                    "type": "user",
                    "content": "Read a file"
                },
                {
                    "id": "22222222-2222-2222-2222-222222222222",
                    "timestamp": "2026-03-04T10:00:01.000Z",
                    "type": "gemini",
                    "content": "",
                    "model": "gemini-2.5-pro",
                    "thoughts": [],
                    "tokens": {"input": 10, "output": 5, "total": 15},
                    "toolCalls": [
                        {
                            "id": "tc-1",
                            "name": "readFile",
                            "args": {"path": "/tmp/test.rs"},
                            "result": [
                                {
                                    "functionResponse": {
                                        "id": "tc-1",
                                        "name": "readFile",
                                        "response": {"output": "fn main() {}"}
                                    }
                                }
                            ],
                            "status": "success",
                            "timestamp": "2026-03-04T10:00:01.500Z",
                            "displayName": "Read File",
                            "resultDisplay": "fn main() {}"
                        }
                    ]
                }
            ]
        });
        create_test_session(tmp.path(), "projhash1", "session-tools", &session);

        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let conv = adapter.read_conversation("session-tools").unwrap();

        assert_eq!(conv.messages.len(), 2);
        let assistant = &conv.messages[1];
        assert_eq!(assistant.role, Role::Assistant);
        assert_eq!(assistant.tool_calls.len(), 1);
        assert_eq!(assistant.tool_calls[0].name, "readFile");
        assert_eq!(assistant.tool_calls[0].status, ToolStatus::Success);
        assert_eq!(
            assistant.tool_calls[0].output.as_deref(),
            Some("fn main() {}")
        );
    }

    #[test]
    fn test_read_conversation_with_error_tool_call() {
        let tmp = TempDir::new().unwrap();
        let session = json!({
            "sessionId": "session-error-tool",
            "projectHash": "hash1",
            "startTime": "2026-03-04T10:00:00.000Z",
            "lastUpdated": "2026-03-04T10:05:00.000Z",
            "messages": [
                {
                    "id": "11111111-1111-1111-1111-111111111111",
                    "timestamp": "2026-03-04T10:00:00.000Z",
                    "type": "user",
                    "content": "Read a missing file"
                },
                {
                    "id": "22222222-2222-2222-2222-222222222222",
                    "timestamp": "2026-03-04T10:00:01.000Z",
                    "type": "gemini",
                    "content": "Let me try.",
                    "toolCalls": [
                        {
                            "id": "tc-1",
                            "name": "readFile",
                            "args": {"path": "/tmp/missing.rs"},
                            "result": [
                                {
                                    "functionResponse": {
                                        "id": "tc-1",
                                        "name": "readFile",
                                        "response": {"error": "file not found"}
                                    }
                                }
                            ],
                            "status": "error"
                        }
                    ]
                }
            ]
        });
        create_test_session(tmp.path(), "projhash1", "session-error-tool", &session);

        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let conv = adapter.read_conversation("session-error-tool").unwrap();

        let assistant = &conv.messages[1];
        assert_eq!(assistant.tool_calls[0].status, ToolStatus::Error);
        assert_eq!(
            assistant.tool_calls[0].output.as_deref(),
            Some("file not found")
        );
    }

    #[test]
    fn test_read_conversation_with_thoughts() {
        let tmp = TempDir::new().unwrap();
        let session = json!({
            "sessionId": "session-thoughts",
            "projectHash": "hash1",
            "startTime": "2026-03-04T10:00:00.000Z",
            "lastUpdated": "2026-03-04T10:05:00.000Z",
            "messages": [
                {
                    "id": "11111111-1111-1111-1111-111111111111",
                    "timestamp": "2026-03-04T10:00:00.000Z",
                    "type": "user",
                    "content": "Think about this"
                },
                {
                    "id": "22222222-2222-2222-2222-222222222222",
                    "timestamp": "2026-03-04T10:00:01.000Z",
                    "type": "gemini",
                    "content": "Here is my answer.",
                    "model": "gemini-2.5-pro",
                    "thoughts": [
                        {
                            "subject": "Analysis",
                            "description": "Let me consider the implications.",
                            "timestamp": "2026-03-04T10:00:01.100Z"
                        }
                    ],
                    "toolCalls": []
                }
            ]
        });
        create_test_session(tmp.path(), "projhash1", "session-thoughts", &session);

        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let conv = adapter.read_conversation("session-thoughts").unwrap();

        let assistant = &conv.messages[1];
        let thoughts = assistant.metadata.get("thoughts").unwrap();
        let arr = thoughts.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["subject"], "Analysis");
        assert_eq!(arr[0]["description"], "Let me consider the implications.");
    }

    #[test]
    fn test_read_conversation_with_info_and_error_messages() {
        let tmp = TempDir::new().unwrap();
        let session = json!({
            "sessionId": "session-sys",
            "projectHash": "hash1",
            "startTime": "2026-03-04T10:00:00.000Z",
            "lastUpdated": "2026-03-04T10:05:00.000Z",
            "messages": [
                {
                    "id": "11111111-1111-1111-1111-111111111111",
                    "timestamp": "2026-03-04T10:00:00.000Z",
                    "type": "info",
                    "content": "MCP server started"
                },
                {
                    "id": "22222222-2222-2222-2222-222222222222",
                    "timestamp": "2026-03-04T10:00:01.000Z",
                    "type": "user",
                    "content": "Hello"
                },
                {
                    "id": "33333333-3333-3333-3333-333333333333",
                    "timestamp": "2026-03-04T10:00:02.000Z",
                    "type": "error",
                    "content": "Rate limit exceeded"
                }
            ]
        });
        create_test_session(tmp.path(), "projhash1", "session-sys", &session);

        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let conv = adapter.read_conversation("session-sys").unwrap();

        assert_eq!(conv.messages.len(), 3);
        assert_eq!(conv.messages[0].role, Role::System);
        assert_eq!(conv.messages[0].content, "MCP server started");
        assert_eq!(conv.messages[0].metadata["system_type"], json!("info"));
        assert_eq!(conv.messages[1].role, Role::User);
        assert_eq!(conv.messages[2].role, Role::System);
        assert_eq!(conv.messages[2].content, "Rate limit exceeded");
        assert_eq!(conv.messages[2].metadata["system_type"], json!("error"));
    }

    #[test]
    fn test_read_conversation_with_model_metadata() {
        let tmp = TempDir::new().unwrap();
        let session = sample_session_json(
            "session-model",
            "2026-03-04T10:00:00.000Z",
            "2026-03-04T10:05:00.000Z",
        );
        create_test_session(tmp.path(), "projhash1", "session-model", &session);

        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let conv = adapter.read_conversation("session-model").unwrap();

        let assistant = &conv.messages[1];
        assert_eq!(assistant.metadata["model"], json!("gemini-2.5-pro"));
    }

    #[test]
    fn test_read_conversation_with_token_metadata() {
        let tmp = TempDir::new().unwrap();
        let session = sample_session_json(
            "session-tokens",
            "2026-03-04T10:00:00.000Z",
            "2026-03-04T10:05:00.000Z",
        );
        create_test_session(tmp.path(), "projhash1", "session-tokens", &session);

        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let conv = adapter.read_conversation("session-tokens").unwrap();

        let assistant = &conv.messages[1];
        let tokens = assistant.metadata.get("tokens").unwrap();
        assert_eq!(tokens["input"], json!(10));
        assert_eq!(tokens["output"], json!(20));
        assert_eq!(tokens["total"], json!(30));
    }

    #[test]
    fn test_read_conversation_with_summary() {
        let tmp = TempDir::new().unwrap();
        let session = json!({
            "sessionId": "session-summary",
            "projectHash": "hash1",
            "startTime": "2026-03-04T10:00:00.000Z",
            "lastUpdated": "2026-03-04T10:05:00.000Z",
            "summary": "Debugging a tricky bug",
            "messages": []
        });
        create_test_session(tmp.path(), "projhash1", "session-summary", &session);

        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let conv = adapter.read_conversation("session-summary").unwrap();

        assert_eq!(conv.summary.as_deref(), Some("Debugging a tricky bug"));
    }

    #[test]
    fn test_session_not_found() {
        let tmp = TempDir::new().unwrap();
        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let result = adapter.read_conversation("nonexistent-session");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_write_conversation_empty() {
        let tmp = TempDir::new().unwrap();
        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let now = Utc::now();
        let conv = Conversation {
            id: "test".to_string(),
            source_agent: AgentKind::Gemini,
            project_dir: "gemini:abc123hash".to_string(),
            created_at: now,
            updated_at: now,
            summary: None,
            messages: Vec::new(),
            file_changes: Vec::new(),
        };
        let session_id = adapter.write_conversation(&conv).unwrap();
        assert!(!session_id.is_empty());

        // Verify the file was created
        let file_path = tmp
            .path()
            .join("abc123hash")
            .join("chats")
            .join(format!("session-{}.json", session_id));
        assert!(file_path.exists());
    }

    #[test]
    fn test_write_and_read_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let now = Utc::now();

        let conv = Conversation {
            id: "original-id".to_string(),
            source_agent: AgentKind::Gemini,
            project_dir: "gemini:projhash_rt".to_string(),
            created_at: now,
            updated_at: now,
            summary: Some("Round-trip test".to_string()),
            messages: vec![
                Message {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    role: Role::User,
                    content: "Hello Gemini!".to_string(),
                    tool_calls: Vec::new(),
                    metadata: HashMap::new(),
                },
                Message {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    role: Role::Assistant,
                    content: "Hi there!".to_string(),
                    tool_calls: vec![ToolCall {
                        name: "readFile".to_string(),
                        input: json!({"path": "/tmp/test.rs"}),
                        output: Some("fn main() {}".to_string()),
                        status: ToolStatus::Success,
                    }],
                    metadata: HashMap::from([("model".to_string(), json!("gemini-2.5-pro"))]),
                },
                Message {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    role: Role::User,
                    content: "Thanks!".to_string(),
                    tool_calls: Vec::new(),
                    metadata: HashMap::new(),
                },
            ],
            file_changes: Vec::new(),
        };

        // Write
        let session_id = adapter.write_conversation(&conv).unwrap();
        assert!(!session_id.is_empty());

        // Read back
        let read_conv = adapter.read_conversation(&session_id).unwrap();

        assert_eq!(read_conv.source_agent, AgentKind::Gemini);
        assert_eq!(read_conv.summary.as_deref(), Some("Round-trip test"));
        assert_eq!(read_conv.messages.len(), 3);

        // Check user messages
        let user_msgs: Vec<&Message> = read_conv
            .messages
            .iter()
            .filter(|m| m.role == Role::User)
            .collect();
        assert_eq!(user_msgs.len(), 2);
        assert_eq!(user_msgs[0].content, "Hello Gemini!");
        assert_eq!(user_msgs[1].content, "Thanks!");

        // Check assistant message
        let assistant_msgs: Vec<&Message> = read_conv
            .messages
            .iter()
            .filter(|m| m.role == Role::Assistant)
            .collect();
        assert_eq!(assistant_msgs.len(), 1);
        assert_eq!(assistant_msgs[0].content, "Hi there!");
        assert_eq!(assistant_msgs[0].tool_calls.len(), 1);
        assert_eq!(assistant_msgs[0].tool_calls[0].name, "readFile");
        assert_eq!(assistant_msgs[0].tool_calls[0].status, ToolStatus::Success);
        assert_eq!(
            assistant_msgs[0].tool_calls[0].output.as_deref(),
            Some("fn main() {}")
        );
        assert_eq!(assistant_msgs[0].metadata["model"], json!("gemini-2.5-pro"));
    }

    #[test]
    fn test_write_conversation_with_thoughts() {
        let tmp = TempDir::new().unwrap();
        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let now = Utc::now();

        let mut metadata = HashMap::new();
        metadata.insert("thoughts".to_string(), json!([
            {"subject": "Analysis", "description": "Thinking deeply", "timestamp": "2026-03-04T10:00:01.000Z"}
        ]));
        metadata.insert("model".to_string(), json!("gemini-2.5-pro"));

        let conv = Conversation {
            id: "thought-test".to_string(),
            source_agent: AgentKind::Gemini,
            project_dir: "gemini:thought_hash".to_string(),
            created_at: now,
            updated_at: now,
            summary: None,
            messages: vec![
                Message {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    role: Role::User,
                    content: "Think about this".to_string(),
                    tool_calls: Vec::new(),
                    metadata: HashMap::new(),
                },
                Message {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    role: Role::Assistant,
                    content: "Here is my answer.".to_string(),
                    tool_calls: Vec::new(),
                    metadata,
                },
            ],
            file_changes: Vec::new(),
        };

        let session_id = adapter.write_conversation(&conv).unwrap();
        let read_conv = adapter.read_conversation(&session_id).unwrap();

        let assistant = read_conv
            .messages
            .iter()
            .find(|m| m.role == Role::Assistant)
            .unwrap();
        let thoughts = assistant.metadata.get("thoughts").unwrap();
        let arr = thoughts.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["subject"], "Analysis");
    }

    #[test]
    fn test_write_conversation_with_system_messages() {
        let tmp = TempDir::new().unwrap();
        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let now = Utc::now();

        let conv = Conversation {
            id: "sys-test".to_string(),
            source_agent: AgentKind::Gemini,
            project_dir: "gemini:sys_hash".to_string(),
            created_at: now,
            updated_at: now,
            summary: None,
            messages: vec![
                Message {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    role: Role::System,
                    content: "MCP server started".to_string(),
                    tool_calls: Vec::new(),
                    metadata: HashMap::from([("system_type".to_string(), json!("info"))]),
                },
                Message {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    role: Role::User,
                    content: "Hello".to_string(),
                    tool_calls: Vec::new(),
                    metadata: HashMap::new(),
                },
            ],
            file_changes: Vec::new(),
        };

        let session_id = adapter.write_conversation(&conv).unwrap();
        let read_conv = adapter.read_conversation(&session_id).unwrap();

        assert_eq!(read_conv.messages.len(), 2);
        assert_eq!(read_conv.messages[0].role, Role::System);
        assert_eq!(read_conv.messages[0].content, "MCP server started");
        assert_eq!(read_conv.messages[1].role, Role::User);
    }

    #[test]
    fn test_write_conversation_with_error_tool() {
        let tmp = TempDir::new().unwrap();
        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let now = Utc::now();

        let conv = Conversation {
            id: "error-tool-test".to_string(),
            source_agent: AgentKind::Gemini,
            project_dir: "gemini:errhash".to_string(),
            created_at: now,
            updated_at: now,
            summary: None,
            messages: vec![Message {
                id: Uuid::new_v4(),
                timestamp: now,
                role: Role::Assistant,
                content: "".to_string(),
                tool_calls: vec![ToolCall {
                    name: "readFile".to_string(),
                    input: json!({"path": "/missing"}),
                    output: Some("file not found".to_string()),
                    status: ToolStatus::Error,
                }],
                metadata: HashMap::new(),
            }],
            file_changes: Vec::new(),
        };

        let session_id = adapter.write_conversation(&conv).unwrap();
        let read_conv = adapter.read_conversation(&session_id).unwrap();

        let assistant = &read_conv.messages[0];
        assert_eq!(assistant.tool_calls[0].status, ToolStatus::Error);
        assert_eq!(
            assistant.tool_calls[0].output.as_deref(),
            Some("file not found")
        );
    }

    #[test]
    fn test_write_conversation_non_gemini_project_dir() {
        let tmp = TempDir::new().unwrap();
        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let now = Utc::now();

        let conv = Conversation {
            id: "plain-path-test".to_string(),
            source_agent: AgentKind::Claude,
            project_dir: "/Users/test/project".to_string(),
            created_at: now,
            updated_at: now,
            summary: None,
            messages: vec![Message {
                id: Uuid::new_v4(),
                timestamp: now,
                role: Role::User,
                content: "Hello".to_string(),
                tool_calls: Vec::new(),
                metadata: HashMap::new(),
            }],
            file_changes: Vec::new(),
        };

        // Should succeed even with a non-gemini project path
        let session_id = adapter.write_conversation(&conv).unwrap();
        let read_conv = adapter.read_conversation(&session_id).unwrap();
        assert_eq!(read_conv.messages.len(), 1);
        assert_eq!(read_conv.messages[0].content, "Hello");
    }

    #[test]
    fn test_agent_kind_and_display_name() {
        let tmp = TempDir::new().unwrap();
        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        assert_eq!(adapter.agent_kind(), AgentKind::Gemini);
        assert_eq!(adapter.display_name(), "Gemini CLI");
    }

    #[test]
    fn test_render_prompt() {
        let now = Utc::now();
        let conv = Conversation {
            id: "test-conv".to_string(),
            source_agent: AgentKind::Gemini,
            project_dir: "gemini:abc123".to_string(),
            created_at: now,
            updated_at: now,
            summary: Some("Test conversation".to_string()),
            messages: vec![
                Message {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    role: Role::User,
                    content: "Hello!".to_string(),
                    tool_calls: Vec::new(),
                    metadata: HashMap::new(),
                },
                Message {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    role: Role::Assistant,
                    content: "Hi there!".to_string(),
                    tool_calls: Vec::new(),
                    metadata: HashMap::new(),
                },
            ],
            file_changes: Vec::new(),
        };

        let tmp = TempDir::new().unwrap();
        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let rendered = adapter.render_prompt(&conv).unwrap();

        assert!(rendered.contains("# Conversation: Test conversation"));
        assert!(rendered.contains("**Source:** Gemini CLI"));
        assert!(rendered.contains("## User"));
        assert!(rendered.contains("Hello!"));
        assert!(rendered.contains("## Assistant"));
        assert!(rendered.contains("Hi there!"));
    }

    #[test]
    fn test_render_prompt_with_thoughts() {
        let now = Utc::now();
        let mut metadata = HashMap::new();
        metadata.insert(
            "thoughts".to_string(),
            json!([{"subject": "Analysis", "description": "Deep analysis here"}]),
        );

        let conv = Conversation {
            id: "test-conv".to_string(),
            source_agent: AgentKind::Gemini,
            project_dir: "gemini:abc123".to_string(),
            created_at: now,
            updated_at: now,
            summary: None,
            messages: vec![Message {
                id: Uuid::new_v4(),
                timestamp: now,
                role: Role::Assistant,
                content: "Result.".to_string(),
                tool_calls: Vec::new(),
                metadata,
            }],
            file_changes: Vec::new(),
        };

        let tmp = TempDir::new().unwrap();
        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let rendered = adapter.render_prompt(&conv).unwrap();

        assert!(rendered.contains("<thinking subject=\"Analysis\">"));
        assert!(rendered.contains("Deep analysis here"));
        assert!(rendered.contains("</thinking>"));
    }

    #[test]
    fn test_render_prompt_with_tool_calls() {
        let now = Utc::now();
        let conv = Conversation {
            id: "test-conv".to_string(),
            source_agent: AgentKind::Gemini,
            project_dir: "gemini:abc123".to_string(),
            created_at: now,
            updated_at: now,
            summary: None,
            messages: vec![Message {
                id: Uuid::new_v4(),
                timestamp: now,
                role: Role::Assistant,
                content: String::new(),
                tool_calls: vec![ToolCall {
                    name: "readFile".to_string(),
                    input: json!({"path": "/tmp/test.rs"}),
                    output: Some("fn main() {}".to_string()),
                    status: ToolStatus::Success,
                }],
                metadata: HashMap::new(),
            }],
            file_changes: Vec::new(),
        };

        let tmp = TempDir::new().unwrap();
        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let rendered = adapter.render_prompt(&conv).unwrap();

        assert!(rendered.contains("**Tool: readFile**"));
        assert!(rendered.contains("Output: fn main() {}"));
    }

    #[test]
    fn test_cancelled_tool_status_maps_to_error() {
        let tmp = TempDir::new().unwrap();
        let session = json!({
            "sessionId": "session-cancelled",
            "projectHash": "hash1",
            "startTime": "2026-03-04T10:00:00.000Z",
            "lastUpdated": "2026-03-04T10:05:00.000Z",
            "messages": [
                {
                    "id": "22222222-2222-2222-2222-222222222222",
                    "timestamp": "2026-03-04T10:00:01.000Z",
                    "type": "gemini",
                    "content": "",
                    "toolCalls": [
                        {
                            "id": "tc-1",
                            "name": "writeFile",
                            "args": {"path": "/tmp/test.rs"},
                            "result": [],
                            "status": "cancelled"
                        }
                    ]
                }
            ]
        });
        create_test_session(tmp.path(), "projhash1", "session-cancelled", &session);

        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let conv = adapter.read_conversation("session-cancelled").unwrap();

        assert_eq!(conv.messages[0].tool_calls[0].status, ToolStatus::Error);
    }

    #[test]
    fn test_list_ignores_non_chats_directories() {
        let tmp = TempDir::new().unwrap();

        // Create a valid session in chats/
        let session = sample_session_json(
            "session-valid",
            "2026-03-04T10:00:00.000Z",
            "2026-03-04T10:05:00.000Z",
        );
        create_test_session(tmp.path(), "projhash1", "session-valid", &session);

        // Create a JSON file in a non-chats directory (should be ignored)
        let other_dir = tmp.path().join("projhash1").join("other");
        fs::create_dir_all(&other_dir).unwrap();
        let other_file = other_dir.join("session-fake.json");
        fs::write(
            &other_file,
            serde_json::to_string(&sample_session_json(
                "session-fake",
                "2026-03-04T10:00:00.000Z",
                "2026-03-04T10:05:00.000Z",
            ))
            .unwrap(),
        )
        .unwrap();

        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let convos = adapter.list_conversations().unwrap();

        assert_eq!(convos.len(), 1);
        assert_eq!(convos[0].id, "session-valid");
    }

    #[test]
    fn test_data_dir() {
        let tmp = TempDir::new().unwrap();
        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        assert_eq!(adapter.data_dir(), tmp.path().to_path_buf());
    }

    #[test]
    fn test_project_hash_for_path_is_stable_sha256_hex() {
        assert_eq!(
            GeminiAdapter::project_hash_for_path("D:/VSP/agentswap-gui"),
            "2ee9a276c7ae3654802d757aaaf39773e31ae1f98f66fd26a211a4f20e8c58a8"
        );
    }

    #[test]
    fn test_project_path_metadata_restores_real_project_dir() {
        let tmp = TempDir::new().unwrap();
        let session = json!({
            "sessionId": "session-real-path",
            "projectHash": "projhash-real-path",
            "projectPath": "D:/VSP/agentswap-gui",
            "startTime": "2026-03-04T10:00:00.000Z",
            "lastUpdated": "2026-03-04T10:05:00.000Z",
            "messages": [
                {
                    "id": "11111111-1111-1111-1111-111111111111",
                    "timestamp": "2026-03-04T10:00:01.000Z",
                    "type": "user",
                    "content": "Remember: use ChatMem here"
                }
            ]
        });
        create_test_session(tmp.path(), "projhash-real-path", "session-real-path", &session);

        let adapter = GeminiAdapter::with_tmp_dir(tmp.path().to_path_buf());
        let summary = adapter
            .list_conversations()
            .unwrap()
            .into_iter()
            .find(|item| item.id == "session-real-path")
            .unwrap();
        let conv = adapter.read_conversation("session-real-path").unwrap();

        assert_eq!(summary.project_dir, "D:/VSP/agentswap-gui");
        assert_eq!(conv.project_dir, "D:/VSP/agentswap-gui");
    }

    /// Integration test: skip if Gemini CLI is not installed.
    #[test]
    fn test_real_gemini_sessions_if_available() {
        let adapter = GeminiAdapter::new();
        if !adapter.is_available() {
            eprintln!("Skipping real Gemini integration test: ~/.gemini/tmp not found");
            return;
        }

        let convos = adapter.list_conversations().unwrap();
        eprintln!("Found {} Gemini sessions", convos.len());

        if let Some(first) = convos.first() {
            let conv = adapter.read_conversation(&first.id).unwrap();
            assert_eq!(conv.source_agent, AgentKind::Gemini);
            assert!(!conv.messages.is_empty() || conv.messages.is_empty()); // just exercise the path

            let rendered = adapter.render_prompt(&conv).unwrap();
            assert!(rendered.contains("**Source:** Gemini CLI"));
        }
    }
}
