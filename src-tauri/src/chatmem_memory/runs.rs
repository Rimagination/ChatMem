use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{db, repo_identity};

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RunRecord {
    pub run_id: String,
    pub repo_root: String,
    pub source_agent: String,
    pub task_hint: Option<String>,
    pub status: String,
    pub summary: String,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub artifact_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactRecord {
    pub artifact_id: String,
    pub run_id: String,
    pub artifact_type: String,
    pub title: String,
    pub summary: String,
    pub trust_state: String,
    pub created_at: String,
}

pub fn list_runs(repo_root: &str) -> Result<Vec<RunRecord>> {
    let conn = db::open_app_database()?;
    seed_runs_from_repository_evidence(&conn, repo_root)?;
    list_runs_for_repo(&conn, repo_root)
}

pub fn list_artifacts(repo_root: &str) -> Result<Vec<ArtifactRecord>> {
    let conn = db::open_app_database()?;
    seed_runs_from_repository_evidence(&conn, repo_root)?;
    list_artifacts_for_repo(&conn, repo_root)
}

fn list_runs_for_repo(conn: &Connection, repo_root: &str) -> Result<Vec<RunRecord>> {
    let Some(repo_root) = normalized_repo_root(conn, repo_root)? else {
        return Ok(vec![]);
    };

    let mut stmt = conn.prepare(
        "SELECT agent_runs.run_id,
                repos.repo_root,
                agent_runs.source_agent,
                agent_runs.task_hint,
                agent_runs.status,
                agent_runs.summary,
                agent_runs.started_at,
                agent_runs.ended_at,
                COUNT(artifacts.artifact_id) AS artifact_count
         FROM agent_runs
         INNER JOIN repos ON repos.repo_id = agent_runs.repo_id
         LEFT JOIN artifacts ON artifacts.run_id = agent_runs.run_id
         WHERE repos.repo_root = ?1
         GROUP BY agent_runs.run_id,
                  repos.repo_root,
                  agent_runs.source_agent,
                  agent_runs.task_hint,
                  agent_runs.status,
                  agent_runs.summary,
                  agent_runs.started_at,
                  agent_runs.ended_at
         ORDER BY agent_runs.started_at DESC",
    )?;

    let rows = stmt.query_map([repo_root], |row| {
        Ok(RunRecord {
            run_id: row.get(0)?,
            repo_root: row.get(1)?,
            source_agent: row.get(2)?,
            task_hint: row.get(3)?,
            status: row.get(4)?,
            summary: row.get(5)?,
            started_at: row.get(6)?,
            ended_at: row.get(7)?,
            artifact_count: row.get(8)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn list_artifacts_for_repo(conn: &Connection, repo_root: &str) -> Result<Vec<ArtifactRecord>> {
    let Some(repo_root) = normalized_repo_root(conn, repo_root)? else {
        return Ok(vec![]);
    };

    let mut stmt = conn.prepare(
        "SELECT artifacts.artifact_id,
                artifacts.run_id,
                artifacts.artifact_type,
                artifacts.title,
                artifacts.summary,
                artifacts.trust_state,
                artifacts.created_at
         FROM artifacts
         INNER JOIN agent_runs ON agent_runs.run_id = artifacts.run_id
         INNER JOIN repos ON repos.repo_id = agent_runs.repo_id
         WHERE repos.repo_root = ?1
         ORDER BY artifacts.created_at DESC",
    )?;

    let rows = stmt.query_map([repo_root], |row| {
        Ok(ArtifactRecord {
            artifact_id: row.get(0)?,
            run_id: row.get(1)?,
            artifact_type: row.get(2)?,
            title: row.get(3)?,
            summary: row.get(4)?,
            trust_state: row.get(5)?,
            created_at: row.get(6)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn seed_runs_from_repository_evidence(conn: &Connection, repo_root: &str) -> Result<()> {
    let Some(repo_root) = normalized_repo_root(conn, repo_root)? else {
        return Ok(());
    };

    let mut stmt = conn.prepare(
        "SELECT conversations.conversation_id,
                conversations.repo_id,
                conversations.source_agent,
                conversations.summary,
                conversations.source_conversation_id,
                conversations.started_at,
                conversations.updated_at
         FROM conversations
         INNER JOIN repos ON repos.repo_id = conversations.repo_id
         WHERE repos.repo_root = ?1
         ORDER BY conversations.started_at DESC",
    )?;

    let conversations = stmt
        .query_map([repo_root], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        })?
        .collect::<Result<Vec<_>, _>>()?;

    for (
        conversation_id,
        repo_id,
        source_agent,
        summary,
        source_conversation_id,
        started_at,
        updated_at,
    ) in conversations
    {
        let run_id = format!("run:{conversation_id}");
        let file_paths = file_change_paths(conn, &conversation_id)?;
        let tool_names = tool_call_names(conn, &conversation_id)?;
        let has_failed_tool = has_failed_tool_call(conn, &conversation_id)?;
        let run_summary = derive_run_summary(
            conn,
            &conversation_id,
            summary.as_deref(),
            &source_conversation_id,
        )?;
        let task_hint = summary
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string);
        let status = if has_failed_tool {
            "failed"
        } else if !file_paths.is_empty() || !tool_names.is_empty() {
            "waiting_for_review"
        } else {
            "completed"
        };

        conn.execute(
            "INSERT INTO agent_runs (
                run_id, repo_id, source_agent, task_hint, status, summary, started_at, ended_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(run_id) DO UPDATE SET
                repo_id = excluded.repo_id,
                source_agent = excluded.source_agent,
                task_hint = excluded.task_hint,
                status = excluded.status,
                summary = excluded.summary,
                started_at = excluded.started_at,
                ended_at = excluded.ended_at",
            params![
                run_id,
                repo_id,
                source_agent,
                task_hint,
                status,
                run_summary,
                started_at,
                Some(updated_at.clone()),
            ],
        )?;

        conn.execute("DELETE FROM artifacts WHERE run_id = ?1", params![run_id])?;

        if !file_paths.is_empty() {
            conn.execute(
                "INSERT INTO artifacts (
                    artifact_id, run_id, artifact_type, title, summary, body, file_path, trust_state, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, ?6, ?7)",
                params![
                    format!("{run_id}:files"),
                    run_id,
                    "file_change_set",
                    "Repository file changes",
                    format_file_change_summary(&file_paths),
                    "pending_review",
                    updated_at,
                ],
            )?;
        }

        if !tool_names.is_empty() {
            conn.execute(
                "INSERT INTO artifacts (
                    artifact_id, run_id, artifact_type, title, summary, body, file_path, trust_state, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, NULL, NULL, ?6, ?7)",
                params![
                    format!("{run_id}:tools"),
                    run_id,
                    "tool_output_digest",
                    "Tool call outputs",
                    format_tool_summary(&tool_names),
                    "pending_review",
                    updated_at,
                ],
            )?;
        }
    }

    Ok(())
}

fn normalized_repo_root(conn: &Connection, repo_root: &str) -> Result<Option<String>> {
    let normalized = repo_identity::normalize_repo_root(repo_root);

    conn.query_row(
        "SELECT repo_root FROM repos WHERE repo_root = ?1",
        params![normalized],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(Into::into)
}

fn file_change_paths(conn: &Connection, conversation_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT path
         FROM file_changes
         WHERE conversation_id = ?1
         ORDER BY timestamp ASC, path ASC",
    )?;

    let rows = stmt
        .query_map([conversation_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into);
    rows
}

fn tool_call_names(conn: &Connection, conversation_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT DISTINCT tool_calls.name
         FROM tool_calls
         INNER JOIN messages ON messages.message_id = tool_calls.message_id
         WHERE messages.conversation_id = ?1
         ORDER BY tool_calls.name ASC",
    )?;

    let rows = stmt
        .query_map([conversation_id], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(Into::into);
    rows
}

fn has_failed_tool_call(conn: &Connection, conversation_id: &str) -> Result<bool> {
    conn.query_row(
        "SELECT EXISTS(
            SELECT 1
            FROM tool_calls
            INNER JOIN messages ON messages.message_id = tool_calls.message_id
            WHERE messages.conversation_id = ?1
              AND tool_calls.status = 'error'
         )",
        [conversation_id],
        |row| row.get::<_, i64>(0),
    )
    .map(|value| value != 0)
    .map_err(Into::into)
}

fn derive_run_summary(
    conn: &Connection,
    conversation_id: &str,
    summary: Option<&str>,
    source_conversation_id: &str,
) -> Result<String> {
    if let Some(summary) = summary.map(str::trim).filter(|value| !value.is_empty()) {
        return Ok(summary.to_string());
    }

    let message_excerpt = conn
        .query_row(
            "SELECT content
             FROM messages
             WHERE conversation_id = ?1
               AND TRIM(content) <> ''
             ORDER BY timestamp DESC
             LIMIT 1",
            [conversation_id],
            |row| row.get::<_, String>(0),
        )
        .optional()?;

    Ok(message_excerpt
        .as_deref()
        .map(|content| truncate_text(content, 160))
        .unwrap_or_else(|| source_conversation_id.to_string()))
}

fn format_file_change_summary(file_paths: &[String]) -> String {
    if file_paths.len() == 1 {
        return format!("1 file changed: {}", file_paths[0]);
    }

    format!(
        "{} files changed: {}",
        file_paths.len(),
        file_paths
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn format_tool_summary(tool_names: &[String]) -> String {
    if tool_names.len() == 1 {
        return format!("1 tool call recorded: {}", tool_names[0]);
    }

    format!(
        "{} tool calls recorded: {}",
        tool_names.len(),
        tool_names
            .iter()
            .take(3)
            .cloned()
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::{list_artifacts_for_repo, list_runs_for_repo, seed_runs_from_repository_evidence};
    use crate::chatmem_memory::{db, repo_identity};
    use rusqlite::{params, Connection};

    fn open_test_connection() -> Connection {
        let path =
            std::env::temp_dir().join(format!("chatmem-runs-test-{}.sqlite", uuid::Uuid::new_v4()));
        db::open_connection(&path).unwrap()
    }

    #[test]
    fn seeding_runs_from_repository_conversation_evidence_creates_visible_timeline_records() {
        let conn = open_test_connection();
        let repo_root = "D:/VSP/agentswap-gui";
        let normalized_repo_root = repo_identity::normalize_repo_root(repo_root);
        let repo_id = repo_identity::fingerprint_repo(&normalized_repo_root, None, None);

        conn.execute(
            "INSERT INTO repos (
                repo_id, repo_root, repo_fingerprint, git_remote, default_branch, created_at, updated_at
             ) VALUES (?1, ?2, ?3, NULL, NULL, ?4, ?4)",
            params![
                repo_id,
                normalized_repo_root,
                repo_id,
                "2026-04-20T10:00:00Z",
            ],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO conversations (
                conversation_id, repo_id, source_agent, source_conversation_id, summary,
                started_at, updated_at, storage_path
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                "codex:conv-001",
                repo_id,
                "codex",
                "conv-001",
                "Build the runs panel",
                "2026-04-20T10:00:00Z",
                "2026-04-20T10:30:00Z",
                "C:/Users/demo/.codex/sessions/conv-001.jsonl",
            ],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO messages (message_id, conversation_id, role, content, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                "codex:conv-001:msg-001",
                "codex:conv-001",
                "assistant",
                "Implemented the runs timeline and need review.",
                "2026-04-20T10:05:00Z",
            ],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO tool_calls (tool_call_id, message_id, name, input_json, output_text, status)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "codex:conv-001:msg-001:tool:0",
                "codex:conv-001:msg-001",
                "apply_patch",
                "{\"file\":\"src/App.tsx\"}",
                "Patched the runs timeline view",
                "success",
            ],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO file_changes (file_change_id, conversation_id, message_id, path, change_type, timestamp)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "codex:conv-001:change:0",
                "codex:conv-001",
                "codex:conv-001:msg-001",
                "src/components/RunsPanel.tsx",
                "created",
                "2026-04-20T10:20:00Z",
            ],
        )
        .unwrap();

        seed_runs_from_repository_evidence(&conn, repo_root).unwrap();

        let runs = list_runs_for_repo(&conn, repo_root).unwrap();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].source_agent, "codex");
        assert_eq!(runs[0].status, "waiting_for_review");
        assert_eq!(runs[0].artifact_count, 2);

        let artifacts = list_artifacts_for_repo(&conn, repo_root).unwrap();
        assert_eq!(artifacts.len(), 2);
        assert!(artifacts
            .iter()
            .any(|artifact| artifact.artifact_type == "file_change_set"));
        assert!(artifacts
            .iter()
            .any(|artifact| artifact.artifact_type == "tool_output_digest"));
        assert!(artifacts
            .iter()
            .all(|artifact| artifact.trust_state == "pending_review"));
    }
}
