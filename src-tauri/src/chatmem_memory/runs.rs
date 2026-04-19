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
    let Some(repo_root) = normalized_repo_root(&conn, repo_root)? else {
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

pub fn list_artifacts(repo_root: &str) -> Result<Vec<ArtifactRecord>> {
    let conn = db::open_app_database()?;
    let Some(repo_root) = normalized_repo_root(&conn, repo_root)? else {
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
