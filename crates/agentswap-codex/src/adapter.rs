use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, TimeZone, Utc};
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use uuid::Uuid;

use agentswap_core::adapter::AgentAdapter;
use agentswap_core::files::move_path_to_trash;
use agentswap_core::tool_mapping::map_tool;
use agentswap_core::types::*;

use crate::parser::*;

/// Adapter for reading Codex CLI session data.
///
/// Codex stores thread metadata in SQLite (`~/.codex/state_5.sqlite`) and full
/// conversation transcripts in JSONL rollout files under `~/.codex/sessions/`.
pub struct CodexAdapter {
    /// Path to the Codex home directory, typically `~/.codex`.
    codex_dir: PathBuf,
}

#[derive(Debug, Clone)]
struct CodexThreadDefaults {
    source: String,
    model_provider: String,
    sandbox_policy: String,
    approval_mode: String,
    cli_version: String,
    model: Option<String>,
    reasoning_effort: Option<String>,
    has_user_event: i64,
}

impl Default for CodexThreadDefaults {
    fn default() -> Self {
        Self {
            source: "vscode".to_string(),
            model_provider: "openai".to_string(),
            sandbox_policy: r#"{"type":"workspace-write"}"#.to_string(),
            approval_mode: "on-request".to_string(),
            cli_version: String::new(),
            model: None,
            reasoning_effort: None,
            has_user_event: 0,
        }
    }
}

impl Default for CodexAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl CodexAdapter {
    /// Create a new CodexAdapter using the default `~/.codex` directory.
    pub fn new() -> Self {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        Self {
            codex_dir: home.join(".codex"),
        }
    }

    /// Create a CodexAdapter with a custom Codex directory (for testing).
    #[allow(dead_code)]
    pub fn with_codex_dir(codex_dir: PathBuf) -> Self {
        Self { codex_dir }
    }

    /// Path to the SQLite database.
    fn db_path(&self) -> PathBuf {
        self.codex_dir.join("state_5.sqlite")
    }

    fn normalize_rollout_cwd(project_dir: &str) -> String {
        if project_dir.trim().is_empty() {
            return String::new();
        }

        #[cfg(windows)]
        {
            if project_dir.starts_with('/') {
                return project_dir.to_string();
            }

            let normalized = {
                let replaced = project_dir.replace('/', "\\");
                let mut chars = replaced.chars();
                match (chars.next(), chars.next()) {
                    (Some(drive), Some('\\')) if drive.is_ascii_alphabetic() => {
                        format!("{drive}:\\{}", replaced[2..].trim_start_matches('\\'))
                    }
                    _ => replaced,
                }
            };
            let canonical = PathBuf::from(&normalized)
                .canonicalize()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or(normalized);
            canonical
                .strip_prefix(r"\\?\")
                .unwrap_or(&canonical)
                .to_string()
        }

        #[cfg(not(windows))]
        {
            project_dir.to_string()
        }
    }

    fn normalize_thread_cwd(project_dir: &str) -> String {
        if project_dir.trim().is_empty() {
            return String::new();
        }

        #[cfg(windows)]
        {
            if project_dir.starts_with('/') {
                return project_dir.to_string();
            }

            let normalized = {
                let replaced = project_dir.replace('/', "\\");
                let mut chars = replaced.chars();
                match (chars.next(), chars.next()) {
                    (Some(drive), Some('\\')) if drive.is_ascii_alphabetic() => {
                        format!("{drive}:\\{}", replaced[2..].trim_start_matches('\\'))
                    }
                    _ => replaced,
                }
            };
            PathBuf::from(&normalized)
                .canonicalize()
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or(normalized)
        }

        #[cfg(not(windows))]
        {
            project_dir.to_string()
        }
    }

    fn thread_columns(conn: &Connection) -> Result<HashSet<String>> {
        let mut stmt = conn.prepare("PRAGMA table_info(threads)")?;
        let columns = stmt
            .query_map([], |row| row.get::<_, String>(1))?
            .collect::<std::result::Result<HashSet<_>, _>>()?;
        Ok(columns)
    }

    fn ensure_optional_thread_columns(conn: &Connection) -> Result<()> {
        let columns = Self::thread_columns(conn)?;
        for (name, definition) in [
            ("model", "TEXT"),
            ("reasoning_effort", "TEXT"),
            ("agent_path", "TEXT"),
            ("created_at_ms", "INTEGER"),
            ("updated_at_ms", "INTEGER"),
        ] {
            if !columns.contains(name) {
                conn.execute(
                    &format!("ALTER TABLE threads ADD COLUMN {name} {definition}"),
                    [],
                )?;
            }
        }
        Ok(())
    }

    fn load_thread_defaults(conn: &Connection) -> Result<CodexThreadDefaults> {
        let mut stmt = conn.prepare(
            "SELECT source, model_provider, sandbox_policy, approval_mode, cli_version, \
             model, reasoning_effort, has_user_event \
             FROM threads WHERE source = 'vscode' ORDER BY updated_at DESC LIMIT 1",
        )?;

        match stmt.query_row([], |row| {
            Ok(CodexThreadDefaults {
                source: row.get(0)?,
                model_provider: row.get(1)?,
                sandbox_policy: row.get(2)?,
                approval_mode: row.get(3)?,
                cli_version: row.get(4)?,
                model: row.get(5)?,
                reasoning_effort: row.get(6)?,
                has_user_event: row.get(7)?,
            })
        }) {
            Ok(defaults) => Ok(defaults),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(CodexThreadDefaults::default()),
            Err(err) => Err(err.into()),
        }
    }

    /// Open the SQLite database in read-only mode.
    fn open_db(&self) -> Result<Connection> {
        let path = self.db_path();
        Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY)
            .with_context(|| format!("Failed to open Codex database: {}", path.display()))
    }

    /// Open the SQLite database in read-write mode, creating it if necessary.
    fn open_db_rw(&self) -> Result<Connection> {
        let path = self.db_path();
        let conn = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .with_context(|| {
            format!(
                "Failed to open Codex database for writing: {}",
                path.display()
            )
        })?;

        // Ensure the threads table exists (matches Codex CLI state_5.sqlite schema)
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS threads (
                id TEXT PRIMARY KEY,
                rollout_path TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                source TEXT NOT NULL,
                model_provider TEXT NOT NULL,
                cwd TEXT NOT NULL,
                title TEXT NOT NULL,
                sandbox_policy TEXT NOT NULL,
                approval_mode TEXT NOT NULL,
                tokens_used INTEGER NOT NULL DEFAULT 0,
                has_user_event INTEGER NOT NULL DEFAULT 0,
                archived INTEGER NOT NULL DEFAULT 0,
                archived_at INTEGER,
                git_sha TEXT,
                git_branch TEXT,
                git_origin_url TEXT,
                cli_version TEXT NOT NULL DEFAULT '',
                first_user_message TEXT NOT NULL DEFAULT '',
                agent_nickname TEXT,
                agent_role TEXT,
                memory_mode TEXT NOT NULL DEFAULT 'enabled',
                model TEXT,
                reasoning_effort TEXT,
                agent_path TEXT,
                created_at_ms INTEGER,
                updated_at_ms INTEGER
            );
            CREATE INDEX IF NOT EXISTS idx_threads_created_at ON threads(created_at DESC, id DESC);
            CREATE INDEX IF NOT EXISTS idx_threads_updated_at ON threads(updated_at DESC, id DESC);
            CREATE INDEX IF NOT EXISTS idx_threads_archived ON threads(archived);
            CREATE INDEX IF NOT EXISTS idx_threads_source ON threads(source);
            CREATE INDEX IF NOT EXISTS idx_threads_provider ON threads(model_provider);",
        )?;

        Self::ensure_optional_thread_columns(&conn)?;

