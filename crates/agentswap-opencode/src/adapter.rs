use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

use agentswap_core::adapter::AgentAdapter;
use agentswap_core::types::{
    AgentKind, ChangeType, Conversation, ConversationSummary, FileChange, Message, Role,
    ToolCall, ToolStatus,
};
use anyhow::{anyhow, Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use uuid::Uuid;

pub struct OpenCodeAdapter {
    data_dir: PathBuf,
}

#[derive(Debug, Clone)]
struct OpenCodeSessionRow {
    id: String,
    directory: String,
    title: String,
    created_at_ms: i64,
    updated_at_ms: i64,
    summary_files: Option<i64>,
    worktree: Option<String>,
}

impl Default for OpenCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl OpenCodeAdapter {
    pub fn new() -> Self {
        Self {
            data_dir: Self::default_data_dir(),
        }
    }

    #[allow(dead_code)]
    pub fn with_data_dir(data_dir: PathBuf) -> Self {
        Self { data_dir }
    }

    pub fn default_data_dir() -> PathBuf {
        for candidate in Self::candidate_data_dirs() {
            if Self::find_existing_db(&candidate).is_some() {
                return candidate;
            }
        }

        Self::candidate_data_dirs()
            .into_iter()
            .next()
            .unwrap_or_else(|| PathBuf::from("."))
    }

    fn candidate_data_dirs() -> Vec<PathBuf> {
        let mut candidates = Vec::new();

        if let Ok(xdg_data_home) = env::var("XDG_DATA_HOME") {
            if !xdg_data_home.trim().is_empty() {
                candidates.push(PathBuf::from(xdg_data_home).join("opencode"));
            }
        }

        if let Some(home) = dirs::home_dir() {
            candidates.push(home.join(".local").join("share").join("opencode"));
        }

        if let Some(local) = dirs::data_local_dir() {
            candidates.push(local.join("opencode"));
        }

        if let Some(data) = dirs::data_dir() {
            candidates.push(data.join("opencode"));
        }

        candidates
    }

    pub fn db_path(&self) -> PathBuf {
        if let Ok(custom_db) = env::var("OPENCODE_DB") {
            if !custom_db.trim().is_empty() {
                let path = PathBuf::from(custom_db);
                if path.is_absolute() {
                    return path;
                }
                return self.data_dir.join(path);
            }
        }

        Self::find_existing_db(&self.data_dir).unwrap_or_else(|| self.data_dir.join("opencode.db"))
    }

    fn find_existing_db(data_dir: &Path) -> Option<PathBuf> {
        let primary = data_dir.join("opencode.db");
        if primary.exists() {
            return Some(primary);
        }

        let mut channel_dbs = std::fs::read_dir(data_dir)
            .ok()?
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let path = entry.path();
                let name = path.file_name()?.to_str()?;
                if !name.starts_with("opencode-") || !name.ends_with(".db") {
                    return None;
                }
                let modified = entry.metadata().ok().and_then(|metadata| metadata.modified().ok());
                Some((modified, path))
            })
            .collect::<Vec<_>>();

        channel_dbs.sort_by(|left, right| {
            right
                .0
                .cmp(&left.0)
                .then_with(|| left.1.file_name().cmp(&right.1.file_name()))
        });
        channel_dbs.into_iter().map(|(_, path)| path).next()
    }

    fn open_db(&self) -> Result<Connection> {
        let path = self.db_path();
        Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_URI,
        )
        .with_context(|| format!("Failed to open OpenCode database: {}", path.display()))
    }

    fn open_db_rw(&self) -> Result<Connection> {
        let path = self.db_path();
        Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_URI,
        )
        .with_context(|| {
            format!(
                "Failed to open OpenCode database for writing: {}",
                path.display()
            )
        })
    }

    pub fn restore_conversation_in_db(db_path: &Path, id: &str) -> Result<()> {
        let conn = Connection::open_with_flags(
            db_path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_URI,
        )
        .with_context(|| {
            format!(
                "Failed to open OpenCode database for restore: {}",
                db_path.display()
            )
        })?;
        let changed = conn.execute(
            "UPDATE session SET time_archived = NULL, time_updated = ?1 WHERE id = ?2",
            (Utc::now().timestamp_millis(), id),
        )?;
        if changed == 0 {
            return Err(anyhow!("OpenCode session not found: {id}"));
        }
        Ok(())
    }

    pub fn restore_conversation(&self, id: &str) -> Result<()> {
        Self::restore_conversation_in_db(&self.db_path(), id)
    }

    fn query_sessions(&self) -> Result<Vec<OpenCodeSessionRow>> {
        let conn = self.open_db()?;
        let mut stmt = conn.prepare(
            "SELECT s.id, s.directory, s.title, s.time_created, s.time_updated, \
                    s.summary_files, p.worktree \
             FROM session s \
             LEFT JOIN project p ON p.id = s.project_id \
             WHERE s.time_archived IS NULL \
             ORDER BY s.time_updated DESC, s.id DESC",
        )?;

        let rows = stmt
            .query_map([], |row| {
                Ok(OpenCodeSessionRow {
                    id: row.get(0)?,
                    directory: row.get(1)?,
                    title: row.get(2)?,
                    created_at_ms: row.get(3)?,
                    updated_at_ms: row.get(4)?,
                    summary_files: row.get(5)?,
                    worktree: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    fn find_session(&self, id: &str) -> Result<OpenCodeSessionRow> {
        let conn = self.open_db()?;
        let mut stmt = conn.prepare(
            "SELECT s.id, s.directory, s.title, s.time_created, s.time_updated, \
                    s.summary_files, p.worktree \
             FROM session s \
             LEFT JOIN project p ON p.id = s.project_id \
             WHERE s.id = ?1",
        )?;

        stmt.query_row([id], |row| {
            Ok(OpenCodeSessionRow {
                id: row.get(0)?,
                directory: row.get(1)?,
                title: row.get(2)?,
                created_at_ms: row.get(3)?,
                updated_at_ms: row.get(4)?,
                summary_files: row.get(5)?,
                worktree: row.get(6)?,
            })
        })
        .with_context(|| format!("OpenCode session not found: {id}"))
    }

    fn project_dir(row: &OpenCodeSessionRow) -> String {
        row.worktree
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(&row.directory)
            .to_string()
    }

    fn message_count(conn: &Connection, session_id: &str) -> Result<usize> {
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM message WHERE session_id = ?1",
            [session_id],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as usize)
    }

    fn file_count(conn: &Connection, session_id: &str, summary_files: Option<i64>) -> Result<usize> {
        if let Some(count) = summary_files {
            return Ok(count.max(0) as usize);
        }

        let mut stmt = conn.prepare(
            "SELECT data FROM part WHERE session_id = ?1 AND json_extract(data, '$.type') = 'patch'",
        )?;
        let mut files = std::collections::BTreeSet::new();
        let rows = stmt.query_map([session_id], |row| row.get::<_, String>(0))?;
        for row in rows {
            let raw = row?;
            let Ok(value) = serde_json::from_str::<Value>(&raw) else {
                continue;
            };
            if let Some(items) = value.get("files").and_then(|value| value.as_array()) {
                for item in items {
                    if let Some(path) = item.as_str() {
                        files.insert(path.to_string());
                    }
                }
            }
        }
        Ok(files.len())
    }

    fn ms_to_datetime(ms: i64) -> DateTime<Utc> {
        Utc.timestamp_millis_opt(ms).single().unwrap_or_else(Utc::now)
    }

    fn stable_uuid(source: &str) -> Uuid {
        Uuid::new_v5(&Uuid::NAMESPACE_URL, source.as_bytes())
    }

    fn value_string(value: &Value, key: &str) -> Option<String> {
        value
            .get(key)
            .and_then(|value| value.as_str())
            .map(ToString::to_string)
    }

    fn role_from_value(value: &Value) -> Role {
        match value.get("role").and_then(|value| value.as_str()) {
            Some("assistant") => Role::Assistant,
            Some("system") => Role::System,
            _ => Role::User,
        }
    }

    fn message_timestamp(value: &Value, fallback_ms: i64) -> DateTime<Utc> {
        let created = value
            .get("time")
            .and_then(|time| time.get("created"))
            .and_then(|created| created.as_i64())
            .unwrap_or(fallback_ms);
        Self::ms_to_datetime(created)
    }

    fn part_timestamp(value: &Value, fallback_ms: i64) -> DateTime<Utc> {
        let from_time_object = value
            .get("time")
            .and_then(|time| time.get("start").or_else(|| time.get("created")))
            .and_then(|time| time.as_i64());

        let from_state_time = value
            .get("state")
            .and_then(|state| state.get("time"))
            .and_then(|time| time.get("start").or_else(|| time.get("created")))
            .and_then(|time| time.as_i64());

        Self::ms_to_datetime(from_time_object.or(from_state_time).unwrap_or(fallback_ms))
    }

    fn read_parts_for_message(
        conn: &Connection,
        session_id: &str,
        message_id: &str,
        ucf_message_id: Uuid,
    ) -> Result<(String, Vec<ToolCall>, Vec<FileChange>, Value)> {
        let mut stmt = conn.prepare(
            "SELECT id, time_created, data \
             FROM part \
             WHERE session_id = ?1 AND message_id = ?2 \
             ORDER BY time_created ASC, id ASC",
        )?;
        let rows = stmt.query_map((session_id, message_id), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        let mut content = Vec::new();
        let mut tool_calls = Vec::new();
        let mut file_changes = Vec::new();
        let mut reasoning = Vec::new();
        let mut source_part_ids = Vec::new();

        for row in rows {
            let (part_id, created_at_ms, raw_data) = row?;
            source_part_ids.push(Value::String(part_id));
            let Ok(data) = serde_json::from_str::<Value>(&raw_data) else {
                continue;
            };
            match data.get("type").and_then(|value| value.as_str()) {
                Some("text") => {
                    if data
                        .get("ignored")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false)
                    {
                        continue;
                    }
                    if let Some(text) = data.get("text").and_then(|value| value.as_str()) {
                        if !text.trim().is_empty() {
                            content.push(text.to_string());
                        }
                    }
                }
                Some("reasoning") => {
                    if let Some(text) = data.get("text").and_then(|value| value.as_str()) {
                        reasoning.push(Value::String(text.to_string()));
                    }
                }
                Some("tool") => {
                    let state = data.get("state").cloned().unwrap_or_else(|| json!({}));
                    let status = match state.get("status").and_then(|value| value.as_str()) {
                        Some("error") => ToolStatus::Error,
                        _ => ToolStatus::Success,
                    };
                    let output = state
                        .get("output")
                        .or_else(|| state.get("error"))
                        .or_else(|| {
                            state
                                .get("metadata")
                                .and_then(|metadata| metadata.get("output"))
                        })
                        .and_then(|value| value.as_str())
                        .map(ToString::to_string);
                    tool_calls.push(ToolCall {
                        name: data
                            .get("tool")
                            .and_then(|value| value.as_str())
                            .unwrap_or("tool")
                            .to_string(),
                        input: state.get("input").cloned().unwrap_or_else(|| json!({})),
                        output,
                        status,
                    });
                }
                Some("patch") => {
                    let timestamp = Self::part_timestamp(&data, created_at_ms);
                    if let Some(files) = data.get("files").and_then(|value| value.as_array()) {
                        for file in files {
                            if let Some(path) = file.as_str() {
                                file_changes.push(FileChange {
                                    path: path.to_string(),
                                    change_type: ChangeType::Modified,
                                    timestamp,
                                    message_id: ucf_message_id,
                                });
                            }
                        }
                    }
                }
                Some("file") => {
                    let label = data
                        .get("filename")
                        .or_else(|| data.get("url"))
                        .and_then(|value| value.as_str());
                    if let Some(label) = label {
                        content.push(format!("[file: {label}]"));
                    }
                }
                _ => {}
            }
        }

        let metadata = json!({
            "opencode_part_ids": source_part_ids,
            "reasoning": reasoning,
        });

        Ok((content.join("\n\n"), tool_calls, file_changes, metadata))
    }

    fn messages_for_session(&self, conn: &Connection, session_id: &str) -> Result<(Vec<Message>, Vec<FileChange>)> {
        let mut stmt = conn.prepare(
            "SELECT id, time_created, data \
             FROM message \
             WHERE session_id = ?1 \
             ORDER BY time_created ASC, id ASC",
        )?;
        let rows = stmt.query_map([session_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?;

        let mut messages = Vec::new();
        let mut file_changes = Vec::new();

        for row in rows {
            let (source_message_id, created_at_ms, raw_data) = row?;
            let data = serde_json::from_str::<Value>(&raw_data).unwrap_or_else(|_| json!({}));
            let message_uuid =
                Self::stable_uuid(&format!("opencode:{session_id}:{source_message_id}"));
            let (content, tool_calls, changes, mut metadata) =
                Self::read_parts_for_message(conn, session_id, &source_message_id, message_uuid)?;

            if let Some(object) = metadata.as_object_mut() {
                object.insert(
                    "opencode_message_id".to_string(),
                    Value::String(source_message_id.clone()),
                );
                if let Some(provider) = Self::value_string(&data, "providerID") {
                    object.insert("provider_id".to_string(), Value::String(provider));
                }
                if let Some(model) = Self::value_string(&data, "modelID") {
                    object.insert("model_id".to_string(), Value::String(model));
                }
                if let Some(agent) = Self::value_string(&data, "agent") {
                    object.insert("agent".to_string(), Value::String(agent));
                }
            }

            file_changes.extend(changes);
            messages.push(Message {
                id: message_uuid,
                timestamp: Self::message_timestamp(&data, created_at_ms),
                role: Self::role_from_value(&data),
                content,
                tool_calls,
                metadata: serde_json::from_value(metadata).unwrap_or_else(|_| HashMap::new()),
            });
        }

        Ok((messages, file_changes))
    }

    fn compact_id(prefix: &str) -> String {
        format!("{prefix}_{}", Uuid::new_v4().simple())
    }

    fn conversation_title(conv: &Conversation) -> String {
        conv.summary
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
            .or_else(|| {
                conv.messages
                    .iter()
                    .find(|message| {
                        message.role == Role::User && !message.content.trim().is_empty()
                    })
                    .map(|message| message.content.trim().to_string())
            })
            .map(|title| {
                if title.chars().count() > 80 {
                    format!("{}...", title.chars().take(80).collect::<String>())
                } else {
                    title
                }
            })
            .unwrap_or_else(|| "ChatMem imported conversation".to_string())
    }

    fn slug_from_title(title: &str) -> String {
        let mut slug = String::new();
        let mut last_was_dash = false;
        for ch in title.chars() {
            if ch.is_ascii_alphanumeric() {
                slug.push(ch.to_ascii_lowercase());
                last_was_dash = false;
            } else if ch.is_whitespace() || ch == '-' || ch == '_' {
                if !last_was_dash && !slug.is_empty() {
                    slug.push('-');
                    last_was_dash = true;
                }
            }
        }
        let slug = slug.trim_matches('-');
        if slug.is_empty() {
            "chatmem-import".to_string()
        } else {
            slug.to_string()
        }
    }

    fn project_name(project_dir: &str) -> String {
        Path::new(project_dir)
            .file_name()
            .and_then(|name| name.to_str())
            .map(ToString::to_string)
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| "ChatMem".to_string())
    }

    fn latest_session_version(conn: &Connection) -> String {
        conn.query_row(
            "SELECT version FROM session WHERE version != '' ORDER BY time_updated DESC LIMIT 1",
            [],
            |row| row.get::<_, String>(0),
        )
        .unwrap_or_else(|_| "0.0.0".to_string())
    }

    fn find_or_create_project(conn: &Connection, project_dir: &str, now_ms: i64) -> Result<String> {
        if let Ok(project_id) = conn.query_row(
            "SELECT id FROM project WHERE worktree = ?1 ORDER BY time_updated DESC LIMIT 1",
            [project_dir],
            |row| row.get::<_, String>(0),
        ) {
            return Ok(project_id);
        }

        let project_id = Self::compact_id("project");
        conn.execute(
            "INSERT INTO project (id, worktree, vcs, name, time_created, time_updated, sandboxes)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                project_id.as_str(),
                project_dir,
                "git",
                Self::project_name(project_dir),
                now_ms,
                now_ms,
                "[]",
            ),
        )
        .with_context(|| "Failed to create OpenCode project row")?;
        Ok(project_id)
    }

    fn insert_part(
        conn: &Connection,
        message_id: &str,
        session_id: &str,
        timestamp_ms: i64,
        data: Value,
    ) -> Result<()> {
        let part_id = Self::compact_id("part");
        conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                part_id,
                message_id,
                session_id,
                timestamp_ms,
                timestamp_ms,
                data.to_string(),
            ),
        )?;
        Ok(())
    }
}

impl AgentAdapter for OpenCodeAdapter {
    fn is_available(&self) -> bool {
        self.db_path().exists()
    }

    fn list_conversations(&self) -> Result<Vec<ConversationSummary>> {
        if !self.is_available() {
            return Ok(Vec::new());
        }

        let conn = self.open_db()?;
        let mut conversations = Vec::new();
        for row in self.query_sessions()? {
            let message_count = Self::message_count(&conn, &row.id)?;
            let file_count = Self::file_count(&conn, &row.id, row.summary_files)?;
            conversations.push(ConversationSummary {
                id: row.id.clone(),
                source_agent: AgentKind::OpenCode,
                project_dir: Self::project_dir(&row),
                created_at: Self::ms_to_datetime(row.created_at_ms),
                updated_at: Self::ms_to_datetime(row.updated_at_ms),
                summary: if row.title.trim().is_empty() {
                    None
                } else {
                    Some(row.title)
                },
                message_count,
                file_count,
            });
        }

        Ok(conversations)
    }

    fn read_conversation(&self, id: &str) -> Result<Conversation> {
        let session = self.find_session(id)?;
        let conn = self.open_db()?;
        let (messages, file_changes) = self.messages_for_session(&conn, id)?;

        Ok(Conversation {
            id: session.id.clone(),
            source_agent: AgentKind::OpenCode,
            project_dir: Self::project_dir(&session),
            created_at: Self::ms_to_datetime(session.created_at_ms),
            updated_at: Self::ms_to_datetime(session.updated_at_ms),
            summary: if session.title.trim().is_empty() {
                None
            } else {
                Some(session.title)
            },
            messages,
            file_changes,
        })
    }

    fn write_conversation(&self, conv: &Conversation) -> Result<String> {
        let mut conn = self.open_db_rw()?;
        let tx = conn.transaction()?;
        let created_ms = conv.created_at.timestamp_millis();
        let updated_ms = conv.updated_at.timestamp_millis();
        let project_dir = conv.project_dir.trim();
        let project_dir = if project_dir.is_empty() { "." } else { project_dir };
        let project_id = Self::find_or_create_project(&tx, project_dir, created_ms)?;
        let session_id = Self::compact_id("ses");
        let title = Self::conversation_title(conv);
        let slug = Self::slug_from_title(&title);
        let version = Self::latest_session_version(&tx);
        let file_count = conv
            .file_changes
            .iter()
            .map(|change| change.path.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            .len() as i64;

        tx.execute(
            "INSERT INTO session (
                id, project_id, slug, directory, title, version, summary_files, time_created, time_updated
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            (
                session_id.as_str(),
                project_id.as_str(),
                slug,
                project_dir,
                title,
                version,
                file_count,
                created_ms,
                updated_ms,
            ),
        )
        .with_context(|| "Failed to create OpenCode session row")?;

        let mut previous_message_id: Option<String> = None;
        for message in &conv.messages {
            let message_id = Self::compact_id("msg");
            let timestamp_ms = message.timestamp.timestamp_millis();
            let role = match message.role {
                Role::Assistant => "assistant",
                Role::System => "system",
                Role::User => "user",
            };
            let mut data = json!({
                "role": role,
                "time": { "created": timestamp_ms },
                "path": { "cwd": project_dir, "root": project_dir },
                "source": "chatmem"
            });
            if message.role == Role::Assistant {
                data["time"]["completed"] = json!(timestamp_ms);
                data["providerID"] = json!("chatmem");
                data["modelID"] = json!("imported");
                data["agent"] = json!("build");
            }
            if let Some(parent_id) = previous_message_id.as_deref() {
                data["parentID"] = json!(parent_id);
            }

            tx.execute(
                "INSERT INTO message (id, session_id, time_created, time_updated, data)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                (
                    message_id.as_str(),
                    session_id.as_str(),
                    timestamp_ms,
                    timestamp_ms,
                    data.to_string(),
                ),
            )?;

            if !message.content.trim().is_empty() {
                Self::insert_part(
                    &tx,
                    &message_id,
                    &session_id,
                    timestamp_ms,
                    json!({ "type": "text", "text": message.content.as_str() }),
                )?;
            }

            if let Some(reasoning) = message.metadata.get("reasoning") {
                let reasoning_text = reasoning
                    .as_str()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| reasoning.to_string());
                if !reasoning_text.trim().is_empty() {
                    Self::insert_part(
                        &tx,
                        &message_id,
                        &session_id,
                        timestamp_ms,
                        json!({ "type": "reasoning", "text": reasoning_text }),
                    )?;
                }
            }

            for tool_call in &message.tool_calls {
                let status = match tool_call.status {
                    ToolStatus::Error => "error",
                    ToolStatus::Success => "completed",
                };
                Self::insert_part(
                    &tx,
                    &message_id,
                    &session_id,
                    timestamp_ms,
                    json!({
                        "type": "tool",
                        "callID": Self::compact_id("call"),
                        "tool": tool_call.name.as_str(),
                        "state": {
                            "status": status,
                            "input": tool_call.input.clone(),
                            "output": tool_call.output.as_deref(),
                            "metadata": {},
                            "time": { "start": timestamp_ms, "end": timestamp_ms }
                        }
                    }),
                )?;
            }

            let changed_files = conv
                .file_changes
                .iter()
                .filter(|change| change.message_id == message.id)
                .map(|change| change.path.as_str())
                .collect::<Vec<_>>();
            if !changed_files.is_empty() {
                Self::insert_part(
                    &tx,
                    &message_id,
                    &session_id,
                    timestamp_ms,
                    json!({
                        "type": "patch",
                        "hash": Self::compact_id("patch"),
                        "files": changed_files
                    }),
                )?;
            }

            previous_message_id = Some(message_id);
        }

        if conv.messages.is_empty() {
            let timestamp_ms = created_ms;
            let message_id = Self::compact_id("msg");
            tx.execute(
                "INSERT INTO message (id, session_id, time_created, time_updated, data)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                (
                    message_id.as_str(),
                    session_id.as_str(),
                    timestamp_ms,
                    timestamp_ms,
                    json!({
                        "role": "user",
                        "time": { "created": timestamp_ms },
                        "path": { "cwd": project_dir, "root": project_dir },
                        "source": "chatmem"
                    })
                    .to_string(),
                ),
            )?;
        }

        tx.commit()?;
        Ok(session_id)
    }

    fn delete_conversation(&self, id: &str) -> Result<()> {
        let conn = self.open_db_rw()?;
        let changed = conn.execute(
            "UPDATE session SET time_archived = ?1, time_updated = ?1 WHERE id = ?2",
            (Utc::now().timestamp_millis(), id),
        )?;
        if changed == 0 {
            return Err(anyhow!("OpenCode session not found: {id}"));
        }
        Ok(())
    }

    fn render_prompt(&self, conv: &Conversation) -> Result<String> {
        let mut rendered = String::new();
        rendered.push_str(&format!(
            "# Conversation: {}\n\n",
            conv.summary.as_deref().unwrap_or(&conv.id)
        ));
        rendered.push_str("**Source:** OpenCode\n\n");
        rendered.push_str(&format!("**Project:** `{}`\n\n", conv.project_dir));

        for message in &conv.messages {
            let role = match message.role {
                Role::User => "User",
                Role::Assistant => "Assistant",
                Role::System => "System",
            };
            rendered.push_str(&format!("## {role}\n\n{}\n\n", message.content));
            for tool in &message.tool_calls {
                rendered.push_str(&format!("**Tool: {}**\n", tool.name));
                rendered.push_str(&format!("Input: {}\n", tool.input));
                if let Some(output) = &tool.output {
                    rendered.push_str(&format!("Output: {output}\n"));
                }
                rendered.push('\n');
            }
        }

        if !conv.file_changes.is_empty() {
            rendered.push_str("## Files Changed\n\n");
            for change in &conv.file_changes {
                rendered.push_str(&format!("`{}` ({:?})\n", change.path, change.change_type));
            }
        }

        Ok(rendered)
    }

    fn agent_kind(&self) -> AgentKind {
        AgentKind::OpenCode
    }

    fn display_name(&self) -> &str {
        "OpenCode"
    }

    fn data_dir(&self) -> PathBuf {
        self.data_dir.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agentswap_core::adapter::AgentAdapter;
    use agentswap_core::types::{
        AgentKind, ChangeType, Conversation, Message, Role, ToolCall, ToolStatus,
    };
    use rusqlite::Connection;
    use serde_json::json;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn create_opencode_db(dir: &std::path::Path) -> std::path::PathBuf {
        let db_path = dir.join("opencode.db");
        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE project (
                id TEXT PRIMARY KEY,
                worktree TEXT NOT NULL,
                vcs TEXT,
                name TEXT,
                icon_url TEXT,
                icon_color TEXT,
                time_created INTEGER NOT NULL,
                time_updated INTEGER NOT NULL,
                time_initialized INTEGER,
                sandboxes TEXT NOT NULL
            );

            CREATE TABLE session (
                id TEXT PRIMARY KEY,
                project_id TEXT NOT NULL,
                workspace_id TEXT,
                parent_id TEXT,
                slug TEXT NOT NULL,
                directory TEXT NOT NULL,
                title TEXT NOT NULL,
                version TEXT NOT NULL,
                share_url TEXT,
                summary_additions INTEGER,
                summary_deletions INTEGER,
                summary_files INTEGER,
                summary_diffs TEXT,
                revert TEXT,
                permission TEXT,
                time_created INTEGER NOT NULL,
                time_updated INTEGER NOT NULL,
                time_compacting INTEGER,
                time_archived INTEGER
            );

            CREATE TABLE message (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                time_created INTEGER NOT NULL,
                time_updated INTEGER NOT NULL,
                data TEXT NOT NULL
            );

            CREATE TABLE part (
                id TEXT PRIMARY KEY,
                message_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                time_created INTEGER NOT NULL,
                time_updated INTEGER NOT NULL,
                data TEXT NOT NULL
            );
            "#,
        )
        .unwrap();

        conn.execute(
            "INSERT INTO project (id, worktree, vcs, name, time_created, time_updated, sandboxes)
             VALUES (?1, ?2, 'git', 'ChatMem', ?3, ?4, '[]')",
            ("project-001", "D:/VSP", 1_776_000_000_000i64, 1_776_000_100_000i64),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO session (
                id, project_id, slug, directory, title, version, summary_files, time_created, time_updated
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            (
                "ses_001",
                "project-001",
                "improve-chatmem",
                "D:/VSP",
                "Improve ChatMem memory",
                "0.13.0",
                1i64,
                1_776_000_000_000i64,
                1_776_000_200_000i64,
            ),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, time_updated, data)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                "msg_user",
                "ses_001",
                1_776_000_010_000i64,
                1_776_000_010_000i64,
                json!({
                    "role": "user",
                    "time": { "created": 1_776_000_010_000i64 },
                    "model": { "providerID": "openai", "modelID": "gpt-5" }
                })
                .to_string(),
            ),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                "part_user_text",
                "msg_user",
                "ses_001",
                1_776_000_010_000i64,
                1_776_000_010_000i64,
                json!({ "type": "text", "text": "请支持 OpenCode 对话" }).to_string(),
            ),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO message (id, session_id, time_created, time_updated, data)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            (
                "msg_assistant",
                "ses_001",
                1_776_000_020_000i64,
                1_776_000_030_000i64,
                json!({
                    "role": "assistant",
                    "time": { "created": 1_776_000_020_000i64, "completed": 1_776_000_030_000i64 },
                    "parentID": "msg_user",
                    "modelID": "gpt-5",
                    "providerID": "openai",
                    "mode": "",
                    "agent": "build",
                    "path": { "cwd": "D:/VSP", "root": "D:/VSP" }
                })
                .to_string(),
            ),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                "part_assistant_text",
                "msg_assistant",
                "ses_001",
                1_776_000_020_000i64,
                1_776_000_020_000i64,
                json!({ "type": "text", "text": "我会读取 opencode.db。" }).to_string(),
            ),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                "part_tool",
                "msg_assistant",
                "ses_001",
                1_776_000_021_000i64,
                1_776_000_022_000i64,
                json!({
                    "type": "tool",
                    "callID": "call_001",
                    "tool": "bash",
                    "state": {
                        "status": "completed",
                        "input": { "command": "ls" },
                        "output": "adapter.rs",
                        "title": "ls",
                        "metadata": {},
                        "time": { "start": 1_776_000_021_000i64, "end": 1_776_000_022_000i64 }
                    }
                })
                .to_string(),
            ),
        )
        .unwrap();
        conn.execute(
            "INSERT INTO part (id, message_id, session_id, time_created, time_updated, data)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            (
                "part_patch",
                "msg_assistant",
                "ses_001",
                1_776_000_023_000i64,
                1_776_000_023_000i64,
                json!({
                    "type": "patch",
                    "hash": "abc123",
                    "files": ["src/App.tsx"]
                })
                .to_string(),
            ),
        )
        .unwrap();

        db_path
    }

    #[test]
    fn lists_open_code_sessions_from_sqlite() {
        let tmp = TempDir::new().unwrap();
        create_opencode_db(tmp.path());
        let adapter = OpenCodeAdapter::with_data_dir(tmp.path().to_path_buf());

        let conversations = adapter.list_conversations().unwrap();

        assert_eq!(conversations.len(), 1);
        assert_eq!(conversations[0].id, "ses_001");
        assert_eq!(conversations[0].source_agent, AgentKind::OpenCode);
        assert_eq!(conversations[0].project_dir, "D:/VSP");
        assert_eq!(conversations[0].summary.as_deref(), Some("Improve ChatMem memory"));
        assert_eq!(conversations[0].message_count, 2);
        assert_eq!(conversations[0].file_count, 1);
    }

    #[test]
    fn restores_archived_open_code_session() {
        let tmp = TempDir::new().unwrap();
        create_opencode_db(tmp.path());
        let adapter = OpenCodeAdapter::with_data_dir(tmp.path().to_path_buf());

        adapter.delete_conversation("ses_001").unwrap();
        assert!(adapter.list_conversations().unwrap().is_empty());

        adapter.restore_conversation("ses_001").unwrap();
        let conversations = adapter.list_conversations().unwrap();

        assert_eq!(conversations.len(), 1);
        assert_eq!(conversations[0].id, "ses_001");
    }

    #[test]
    fn discovers_channel_specific_open_code_database() {
        let tmp = TempDir::new().unwrap();
        let db_path = create_opencode_db(tmp.path());
        std::fs::rename(&db_path, tmp.path().join("opencode-dev.db")).unwrap();
        let adapter = OpenCodeAdapter::with_data_dir(tmp.path().to_path_buf());

        let conversations = adapter.list_conversations().unwrap();

        assert_eq!(conversations.len(), 1);
        assert_eq!(conversations[0].id, "ses_001");
        assert_eq!(adapter.db_path(), tmp.path().join("opencode-dev.db"));
    }

    #[test]
    fn reads_open_code_messages_tools_and_patch_parts() {
        let tmp = TempDir::new().unwrap();
        create_opencode_db(tmp.path());
        let adapter = OpenCodeAdapter::with_data_dir(tmp.path().to_path_buf());

        let conversation = adapter.read_conversation("ses_001").unwrap();

        assert_eq!(conversation.source_agent, AgentKind::OpenCode);
        assert_eq!(conversation.project_dir, "D:/VSP");
        assert_eq!(conversation.messages.len(), 2);
        assert_eq!(conversation.messages[0].role, Role::User);
        assert_eq!(conversation.messages[0].content, "请支持 OpenCode 对话");
        assert_eq!(conversation.messages[1].role, Role::Assistant);
        assert!(conversation.messages[1].content.contains("opencode.db"));
        assert_eq!(conversation.messages[1].tool_calls.len(), 1);
        assert_eq!(conversation.messages[1].tool_calls[0].name, "bash");
        assert_eq!(conversation.messages[1].tool_calls[0].input["command"], "ls");
        assert_eq!(conversation.messages[1].tool_calls[0].output.as_deref(), Some("adapter.rs"));
        assert_eq!(conversation.messages[1].tool_calls[0].status, ToolStatus::Success);
        assert_eq!(conversation.file_changes.len(), 1);
        assert_eq!(conversation.file_changes[0].path, "src/App.tsx");
        assert_eq!(conversation.file_changes[0].change_type, ChangeType::Modified);
    }

    #[test]
    fn writes_open_code_conversation_and_reads_it_back() {
        let tmp = TempDir::new().unwrap();
        create_opencode_db(tmp.path());
        let adapter = OpenCodeAdapter::with_data_dir(tmp.path().to_path_buf());
        let now = Utc::now();
        let user_message_id = Uuid::new_v4();

        let conv = Conversation {
            id: "source-001".to_string(),
            source_agent: AgentKind::Claude,
            project_dir: "D:/VSP".to_string(),
            created_at: now,
            updated_at: now,
            summary: Some("迁移到 OpenCode".to_string()),
            messages: vec![
                Message {
                    id: user_message_id,
                    timestamp: now,
                    role: Role::User,
                    content: "请继续这个任务".to_string(),
                    tool_calls: Vec::new(),
                    metadata: HashMap::new(),
                },
                Message {
                    id: Uuid::new_v4(),
                    timestamp: now,
                    role: Role::Assistant,
                    content: "我会继续。".to_string(),
                    tool_calls: vec![ToolCall {
                        name: "bash".to_string(),
                        input: json!({ "command": "pwd" }),
                        output: Some("D:/VSP".to_string()),
                        status: ToolStatus::Success,
                    }],
                    metadata: HashMap::new(),
                },
            ],
            file_changes: vec![FileChange {
                path: "src/App.tsx".to_string(),
                change_type: ChangeType::Modified,
                timestamp: now,
                message_id: user_message_id,
            }],
        };

        let new_id = adapter.write_conversation(&conv).unwrap();
        let read_back = adapter.read_conversation(&new_id).unwrap();

        assert_eq!(read_back.source_agent, AgentKind::OpenCode);
        assert_eq!(read_back.project_dir, "D:/VSP");
        assert_eq!(read_back.summary.as_deref(), Some("迁移到 OpenCode"));
        assert_eq!(read_back.messages.len(), 2);
        assert_eq!(read_back.messages[0].role, Role::User);
        assert_eq!(read_back.messages[0].content, "请继续这个任务");
        assert_eq!(read_back.messages[1].role, Role::Assistant);
        assert!(read_back.messages[1].content.contains("继续"));
        assert_eq!(read_back.messages[1].tool_calls.len(), 1);
        assert_eq!(read_back.messages[1].tool_calls[0].name, "bash");
        assert_eq!(read_back.messages[1].tool_calls[0].input["command"], "pwd");
        assert_eq!(read_back.file_changes.len(), 1);
        assert_eq!(read_back.file_changes[0].path, "src/App.tsx");
    }
}
