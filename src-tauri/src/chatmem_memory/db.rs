use anyhow::{Context, Result};
use rusqlite::Connection;
use std::path::{Path, PathBuf};

pub fn default_db_path() -> Result<PathBuf> {
    let base = dirs::data_local_dir()
        .or_else(dirs::home_dir)
        .context("unable to resolve a local data directory for ChatMem")?;
    Ok(base.join("ChatMem").join("chatmem.db"))
}

pub fn open_app_database() -> Result<Connection> {
    let path = default_db_path()?;
    open_connection(&path)
}

pub fn open_connection(path: &Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let conn = Connection::open(path)?;
    migrate(&conn)?;
    Ok(conn)
}

pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        PRAGMA foreign_keys = OFF;

        CREATE TABLE IF NOT EXISTS repos (
            repo_id TEXT PRIMARY KEY,
            repo_root TEXT NOT NULL UNIQUE,
            repo_fingerprint TEXT NOT NULL,
            git_remote TEXT,
            default_branch TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS conversations (
            conversation_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            source_agent TEXT NOT NULL,
            source_conversation_id TEXT NOT NULL,
            summary TEXT,
            started_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            storage_path TEXT
        );

        CREATE TABLE IF NOT EXISTS messages (
            message_id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            timestamp TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS tool_calls (
            tool_call_id TEXT PRIMARY KEY,
            message_id TEXT NOT NULL,
            name TEXT NOT NULL,
            input_json TEXT NOT NULL,
            output_text TEXT,
            status TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS file_changes (
            file_change_id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            message_id TEXT NOT NULL,
            path TEXT NOT NULL,
            change_type TEXT NOT NULL,
            timestamp TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS episodes (
            episode_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            outcome TEXT NOT NULL,
            created_at TEXT NOT NULL,
            source_conversation_id TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS memory_candidates (
            candidate_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            summary TEXT NOT NULL,
            value TEXT NOT NULL,
            why_it_matters TEXT NOT NULL,
            confidence REAL NOT NULL,
            proposed_by TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL,
            reviewed_at TEXT
        );

        CREATE TABLE IF NOT EXISTS approved_memories (
            memory_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            title TEXT NOT NULL,
            value TEXT NOT NULL,
            usage_hint TEXT NOT NULL,
            status TEXT NOT NULL,
            last_verified_at TEXT,
            created_from_candidate_id TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS handoff_packets (
            handoff_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            from_agent TEXT NOT NULL,
            to_agent TEXT NOT NULL,
            current_goal TEXT NOT NULL,
            done_json TEXT NOT NULL,
            next_json TEXT NOT NULL,
            key_files_json TEXT NOT NULL,
            commands_json TEXT NOT NULL,
            related_memories_json TEXT NOT NULL DEFAULT '[]',
            related_episodes_json TEXT NOT NULL DEFAULT '[]',
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS evidence_refs (
            evidence_id TEXT PRIMARY KEY,
            owner_type TEXT NOT NULL,
            owner_id TEXT NOT NULL,
            conversation_id TEXT,
            message_id TEXT,
            tool_call_id TEXT,
            file_change_id TEXT,
            excerpt TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS search_documents (
            doc_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            doc_type TEXT NOT NULL,
            doc_ref_id TEXT NOT NULL,
            title TEXT NOT NULL,
            body TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS search_documents_fts USING fts5(
            doc_id UNINDEXED,
            title,
            body
        );
        ",
    )?;

    ensure_column(
        conn,
        "approved_memories",
        "freshness_status",
        "TEXT NOT NULL DEFAULT 'unknown'",
    )?;
    ensure_column(
        conn,
        "approved_memories",
        "freshness_score",
        "REAL NOT NULL DEFAULT 0.0",
    )?;
    ensure_column(conn, "approved_memories", "verified_at", "TEXT")?;
    ensure_column(conn, "approved_memories", "verified_by", "TEXT")?;
    backfill_legacy_memory_verification_metadata(conn)?;

    ensure_column(
        conn,
        "handoff_packets",
        "status",
        "TEXT NOT NULL DEFAULT 'draft'",
    )?;
    ensure_column(conn, "handoff_packets", "target_profile", "TEXT")?;
    ensure_column(conn, "handoff_packets", "checkpoint_id", "TEXT")?;
    ensure_column(conn, "handoff_packets", "compression_strategy", "TEXT")?;
    ensure_column(conn, "handoff_packets", "consumed_at", "TEXT")?;
    ensure_column(conn, "handoff_packets", "consumed_by", "TEXT")?;

    Ok(())
}

fn backfill_legacy_memory_verification_metadata(conn: &Connection) -> Result<()> {
    conn.execute(
        "UPDATE approved_memories
         SET freshness_status = CASE
                 WHEN freshness_status = 'unknown' THEN 'fresh'
                 ELSE freshness_status
             END,
             freshness_score = CASE
                 WHEN freshness_score = 0.0 THEN 1.0
                 ELSE freshness_score
             END,
             verified_at = COALESCE(verified_at, last_verified_at),
             verified_by = COALESCE(verified_by, 'legacy_migration')
         WHERE last_verified_at IS NOT NULL",
        [],
    )?;

    Ok(())
}

fn ensure_column(conn: &Connection, table: &str, column: &str, definition: &str) -> Result<()> {
    if !table_has_column(conn, table, column)? {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {definition}"),
            [],
        )?;
    }

    Ok(())
}

fn table_has_column(conn: &Connection, table: &str, column: &str) -> Result<bool> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let columns = stmt.query_map([], |row| row.get::<_, String>(1))?;

    for existing in columns {
        if existing? == column {
            return Ok(true);
        }
    }

    Ok(false)
}

#[cfg(test)]
mod tests {
    use super::{migrate, open_connection};
    use rusqlite::Connection;

    fn column_names(conn: &Connection, table: &str) -> Vec<String> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({table})"))
            .unwrap();

        stmt.query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    #[test]
    fn migrations_create_memory_tables() {
        let path = std::env::temp_dir().join(format!("chatmem-db-test-{}.sqlite", uuid::Uuid::new_v4()));
        let conn = open_connection(&path).unwrap();
        migrate(&conn).unwrap();

        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master
                 WHERE type='table' AND name IN (
                   'repos',
                   'conversations',
                   'messages',
                   'tool_calls',
                   'file_changes',
                   'episodes',
                   'memory_candidates',
                   'approved_memories',
                   'handoff_packets',
                   'evidence_refs',
                   'search_documents'
                 )
                 ORDER BY name",
            )
            .unwrap();

        let names = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(
            names,
            vec![
                "approved_memories",
                "conversations",
                "episodes",
                "evidence_refs",
                "file_changes",
                "handoff_packets",
                "memory_candidates",
                "messages",
                "repos",
                "search_documents",
                "tool_calls",
            ]
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn migrations_add_handoff_lifecycle_columns() {
        let path = std::env::temp_dir().join(format!("chatmem-db-test-{}.sqlite", uuid::Uuid::new_v4()));
        let conn = open_connection(&path).unwrap();
        migrate(&conn).unwrap();

        let columns = column_names(&conn, "handoff_packets");
        assert!(columns.contains(&"status".to_string()));
        assert!(columns.contains(&"target_profile".to_string()));
        assert!(columns.contains(&"checkpoint_id".to_string()));
        assert!(columns.contains(&"consumed_at".to_string()));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn migrations_add_memory_freshness_columns() {
        let path = std::env::temp_dir().join(format!("chatmem-db-test-{}.sqlite", uuid::Uuid::new_v4()));
        let conn = open_connection(&path).unwrap();
        migrate(&conn).unwrap();

        let columns = column_names(&conn, "approved_memories");
        assert!(columns.contains(&"freshness_status".to_string()));
        assert!(columns.contains(&"freshness_score".to_string()));
        assert!(columns.contains(&"verified_at".to_string()));
        assert!(columns.contains(&"verified_by".to_string()));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn migrations_backfill_legacy_memory_verification_fields() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE approved_memories (
                memory_id TEXT PRIMARY KEY,
                repo_id TEXT NOT NULL,
                kind TEXT NOT NULL,
                title TEXT NOT NULL,
                value TEXT NOT NULL,
                usage_hint TEXT NOT NULL,
                status TEXT NOT NULL,
                last_verified_at TEXT,
                created_from_candidate_id TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            ",
        )
        .unwrap();

        conn.execute(
            "INSERT INTO approved_memories (
                memory_id, repo_id, kind, title, value, usage_hint, status,
                last_verified_at, created_from_candidate_id, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, NULL, ?9, ?9)",
            rusqlite::params![
                "mem-legacy",
                "repo-1",
                "command",
                "Legacy verification",
                "cargo test",
                "Use before shipping",
                "active",
                "2026-04-19T09:00:00Z",
                "2026-04-19T09:00:00Z",
            ],
        )
        .unwrap();

        migrate(&conn).unwrap();

        let row = conn
            .query_row(
                "SELECT freshness_status, freshness_score, verified_at, verified_by
                 FROM approved_memories
                 WHERE memory_id = ?1",
                ["mem-legacy"],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, f64>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .unwrap();

        assert_eq!(row.0, "fresh");
        assert_eq!(row.1, 1.0);
        assert_eq!(row.2.as_deref(), Some("2026-04-19T09:00:00Z"));
        assert_eq!(row.3.as_deref(), Some("legacy_migration"));
    }
}
