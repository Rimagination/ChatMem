use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::Connection;
use serde_json::{json, Value};
use uuid::Uuid;

use agentswap_core::adapter::AgentAdapter;
use agentswap_core::types::*;

/// Adapter for reading Hermes Agent conversations from its SQLite database.
pub struct HermesAdapter {
    db_path: PathBuf,
}

impl Default for HermesAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl HermesAdapter {
    pub fn new() -> Self {
        let base = dirs::data_local_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("/"));
        // Windows: AppData/Local/hermes/state.db
        // macOS/Linux: ~/.hermes/state.db  (data_local_dir returns ~/.local/share on Linux)
        // Prefer AppData/Local/hermes if it exists, otherwise fall back to ~/.hermes
        let appdata_path = base.join("hermes").join("state.db");
        let home_path = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("/"))
            .join(".hermes")
            .join("state.db");
        Self {
            db_path: if appdata_path.exists() || !home_path.exists() {
                appdata_path
            } else {
                home_path
            },
        }
    }

    fn open_db(&self) -> Result<Connection> {
        Connection::open_with_flags(&self.db_path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .with_context(|| format!("Cannot open Hermes DB: {}", self.db_path.display()))
    }

    fn parse_timestamp(ts: f64) -> DateTime<Utc> {
        Utc.timestamp_opt(ts as i64, ((ts.fract()) * 1_000_000_000.0) as u32)
            .single()
            .unwrap_or_else(Utc::now)
    }
}

impl AgentAdapter for HermesAdapter {
    fn is_available(&self) -> bool {
        self.db_path.exists()
    }

    fn list_conversations(&self) -> Result<Vec<ConversationSummary>> {
        let conn = self.open_db()?;
        let mut stmt = conn.prepare(
            "SELECT id, title, started_at, ended_at, message_count, cwd, archived
             FROM sessions WHERE archived = 0 ORDER BY started_at DESC",
        )?;

        let summaries = stmt
            .query_map([], |row| {
                Ok(ConversationSummary {
                    id: row.get(0)?,
                    source_agent: AgentKind::Hermes,
                    project_dir: row.get::<_, Option<String>>(5)?.unwrap_or_default(),
                    created_at: HermesAdapter::parse_timestamp(row.get(2)?),
                    updated_at: {
                        let started = HermesAdapter::parse_timestamp(row.get(2)?);
                        row.get::<_, Option<f64>>(3)?.map(HermesAdapter::parse_timestamp).unwrap_or(started)
                    },
                    summary: row.get(1)?,
                    message_count: row.get::<_, usize>(4).unwrap_or(0),
                    file_count: 0,
                })
            })?
            .filter_map(|r| r.ok())
            .collect();

        Ok(summaries)
    }