        Ok(conn)
    }

    /// Query all threads from the database.
    fn query_threads(&self) -> Result<Vec<CodexThread>> {
        let conn = self.open_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, rollout_path, cwd, title, created_at, updated_at, \
             tokens_used, git_branch, first_user_message \
             FROM threads \
             ORDER BY updated_at DESC, id DESC",
        )?;

        let threads = stmt
            .query_map([], |row| {
                Ok(CodexThread {
                    id: row.get(0)?,
                    rollout_path: row.get(1)?,
                    cwd: row.get(2)?,
                    title: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                    tokens_used: row.get(6)?,
                    git_branch: row.get(7)?,
                    first_user_message: row.get(8)?,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(threads)
    }

    /// Find a specific thread by ID.
    fn find_thread(&self, id: &str) -> Result<CodexThread> {
        let conn = self.open_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, rollout_path, cwd, title, created_at, updated_at, \
             tokens_used, git_branch, first_user_message \
             FROM threads WHERE id = ?1",
        )?;

        stmt.query_row([id], |row| {
            Ok(CodexThread {
                id: row.get(0)?,
                rollout_path: row.get(1)?,
                cwd: row.get(2)?,
                title: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                tokens_used: row.get(6)?,
                git_branch: row.get(7)?,
                first_user_message: row.get(8)?,
            })
        })
        .with_context(|| format!("Thread not found: {}", id))
    }

    /// Convert a unix timestamp (seconds) to a `DateTime<Utc>`.
    fn unix_to_datetime(ts: i64) -> DateTime<Utc> {
        Utc.timestamp_opt(ts, 0).single().unwrap_or_else(Utc::now)
    }
}

/// Intermediate structure for tracking pending tool calls during parsing.
#[derive(Debug)]
struct PendingToolCall {
    name: String,
    input: Value,
}

/// Parse a rollout JSONL file into UCF messages and file changes.
fn parse_rollout(path: &Path) -> Result<(Vec<Message>, Vec<FileChange>)> {
    let file = fs::File::open(path)
        .with_context(|| format!("Failed to open rollout file: {}", path.display()))?;
    let reader = BufReader::new(file);

    let mut messages: Vec<Message> = Vec::new();
    let mut file_changes: Vec<FileChange> = Vec::new();
    let mut pending_tools: HashMap<String, PendingToolCall> = HashMap::new();

    for line_result in reader.lines() {
        let line = match line_result {
            Ok(l) => l,
            Err(_) => continue,
        };

        let event = match parse_event(&line) {
            Some(e) => e,
            None => continue,
        };

        let ptype = match payload_type(&event) {
            Some(t) => t.to_string(),
            None => continue,
        };

        let ts = event
            .timestamp
            .as_ref()
            .and_then(|t| t.parse::<DateTime<Utc>>().ok())
            .unwrap_or_else(Utc::now);

        match event.event_type.as_str() {
            "event_msg" => {
                handle_event_msg(&ptype, &event, ts, &mut messages);
            }
            "response_item" => {
                handle_response_item(
                    &ptype,
                    &event,
                    ts,
                    &mut messages,
                    &mut file_changes,
                    &mut pending_tools,
                );
            }
            // session_meta, turn_context, compacted -> skip
            _ => {}
        }
    }

    Ok((messages, file_changes))
}

/// Handle an `event_msg` event.
fn handle_event_msg(
    ptype: &str,
    event: &CodexEvent,
    ts: DateTime<Utc>,
    messages: &mut Vec<Message>,
) {
    match ptype {
        "user_message" => {
            let text = payload_str(event, "message").unwrap_or("").to_string();
            messages.push(Message {
                id: Uuid::new_v4(),
                timestamp: ts,
                role: Role::User,
                content: text,
                tool_calls: Vec::new(),
                metadata: HashMap::new(),
            });
        }
        "agent_message" => {
            let text = payload_str(event, "message").unwrap_or("").to_string();
            if !text.is_empty() {
                messages.push(Message {
                    id: Uuid::new_v4(),
                    timestamp: ts,
                    role: Role::Assistant,
                    content: text,
                    tool_calls: Vec::new(),
                    metadata: HashMap::new(),
                });
            }
        }
        "agent_reasoning" => {
            let reasoning = payload_str(event, "text").unwrap_or("").to_string();
            if !reasoning.is_empty() {
                // Attach as metadata on the last assistant message, or create one
                if let Some(last) = messages.last_mut() {
                    if last.role == Role::Assistant {
                        // Append reasoning to existing reasoning metadata
                        let existing = last
                            .metadata
                            .get("reasoning")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let combined = if existing.is_empty() {
                            reasoning
                        } else {
                            format!("{}\n{}", existing, reasoning)
                        };
                        last.metadata
                            .insert("reasoning".to_string(), json!(combined));
                        return;
                    }
                }
                // No assistant message to attach to; create a placeholder
                let mut metadata = HashMap::new();
                metadata.insert("reasoning".to_string(), json!(reasoning));
                messages.push(Message {
                    id: Uuid::new_v4(),
                    timestamp: ts,
                    role: Role::Assistant,
                    content: String::new(),
                    tool_calls: Vec::new(),
                    metadata,
                });
            }
        }
        // token_count, task_started, task_complete, turn_aborted, context_compacted -> skip
        _ => {}
    }
}

/// Handle a `response_item` event.
fn handle_response_item(
    ptype: &str,
    event: &CodexEvent,
    ts: DateTime<Utc>,
    messages: &mut Vec<Message>,
    file_changes: &mut Vec<FileChange>,
    pending_tools: &mut HashMap<String, PendingToolCall>,
) {
    let payload = match &event.payload {
        Some(p) => p,
        None => return,
    };

    match ptype {
        "function_call" => {
            let name = payload
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let call_id = payload
                .get("call_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // `arguments` is a JSON-encoded string - parse it
            let input = payload
                .get("arguments")
                .and_then(|v| v.as_str())
                .and_then(|s| serde_json::from_str::<Value>(s).ok())
                .unwrap_or(json!({}));

            if !call_id.is_empty() {
                pending_tools.insert(call_id, PendingToolCall { name, input });
            }
        }
        "function_call_output" => {
            let call_id = payload
                .get("call_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let output = payload
                .get("output")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if let Some(pending) = pending_tools.remove(call_id) {
                let tool_call = ToolCall {
                    name: pending.name,
                    input: pending.input,
                    output: Some(output),
                    status: ToolStatus::Success,
                };
                attach_tool_call(messages, ts, tool_call);
            }
        }
        "custom_tool_call" => {
            let name = payload
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let call_id = payload
                .get("call_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // `input` in custom_tool_call is plain text, not JSON
            let input_text = payload
                .get("input")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let input = Value::String(input_text);

            if !call_id.is_empty() {
                pending_tools.insert(call_id, PendingToolCall { name, input });
            }
        }
        "custom_tool_call_output" => {
            let call_id = payload
                .get("call_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let output = payload
                .get("output")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Detect file changes from the output
            let changes = extract_file_changes(&output);
            let msg_id = messages.last().map(|m| m.id).unwrap_or_else(Uuid::new_v4);
            for (indicator, path) in &changes {
                let change_type = match indicator.as_str() {
                    "A" => ChangeType::Created,
                    "D" => ChangeType::Deleted,
                    _ => ChangeType::Modified, // "M" and anything else
                };
                file_changes.push(FileChange {
                    path: path.clone(),
                    change_type,
                    timestamp: ts,
                    message_id: msg_id,
                });
            }

            if let Some(pending) = pending_tools.remove(call_id) {
                let tool_call = ToolCall {
                    name: pending.name,
                    input: pending.input,
                    output: Some(output),
                    status: ToolStatus::Success,
                };
                attach_tool_call(messages, ts, tool_call);
            }
        }
        // message, reasoning, web_search_call -> skip
        _ => {}
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

/// Attach a completed tool call to the last assistant message,
/// or create a new assistant message if needed.
fn attach_tool_call(messages: &mut Vec<Message>, ts: DateTime<Utc>, tool_call: ToolCall) {
    // Try to attach to the last assistant message
    if let Some(last) = messages.last_mut() {
        if last.role == Role::Assistant {
            last.tool_calls.push(tool_call);
            return;
        }
    }

    // Otherwise create a new assistant message to hold this tool call
    messages.push(Message {
        id: Uuid::new_v4(),
        timestamp: ts,
        role: Role::Assistant,
        content: String::new(),
        tool_calls: vec![tool_call],
        metadata: HashMap::new(),
    });
}

impl AgentAdapter for CodexAdapter {
    fn is_available(&self) -> bool {
        self.db_path().exists()
    }

    fn list_conversations(&self) -> Result<Vec<ConversationSummary>> {
        if !self.is_available() {
            return Ok(Vec::new());
        }

        let threads = self.query_threads()?;
        let mut summaries: Vec<ConversationSummary> = threads
            .iter()
            .map(|t| {
                let created = Self::unix_to_datetime(t.created_at);
                let updated = Self::unix_to_datetime(t.updated_at);

                // Use title if non-empty, otherwise first user message
                let summary = if !t.title.is_empty() {
                    Some(t.title.clone())
                } else if !t.first_user_message.is_empty() {
                    Some(truncate_str(&t.first_user_message, 100))
                } else {
                    None
                };

                // We don't know message/file counts without parsing the rollout,
                // so we set them to 0 in the summary (they are cheap metadata).
                ConversationSummary {
                    id: t.id.clone(),
                    source_agent: AgentKind::Codex,
                    project_dir: t.cwd.clone(),
                    created_at: created,
                    updated_at: updated,
                    summary,
                    message_count: 0,
                    file_count: 0,
                }
            })
            .collect();

        // Already sorted by SQL ORDER BY, but ensure consistency
        summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));

        Ok(summaries)
    }

    fn read_conversation(&self, id: &str) -> Result<Conversation> {
        let thread = self.find_thread(id)?;

        let rollout_path = PathBuf::from(&thread.rollout_path);
        if !rollout_path.exists() {
            anyhow::bail!(
                "Rollout file not found: {} (thread: {})",
                thread.rollout_path,
                id
            );
        }

        let (messages, file_changes) = parse_rollout(&rollout_path)?;

        let created = Self::unix_to_datetime(thread.created_at);
        let updated = Self::unix_to_datetime(thread.updated_at);

        let summary = if !thread.title.is_empty() {
            Some(thread.title)
        } else if !thread.first_user_message.is_empty() {
            Some(thread.first_user_message)
        } else {
            None
        };

        Ok(Conversation {
            id: thread.id,
            source_agent: AgentKind::Codex,
            project_dir: thread.cwd,
            created_at: created,
            updated_at: updated,
            summary,
            messages,
            file_changes,
        })
    }

    fn write_conversation(&self, conv: &Conversation) -> Result<String> {
        // Generate a new thread ID
        let thread_id = Uuid::new_v4().to_string();
        let rollout_cwd = Self::normalize_rollout_cwd(&conv.project_dir);
        let thread_cwd = Self::normalize_thread_cwd(&conv.project_dir);
        let conn = self.open_db_rw()?;
        let defaults = Self::load_thread_defaults(&conn)?;

        // Create the rollout file path: sessions/YYYY/MM/DD/rollout-<uuid>.jsonl
        let now = conv.created_at;
        let sessions_dir = self
            .codex_dir
            .join("sessions")
            .join(format!("{:04}", now.year()))
            .join(format!("{:02}", now.month()))
            .join(format!("{:02}", now.day()));
        fs::create_dir_all(&sessions_dir).with_context(|| {
            format!(
                "Failed to create sessions directory: {}",
                sessions_dir.display()
            )
        })?;

        let rollout_ts = now.format("%Y-%m-%dT%H-%M-%S").to_string();
        let rollout_filename = format!("rollout-{}-{}.jsonl", rollout_ts, thread_id);
        let rollout_path = sessions_dir.join(&rollout_filename);

        let mut file = fs::File::create(&rollout_path).with_context(|| {
            format!("Failed to create rollout file: {}", rollout_path.display())
        })?;

        // Emit session_meta event
        let session_meta_ts = conv
            .created_at
            .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let session_meta = json!({
            "timestamp": session_meta_ts,
            "type": "session_meta",
            "payload": {
                "id": thread_id,
                "timestamp": session_meta_ts,
                "cwd": rollout_cwd,
                "originator": "ChatMem",
                "cli_version": defaults.cli_version,
                "source": defaults.source,
                "model_provider": defaults.model_provider
            }
        });
        writeln!(file, "{}", serde_json::to_string(&session_meta)?)?;

        // Track the first user message for the DB
        let mut first_user_message = String::new();

        // Emit events for each message
        for msg in &conv.messages {
            let ts = msg
                .timestamp
                .to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

            match msg.role {
                Role::User | Role::System => {
                    let event = json!({
                        "timestamp": ts,
                        "type": "event_msg",
                        "payload": {
                            "type": "user_message",
                            "message": msg.content
                        }
                    });
                    writeln!(file, "{}", serde_json::to_string(&event)?)?;

                    if first_user_message.is_empty()
                        && msg.role == Role::User
                        && !msg.content.is_empty()
                    {
                        first_user_message = msg.content.clone();
                    }
                }
                Role::Assistant => {
                    // Emit reasoning if present
                    if let Some(reasoning) = msg.metadata.get("reasoning") {
                        if let Some(text) = reasoning.as_str() {
                            if !text.is_empty() {
                                let event = json!({
                                    "timestamp": ts,
                                    "type": "event_msg",
                                    "payload": {
                                        "type": "agent_reasoning",
                                        "text": text
                                    }
                                });
                                writeln!(file, "{}", serde_json::to_string(&event)?)?;
                            }
                        }
                    }

                    // Emit agent_message if content is non-empty
                    if !msg.content.is_empty() {
                        let event = json!({
                            "timestamp": ts,
                            "type": "event_msg",
                            "payload": {
                                "type": "agent_message",
                                "message": msg.content,
                                "phase": "commentary"
                            }
                        });
                        writeln!(file, "{}", serde_json::to_string(&event)?)?;
                    }

                    // Emit tool calls as function_call + function_call_output pairs
                    for tc in &msg.tool_calls {
                        let mapped =
                            map_tool(&conv.source_agent, &AgentKind::Codex, &tc.name, &tc.input);
                        let call_id =
                            format!("call_{}", Uuid::new_v4().to_string().replace('-', ""));

                        // Determine if this is a "custom_tool_call" (like apply_patch)
                        // or a regular "function_call"
                        let is_custom = mapped.input.is_string();

                        if is_custom {
                            let input_text = mapped.input.as_str().unwrap_or("").to_string();
                            let function_call = json!({
                                "timestamp": ts,
                                "type": "response_item",
                                "payload": {
                                    "type": "custom_tool_call",
                                    "status": "completed",
                                    "call_id": call_id,
                                    "name": mapped.name,
                                    "input": input_text
                                }
                            });
                            writeln!(file, "{}", serde_json::to_string(&function_call)?)?;

                            let output = tc.output.as_deref().unwrap_or("").to_string();
                            let call_output = json!({
                                "timestamp": ts,
                                "type": "response_item",
                                "payload": {
                                    "type": "custom_tool_call_output",
                                    "call_id": call_id,
                                    "output": output
                                }
                            });
                            writeln!(file, "{}", serde_json::to_string(&call_output)?)?;
                        } else {
                            // Regular function_call: arguments is JSON-encoded string
                            let arguments = serde_json::to_string(&mapped.input)?;
                            let function_call = json!({
                                "timestamp": ts,
                                "type": "response_item",
                                "payload": {
                                    "type": "function_call",
                                    "name": mapped.name,
                                    "arguments": arguments,
                                    "call_id": call_id
                                }
                            });
                            writeln!(file, "{}", serde_json::to_string(&function_call)?)?;

                            let output = tc.output.as_deref().unwrap_or("").to_string();
                            let call_output = json!({
                                "timestamp": ts,
                                "type": "response_item",
                                "payload": {
                                    "type": "function_call_output",
                                    "call_id": call_id,
                                    "output": output
                                }
                            });
                            writeln!(file, "{}", serde_json::to_string(&call_output)?)?;
                        }
                    }
                }
            }
        }

        // Insert into the SQLite threads table
        let title = conv
            .summary
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| first_user_message.clone());
        let created_at = conv.created_at.timestamp();
        let updated_at = conv.updated_at.timestamp();
        let created_at_ms = conv.created_at.timestamp_millis();
        let updated_at_ms = conv.updated_at.timestamp_millis();
        let rollout_path_str = rollout_path.to_string_lossy().to_string();

        conn.execute(
            "INSERT INTO threads (id, rollout_path, created_at, updated_at, \
             source, model_provider, cwd, title, sandbox_policy, approval_mode, \
             tokens_used, has_user_event, archived, git_branch, cli_version, \
             first_user_message, memory_mode, model, reasoning_effort, agent_path, \
             created_at_ms, updated_at_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, \
             ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
            rusqlite::params![
                thread_id,
                rollout_path_str,
                created_at,
                updated_at,
                defaults.source,
                defaults.model_provider,
                thread_cwd,
                title,
                defaults.sandbox_policy,
                defaults.approval_mode,
                0i64, // tokens_used
                defaults.has_user_event,
                0i64, // archived
                None::<String>, // git_branch
                defaults.cli_version,
                first_user_message,
                "enabled", // memory_mode
                defaults.model,
                defaults.reasoning_effort,
                None::<String>, // agent_path
                created_at_ms,
                updated_at_ms,
            ],
        )?;

        Ok(thread_id)
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

            // Include reasoning if present (Codex-specific)
            if let Some(reasoning) = msg.metadata.get("reasoning") {
                if let Some(text) = reasoning.as_str() {
                    output.push_str("<thinking>\n");
                    output.push_str(text);
                    output.push_str("\n</thinking>\n\n");
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
        AgentKind::Codex
    }

    fn display_name(&self) -> &str {
        "Codex CLI"
    }

    fn data_dir(&self) -> PathBuf {
        self.codex_dir.clone()
    }

    fn delete_conversation(&self, id: &str) -> Result<()> {
        // Find the thread to get the rollout path
        let thread = self.find_thread(id)?;

        // Delete the rollout file
        let rollout_path = PathBuf::from(&thread.rollout_path);
        move_path_to_trash(&rollout_path)?;

        // Delete from database
        let conn = self.open_db_rw()?;
        conn.execute("DELETE FROM threads WHERE id = ?1", [id])
            .with_context(|| format!("Failed to delete thread from database: {}", id))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write as IoWrite;
    use tempfile::TempDir;

    /// Helper: create a minimal SQLite database with the threads table.
    fn create_test_db(dir: &Path) -> PathBuf {
        let db_path = dir.join("state_5.sqlite");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE threads (
                id TEXT PRIMARY KEY,
                rollout_path TEXT NOT NULL,
                cwd TEXT NOT NULL,
                title TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                tokens_used INTEGER NOT NULL DEFAULT 0,
                git_branch TEXT,
                first_user_message TEXT NOT NULL DEFAULT ''
            );",
        )
        .unwrap();
        db_path
    }

    /// Helper: insert a thread into the test database.
    fn insert_thread(db_path: &Path, thread: &CodexThread) {
        let conn = Connection::open(db_path).unwrap();
        conn.execute(
            "INSERT INTO threads (id, rollout_path, cwd, title, created_at, updated_at, \
             tokens_used, git_branch, first_user_message) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            rusqlite::params![
                thread.id,
                thread.rollout_path,
                thread.cwd,
                thread.title,
                thread.created_at,
                thread.updated_at,
                thread.tokens_used,
                thread.git_branch,
                thread.first_user_message,
            ],
        )
        .unwrap();
    }

    /// Helper: create a rollout JSONL file with the given lines.
    fn create_rollout_file(dir: &Path, filename: &str, lines: &[&str]) -> PathBuf {
        let sessions_dir = dir.join("sessions");
        fs::create_dir_all(&sessions_dir).unwrap();
        let file_path = sessions_dir.join(filename);
        let mut file = fs::File::create(&file_path).unwrap();
        for line in lines {
            writeln!(file, "{}", line).unwrap();
        }
        file_path
    }

    fn sample_rollout_lines() -> Vec<String> {
        let lines = vec![
            // session_meta - skip
            json!({
                "timestamp": "2026-03-01T10:00:00.000Z",
                "type": "session_meta",
                "payload": {"id": "thread-001", "cwd": "/tmp/project"}
            }),
            // user_message
            json!({
                "timestamp": "2026-03-01T10:00:01.000Z",
                "type": "event_msg",
                "payload": {"type": "user_message", "message": "Hello Codex"}
            }),
            // agent_reasoning
            json!({
                "timestamp": "2026-03-01T10:00:02.000Z",
                "type": "event_msg",
                "payload": {"type": "agent_reasoning", "text": "Planning my response"}
            }),
            // agent_message
            json!({
                "timestamp": "2026-03-01T10:00:03.000Z",
                "type": "event_msg",
                "payload": {"type": "agent_message", "message": "Let me help you.", "phase": "commentary"}
            }),
            // function_call
            json!({
                "timestamp": "2026-03-01T10:00:04.000Z",
                "type": "response_item",
                "payload": {
                    "type": "function_call",
                    "name": "exec_command",
                    "arguments": "{\"cmd\":\"ls -la\"}",
                    "call_id": "call_001"
                }
            }),
            // function_call_output
            json!({
                "timestamp": "2026-03-01T10:00:05.000Z",
                "type": "response_item",
                "payload": {
                    "type": "function_call_output",
                    "call_id": "call_001",
                    "output": "total 8\ndrwxr-xr-x 2 user staff 64 Mar 1 10:00 ."
                }
            }),
            // custom_tool_call (apply_patch)
            json!({
                "timestamp": "2026-03-01T10:00:06.000Z",
                "type": "response_item",
                "payload": {
                    "type": "custom_tool_call",
                    "status": "completed",
                    "call_id": "call_002",
                    "name": "apply_patch",
                    "input": "*** Begin Patch\n*** Update File: /tmp/test.rs\n"
                }
            }),
            // custom_tool_call_output with file changes
            json!({
                "timestamp": "2026-03-01T10:00:07.000Z",
                "type": "response_item",
                "payload": {
                    "type": "custom_tool_call_output",
                    "call_id": "call_002",
                    "output": "{\"output\":\"Success. Updated the following files:\\nM /tmp/test.rs\\n\",\"metadata\":{\"exit_code\":0}}"
                }
            }),
            // token_count - skip
            json!({
                "timestamp": "2026-03-01T10:00:08.000Z",
                "type": "event_msg",
                "payload": {"type": "token_count", "info": null}
            }),
            // task_complete - skip
            json!({
                "timestamp": "2026-03-01T10:00:09.000Z",
                "type": "event_msg",
                "payload": {"type": "task_complete", "turn_id": "turn-001", "last_agent_message": "Done."}
            }),
            // response_item/message - skip (duplicate)
            json!({
                "timestamp": "2026-03-01T10:00:03.000Z",
                "type": "response_item",
                "payload": {"type": "message", "role": "user", "content": []}
            }),
            // response_item/reasoning - skip
            json!({
                "timestamp": "2026-03-01T10:00:02.500Z",
                "type": "response_item",
                "payload": {"type": "reasoning", "summary": [{"type": "summary_text", "text": "thinking"}]}
            }),
            // turn_context - skip
            json!({
                "timestamp": "2026-03-01T10:00:00.500Z",
                "type": "turn_context",
                "payload": {"turn_id": "turn-001", "cwd": "/tmp", "model": "gpt-5"}
            }),
        ];
        lines.into_iter().map(|v| v.to_string()).collect()
    }

    #[test]
    fn test_is_available_with_db() {
        let tmp = TempDir::new().unwrap();
        create_test_db(tmp.path());
        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        assert!(adapter.is_available());
    }

    #[test]
    fn test_is_available_without_db() {
        let tmp = TempDir::new().unwrap();
        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        assert!(!adapter.is_available());
    }

    #[test]
    fn test_is_available_nonexistent_dir() {
        let adapter = CodexAdapter::with_codex_dir(PathBuf::from("/nonexistent/codex/dir"));
        assert!(!adapter.is_available());
    }

    #[test]
    fn test_list_conversations_empty() {
        let tmp = TempDir::new().unwrap();
        create_test_db(tmp.path());
        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        let convos = adapter.list_conversations().unwrap();
        assert!(convos.is_empty());
    }

    #[test]
    fn test_list_conversations_returns_threads() {
        let tmp = TempDir::new().unwrap();
        let db_path = create_test_db(tmp.path());

        let rollout_path = create_rollout_file(tmp.path(), "rollout-001.jsonl", &[]);

        insert_thread(
            &db_path,
            &CodexThread {
                id: "thread-001".to_string(),
                rollout_path: rollout_path.to_string_lossy().to_string(),
                cwd: "/tmp/project".to_string(),
                title: "Fix the bug".to_string(),
                created_at: 1770798002,
                updated_at: 1770798114,
                tokens_used: 5000,
                git_branch: Some("main".to_string()),
                first_user_message: "fix the bug in main.rs".to_string(),
            },
        );

        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        let convos = adapter.list_conversations().unwrap();

        assert_eq!(convos.len(), 1);
        assert_eq!(convos[0].id, "thread-001");
        assert_eq!(convos[0].source_agent, AgentKind::Codex);
        assert_eq!(convos[0].project_dir, "/tmp/project");
        assert_eq!(convos[0].summary.as_deref(), Some("Fix the bug"));
    }

    #[test]
    fn test_list_conversations_sorted_by_recent() {
        let tmp = TempDir::new().unwrap();
        let db_path = create_test_db(tmp.path());

        let rollout1 = create_rollout_file(tmp.path(), "rollout-old.jsonl", &[]);
        let rollout2 = create_rollout_file(tmp.path(), "rollout-new.jsonl", &[]);

        insert_thread(
            &db_path,
            &CodexThread {
                id: "thread-old".to_string(),
                rollout_path: rollout1.to_string_lossy().to_string(),
                cwd: "/tmp".to_string(),
                title: "Old thread".to_string(),
                created_at: 1700000000,
                updated_at: 1700000100,
                tokens_used: 100,
                git_branch: None,
                first_user_message: "old message".to_string(),
            },
        );
        insert_thread(
            &db_path,
            &CodexThread {
                id: "thread-new".to_string(),
                rollout_path: rollout2.to_string_lossy().to_string(),
                cwd: "/tmp".to_string(),
                title: "New thread".to_string(),
                created_at: 1770798002,
                updated_at: 1770798114,
                tokens_used: 200,
                git_branch: Some("dev".to_string()),
                first_user_message: "new message".to_string(),
            },
        );

        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        let convos = adapter.list_conversations().unwrap();

        assert_eq!(convos.len(), 2);
        assert_eq!(convos[0].id, "thread-new");
        assert_eq!(convos[1].id, "thread-old");
    }

    #[test]
    fn test_list_conversations_uses_first_user_message_if_no_title() {
        let tmp = TempDir::new().unwrap();
        let db_path = create_test_db(tmp.path());
        let rollout_path = create_rollout_file(tmp.path(), "rollout-notitle.jsonl", &[]);

        insert_thread(
            &db_path,
            &CodexThread {
                id: "thread-notitle".to_string(),
                rollout_path: rollout_path.to_string_lossy().to_string(),
                cwd: "/tmp".to_string(),
                title: "".to_string(),
                created_at: 1770798002,
                updated_at: 1770798114,
                tokens_used: 0,
                git_branch: None,
                first_user_message: "analyze the current codebase".to_string(),
            },
        );

        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        let convos = adapter.list_conversations().unwrap();

        assert_eq!(convos.len(), 1);
        assert_eq!(
            convos[0].summary.as_deref(),
            Some("analyze the current codebase")
        );
    }

    #[test]
    fn test_read_conversation_full() {
        let tmp = TempDir::new().unwrap();
        let db_path = create_test_db(tmp.path());

        let lines = sample_rollout_lines();
        let line_refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        let rollout_path = create_rollout_file(tmp.path(), "rollout-full.jsonl", &line_refs);

        insert_thread(
            &db_path,
            &CodexThread {
                id: "thread-full".to_string(),
                rollout_path: rollout_path.to_string_lossy().to_string(),
                cwd: "/tmp/project".to_string(),
                title: "Full conversation".to_string(),
                created_at: 1770798002,
                updated_at: 1770798114,
                tokens_used: 5000,
                git_branch: Some("main".to_string()),
                first_user_message: "Hello Codex".to_string(),
            },
        );

        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        let conv = adapter.read_conversation("thread-full").unwrap();

        assert_eq!(conv.id, "thread-full");
        assert_eq!(conv.source_agent, AgentKind::Codex);
        assert_eq!(conv.project_dir, "/tmp/project");
        assert_eq!(conv.summary.as_deref(), Some("Full conversation"));

        // Check messages were parsed correctly
        // Should have: user_message, agent_message (with reasoning attached or separate),
        // and tool calls
        assert!(!conv.messages.is_empty(), "Should have parsed messages");

        // First message should be user
        assert_eq!(conv.messages[0].role, Role::User);
        assert_eq!(conv.messages[0].content, "Hello Codex");

        // Should have file changes from custom_tool_call_output
        assert!(!conv.file_changes.is_empty(), "Should have file changes");
        assert_eq!(conv.file_changes[0].path, "/tmp/test.rs");
        assert_eq!(conv.file_changes[0].change_type, ChangeType::Modified);
    }

    #[test]
    fn test_read_conversation_with_function_calls() {
        let tmp = TempDir::new().unwrap();
        let db_path = create_test_db(tmp.path());

        let lines = vec![
            json!({
                "timestamp": "2026-03-01T10:00:00.000Z",
                "type": "event_msg",
                "payload": {"type": "user_message", "message": "run ls"}
            })
            .to_string(),
            json!({
                "timestamp": "2026-03-01T10:00:01.000Z",
                "type": "event_msg",
                "payload": {"type": "agent_message", "message": "Running ls.", "phase": "commentary"}
            })
            .to_string(),
            json!({
                "timestamp": "2026-03-01T10:00:02.000Z",
                "type": "response_item",
                "payload": {
                    "type": "function_call",
                    "name": "exec_command",
                    "arguments": "{\"cmd\":\"ls\"}",
                    "call_id": "call_fc1"
                }
            })
            .to_string(),
            json!({
                "timestamp": "2026-03-01T10:00:03.000Z",
                "type": "response_item",
                "payload": {
                    "type": "function_call_output",
                    "call_id": "call_fc1",
                    "output": "file1.rs\nfile2.rs"
                }
            })
            .to_string(),
        ];
        let line_refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        let rollout_path = create_rollout_file(tmp.path(), "rollout-fc.jsonl", &line_refs);

        insert_thread(
            &db_path,
            &CodexThread {
                id: "thread-fc".to_string(),
                rollout_path: rollout_path.to_string_lossy().to_string(),
                cwd: "/tmp".to_string(),
                title: "Function call test".to_string(),
                created_at: 1770798002,
                updated_at: 1770798114,
                tokens_used: 100,
                git_branch: None,
                first_user_message: "run ls".to_string(),
            },
        );

        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        let conv = adapter.read_conversation("thread-fc").unwrap();

        // Find the assistant message with tool calls
        let assistant_with_tools: Vec<&Message> = conv
            .messages
            .iter()
            .filter(|m| m.role == Role::Assistant && !m.tool_calls.is_empty())
            .collect();
        assert!(
            !assistant_with_tools.is_empty(),
            "Should have assistant message with tool calls"
        );
        let tc = &assistant_with_tools[0].tool_calls[0];
        assert_eq!(tc.name, "exec_command");
        assert_eq!(tc.input["cmd"], "ls");
        assert_eq!(tc.output.as_deref(), Some("file1.rs\nfile2.rs"));
        assert_eq!(tc.status, ToolStatus::Success);
    }

    #[test]
    fn test_read_conversation_with_custom_tool_calls() {
        let tmp = TempDir::new().unwrap();
        let db_path = create_test_db(tmp.path());

        let lines = vec![
            json!({
                "timestamp": "2026-03-01T10:00:00.000Z",
                "type": "event_msg",
                "payload": {"type": "user_message", "message": "patch a file"}
            })
            .to_string(),
            json!({
                "timestamp": "2026-03-01T10:00:01.000Z",
                "type": "event_msg",
                "payload": {"type": "agent_message", "message": "Applying patch.", "phase": "commentary"}
            })
            .to_string(),
            json!({
                "timestamp": "2026-03-01T10:00:02.000Z",
                "type": "response_item",
                "payload": {
                    "type": "custom_tool_call",
                    "status": "completed",
                    "call_id": "call_ct1",
                    "name": "apply_patch",
                    "input": "*** Begin Patch\n*** Update File: /tmp/main.rs\n@@\n+fn main() {}\n"
                }
            })
            .to_string(),
            json!({
                "timestamp": "2026-03-01T10:00:03.000Z",
                "type": "response_item",
                "payload": {
                    "type": "custom_tool_call_output",
                    "call_id": "call_ct1",
                    "output": "{\"output\":\"Success. Updated the following files:\\nM /tmp/main.rs\\nA /tmp/new_file.rs\\n\"}"
                }
            })
            .to_string(),
        ];
        let line_refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        let rollout_path = create_rollout_file(tmp.path(), "rollout-ct.jsonl", &line_refs);

        insert_thread(
            &db_path,
            &CodexThread {
                id: "thread-ct".to_string(),
                rollout_path: rollout_path.to_string_lossy().to_string(),
                cwd: "/tmp".to_string(),
                title: "Custom tool test".to_string(),
                created_at: 1770798002,
                updated_at: 1770798114,
                tokens_used: 200,
                git_branch: None,
                first_user_message: "patch a file".to_string(),
            },
        );

        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        let conv = adapter.read_conversation("thread-ct").unwrap();

        // Check custom tool call was parsed
        let assistant_with_tools: Vec<&Message> = conv
            .messages
            .iter()
            .filter(|m| m.role == Role::Assistant && !m.tool_calls.is_empty())
            .collect();
        assert!(!assistant_with_tools.is_empty());
        let tc = &assistant_with_tools[0].tool_calls[0];
        assert_eq!(tc.name, "apply_patch");
        // Input should be a plain string (not parsed as JSON)
        assert!(tc.input.is_string());
        assert!(tc.input.as_str().unwrap().contains("Begin Patch"));

        // Check file changes
        assert_eq!(conv.file_changes.len(), 2);
        assert_eq!(conv.file_changes[0].path, "/tmp/main.rs");
        assert_eq!(conv.file_changes[0].change_type, ChangeType::Modified);
        assert_eq!(conv.file_changes[1].path, "/tmp/new_file.rs");
        assert_eq!(conv.file_changes[1].change_type, ChangeType::Created);
    }

    #[test]
    fn test_read_conversation_with_reasoning() {
        let tmp = TempDir::new().unwrap();
        let db_path = create_test_db(tmp.path());

        let lines = vec![
            json!({
                "timestamp": "2026-03-01T10:00:00.000Z",
                "type": "event_msg",
                "payload": {"type": "user_message", "message": "think about this"}
            })
            .to_string(),
            json!({
                "timestamp": "2026-03-01T10:00:01.000Z",
                "type": "event_msg",
                "payload": {"type": "agent_message", "message": "Here is my analysis.", "phase": "commentary"}
            })
            .to_string(),
            json!({
                "timestamp": "2026-03-01T10:00:02.000Z",
                "type": "event_msg",
                "payload": {"type": "agent_reasoning", "text": "Deep thinking about the problem"}
            })
            .to_string(),
        ];
        let line_refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        let rollout_path = create_rollout_file(tmp.path(), "rollout-reason.jsonl", &line_refs);

        insert_thread(
            &db_path,
            &CodexThread {
                id: "thread-reason".to_string(),
                rollout_path: rollout_path.to_string_lossy().to_string(),
                cwd: "/tmp".to_string(),
                title: "Reasoning test".to_string(),
                created_at: 1770798002,
                updated_at: 1770798114,
                tokens_used: 100,
                git_branch: None,
                first_user_message: "think about this".to_string(),
            },
        );

        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        let conv = adapter.read_conversation("thread-reason").unwrap();

        // The agent_reasoning should be attached to the last assistant message
        let assistant_msgs: Vec<&Message> = conv
            .messages
            .iter()
            .filter(|m| m.role == Role::Assistant)
            .collect();
        assert!(!assistant_msgs.is_empty());
        let last_assistant = assistant_msgs.last().unwrap();
        let reasoning = last_assistant.metadata.get("reasoning");
        assert!(reasoning.is_some());
        assert!(reasoning
            .unwrap()
            .as_str()
            .unwrap()
            .contains("Deep thinking"));
    }

    #[test]
    fn test_read_conversation_skips_irrelevant_events() {
        let tmp = TempDir::new().unwrap();
        let db_path = create_test_db(tmp.path());

        let lines = vec![
            // Only irrelevant events
            json!({"timestamp": "2026-03-01T10:00:00.000Z", "type": "session_meta", "payload": {"id": "abc"}}).to_string(),
            json!({"timestamp": "2026-03-01T10:00:01.000Z", "type": "turn_context", "payload": {"turn_id": "t1"}}).to_string(),
            json!({"timestamp": "2026-03-01T10:00:02.000Z", "type": "event_msg", "payload": {"type": "token_count"}}).to_string(),
            json!({"timestamp": "2026-03-01T10:00:03.000Z", "type": "event_msg", "payload": {"type": "task_started"}}).to_string(),
            json!({"timestamp": "2026-03-01T10:00:04.000Z", "type": "event_msg", "payload": {"type": "task_complete"}}).to_string(),
            json!({"timestamp": "2026-03-01T10:00:05.000Z", "type": "response_item", "payload": {"type": "message", "role": "user"}}).to_string(),
            json!({"timestamp": "2026-03-01T10:00:06.000Z", "type": "response_item", "payload": {"type": "reasoning", "summary": []}}).to_string(),
        ];
        let line_refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
        let rollout_path = create_rollout_file(tmp.path(), "rollout-skip.jsonl", &line_refs);

        insert_thread(
            &db_path,
            &CodexThread {
                id: "thread-skip".to_string(),
                rollout_path: rollout_path.to_string_lossy().to_string(),
                cwd: "/tmp".to_string(),
                title: "Skip test".to_string(),
                created_at: 1770798002,
                updated_at: 1770798114,
                tokens_used: 0,
                git_branch: None,
                first_user_message: "".to_string(),
            },
        );

        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        let conv = adapter.read_conversation("thread-skip").unwrap();

        // All events should be skipped, no messages
        assert!(conv.messages.is_empty());
        assert!(conv.file_changes.is_empty());
    }

    #[test]
    fn test_read_conversation_thread_not_found() {
        let tmp = TempDir::new().unwrap();
        create_test_db(tmp.path());

        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        let result = adapter.read_conversation("nonexistent-thread");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_read_conversation_rollout_not_found() {
        let tmp = TempDir::new().unwrap();
        let db_path = create_test_db(tmp.path());

        insert_thread(
            &db_path,
            &CodexThread {
                id: "thread-nofile".to_string(),
                rollout_path: "/nonexistent/rollout.jsonl".to_string(),
                cwd: "/tmp".to_string(),
                title: "Missing rollout".to_string(),
                created_at: 1770798002,
                updated_at: 1770798114,
                tokens_used: 0,
                git_branch: None,
                first_user_message: "".to_string(),
            },
        );

        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        let result = adapter.read_conversation("thread-nofile");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_write_conversation_empty() {
        let tmp = TempDir::new().unwrap();
        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        let now = Utc::now();
        let conv = Conversation {
            id: "test".to_string(),
            source_agent: AgentKind::Codex,
            project_dir: "/tmp/project".to_string(),
            created_at: now,
            updated_at: now,
            summary: None,
            messages: Vec::new(),
            file_changes: Vec::new(),
        };
        let thread_id = adapter.write_conversation(&conv).unwrap();
        assert!(!thread_id.is_empty());

        // The adapter should have created the DB and the rollout file
        assert!(adapter.db_path().exists());

        // Should be readable via find_thread
        let thread = adapter.find_thread(&thread_id).unwrap();
        assert_eq!(thread.cwd, "/tmp/project");
    }

    #[test]
    fn test_write_and_read_roundtrip() {
        let tmp = TempDir::new().unwrap();
        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        let now = Utc::now();

        let conv = Conversation {
            id: "original-id".to_string(),
            source_agent: AgentKind::Codex,
            project_dir: "/tmp/project".to_string(),
            created_at: now,
            updated_at: now,
            summary: Some("Test conversation".to_string()),
            messages: vec![
                Message {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    role: Role::User,
                    content: "Hello Codex!".to_string(),
                    tool_calls: Vec::new(),
                    metadata: HashMap::new(),
                },
                Message {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    role: Role::Assistant,
                    content: "Let me help you.".to_string(),
                    tool_calls: vec![ToolCall {
                        name: "exec_command".to_string(),
                        input: json!({"cmd": "ls -la"}),
                        output: Some("file1.rs\nfile2.rs".to_string()),
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
        let thread_id = adapter.write_conversation(&conv).unwrap();
        assert!(!thread_id.is_empty());

        // Read back
        let read_conv = adapter.read_conversation(&thread_id).unwrap();

        assert_eq!(read_conv.source_agent, AgentKind::Codex);
        assert_eq!(read_conv.project_dir, "/tmp/project");
        assert_eq!(read_conv.summary.as_deref(), Some("Test conversation"));

        // Verify user messages
        let user_msgs: Vec<&Message> = read_conv
            .messages
            .iter()
            .filter(|m| m.role == Role::User)
            .collect();
        assert_eq!(user_msgs.len(), 2);
        assert_eq!(user_msgs[0].content, "Hello Codex!");
        assert_eq!(user_msgs[1].content, "Thanks!");

        // Verify assistant message with tool call
        let assistant_msgs: Vec<&Message> = read_conv
            .messages
            .iter()
            .filter(|m| m.role == Role::Assistant)
            .collect();
        assert!(!assistant_msgs.is_empty());

        // Find the assistant message that has tool calls
        let assistant_with_tools: Vec<&&Message> = assistant_msgs
            .iter()
            .filter(|m| !m.tool_calls.is_empty())
            .collect();
        assert!(!assistant_with_tools.is_empty());
        let tc = &assistant_with_tools[0].tool_calls[0];
        assert_eq!(tc.name, "exec_command");
        assert_eq!(tc.input["cmd"], "ls -la");
        assert_eq!(tc.output.as_deref(), Some("file1.rs\nfile2.rs"));
    }

    #[test]
    fn test_write_conversation_with_reasoning() {
        let tmp = TempDir::new().unwrap();
        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        let now = Utc::now();

        let mut metadata = HashMap::new();
        metadata.insert("reasoning".to_string(), json!("Deep analysis here"));

        let conv = Conversation {
            id: "reason-test".to_string(),
            source_agent: AgentKind::Codex,
            project_dir: "/tmp".to_string(),
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

        let thread_id = adapter.write_conversation(&conv).unwrap();
        let read_conv = adapter.read_conversation(&thread_id).unwrap();

        // The reasoning should be attached to the assistant message
        let assistant_msgs: Vec<&Message> = read_conv
            .messages
            .iter()
            .filter(|m| m.role == Role::Assistant)
            .collect();
        assert!(!assistant_msgs.is_empty());

        // Find the assistant message with reasoning in metadata
        let with_reasoning: Vec<&&Message> = assistant_msgs
            .iter()
            .filter(|m| m.metadata.get("reasoning").is_some())
            .collect();
        assert!(!with_reasoning.is_empty());
        let reasoning = with_reasoning[0].metadata.get("reasoning").unwrap();
        assert!(reasoning.as_str().unwrap().contains("Deep analysis"));
    }

    #[test]
    fn test_write_conversation_with_custom_tool_call() {
        let tmp = TempDir::new().unwrap();
        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        let now = Utc::now();

        let conv = Conversation {
            id: "custom-tool-test".to_string(),
            source_agent: AgentKind::Codex,
            project_dir: "/tmp".to_string(),
            created_at: now,
            updated_at: now,
            summary: None,
            messages: vec![
                Message {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    role: Role::User,
                    content: "patch it".to_string(),
                    tool_calls: Vec::new(),
                    metadata: HashMap::new(),
                },
                Message {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    role: Role::Assistant,
                    content: "Patching.".to_string(),
                    tool_calls: vec![ToolCall {
                        name: "apply_patch".to_string(),
                        input: Value::String("*** Begin Patch\n*** Update File: /tmp/main.rs\n".to_string()),
                        output: Some(r#"{"output":"Success. Updated the following files:\nM /tmp/main.rs\n"}"#.to_string()),
                        status: ToolStatus::Success,
                    }],
                    metadata: HashMap::new(),
                },
            ],
            file_changes: Vec::new(),
        };

        let thread_id = adapter.write_conversation(&conv).unwrap();
        let read_conv = adapter.read_conversation(&thread_id).unwrap();

        // Find the assistant message with the custom tool call
        let assistant_with_tools: Vec<&Message> = read_conv
            .messages
            .iter()
            .filter(|m| m.role == Role::Assistant && !m.tool_calls.is_empty())
            .collect();
        assert!(!assistant_with_tools.is_empty());
        let tc = &assistant_with_tools[0].tool_calls[0];
        assert_eq!(tc.name, "apply_patch");
        assert!(tc.input.is_string());
        assert!(tc.input.as_str().unwrap().contains("Begin Patch"));
    }

    #[test]
    fn test_write_conversation_with_system_message() {
        let tmp = TempDir::new().unwrap();
        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        let now = Utc::now();

        let conv = Conversation {
            id: "sys-test".to_string(),
            source_agent: AgentKind::Gemini,
            project_dir: "/tmp".to_string(),
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

        let thread_id = adapter.write_conversation(&conv).unwrap();
        let read_conv = adapter.read_conversation(&thread_id).unwrap();

        // System messages are written as user_message events in Codex
        assert!(!read_conv.messages.is_empty());
        // The first user message in DB should be "Hello" (not system message)
        let thread = adapter.find_thread(&thread_id).unwrap();
        assert_eq!(thread.first_user_message, "Hello");
    }

    #[test]
    fn test_write_conversation_db_thread_metadata() {
        let tmp = TempDir::new().unwrap();
        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        let now = Utc::now();

        let conv = Conversation {
            id: "meta-test".to_string(),
            source_agent: AgentKind::Codex,
            project_dir: "/home/user/project".to_string(),
            created_at: now,
            updated_at: now,
            summary: Some("Important work".to_string()),
            messages: vec![Message {
                id: Uuid::new_v4(),
                timestamp: now,
                role: Role::User,
                content: "first message".to_string(),
                tool_calls: Vec::new(),
                metadata: HashMap::new(),
            }],
            file_changes: Vec::new(),
        };

        let thread_id = adapter.write_conversation(&conv).unwrap();

        // Verify the thread metadata in the DB
        let thread = adapter.find_thread(&thread_id).unwrap();
        assert_eq!(thread.cwd, "/home/user/project");
        assert_eq!(thread.title, "Important work");
        assert_eq!(thread.first_user_message, "first message");
        assert_eq!(thread.created_at, now.timestamp());
        assert_eq!(thread.updated_at, now.timestamp());
    }

    #[test]
    fn test_write_conversation_matches_codex_desktop_defaults() {
        let tmp = TempDir::new().unwrap();
        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        let template_now = Utc::now();

        let conn = adapter.open_db_rw().unwrap();
        conn.execute(
            "INSERT INTO threads (id, rollout_path, created_at, updated_at, \
             source, model_provider, cwd, title, sandbox_policy, approval_mode, \
             tokens_used, has_user_event, archived, git_branch, cli_version, \
             first_user_message, memory_mode, model, reasoning_effort, agent_path, \
             created_at_ms, updated_at_ms) \
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, \
             ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22)",
            rusqlite::params![
                "desktop-template",
                tmp.path()
                    .join("sessions/template-rollout.jsonl")
                    .to_string_lossy()
                    .to_string(),
                template_now.timestamp(),
                template_now.timestamp(),
                "vscode",
                "openai",
                if cfg!(windows) {
                    r"\\?\D:\VSP"
                } else {
                    "/tmp/project"
                },
                "Desktop template",
                r#"{"type":"danger-full-access"}"#,
                "never",
                0i64,
                0i64,
                0i64,
                None::<String>,
                "0.122.0-alpha.1",
                "template prompt",
                "enabled",
                Some("gpt-5.4".to_string()),
                Some("xhigh".to_string()),
                None::<String>,
                template_now.timestamp_millis(),
                template_now.timestamp_millis(),
            ],
        )
        .unwrap();

        let created_at = Utc.timestamp_opt(1_776_653_543, 0).single().unwrap();
        let updated_at = Utc.timestamp_opt(1_776_654_321, 0).single().unwrap();
        let project_dir = if cfg!(windows) {
            "D//VSP".to_string()
        } else {
            "/tmp/project".to_string()
        };

        let conv = Conversation {
            id: "desktop-defaults".to_string(),
            source_agent: AgentKind::Codex,
            project_dir,
            created_at,
            updated_at,
            summary: None,
            messages: vec![Message {
                id: Uuid::new_v4(),
                timestamp: created_at,
                role: Role::User,
                content: "desktop-visible prompt".to_string(),
                tool_calls: Vec::new(),
                metadata: HashMap::new(),
            }],
            file_changes: Vec::new(),
        };

        let thread_id = adapter.write_conversation(&conv).unwrap();

        let conn = adapter.open_db().unwrap();
        let row = conn
            .query_row(
                "SELECT source, sandbox_policy, approval_mode, cli_version, model, \
                 reasoning_effort, has_user_event, created_at_ms, updated_at_ms, cwd, \
                 title FROM threads WHERE id = ?1",
                [thread_id.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, Option<i64>>(7)?,
                        row.get::<_, Option<i64>>(8)?,
                        row.get::<_, String>(9)?,
                        row.get::<_, String>(10)?,
                    ))
                },
            )
            .unwrap();

        assert_eq!(row.0, "vscode");
        assert_eq!(row.1, r#"{"type":"danger-full-access"}"#);
        assert_eq!(row.2, "never");
        assert_eq!(row.3, "0.122.0-alpha.1");
        assert_eq!(row.4.as_deref(), Some("gpt-5.4"));
        assert_eq!(row.5.as_deref(), Some("xhigh"));
        assert_eq!(row.6, 0);
        assert_eq!(row.7, Some(created_at.timestamp_millis()));
        assert_eq!(row.8, Some(updated_at.timestamp_millis()));
        assert_eq!(row.10, "desktop-visible prompt");
        if cfg!(windows) {
            assert!(row.9.ends_with(r"D:\VSP"));
            assert!(row.9.contains(':'));
        } else {
            assert_eq!(row.9, "/tmp/project");
        }

        let thread = adapter.find_thread(&thread_id).unwrap();
        let rollout = fs::read_to_string(&thread.rollout_path).unwrap();
        let session_meta: Value = serde_json::from_str(rollout.lines().next().unwrap()).unwrap();
        assert_eq!(session_meta["payload"]["source"], "vscode");
        assert_eq!(session_meta["payload"]["cli_version"], "0.122.0-alpha.1");
        if cfg!(windows) {
            let rollout_cwd = session_meta["payload"]["cwd"].as_str().unwrap();
            assert!(rollout_cwd.ends_with(r"D:\VSP"));
            assert!(!rollout_cwd.starts_with(r"\\?\"));
        } else {
            assert_eq!(session_meta["payload"]["cwd"], "/tmp/project");
        }
    }

    #[test]
    fn test_agent_kind_and_display_name() {
        let tmp = TempDir::new().unwrap();
        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        assert_eq!(adapter.agent_kind(), AgentKind::Codex);
        assert_eq!(adapter.display_name(), "Codex CLI");
    }

    #[test]
    fn test_data_dir() {
        let tmp = TempDir::new().unwrap();
        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        assert_eq!(adapter.data_dir(), tmp.path().to_path_buf());
    }

    #[test]
    fn test_render_prompt_basic() {
        let now = Utc::now();
        let conv = Conversation {
            id: "test-conv".to_string(),
            source_agent: AgentKind::Codex,
            project_dir: "/tmp/project".to_string(),
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
        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        let rendered = adapter.render_prompt(&conv).unwrap();

        assert!(rendered.contains("# Conversation: Test conversation"));
        assert!(rendered.contains("**Source:** Codex CLI"));
        assert!(rendered.contains("**Project:** /tmp/project"));
        assert!(rendered.contains("## User"));
        assert!(rendered.contains("Hello!"));
        assert!(rendered.contains("## Assistant"));
        assert!(rendered.contains("Hi there!"));
    }

    #[test]
    fn test_render_prompt_with_reasoning() {
        let now = Utc::now();
        let mut metadata = HashMap::new();
        metadata.insert(
            "reasoning".to_string(),
            json!("Deep analysis of the problem"),
        );

        let conv = Conversation {
            id: "test-conv".to_string(),
            source_agent: AgentKind::Codex,
            project_dir: "/tmp".to_string(),
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
        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        let rendered = adapter.render_prompt(&conv).unwrap();

        assert!(rendered.contains("<thinking>"));
        assert!(rendered.contains("Deep analysis of the problem"));
        assert!(rendered.contains("</thinking>"));
    }

    #[test]
    fn test_render_prompt_with_tool_calls() {
        let now = Utc::now();
        let conv = Conversation {
            id: "test-conv".to_string(),
            source_agent: AgentKind::Codex,
            project_dir: "/tmp".to_string(),
            created_at: now,
            updated_at: now,
            summary: None,
            messages: vec![Message {
                id: Uuid::new_v4(),
                timestamp: now,
                role: Role::Assistant,
                content: String::new(),
                tool_calls: vec![ToolCall {
                    name: "exec_command".to_string(),
                    input: json!({"cmd": "ls -la"}),
                    output: Some("file1.rs\nfile2.rs".to_string()),
                    status: ToolStatus::Success,
                }],
                metadata: HashMap::new(),
            }],
            file_changes: Vec::new(),
        };

        let tmp = TempDir::new().unwrap();
        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        let rendered = adapter.render_prompt(&conv).unwrap();

        assert!(rendered.contains("**Tool: exec_command**"));
        assert!(rendered.contains("Output: file1.rs"));
    }

    #[test]
    fn test_render_prompt_with_file_changes() {
        let now = Utc::now();
        let msg_id = Uuid::new_v4();
        let conv = Conversation {
            id: "test-conv".to_string(),
            source_agent: AgentKind::Codex,
            project_dir: "/tmp".to_string(),
            created_at: now,
            updated_at: now,
            summary: None,
            messages: Vec::new(),
            file_changes: vec![
                FileChange {
                    path: "/tmp/main.rs".to_string(),
                    change_type: ChangeType::Modified,
                    timestamp: now,
                    message_id: msg_id,
                },
                FileChange {
                    path: "/tmp/new.rs".to_string(),
                    change_type: ChangeType::Created,
                    timestamp: now,
                    message_id: msg_id,
                },
            ],
        };

        let tmp = TempDir::new().unwrap();
        let adapter = CodexAdapter::with_codex_dir(tmp.path().to_path_buf());
        let rendered = adapter.render_prompt(&conv).unwrap();

        assert!(rendered.contains("## Files Changed"));
        assert!(rendered.contains("`/tmp/main.rs` (modified)"));
        assert!(rendered.contains("`/tmp/new.rs` (created)"));
    }

    #[test]
    fn test_unix_to_datetime() {
        let dt = CodexAdapter::unix_to_datetime(1770798002);
        assert_eq!(dt.timestamp(), 1770798002);
    }

    #[test]
    fn test_parse_rollout_empty_file() {
        let tmp = TempDir::new().unwrap();
        let path = create_rollout_file(tmp.path(), "empty.jsonl", &[]);
        let (messages, file_changes) = parse_rollout(&path).unwrap();
        assert!(messages.is_empty());
        assert!(file_changes.is_empty());
    }

    #[test]
    fn test_parse_rollout_malformed_lines() {
        let tmp = TempDir::new().unwrap();
        let path = create_rollout_file(
            tmp.path(),
            "malformed.jsonl",
            &["not json", "{incomplete", ""],
        );
        let (messages, _) = parse_rollout(&path).unwrap();
        assert!(messages.is_empty()); // All lines should be gracefully skipped
    }

    // Integration test: skip if Codex CLI is not installed
    #[test]
    fn test_real_codex_sessions_if_available() {
        let adapter = CodexAdapter::new();
        if !adapter.is_available() {
            eprintln!("Skipping real Codex integration test: ~/.codex/state_5.sqlite not found");
            return;
        }

        let convos = adapter.list_conversations().unwrap();
        eprintln!("Found {} Codex threads", convos.len());

        assert!(!convos.is_empty(), "Expected at least one Codex thread");

        // Verify all entries have sensible fields
        for conv in &convos {
            assert!(!conv.id.is_empty(), "Thread id should not be empty");
            assert_eq!(conv.source_agent, AgentKind::Codex);
        }

        // Verify sorted by most recent first
        for window in convos.windows(2) {
            assert!(
                window[0].updated_at >= window[1].updated_at,
                "Conversations should be sorted by most recent first"
            );
        }
    }

    #[test]
    fn test_real_read_conversation_if_available() {
        let adapter = CodexAdapter::new();
        if !adapter.is_available() {
            eprintln!("Skipping real Codex read test: ~/.codex/state_5.sqlite not found");
            return;
        }

        let convos = adapter.list_conversations().unwrap();
        if convos.is_empty() {
            eprintln!("Skipping: no Codex threads found");
            return;
        }

        // Try to find a thread with an existing rollout file
        for summary in &convos {
            match adapter.read_conversation(&summary.id) {
                Ok(conv) => {
                    assert_eq!(conv.source_agent, AgentKind::Codex);
                    eprintln!(
                        "Read Codex thread '{}': {} messages, {} file changes",
                        conv.summary.as_deref().unwrap_or(&conv.id),
                        conv.messages.len(),
                        conv.file_changes.len()
                    );

                    // Test render_prompt too
                    let rendered = adapter.render_prompt(&conv).unwrap();
                    assert!(rendered.contains("**Source:** Codex CLI"));

                    return; // Success, one is enough
                }
                Err(e) => {
                    // Rollout file might not exist, try next
                    eprintln!("Skipping thread {}: {}", summary.id, e);
                    continue;
                }
            }
        }

        eprintln!("Skipping: no Codex threads with accessible rollout files found");
    }

    #[test]
    fn test_real_agent_metadata() {
        let adapter = CodexAdapter::new();
        assert_eq!(adapter.agent_kind(), AgentKind::Codex);
        assert_eq!(adapter.display_name(), "Codex CLI");
    }

    #[test]
    fn test_real_data_dir() {
        let adapter = CodexAdapter::new();
        let data_dir = adapter.data_dir();
        let data_dir_str = data_dir.to_string_lossy();
        assert!(
            data_dir_str.ends_with(".codex"),
            "data_dir should end with .codex, got: {}",
            data_dir_str
        );
    }
}
