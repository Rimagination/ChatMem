use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};
use uuid::Uuid;
use walkdir::WalkDir;

use agentswap_core::adapter::AgentAdapter;
use agentswap_core::files::move_path_to_trash;
use agentswap_core::tool_mapping::map_tool;
use agentswap_core::types::*;

use crate::parser::*;

/// Adapter for reading and writing Claude Code session files.
pub struct ClaudeAdapter {
    /// The root directory containing Claude projects, typically `~/.claude/projects`.
    projects_dir: PathBuf,
}

impl Default for ClaudeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeAdapter {
    /// Create a new ClaudeAdapter using the default `~/.claude/projects` directory.
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        Self {
            projects_dir: home.join(".claude").join("projects"),
        }
    }

    /// Create a ClaudeAdapter with a custom projects directory (for testing).
    #[allow(dead_code)]
    pub fn with_projects_dir(projects_dir: PathBuf) -> Self {
        Self { projects_dir }
    }

    /// Find the JSONL file path for a given session UUID.
    fn find_session_file(&self, session_id: &str) -> Result<PathBuf> {
        let filename = format!("{}.jsonl", session_id);
        for entry in WalkDir::new(&self.projects_dir)
            .min_depth(2)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if entry.file_name().to_string_lossy() == filename {
                return Ok(entry.path().to_path_buf());
            }
        }
        anyhow::bail!("Session file not found for id: {}", session_id)
    }

    /// Quick-parse a JSONL file to extract summary metadata without building
    /// the full conversation.
    fn quick_parse_metadata(&self, path: &Path) -> Result<ConversationSummary> {
        let file = fs::File::open(path)
            .with_context(|| format!("Failed to open session file: {}", path.display()))?;
        let reader = BufReader::new(file);

        let session_id = path
            .file_stem()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();

        // Derive project_dir from the parent directory name
        let project_dir_encoded = path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let project_dir = decode_project_path(&project_dir_encoded);

        let mut message_count: usize = 0;
        let mut file_paths: std::collections::HashSet<String> = std::collections::HashSet::new();
        let mut summary: Option<String> = None;
        let mut first_user_message: Option<String> = None;
        let mut first_timestamp: Option<DateTime<Utc>> = None;
        let mut last_timestamp: Option<DateTime<Utc>> = None;

        for line in reader.lines() {
            let line = line?;
            let event = match parse_event(&line) {
                Some(e) => e,
                None => continue,
            };

            // Track timestamps
            if let Some(ts_str) = &event.timestamp {
                if let Ok(ts) = ts_str.parse::<DateTime<Utc>>() {
                    if first_timestamp.is_none() || ts < first_timestamp.unwrap() {
                        first_timestamp = Some(ts);
                    }
                    if last_timestamp.is_none() || ts > last_timestamp.unwrap() {
                        last_timestamp = Some(ts);
                    }
                }
            }

            match event.event_type.as_str() {
                "user" if !event.is_sidechain => {
                    if let Some(msg) = &event.message {
                        match &msg.content {
                            ClaudeContent::Text(text) => {
                                message_count += 1;
                                if first_user_message.is_none() && !text.is_empty() {
                                    first_user_message = Some(truncate_str(text, 100));
                                }
                            }
                            ClaudeContent::Blocks(_) => {
                                // tool_result blocks don't count as separate user messages
                            }
                        }
                    }
                }
                "assistant" if !event.is_sidechain => {
                    // Count unique assistant message IDs as one message
                    // For quick parse, just count text blocks
                    if let Some(msg) = &event.message {
                        if let ClaudeContent::Blocks(blocks) = &msg.content {
                            for block in blocks {
                                if matches!(block, ClaudeContentBlock::Text { .. }) {
                                    message_count += 1;
                                }
                                // Track file changes from tool_use
                                if let ClaudeContentBlock::ToolUse { name, input, .. } = block {
                                    if let Some(fp) = extract_file_path(name, input) {
                                        file_paths.insert(fp);
                                    }
                                }
                            }
                        }
                    }
                }
                "summary" => {
                    summary = event.summary.clone();
                }
                "file-history-snapshot" => {
                    if let Some(snapshot) = &event.snapshot {
                        if let Some(backups) = snapshot.get("trackedFileBackups") {
                            if let Some(obj) = backups.as_object() {
                                for key in obj.keys() {
                                    file_paths.insert(key.clone());
                                }
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        let now = Utc::now();
        Ok(ConversationSummary {
            id: session_id,
            source_agent: AgentKind::Claude,
            project_dir,
            created_at: first_timestamp.unwrap_or(now),
            updated_at: last_timestamp.unwrap_or(now),
            summary: summary.or(first_user_message),
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

/// Extract a file path from a tool_use block's input if the tool is a file-modifying tool.
fn extract_file_path(tool_name: &str, input: &Value) -> Option<String> {
    match tool_name {
        "Write" | "Edit" | "NotebookEdit" => input
            .get("file_path")
            .or_else(|| input.get("notebook_path"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        _ => None,
    }
}

/// Encode a project directory path into Claude's path-encoded format.
///
/// `/Users/foo/bar` becomes `-Users-foo-bar`.
fn encode_project_path(path: &str) -> String {
    path.replace('/', "-")
}

/// Pending tool use info collected from assistant tool_use blocks.
struct PendingTool {
    name: String,
    input: Value,
}

impl AgentAdapter for ClaudeAdapter {
    fn is_available(&self) -> bool {
        self.projects_dir.exists() && self.projects_dir.is_dir()
    }

    fn list_conversations(&self) -> Result<Vec<ConversationSummary>> {
        if !self.is_available() {
            return Ok(Vec::new());
        }

        let mut summaries = Vec::new();

        for entry in WalkDir::new(&self.projects_dir)
            .min_depth(2)
            .max_depth(2)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
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
        let file = fs::File::open(&path)
            .with_context(|| format!("Failed to open session file: {}", path.display()))?;
        let reader = BufReader::new(file);

        let project_dir_encoded = path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        let project_dir = decode_project_path(&project_dir_encoded);

        let mut messages: Vec<Message> = Vec::new();
        let mut file_changes: Vec<FileChange> = Vec::new();
        let mut file_paths_seen: std::collections::HashSet<String> =
            std::collections::HashSet::new();
        let mut summary: Option<String> = None;
        let mut first_timestamp: Option<DateTime<Utc>> = None;
        let mut last_timestamp: Option<DateTime<Utc>> = None;

        // Track pending tool_use blocks: tool_use_id -> (name, input)
        let mut pending_tools: HashMap<String, PendingTool> = HashMap::new();

        // Track the current assistant message being accumulated (multiple JSONL lines
        // share the same message.id for one API response)
        let mut current_assistant_msg_api_id: Option<String> = None;

        for line in reader.lines() {
            let line = line?;
            let event = match parse_event(&line) {
                Some(e) => e,
                None => continue,
            };

            // Skip sidechain messages
            if event.is_sidechain {
                continue;
            }

            // Track timestamps
            let event_ts = event
                .timestamp
                .as_ref()
                .and_then(|ts| ts.parse::<DateTime<Utc>>().ok());
            if let Some(ts) = event_ts {
                if first_timestamp.is_none() || ts < first_timestamp.unwrap() {
                    first_timestamp = Some(ts);
                }
                if last_timestamp.is_none() || ts > last_timestamp.unwrap() {
                    last_timestamp = Some(ts);
                }
            }

            match event.event_type.as_str() {
                "user" => {
                    let msg = match &event.message {
                        Some(m) => m,
                        None => continue,
                    };
                    let ts = event_ts.unwrap_or_else(Utc::now);

                    match &msg.content {
                        ClaudeContent::Text(text) => {
                            // Plain user message
                            let msg_id = event
                                .uuid
                                .as_ref()
                                .and_then(|u| Uuid::parse_str(u).ok())
                                .unwrap_or_else(Uuid::new_v4);
                            messages.push(Message {
                                id: msg_id,
                                timestamp: ts,
                                role: Role::User,
                                content: text.clone(),
                                tool_calls: Vec::new(),
                                metadata: HashMap::new(),
                            });
                            // Reset assistant tracking since we have a new user turn
                            current_assistant_msg_api_id = None;
                        }
                        ClaudeContent::Blocks(blocks) => {
                            // Tool result blocks — match them back to pending tool_use calls
                            for block in blocks {
                                if let ClaudeContentBlock::ToolResult {
                                    tool_use_id,
                                    content,
                                } = block
                                {
                                    let output = content.as_ref().map(|c| match c {
                                        ToolResultContent::Text(t) => t.clone(),
                                        ToolResultContent::Other(v) => v.to_string(),
                                    });

                                    if let Some(pending) = pending_tools.remove(tool_use_id) {
                                        let tool_call = ToolCall {
                                            name: pending.name.clone(),
                                            input: pending.input.clone(),
                                            output: output.clone(),
                                            status: ToolStatus::Success,
                                        };

                                        // Detect file changes
                                        if let Some(fp) =
                                            extract_file_path(&pending.name, &pending.input)
                                        {
                                            let change_type = if file_paths_seen.contains(&fp) {
                                                ChangeType::Modified
                                            } else {
                                                file_paths_seen.insert(fp.clone());
                                                ChangeType::Created
                                            };

                                            // Use the last assistant message's id for the file change
                                            let msg_id = messages
                                                .last()
                                                .map(|m| m.id)
                                                .unwrap_or_else(Uuid::new_v4);

                                            file_changes.push(FileChange {
                                                path: fp,
                                                change_type,
                                                timestamp: ts,
                                                message_id: msg_id,
                                            });
                                        }

                                        // Attach tool call to the last assistant message
                                        if let Some(last_msg) = messages.last_mut() {
                                            if last_msg.role == Role::Assistant {
                                                last_msg.tool_calls.push(tool_call);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                "assistant" => {
                    let msg = match &event.message {
                        Some(m) => m,
                        None => continue,
                    };
                    let ts = event_ts.unwrap_or_else(Utc::now);
                    let api_id = msg.id.clone();

                    if let ClaudeContent::Blocks(blocks) = &msg.content {
                        for block in blocks {
                            match block {
                                ClaudeContentBlock::Text { text } => {
                                    // Check if this is the same API response as the current
                                    // assistant message (same message.id)
                                    let is_same_api_response = api_id.is_some()
                                        && current_assistant_msg_api_id.is_some()
                                        && api_id == current_assistant_msg_api_id;

                                    if is_same_api_response {
                                        // Append text to the existing assistant message
                                        if let Some(last_msg) = messages.last_mut() {
                                            if last_msg.role == Role::Assistant {
                                                if !last_msg.content.is_empty() {
                                                    last_msg.content.push('\n');
                                                }
                                                last_msg.content.push_str(text);
                                            }
                                        }
                                    } else {
                                        // New assistant message
                                        let msg_id = event
                                            .uuid
                                            .as_ref()
                                            .and_then(|u| Uuid::parse_str(u).ok())
                                            .unwrap_or_else(Uuid::new_v4);
                                        messages.push(Message {
                                            id: msg_id,
                                            timestamp: ts,
                                            role: Role::Assistant,
                                            content: text.clone(),
                                            tool_calls: Vec::new(),
                                            metadata: HashMap::new(),
                                        });
                                        current_assistant_msg_api_id = api_id.clone();
                                    }
                                }
                                ClaudeContentBlock::Thinking { thinking } => {
                                    // Check if we already have an assistant message for this
                                    // API response
                                    let is_same_api_response = api_id.is_some()
                                        && current_assistant_msg_api_id.is_some()
                                        && api_id == current_assistant_msg_api_id;

                                    if is_same_api_response {
                                        // Append thinking to existing message metadata
                                        if let Some(last_msg) = messages.last_mut() {
                                            if last_msg.role == Role::Assistant {
                                                let existing = last_msg
                                                    .metadata
                                                    .entry("thinking".to_string())
                                                    .or_insert_with(|| json!([]));
                                                if let Some(arr) = existing.as_array_mut() {
                                                    arr.push(json!(thinking));
                                                }
                                            }
                                        }
                                    } else {
                                        // Thinking block comes before text in a new response;
                                        // create the assistant message now
                                        let msg_id = event
                                            .uuid
                                            .as_ref()
                                            .and_then(|u| Uuid::parse_str(u).ok())
                                            .unwrap_or_else(Uuid::new_v4);
                                        let mut metadata = HashMap::new();
                                        metadata.insert("thinking".to_string(), json!([thinking]));
                                        messages.push(Message {
                                            id: msg_id,
                                            timestamp: ts,
                                            role: Role::Assistant,
                                            content: String::new(),
                                            tool_calls: Vec::new(),
                                            metadata,
                                        });
                                        current_assistant_msg_api_id = api_id.clone();
                                    }
                                }
                                ClaudeContentBlock::ToolUse { id, name, input } => {
                                    // Ensure we have an assistant message for this API response
                                    let is_same_api_response = api_id.is_some()
                                        && current_assistant_msg_api_id.is_some()
                                        && api_id == current_assistant_msg_api_id;

                                    if !is_same_api_response {
                                        let msg_id = event
                                            .uuid
                                            .as_ref()
                                            .and_then(|u| Uuid::parse_str(u).ok())
                                            .unwrap_or_else(Uuid::new_v4);
                                        messages.push(Message {
                                            id: msg_id,
                                            timestamp: ts,
                                            role: Role::Assistant,
                                            content: String::new(),
                                            tool_calls: Vec::new(),
                                            metadata: HashMap::new(),
                                        });
                                        current_assistant_msg_api_id = api_id.clone();
                                    }

                                    // Store pending tool for matching with tool_result
                                    pending_tools.insert(
                                        id.clone(),
                                        PendingTool {
                                            name: name.clone(),
                                            input: input.clone(),
                                        },
                                    );
                                }
                                ClaudeContentBlock::ToolResult { .. } => {
                                    // Should not appear in assistant messages, skip
                                }
                            }
                        }
                    }
                }
                "summary" => {
                    summary = event.summary.clone();
                }
                "file-history-snapshot" => {
                    // Track files from snapshots for file_changes
                    if let Some(snapshot) = &event.snapshot {
                        if let Some(backups) = snapshot.get("trackedFileBackups") {
                            if let Some(obj) = backups.as_object() {
                                for (file_path, info) in obj {
                                    if !file_paths_seen.contains(file_path) {
                                        file_paths_seen.insert(file_path.clone());
                                        let is_new = info
                                            .get("backupFileName")
                                            .map(|v| v.is_null())
                                            .unwrap_or(false);
                                        let change_type = if is_new {
                                            ChangeType::Created
                                        } else {
                                            ChangeType::Modified
                                        };
                                        let ts = info
                                            .get("backupTime")
                                            .and_then(|v| v.as_str())
                                            .and_then(|s| s.parse::<DateTime<Utc>>().ok())
                                            .or(event_ts)
                                            .unwrap_or_else(Utc::now);

                                        file_changes.push(FileChange {
                                            path: file_path.clone(),
                                            change_type,
                                            timestamp: ts,
                                            message_id: messages
                                                .last()
                                                .map(|m| m.id)
                                                .unwrap_or_else(Uuid::new_v4),
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
                // Skip: system, progress, queue-operation, and any unknown types
                _ => {}
            }
        }

        let now = Utc::now();
        Ok(Conversation {
            id: id.to_string(),
            source_agent: AgentKind::Claude,
            project_dir,
            created_at: first_timestamp.unwrap_or(now),
            updated_at: last_timestamp.unwrap_or(now),
            summary,
            messages,
            file_changes,
        })
    }

    fn write_conversation(&self, conv: &Conversation) -> Result<String> {
        // Generate a new session UUID for this conversation
        let session_id = Uuid::new_v4().to_string();

        // Determine the project subdirectory using path encoding
        let encoded_project = encode_project_path(&conv.project_dir);
        let project_dir = self.projects_dir.join(&encoded_project);
        fs::create_dir_all(&project_dir).with_context(|| {
            format!(
                "Failed to create project directory: {}",
                project_dir.display()
            )
        })?;

        let file_path = project_dir.join(format!("{}.jsonl", session_id));
        let mut file = fs::File::create(&file_path)
            .with_context(|| format!("Failed to create session file: {}", file_path.display()))?;

        // Track parentUuid for conversation tree chaining
        let mut prev_uuid: Option<String> = None;

        // Emit file-history-snapshot if there are file changes
        if !conv.file_changes.is_empty() {
            let mut backups = serde_json::Map::new();
            for fc in &conv.file_changes {
                backups.insert(
                    fc.path.clone(),
                    json!({
                        "backupFileName": null,
                        "version": 1,
                        "backupTime": fc.timestamp.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                    }),
                );
            }
            let snapshot_event = json!({
                "type": "file-history-snapshot",
                "snapshot": {
                    "trackedFileBackups": backups,
                    "timestamp": conv.created_at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
                }
            });
            writeln!(file, "{}", serde_json::to_string(&snapshot_event)?)?;
        }

        // Emit message events with parentUuid chaining
        for msg in &conv.messages {
            let ts = msg
                .timestamp
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
            let event_uuid = Uuid::new_v4().to_string();

            match msg.role {
                Role::User | Role::System => {
                    let mut event = json!({
                        "type": "user",
                        "uuid": event_uuid,
                        "timestamp": ts,
                        "sessionId": session_id,
                        "isSidechain": false,
                        "message": {
                            "role": "user",
                            "content": msg.content.clone()
                        }
                    });
                    if let Some(parent) = &prev_uuid {
                        event
                            .as_object_mut()
                            .unwrap()
                            .insert("parentUuid".to_string(), json!(parent));
                    }
                    writeln!(file, "{}", serde_json::to_string(&event)?)?;
                    prev_uuid = Some(event_uuid);
                }
                Role::Assistant => {
                    // Build content blocks for the assistant message
                    let mut content_blocks: Vec<Value> = Vec::new();
                    let msg_api_id = format!("msg_{}", Uuid::new_v4().to_string().replace('-', ""));

                    // Add thinking blocks if present in metadata
                    if let Some(thinking) = msg.metadata.get("thinking") {
                        if let Some(arr) = thinking.as_array() {
                            for thought in arr {
                                if let Some(t) = thought.as_str() {
                                    content_blocks.push(json!({
                                        "type": "thinking",
                                        "thinking": t,
                                        "signature": "imported"
                                    }));
                                }
                            }
                        }
                    }

                    // Add text content block if non-empty
                    if !msg.content.is_empty() {
                        content_blocks.push(json!({
                            "type": "text",
                            "text": msg.content
                        }));
                    }

                    // Add tool_use blocks (with tool name mapping)
                    let mut tool_result_events: Vec<(String, Value)> = Vec::new();
                    for tc in &msg.tool_calls {
                        let mapped =
                            map_tool(&conv.source_agent, &AgentKind::Claude, &tc.name, &tc.input);
                        let tool_use_id =
                            format!("toolu_{}", Uuid::new_v4().to_string().replace('-', ""));
                        content_blocks.push(json!({
                            "type": "tool_use",
                            "id": tool_use_id,
                            "name": mapped.name,
                            "input": mapped.input
                        }));

                        // Prepare corresponding tool_result event (emitted as a user event)
                        let result_uuid = Uuid::new_v4().to_string();
                        let result_content = tc.output.as_deref().unwrap_or("");
                        tool_result_events.push((
                            result_uuid.clone(),
                            json!({
                                "type": "user",
                                "uuid": result_uuid,
                                "timestamp": ts,
                                "sessionId": session_id,
                                "isSidechain": false,
                                "message": {
                                    "role": "user",
                                    "content": [{
                                        "type": "tool_result",
                                        "tool_use_id": tool_use_id,
                                        "content": result_content
                                    }]
                                }
                            }),
                        ));
                    }

                    // If no content blocks at all, add an empty text block
                    if content_blocks.is_empty() {
                        content_blocks.push(json!({
                            "type": "text",
                            "text": ""
                        }));
                    }

                    // Emit the assistant event with parentUuid
                    let mut assistant_event = json!({
                        "type": "assistant",
                        "uuid": event_uuid,
                        "timestamp": ts,
                        "sessionId": session_id,
                        "isSidechain": false,
                        "message": {
                            "role": "assistant",
                            "id": msg_api_id,
                            "content": content_blocks
                        }
                    });
                    if let Some(parent) = &prev_uuid {
                        assistant_event
                            .as_object_mut()
                            .unwrap()
                            .insert("parentUuid".to_string(), json!(parent));
                    }
                    writeln!(file, "{}", serde_json::to_string(&assistant_event)?)?;
                    prev_uuid = Some(event_uuid);

                    // Emit tool_result events after the assistant event, chaining parentUuid
                    for (result_uuid, mut result_event) in tool_result_events {
                        if let Some(parent) = &prev_uuid {
                            result_event
                                .as_object_mut()
                                .unwrap()
                                .insert("parentUuid".to_string(), json!(parent));
                        }
                        writeln!(file, "{}", serde_json::to_string(&result_event)?)?;
                        prev_uuid = Some(result_uuid);
                    }
                }
            }
        }

        // Emit summary event with leafUuid pointing to the last emitted event
        if let Some(summary_text) = &conv.summary {
            let leaf_uuid = prev_uuid.unwrap_or_else(|| Uuid::new_v4().to_string());
            let summary_event = json!({
                "type": "summary",
                "summary": summary_text,
                "leafUuid": leaf_uuid
            });
            writeln!(file, "{}", serde_json::to_string(&summary_event)?)?;
        }

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

            // Include thinking blocks if present
            if let Some(thinking) = msg.metadata.get("thinking") {
                if let Some(arr) = thinking.as_array() {
                    for thought in arr {
                        if let Some(t) = thought.as_str() {
                            output.push_str("<thinking>\n");
                            output.push_str(t);
                            output.push_str("\n</thinking>\n\n");
                        }
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
                    let truncated = if out.len() > 500 {
                        // Find a valid UTF-8 char boundary at or before byte 500
                        let mut end = 500;
                        while end > 0 && !out.is_char_boundary(end) {
                            end -= 1;
                        }
                        format!("{}... (truncated)", &out[..end])
                    } else {
                        out.clone()
                    };
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
        AgentKind::Claude
    }

    fn display_name(&self) -> &str {
        "Claude Code"
    }

    fn data_dir(&self) -> PathBuf {
        self.projects_dir.clone()
    }

    fn delete_conversation(&self, id: &str) -> Result<()> {
        let path = self.find_session_file(id)?;
        move_path_to_trash(&path)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    /// Helper to create a test session file with given JSONL lines.
    fn create_test_session(
        projects_dir: &Path,
        project_name: &str,
        session_id: &str,
        lines: &[&str],
    ) -> PathBuf {
        let project_dir = projects_dir.join(project_name);
        fs::create_dir_all(&project_dir).unwrap();
        let file_path = project_dir.join(format!("{}.jsonl", session_id));
        let mut file = fs::File::create(&file_path).unwrap();
        for line in lines {
            writeln!(file, "{}", line).unwrap();
        }
        file_path
    }

    #[test]
    fn test_is_available_with_existing_dir() {
        let tmp = TempDir::new().unwrap();
        let adapter = ClaudeAdapter::with_projects_dir(tmp.path().to_path_buf());
        assert!(adapter.is_available());
    }

    #[test]
    fn test_is_available_with_missing_dir() {
        let adapter = ClaudeAdapter::with_projects_dir(PathBuf::from("/nonexistent/path"));
        assert!(!adapter.is_available());
    }

    #[test]
    fn test_list_conversations_empty() {
        let tmp = TempDir::new().unwrap();
        let adapter = ClaudeAdapter::with_projects_dir(tmp.path().to_path_buf());
        let convos = adapter.list_conversations().unwrap();
        assert!(convos.is_empty());
    }

    #[test]
    fn test_read_simple_conversation() {
        let tmp = TempDir::new().unwrap();
        let session_id = "test-session-001";
        let lines = &[
            r#"{"type":"user","uuid":"11111111-1111-1111-1111-111111111111","timestamp":"2026-03-04T15:00:00.000Z","isSidechain":false,"message":{"role":"user","content":"Hello!"}}"#,
            r#"{"type":"assistant","uuid":"22222222-2222-2222-2222-222222222222","timestamp":"2026-03-04T15:00:01.000Z","isSidechain":false,"message":{"role":"assistant","id":"msg_001","content":[{"type":"text","text":"Hi there!"}]}}"#,
        ];
        create_test_session(tmp.path(), "-Users-test-project", session_id, lines);

        let adapter = ClaudeAdapter::with_projects_dir(tmp.path().to_path_buf());
        let conv = adapter.read_conversation(session_id).unwrap();

        assert_eq!(conv.id, session_id);
        assert_eq!(conv.source_agent, AgentKind::Claude);
        assert_eq!(conv.project_dir, "/Users/test/project");
        assert_eq!(conv.messages.len(), 2);
        assert_eq!(conv.messages[0].role, Role::User);
        assert_eq!(conv.messages[0].content, "Hello!");
        assert_eq!(conv.messages[1].role, Role::Assistant);
        assert_eq!(conv.messages[1].content, "Hi there!");
    }

    #[test]
    fn test_read_conversation_with_tool_calls() {
        let tmp = TempDir::new().unwrap();
        let session_id = "test-session-002";
        let lines = &[
            r#"{"type":"user","uuid":"11111111-1111-1111-1111-111111111111","timestamp":"2026-03-04T15:00:00.000Z","isSidechain":false,"message":{"role":"user","content":"Read a file"}}"#,
            r#"{"type":"assistant","uuid":"22222222-2222-2222-2222-222222222222","timestamp":"2026-03-04T15:00:01.000Z","isSidechain":false,"message":{"role":"assistant","id":"msg_001","content":[{"type":"text","text":"Let me read that file."}]}}"#,
            r#"{"type":"assistant","uuid":"33333333-3333-3333-3333-333333333333","timestamp":"2026-03-04T15:00:02.000Z","isSidechain":false,"message":{"role":"assistant","id":"msg_001","content":[{"type":"tool_use","id":"toolu_001","name":"Read","input":{"file_path":"/tmp/test.rs"}}]}}"#,
            r#"{"type":"user","uuid":"44444444-4444-4444-4444-444444444444","timestamp":"2026-03-04T15:00:03.000Z","isSidechain":false,"message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_001","content":"fn main() {}"}]}}"#,
        ];
        create_test_session(tmp.path(), "-Users-test-project", session_id, lines);

        let adapter = ClaudeAdapter::with_projects_dir(tmp.path().to_path_buf());
        let conv = adapter.read_conversation(session_id).unwrap();

        assert_eq!(conv.messages.len(), 2);
        // The assistant message should have a tool call attached
        assert_eq!(conv.messages[1].tool_calls.len(), 1);
        assert_eq!(conv.messages[1].tool_calls[0].name, "Read");
        assert_eq!(
            conv.messages[1].tool_calls[0].output.as_deref(),
            Some("fn main() {}")
        );
    }

    #[test]
    fn test_read_conversation_with_file_changes() {
        let tmp = TempDir::new().unwrap();
        let session_id = "test-session-003";
        let lines = &[
            r#"{"type":"user","uuid":"11111111-1111-1111-1111-111111111111","timestamp":"2026-03-04T15:00:00.000Z","isSidechain":false,"message":{"role":"user","content":"Create a file"}}"#,
            r#"{"type":"assistant","uuid":"22222222-2222-2222-2222-222222222222","timestamp":"2026-03-04T15:00:01.000Z","isSidechain":false,"message":{"role":"assistant","id":"msg_001","content":[{"type":"tool_use","id":"toolu_001","name":"Write","input":{"file_path":"/tmp/new_file.rs","content":"fn main() {}"}}]}}"#,
            r#"{"type":"user","uuid":"33333333-3333-3333-3333-333333333333","timestamp":"2026-03-04T15:00:02.000Z","isSidechain":false,"message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_001","content":"File written successfully"}]}}"#,
        ];
        create_test_session(tmp.path(), "-Users-test-project", session_id, lines);

        let adapter = ClaudeAdapter::with_projects_dir(tmp.path().to_path_buf());
        let conv = adapter.read_conversation(session_id).unwrap();

        assert_eq!(conv.file_changes.len(), 1);
        assert_eq!(conv.file_changes[0].path, "/tmp/new_file.rs");
        assert_eq!(conv.file_changes[0].change_type, ChangeType::Created);
    }

    #[test]
    fn test_skip_sidechain_messages() {
        let tmp = TempDir::new().unwrap();
        let session_id = "test-session-004";
        let lines = &[
            r#"{"type":"user","uuid":"11111111-1111-1111-1111-111111111111","timestamp":"2026-03-04T15:00:00.000Z","isSidechain":false,"message":{"role":"user","content":"Hello"}}"#,
            r#"{"type":"user","uuid":"22222222-2222-2222-2222-222222222222","timestamp":"2026-03-04T15:00:01.000Z","isSidechain":true,"message":{"role":"user","content":"Sidechain message"}}"#,
            r#"{"type":"assistant","uuid":"33333333-3333-3333-3333-333333333333","timestamp":"2026-03-04T15:00:02.000Z","isSidechain":false,"message":{"role":"assistant","id":"msg_001","content":[{"type":"text","text":"Response"}]}}"#,
        ];
        create_test_session(tmp.path(), "-Users-test-project", session_id, lines);

        let adapter = ClaudeAdapter::with_projects_dir(tmp.path().to_path_buf());
        let conv = adapter.read_conversation(session_id).unwrap();

        // Only 2 messages (sidechain skipped)
        assert_eq!(conv.messages.len(), 2);
        assert_eq!(conv.messages[0].content, "Hello");
        assert_eq!(conv.messages[1].content, "Response");
    }

    #[test]
    fn test_read_conversation_with_thinking() {
        let tmp = TempDir::new().unwrap();
        let session_id = "test-session-005";
        let lines = &[
            r#"{"type":"user","uuid":"11111111-1111-1111-1111-111111111111","timestamp":"2026-03-04T15:00:00.000Z","isSidechain":false,"message":{"role":"user","content":"Think about this"}}"#,
            r#"{"type":"assistant","uuid":"22222222-2222-2222-2222-222222222222","timestamp":"2026-03-04T15:00:01.000Z","isSidechain":false,"message":{"role":"assistant","id":"msg_001","content":[{"type":"thinking","thinking":"Let me consider...","signature":"sig123"}]}}"#,
            r#"{"type":"assistant","uuid":"33333333-3333-3333-3333-333333333333","timestamp":"2026-03-04T15:00:02.000Z","isSidechain":false,"message":{"role":"assistant","id":"msg_001","content":[{"type":"text","text":"Here is my answer."}]}}"#,
        ];
        create_test_session(tmp.path(), "-Users-test-project", session_id, lines);

        let adapter = ClaudeAdapter::with_projects_dir(tmp.path().to_path_buf());
        let conv = adapter.read_conversation(session_id).unwrap();

        assert_eq!(conv.messages.len(), 2);
        let assistant_msg = &conv.messages[1];
        assert_eq!(assistant_msg.content, "Here is my answer.");
        let thinking = assistant_msg.metadata.get("thinking").unwrap();
        assert_eq!(thinking.as_array().unwrap().len(), 1);
        assert_eq!(thinking[0].as_str().unwrap(), "Let me consider...");
    }

    #[test]
    fn test_read_conversation_with_summary() {
        let tmp = TempDir::new().unwrap();
        let session_id = "test-session-006";
        let lines = &[
            r#"{"type":"user","uuid":"11111111-1111-1111-1111-111111111111","timestamp":"2026-03-04T15:00:00.000Z","isSidechain":false,"message":{"role":"user","content":"Hello"}}"#,
            r#"{"type":"summary","summary":"A test conversation","leafUuid":"11111111-1111-1111-1111-111111111111"}"#,
        ];
        create_test_session(tmp.path(), "-Users-test-project", session_id, lines);

        let adapter = ClaudeAdapter::with_projects_dir(tmp.path().to_path_buf());
        let conv = adapter.read_conversation(session_id).unwrap();

        assert_eq!(conv.summary.as_deref(), Some("A test conversation"));
    }

    #[test]
    fn test_list_conversations_sorted_by_recent() {
        let tmp = TempDir::new().unwrap();

        // Older session
        create_test_session(
            tmp.path(),
            "-Users-test-project",
            "session-old",
            &[
                r#"{"type":"user","uuid":"11111111-1111-1111-1111-111111111111","timestamp":"2026-01-01T10:00:00.000Z","isSidechain":false,"message":{"role":"user","content":"Old message"}}"#,
            ],
        );

        // Newer session
        create_test_session(
            tmp.path(),
            "-Users-test-project",
            "session-new",
            &[
                r#"{"type":"user","uuid":"22222222-2222-2222-2222-222222222222","timestamp":"2026-03-04T15:00:00.000Z","isSidechain":false,"message":{"role":"user","content":"New message"}}"#,
            ],
        );

        let adapter = ClaudeAdapter::with_projects_dir(tmp.path().to_path_buf());
        let convos = adapter.list_conversations().unwrap();

        assert_eq!(convos.len(), 2);
        assert_eq!(convos[0].id, "session-new");
        assert_eq!(convos[1].id, "session-old");
    }

    #[test]
    fn test_render_prompt() {
        let now = Utc::now();
        let msg_id = Uuid::new_v4();
        let conv = Conversation {
            id: "test-conv".to_string(),
            source_agent: AgentKind::Claude,
            project_dir: "/tmp/project".to_string(),
            created_at: now,
            updated_at: now,
            summary: Some("Test conversation".to_string()),
            messages: vec![
                Message {
                    id: msg_id,
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
        let adapter = ClaudeAdapter::with_projects_dir(tmp.path().to_path_buf());
        let rendered = adapter.render_prompt(&conv).unwrap();

        assert!(rendered.contains("# Conversation: Test conversation"));
        assert!(rendered.contains("**Source:** Claude Code"));
        assert!(rendered.contains("## User"));
        assert!(rendered.contains("Hello!"));
        assert!(rendered.contains("## Assistant"));
        assert!(rendered.contains("Hi there!"));
    }

    #[test]
    fn test_write_conversation_empty() {
        let tmp = TempDir::new().unwrap();
        let adapter = ClaudeAdapter::with_projects_dir(tmp.path().to_path_buf());
        let now = Utc::now();
        let conv = Conversation {
            id: "test".to_string(),
            source_agent: AgentKind::Claude,
            project_dir: "/tmp/project".to_string(),
            created_at: now,
            updated_at: now,
            summary: None,
            messages: Vec::new(),
            file_changes: Vec::new(),
        };
        let session_id = adapter.write_conversation(&conv).unwrap();
        assert!(!session_id.is_empty());

        // Verify the file was created
        let encoded = encode_project_path("/tmp/project");
        let session_file = tmp
            .path()
            .join(&encoded)
            .join(format!("{}.jsonl", session_id));
        assert!(session_file.exists());
    }

    #[test]
    fn test_write_and_read_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let adapter = ClaudeAdapter::with_projects_dir(tmp.path().to_path_buf());
        let now = Utc::now();

        let conv = Conversation {
            id: "original-id".to_string(),
            source_agent: AgentKind::Claude,
            project_dir: "/Users/test/myproject".to_string(),
            created_at: now,
            updated_at: now,
            summary: Some("A test conversation".to_string()),
            messages: vec![
                Message {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    role: Role::User,
                    content: "Hello, write a file!".to_string(),
                    tool_calls: Vec::new(),
                    metadata: HashMap::new(),
                },
                Message {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    role: Role::Assistant,
                    content: "Sure, I will write a file.".to_string(),
                    tool_calls: vec![ToolCall {
                        name: "Write".to_string(),
                        input: serde_json::json!({"file_path": "/tmp/test.rs", "content": "fn main() {}"}),
                        output: Some("File written successfully".to_string()),
                        status: ToolStatus::Success,
                    }],
                    metadata: HashMap::new(),
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

        assert_eq!(read_conv.source_agent, AgentKind::Claude);
        assert_eq!(read_conv.project_dir, "/Users/test/myproject");
        assert_eq!(read_conv.summary.as_deref(), Some("A test conversation"));

        // Verify message content (order and text)
        // After writing, user and assistant messages should round-trip
        // Note: tool results emit extra "user" events in Claude format, so
        // the read back may contain more events. Focus on text messages.
        let user_msgs: Vec<&Message> = read_conv
            .messages
            .iter()
            .filter(|m| m.role == Role::User && !m.content.is_empty())
            .collect();
        let assistant_msgs: Vec<&Message> = read_conv
            .messages
            .iter()
            .filter(|m| m.role == Role::Assistant)
            .collect();

        assert_eq!(user_msgs.len(), 2);
        assert_eq!(user_msgs[0].content, "Hello, write a file!");
        assert_eq!(user_msgs[1].content, "Thanks!");
        assert!(!assistant_msgs.is_empty());
        assert_eq!(assistant_msgs[0].content, "Sure, I will write a file.");
        assert_eq!(assistant_msgs[0].tool_calls.len(), 1);
        assert_eq!(assistant_msgs[0].tool_calls[0].name, "Write");
    }

    #[test]
    fn test_write_conversation_with_thinking() {
        let tmp = TempDir::new().unwrap();
        let adapter = ClaudeAdapter::with_projects_dir(tmp.path().to_path_buf());
        let now = Utc::now();

        let mut metadata = HashMap::new();
        metadata.insert(
            "thinking".to_string(),
            serde_json::json!(["Let me think..."]),
        );

        let conv = Conversation {
            id: "think-test".to_string(),
            source_agent: AgentKind::Claude,
            project_dir: "/tmp/think".to_string(),
            created_at: now,
            updated_at: now,
            summary: None,
            messages: vec![
                Message {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    role: Role::User,
                    content: "Question".to_string(),
                    tool_calls: Vec::new(),
                    metadata: HashMap::new(),
                },
                Message {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    role: Role::Assistant,
                    content: "Answer".to_string(),
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
        assert_eq!(assistant.content, "Answer");
        // The thinking metadata should be preserved
        let thinking = assistant.metadata.get("thinking").unwrap();
        let arr = thinking.as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0].as_str().unwrap(), "Let me think...");
    }

    #[test]
    fn test_write_conversation_with_system_message() {
        let tmp = TempDir::new().unwrap();
        let adapter = ClaudeAdapter::with_projects_dir(tmp.path().to_path_buf());
        let now = Utc::now();

        let conv = Conversation {
            id: "sys-test".to_string(),
            source_agent: AgentKind::Gemini,
            project_dir: "/tmp/sys".to_string(),
            created_at: now,
            updated_at: now,
            summary: None,
            messages: vec![
                Message {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    role: Role::System,
                    content: "System message".to_string(),
                    tool_calls: Vec::new(),
                    metadata: HashMap::new(),
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

        // System messages are written as user messages in Claude format
        let session_id = adapter.write_conversation(&conv).unwrap();
        let read_conv = adapter.read_conversation(&session_id).unwrap();

        // Both system and user messages appear as User role in Claude format
        assert!(read_conv.messages.len() >= 2);
    }

    #[test]
    fn test_encode_project_path() {
        assert_eq!(encode_project_path("/Users/foo/bar"), "-Users-foo-bar");
        assert_eq!(encode_project_path("/tmp"), "-tmp");
        assert_eq!(encode_project_path(""), "");
    }

    #[test]
    fn test_agent_kind_and_display_name() {
        let tmp = TempDir::new().unwrap();
        let adapter = ClaudeAdapter::with_projects_dir(tmp.path().to_path_buf());
        assert_eq!(adapter.agent_kind(), AgentKind::Claude);
        assert_eq!(adapter.display_name(), "Claude Code");
    }

    #[test]
    fn test_session_not_found() {
        let tmp = TempDir::new().unwrap();
        let adapter = ClaudeAdapter::with_projects_dir(tmp.path().to_path_buf());
        let result = adapter.read_conversation("nonexistent-session");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_multiple_tool_calls_in_one_response() {
        let tmp = TempDir::new().unwrap();
        let session_id = "test-session-multi-tool";
        let lines = &[
            r#"{"type":"user","uuid":"11111111-1111-1111-1111-111111111111","timestamp":"2026-03-04T15:00:00.000Z","isSidechain":false,"message":{"role":"user","content":"Create two files"}}"#,
            r#"{"type":"assistant","uuid":"22222222-2222-2222-2222-222222222222","timestamp":"2026-03-04T15:00:01.000Z","isSidechain":false,"message":{"role":"assistant","id":"msg_001","content":[{"type":"text","text":"I will create two files."}]}}"#,
            r#"{"type":"assistant","uuid":"33333333-3333-3333-3333-333333333333","timestamp":"2026-03-04T15:00:02.000Z","isSidechain":false,"message":{"role":"assistant","id":"msg_001","content":[{"type":"tool_use","id":"toolu_001","name":"Write","input":{"file_path":"/tmp/a.rs","content":"a"}}]}}"#,
            r#"{"type":"user","uuid":"44444444-4444-4444-4444-444444444444","timestamp":"2026-03-04T15:00:03.000Z","isSidechain":false,"message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_001","content":"OK"}]}}"#,
            r#"{"type":"assistant","uuid":"55555555-5555-5555-5555-555555555555","timestamp":"2026-03-04T15:00:04.000Z","isSidechain":false,"message":{"role":"assistant","id":"msg_002","content":[{"type":"tool_use","id":"toolu_002","name":"Write","input":{"file_path":"/tmp/b.rs","content":"b"}}]}}"#,
            r#"{"type":"user","uuid":"66666666-6666-6666-6666-666666666666","timestamp":"2026-03-04T15:00:05.000Z","isSidechain":false,"message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"toolu_002","content":"OK"}]}}"#,
        ];
        create_test_session(tmp.path(), "-Users-test-project", session_id, lines);

        let adapter = ClaudeAdapter::with_projects_dir(tmp.path().to_path_buf());
        let conv = adapter.read_conversation(session_id).unwrap();

        // user message + 1 assistant message (msg_001) + 1 assistant message (msg_002)
        assert_eq!(conv.messages.len(), 3);
        assert_eq!(conv.messages[1].tool_calls.len(), 1);
        assert_eq!(conv.messages[1].tool_calls[0].name, "Write");
        assert_eq!(conv.messages[2].tool_calls.len(), 1);
        assert_eq!(conv.messages[2].tool_calls[0].name, "Write");
        // Two file changes
        assert_eq!(conv.file_changes.len(), 2);
    }

    #[test]
    fn test_write_emits_file_history_snapshot() {
        let tmp = TempDir::new().unwrap();
        let adapter = ClaudeAdapter::with_projects_dir(tmp.path().to_path_buf());
        let now = Utc::now();

        let conv = Conversation {
            id: "snap-test".to_string(),
            source_agent: AgentKind::Claude,
            project_dir: "/tmp/snap".to_string(),
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
            file_changes: vec![FileChange {
                path: "src/main.rs".to_string(),
                change_type: ChangeType::Created,
                timestamp: now,
                message_id: Uuid::new_v4(),
            }],
        };

        let session_id = adapter.write_conversation(&conv).unwrap();
        let encoded = encode_project_path("/tmp/snap");
        let session_file = tmp
            .path()
            .join(&encoded)
            .join(format!("{}.jsonl", session_id));
        let content = fs::read_to_string(&session_file).unwrap();
        let first_line = content.lines().next().unwrap();
        let first_event: Value = serde_json::from_str(first_line).unwrap();

        assert_eq!(first_event["type"], "file-history-snapshot");
        assert!(first_event["snapshot"]["trackedFileBackups"]["src/main.rs"].is_object());

        // Read back should recover file changes
        let read_conv = adapter.read_conversation(&session_id).unwrap();
        assert!(!read_conv.file_changes.is_empty());
        assert_eq!(read_conv.file_changes[0].path, "src/main.rs");
    }

    #[test]
    fn test_write_emits_parent_uuid_chain() {
        let tmp = TempDir::new().unwrap();
        let adapter = ClaudeAdapter::with_projects_dir(tmp.path().to_path_buf());
        let now = Utc::now();

        let conv = Conversation {
            id: "chain-test".to_string(),
            source_agent: AgentKind::Claude,
            project_dir: "/tmp/chain".to_string(),
            created_at: now,
            updated_at: now,
            summary: Some("Chain test".to_string()),
            messages: vec![
                Message {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    role: Role::User,
                    content: "First".to_string(),
                    tool_calls: Vec::new(),
                    metadata: HashMap::new(),
                },
                Message {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    role: Role::Assistant,
                    content: "Second".to_string(),
                    tool_calls: Vec::new(),
                    metadata: HashMap::new(),
                },
                Message {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    role: Role::User,
                    content: "Third".to_string(),
                    tool_calls: Vec::new(),
                    metadata: HashMap::new(),
                },
            ],
            file_changes: Vec::new(),
        };

        let session_id = adapter.write_conversation(&conv).unwrap();
        let encoded = encode_project_path("/tmp/chain");
        let session_file = tmp
            .path()
            .join(&encoded)
            .join(format!("{}.jsonl", session_id));
        let content = fs::read_to_string(&session_file).unwrap();
        let events: Vec<Value> = content
            .lines()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();

        // First event has no parentUuid, subsequent events do
        assert!(events[0].get("parentUuid").is_none());
        for i in 1..events.len() {
            if events[i]["type"] == "summary" {
                continue;
            }
            assert!(
                events[i].get("parentUuid").is_some(),
                "Event {} should have parentUuid",
                i
            );
        }

        // Summary leafUuid should match the last message event's uuid
        let summary = events.last().unwrap();
        assert_eq!(summary["type"], "summary");
        let last_msg_uuid = &events[events.len() - 2]["uuid"];
        assert_eq!(summary["leafUuid"], *last_msg_uuid);
    }
}