    fn read_conversation(&self, id: &str) -> Result<Conversation> {
        let conn = self.open_db()?;

        let (title, started_at, ended_at, cwd): (Option<String>, f64, Option<f64>, Option<String>) =
            conn.query_row(
                "SELECT title, started_at, ended_at, cwd FROM sessions WHERE id = ?1",
                [id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .with_context(|| format!("Hermes session not found: {id}"))?;

        let mut stmt = conn.prepare(
            "SELECT id, role, content, tool_calls, tool_name, timestamp
             FROM messages WHERE session_id = ?1 AND active = 1 ORDER BY timestamp ASC",
        )?;

        let mut messages: Vec<Message> = Vec::new();

        let rows: Vec<_> = stmt
            .query_map([id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, f64>(5)?,
                ))
            })?
            .filter_map(|r| r.ok())
            .collect();

        for (msg_id, role, content, tool_calls_json, tool_name, timestamp) in rows {
            let parsed_role = match role.as_str() {
                "user" => Role::User,
                "assistant" => Role::Assistant,
                "system" => Role::System,
                "tool" => {
                    // Tool result: attach output to the matching tool_call in the previous assistant message
                    if let (Some(ref tn), Some(ref tc_content)) = (&tool_name, &content) {
                        if let Some(last_msg) = messages.last_mut() {
                            if last_msg.role == Role::Assistant {
                                if let Some(tc) = last_msg.tool_calls.iter_mut().rev().find(|tc| tc.output.is_none() && tc.name == *tn) {
                                    tc.output = Some(tc_content.clone());
                                }
                            }
                        }
                    }
                    continue;
                },
                _ => Role::Assistant,
            };

            // Parse tool_calls JSON array (OpenAI format)
            let mut tool_calls = Vec::new();
            if let Some(ref tc_json) = tool_calls_json {
                if let Ok(Value::Array(tc_array)) = serde_json::from_str::<Value>(tc_json) {
                    for tc in &tc_array {
                        // OpenAI format: { "type": "function", "function": { "name": "...", "arguments": "..." } }
                        let func = tc.get("function");
                        let name = func
                            .and_then(|f| f.get("name"))
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string();
                        let input = func
                            .and_then(|f| f.get("arguments"))
                            .and_then(|v| v.as_str())
                            .and_then(|s| serde_json::from_str::<Value>(s).ok())
                            .unwrap_or(json!({}));
                        tool_calls.push(ToolCall { name, input, output: None, status: ToolStatus::Success });
                    }
                }
            }

            let msg_content = content.unwrap_or_default();
            if parsed_role == Role::Assistant && msg_content.is_empty() && tool_calls.is_empty() {
                continue;
            }

            messages.push(Message {
                id: Uuid::from_u128(msg_id as u128),
                timestamp: Self::parse_timestamp(timestamp),
                role: parsed_role,
                content: msg_content,
                tool_calls,
                metadata: HashMap::new(),
            });
        }

        Ok(Conversation {
            id: id.to_string(),
            source_agent: AgentKind::Hermes,
            project_dir: cwd.unwrap_or_default(),
            created_at: Self::parse_timestamp(started_at),
            updated_at: ended_at
                .map(Self::parse_timestamp)
                .unwrap_or_else(|| Self::parse_timestamp(started_at)),
            summary: title,
            messages,
            file_changes: Vec::new(),
        })
    }

    fn write_conversation(&self, _conv: &Conversation) -> Result<String> {
        anyhow::bail!("Hermes adapter is read-only")
    }

    fn delete_conversation(&self, _id: &str) -> Result<()> {
        anyhow::bail!("Hermes adapter is read-only")
    }

    fn render_prompt(&self, conv: &Conversation) -> Result<String> {
        let mut output = format!(
            "# Hermes Agent Conversation: {}\n\nProject: {}\nStarted: {}\n\n",
            conv.summary.as_deref().unwrap_or("Untitled"),
            conv.project_dir,
            conv.created_at,
        );
        for msg in &conv.messages {
            let label = match msg.role {
                Role::User => "User",
                Role::Assistant => "Assistant",
                Role::System => "System",
            };
            output.push_str(&format!("## {label}\n\n{}\n\n", msg.content));
            for tc in &msg.tool_calls {
                output.push_str(&format!("### Tool: {}\nInput: `{}`\n", tc.name, tc.input));
                if let Some(ref out) = tc.output {
                    if !out.is_empty() {
                        let preview = if out.len() > 500 { format!("{}...", &out[..500]) } else { out.clone() };
                        output.push_str(&format!("Output: `{preview}`\n"));
                    }
                }
                output.push('\n');
            }
        }
        Ok(output)
    }

    fn agent_kind(&self) -> AgentKind {
        AgentKind::Hermes
    }

    fn display_name(&self) -> &str {
        "Hermes Agent"
    }

    fn data_dir(&self) -> PathBuf {
        let base = dirs::data_local_dir()
            .or_else(dirs::home_dir)
            .unwrap_or_else(|| PathBuf::from("/"));
        let appdata_dir = base.join("hermes");
        if appdata_dir.exists() {
            return appdata_dir;
        }
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
        home.join(".hermes")
    }
}
