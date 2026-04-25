use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

use agentswap_core::adapter::AgentAdapter;
use agentswap_core::types::{
    AgentKind, ChangeType, Conversation, ConversationSummary, FileChange, Message, Role,
    ToolCall, ToolStatus,
};
use anyhow::{anyhow, bail, Context, Result};
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

    fn write_conversation(&self, _conv: &Conversation) -> Result<String> {
        bail!("Writing conversations into OpenCode is not supported yet; use opencode import/export")
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
    use agentswap_core::types::{AgentKind, ChangeType, Role, ToolStatus};
    use rusqlite::Connection;
    use serde_json::json;
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
}
