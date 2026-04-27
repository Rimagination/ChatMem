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

        CREATE TABLE IF NOT EXISTS repo_aliases (
            alias_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            alias_root TEXT NOT NULL,
            alias_kind TEXT NOT NULL,
            confidence REAL NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(repo_id, alias_root)
        );

        CREATE TABLE IF NOT EXISTS conversation_repo_links (
            link_id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            repo_id TEXT NOT NULL,
            confidence REAL NOT NULL,
            reason TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(conversation_id, repo_id)
        );

        CREATE TABLE IF NOT EXISTS repo_scan_runs (
            scan_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            requested_repo_root TEXT NOT NULL,
            canonical_repo_root TEXT NOT NULL,
            scanned_conversation_count INTEGER NOT NULL,
            linked_conversation_count INTEGER NOT NULL,
            skipped_conversation_count INTEGER NOT NULL,
            source_agents_json TEXT NOT NULL DEFAULT '[]',
            unmatched_project_roots_json TEXT NOT NULL DEFAULT '[]',
            warnings_json TEXT NOT NULL DEFAULT '[]',
            scanned_at TEXT NOT NULL
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

        CREATE TABLE IF NOT EXISTS conversation_chunks (
            chunk_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            conversation_id TEXT NOT NULL,
            chunk_type TEXT NOT NULL,
            title TEXT NOT NULL,
            body TEXT NOT NULL,
            message_ids_json TEXT NOT NULL DEFAULT '[]',
            ordinal INTEGER NOT NULL,
            token_estimate INTEGER NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
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

        CREATE TABLE IF NOT EXISTS checkpoints (
            checkpoint_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            conversation_id TEXT NOT NULL,
            source_agent TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'active',
            summary TEXT NOT NULL,
            resume_command TEXT,
            metadata_json TEXT NOT NULL DEFAULT '{}',
            handoff_id TEXT,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS agent_runs (
            run_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            source_agent TEXT NOT NULL,
            task_hint TEXT,
            status TEXT NOT NULL,
            summary TEXT NOT NULL,
            started_at TEXT NOT NULL,
            ended_at TEXT
        );

        CREATE TABLE IF NOT EXISTS run_events (
            event_id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            detail_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS artifacts (
            artifact_id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            artifact_type TEXT NOT NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            body TEXT,
            file_path TEXT,
            trust_state TEXT NOT NULL,
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

        CREATE TABLE IF NOT EXISTS memory_conflicts (
            conflict_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            candidate_id TEXT NOT NULL,
            memory_id TEXT NOT NULL,
            reason TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL,
            resolved_at TEXT,
            UNIQUE(candidate_id, memory_id)
        );

        CREATE TABLE IF NOT EXISTS memory_merge_proposals (
            proposal_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            candidate_id TEXT NOT NULL,
            target_memory_id TEXT NOT NULL,
            proposed_title TEXT NOT NULL,
            proposed_value TEXT NOT NULL,
            proposed_usage_hint TEXT NOT NULL,
            risk_note TEXT,
            proposed_by TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(candidate_id, target_memory_id)
        );

        CREATE TABLE IF NOT EXISTS memory_entities (
            entity_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            name TEXT NOT NULL,
            normalized_name TEXT NOT NULL,
            kind TEXT NOT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(repo_id, normalized_name)
        );

        CREATE TABLE IF NOT EXISTS memory_entity_links (
            link_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            owner_type TEXT NOT NULL,
            owner_id TEXT NOT NULL,
            relationship TEXT NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE(repo_id, entity_id, owner_type, owner_id, relationship)
        );

        CREATE TABLE IF NOT EXISTS wiki_pages (
            page_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            slug TEXT NOT NULL,
            title TEXT NOT NULL,
            body TEXT NOT NULL,
            status TEXT NOT NULL,
            source_memory_ids_json TEXT NOT NULL DEFAULT '[]',
            source_episode_ids_json TEXT NOT NULL DEFAULT '[]',
            last_built_at TEXT NOT NULL,
            last_verified_at TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            UNIQUE(repo_id, slug)
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

        CREATE TABLE IF NOT EXISTS document_embeddings (
            doc_id TEXT NOT NULL,
            repo_id TEXT NOT NULL,
            embedding_model TEXT NOT NULL,
            dimensions INTEGER NOT NULL,
            vector_json TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (doc_id, embedding_model)
        );

        CREATE INDEX IF NOT EXISTS idx_conversation_chunks_repo_updated
        ON conversation_chunks(repo_id, updated_at);

        CREATE INDEX IF NOT EXISTS idx_conversation_chunks_conversation
        ON conversation_chunks(conversation_id, ordinal);

        CREATE INDEX IF NOT EXISTS idx_repo_aliases_alias_root
        ON repo_aliases(alias_root);

        CREATE INDEX IF NOT EXISTS idx_conversation_repo_links_repo
        ON conversation_repo_links(repo_id, confidence);

        CREATE INDEX IF NOT EXISTS idx_repo_scan_runs_repo_scanned_at
        ON repo_scan_runs(repo_id, scanned_at DESC);

        CREATE INDEX IF NOT EXISTS idx_document_embeddings_repo_model
        ON document_embeddings(repo_id, embedding_model, dimensions);

        CREATE INDEX IF NOT EXISTS idx_memory_conflicts_repo_status
        ON memory_conflicts(repo_id, status);

        CREATE INDEX IF NOT EXISTS idx_memory_merge_proposals_repo_status
        ON memory_merge_proposals(repo_id, status);

        CREATE INDEX IF NOT EXISTS idx_memory_entity_links_repo_owner
        ON memory_entity_links(repo_id, owner_type, owner_id);

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
    ensure_column(
        conn,
        "repo_scan_runs",
        "unmatched_project_roots_json",
        "TEXT NOT NULL DEFAULT '[]'",
    )?;
    dedupe_legacy_checkpoint_handoff_links(conn)?;
    migrate_document_embeddings_to_composite_key(conn)?;
    conn.execute(
        "CREATE UNIQUE INDEX IF NOT EXISTS idx_handoff_packets_checkpoint_id_unique
         ON handoff_packets(checkpoint_id)
         WHERE checkpoint_id IS NOT NULL",
        [],
    )?;

    Ok(())
}

fn dedupe_legacy_checkpoint_handoff_links(conn: &Connection) -> Result<()> {
    if !table_has_column(conn, "handoff_packets", "checkpoint_id")? {
        return Ok(());
    }

    // Legacy Task 5 builds could race and persist multiple handoffs for the same checkpoint.
    // Keep the earliest created linkage for audit continuity, and detach later duplicates by
    // nulling their checkpoint_id so startup can safely add the unique index without deleting
    // historical handoff rows.
    conn.execute_batch(
        "
        WITH ranked_duplicates AS (
            SELECT rowid,
                   checkpoint_id,
                   ROW_NUMBER() OVER (
                       PARTITION BY checkpoint_id
                       ORDER BY created_at ASC, handoff_id ASC, rowid ASC
                   ) AS duplicate_rank
            FROM handoff_packets
            WHERE checkpoint_id IS NOT NULL
        )
        UPDATE handoff_packets
        SET checkpoint_id = NULL
        WHERE rowid IN (
            SELECT rowid
            FROM ranked_duplicates
            WHERE duplicate_rank > 1
        );

        WITH surviving_links AS (
            SELECT checkpoint_id, handoff_id
            FROM (
                SELECT checkpoint_id,
                       handoff_id,
                       ROW_NUMBER() OVER (
                           PARTITION BY checkpoint_id
                           ORDER BY created_at ASC, handoff_id ASC, rowid ASC
                       ) AS duplicate_rank
                FROM handoff_packets
                WHERE checkpoint_id IS NOT NULL
            )
            WHERE duplicate_rank = 1
        )
        UPDATE checkpoints
        SET handoff_id = (
                SELECT surviving_links.handoff_id
                FROM surviving_links
                WHERE surviving_links.checkpoint_id = checkpoints.checkpoint_id
            )
        WHERE checkpoint_id IN (SELECT checkpoint_id FROM surviving_links);
        ",
    )?;

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
         WHERE last_verified_at IS NOT NULL
           AND freshness_status = 'unknown'
           AND freshness_score = 0.0
           AND verified_at IS NULL
           AND verified_by IS NULL",
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

fn migrate_document_embeddings_to_composite_key(conn: &Connection) -> Result<()> {
    if document_embeddings_has_composite_key(conn)? {
        return Ok(());
    }

    conn.execute_batch(
        "
        DROP INDEX IF EXISTS idx_document_embeddings_repo_model;
        DROP TABLE IF EXISTS document_embeddings_migration_backup;
        ALTER TABLE document_embeddings RENAME TO document_embeddings_migration_backup;

        CREATE TABLE document_embeddings (
            doc_id TEXT NOT NULL,
            repo_id TEXT NOT NULL,
            embedding_model TEXT NOT NULL,
            dimensions INTEGER NOT NULL,
            vector_json TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            PRIMARY KEY (doc_id, embedding_model)
        );

        INSERT OR REPLACE INTO document_embeddings (
            doc_id, repo_id, embedding_model, dimensions, vector_json, updated_at
        )
        SELECT doc_id, repo_id, embedding_model, dimensions, vector_json, updated_at
        FROM document_embeddings_migration_backup;

        DROP TABLE document_embeddings_migration_backup;

        CREATE INDEX IF NOT EXISTS idx_document_embeddings_repo_model
        ON document_embeddings(repo_id, embedding_model, dimensions);
        ",
    )?;

    Ok(())
}

fn document_embeddings_has_composite_key(conn: &Connection) -> Result<bool> {
    let mut stmt = conn.prepare("PRAGMA table_info(document_embeddings)")?;
    let columns = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(1)?, row.get::<_, i64>(5)?))
    })?;

    let mut primary_key_columns = columns
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|(_, pk_position)| *pk_position > 0)
        .collect::<Vec<_>>();
    primary_key_columns.sort_by_key(|(_, pk_position)| *pk_position);

    let names = primary_key_columns
        .into_iter()
        .map(|(name, _)| name)
        .collect::<Vec<_>>();

    Ok(names == vec!["doc_id".to_string(), "embedding_model".to_string()])
}

#[cfg(test)]
mod tests {
    use super::{migrate, open_connection};
    use rusqlite::{params, Connection};

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
        let path =
            std::env::temp_dir().join(format!("chatmem-db-test-{}.sqlite", uuid::Uuid::new_v4()));
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
                   'checkpoints',
                   'handoff_packets',
                   'evidence_refs',
                   'document_embeddings',
                   'memory_conflicts',
                   'memory_merge_proposals',
                   'memory_entities',
                   'memory_entity_links',
                   'repo_scan_runs',
                   'search_documents',
                   'wiki_pages'
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
                "checkpoints",
                "conversations",
                "document_embeddings",
                "episodes",
                "evidence_refs",
                "file_changes",
                "handoff_packets",
                "memory_candidates",
                "memory_conflicts",
                "memory_entities",
                "memory_entity_links",
                "memory_merge_proposals",
                "messages",
                "repo_scan_runs",
                "repos",
                "search_documents",
                "tool_calls",
                "wiki_pages",
            ]
        );

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn migrations_add_handoff_lifecycle_columns() {
        let path =
            std::env::temp_dir().join(format!("chatmem-db-test-{}.sqlite", uuid::Uuid::new_v4()));
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
    fn checkpoint_defaults_to_active_state() {
        let path =
            std::env::temp_dir().join(format!("chatmem-db-test-{}.sqlite", uuid::Uuid::new_v4()));
        let conn = open_connection(&path).unwrap();
        migrate(&conn).unwrap();

        conn.execute(
            "INSERT INTO checkpoints (
                checkpoint_id,
                repo_id,
                conversation_id,
                source_agent,
                resume_command,
                summary,
                metadata_json,
                created_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![
                "checkpoint-001",
                "repo-001",
                "claude:conv-001",
                "claude",
                "claude --resume conv-001",
                "Freeze the current debugging state",
                "{}",
                "2026-04-20T12:00:00Z",
            ],
        )
        .unwrap();

        let row = conn
            .query_row(
                "SELECT status, resume_command
                 FROM checkpoints
                 WHERE checkpoint_id = ?1",
                ["checkpoint-001"],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .unwrap();

        assert_eq!(row.0, "active");
        assert_eq!(row.1.as_deref(), Some("claude --resume conv-001"));

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn migrations_dedupe_legacy_duplicate_checkpoint_handoffs_before_creating_unique_index() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE checkpoints (
                checkpoint_id TEXT PRIMARY KEY,
                repo_id TEXT NOT NULL,
                conversation_id TEXT NOT NULL,
                source_agent TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'active',
                summary TEXT NOT NULL,
                resume_command TEXT,
                metadata_json TEXT NOT NULL DEFAULT '{}',
                handoff_id TEXT,
                created_at TEXT NOT NULL
            );

            CREATE TABLE handoff_packets (
                handoff_id TEXT PRIMARY KEY,
                repo_id TEXT NOT NULL,
                from_agent TEXT NOT NULL,
                to_agent TEXT NOT NULL,
                status TEXT NOT NULL DEFAULT 'draft',
                target_profile TEXT,
                checkpoint_id TEXT,
                compression_strategy TEXT,
                current_goal TEXT NOT NULL,
                done_json TEXT NOT NULL,
                next_json TEXT NOT NULL,
                key_files_json TEXT NOT NULL,
                commands_json TEXT NOT NULL,
                related_memories_json TEXT NOT NULL DEFAULT '[]',
                related_episodes_json TEXT NOT NULL DEFAULT '[]',
                consumed_at TEXT,
                consumed_by TEXT,
                created_at TEXT NOT NULL
            );
            ",
        )
        .unwrap();

        conn.execute(
            "INSERT INTO checkpoints (
                checkpoint_id, repo_id, conversation_id, source_agent, status, summary,
                resume_command, metadata_json, handoff_id, created_at
             ) VALUES (?1, ?2, ?3, ?4, 'promoted', ?5, ?6, '{}', ?7, ?8)",
            params![
                "checkpoint-001",
                "repo-001",
                "codex:conv-001",
                "codex",
                "Checkpoint summary",
                "codex resume conv-001",
                "handoff-newer",
                "2026-04-20T10:00:00Z",
            ],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO handoff_packets (
                handoff_id, repo_id, from_agent, to_agent, status, target_profile, checkpoint_id,
                compression_strategy, current_goal, done_json, next_json, key_files_json,
                commands_json, related_memories_json, related_episodes_json, consumed_at,
                consumed_by, created_at
             ) VALUES (?1, ?2, ?3, ?4, 'draft', NULL, ?5, NULL, ?6, '[]', '[]', '[]', '[]', '[]', '[]', NULL, NULL, ?7)",
            params![
                "handoff-older",
                "repo-001",
                "codex",
                "claude",
                "checkpoint-001",
                "Checkpoint summary",
                "2026-04-20T10:01:00Z",
            ],
        )
        .unwrap();

        conn.execute(
            "INSERT INTO handoff_packets (
                handoff_id, repo_id, from_agent, to_agent, status, target_profile, checkpoint_id,
                compression_strategy, current_goal, done_json, next_json, key_files_json,
                commands_json, related_memories_json, related_episodes_json, consumed_at,
                consumed_by, created_at
             ) VALUES (?1, ?2, ?3, ?4, 'draft', NULL, ?5, NULL, ?6, '[]', '[]', '[]', '[]', '[]', '[]', NULL, NULL, ?7)",
            params![
                "handoff-newer",
                "repo-001",
                "codex",
                "gemini",
                "checkpoint-001",
                "Checkpoint summary",
                "2026-04-20T10:02:00Z",
            ],
        )
        .unwrap();

        migrate(&conn).unwrap();

        let linked_handoffs = conn
            .prepare(
                "SELECT handoff_id
                 FROM handoff_packets
                 WHERE checkpoint_id = ?1
                 ORDER BY handoff_id ASC",
            )
            .unwrap()
            .query_map(["checkpoint-001"], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(linked_handoffs, vec!["handoff-older"]);

        let nulled_handoffs = conn
            .prepare(
                "SELECT handoff_id
                 FROM handoff_packets
                 WHERE checkpoint_id IS NULL
                 ORDER BY handoff_id ASC",
            )
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(nulled_handoffs, vec!["handoff-newer"]);

        let checkpoint_handoff = conn
            .query_row(
                "SELECT handoff_id FROM checkpoints WHERE checkpoint_id = ?1",
                ["checkpoint-001"],
                |row| row.get::<_, Option<String>>(0),
            )
            .unwrap();

        assert_eq!(checkpoint_handoff.as_deref(), Some("handoff-older"));
    }

    #[test]
    fn migrations_create_run_timeline_tables() {
        let path =
            std::env::temp_dir().join(format!("chatmem-db-test-{}.sqlite", uuid::Uuid::new_v4()));
        let conn = open_connection(&path).unwrap();
        migrate(&conn).unwrap();

        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master
                 WHERE type='table' AND name IN (
                   'agent_runs',
                   'run_events',
                   'artifacts'
                 )
                 ORDER BY name",
            )
            .unwrap();

        let names = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(names, vec!["agent_runs", "artifacts", "run_events"]);

        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn migration_creates_conversation_chunk_tables() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();

        let tables = [
            "conversation_chunks",
            "repo_aliases",
            "conversation_repo_links",
        ];

        for table in tables {
            let exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(exists, 1, "expected table {table}");
        }
    }

    #[test]
    fn migration_creates_chunk_search_indexes() {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();

        let indexes = [
            "idx_conversation_chunks_repo_updated",
            "idx_conversation_chunks_conversation",
            "idx_repo_aliases_alias_root",
            "idx_conversation_repo_links_repo",
        ];

        for index in indexes {
            let exists: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'index' AND name = ?1",
                    [index],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(exists, 1, "expected index {index}");
        }
    }

    #[test]
    fn migrations_add_memory_freshness_columns() {
        let path =
            std::env::temp_dir().join(format!("chatmem-db-test-{}.sqlite", uuid::Uuid::new_v4()));
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
    fn migrations_allow_multiple_embedding_models_per_document() {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE document_embeddings (
                doc_id TEXT PRIMARY KEY,
                repo_id TEXT NOT NULL,
                embedding_model TEXT NOT NULL,
                dimensions INTEGER NOT NULL,
                vector_json TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            INSERT INTO document_embeddings (
                doc_id, repo_id, embedding_model, dimensions, vector_json, updated_at
            ) VALUES (
                'doc-1', 'repo-1', 'chatmem-local-hash-v1', 384, '[0.1,0.2]', '2026-04-22T00:00:00Z'
            );
            ",
        )
        .unwrap();

        migrate(&conn).unwrap();

        conn.execute(
            "INSERT INTO document_embeddings (
                doc_id, repo_id, embedding_model, dimensions, vector_json, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            rusqlite::params![
                "doc-1",
                "repo-1",
                "openai-compatible:text-embedding-3-small:1536",
                1536_i64,
                "[0.3,0.4]",
                "2026-04-22T00:01:00Z",
            ],
        )
        .unwrap();

        let models = conn
            .prepare(
                "SELECT embedding_model
                 FROM document_embeddings
                 WHERE doc_id = 'doc-1'
                 ORDER BY embedding_model ASC",
            )
            .unwrap()
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(
            models,
            vec![
                "chatmem-local-hash-v1".to_string(),
                "openai-compatible:text-embedding-3-small:1536".to_string(),
            ]
        );
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

    #[test]
    fn migrations_do_not_rewrite_non_legacy_memory_verifier() {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();

        conn.execute(
            "INSERT INTO approved_memories (
                memory_id, repo_id, kind, title, value, usage_hint, status,
                last_verified_at, freshness_status, freshness_score,
                verified_at, verified_by, created_from_candidate_id, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, NULL, ?13, ?13)",
            rusqlite::params![
                "mem-current",
                "repo-1",
                "command",
                "Current verification",
                "cargo test",
                "Use before shipping",
                "active",
                "2026-04-20T09:00:00Z",
                "fresh",
                1.0_f64,
                "2026-04-20T09:00:00Z",
                Option::<String>::None,
                "2026-04-20T09:00:00Z",
            ],
        )
        .unwrap();

        migrate(&conn).unwrap();

        let row = conn
            .query_row(
                "SELECT freshness_status, freshness_score, verified_at, verified_by
                 FROM approved_memories
                 WHERE memory_id = ?1",
                ["mem-current"],
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
        assert_eq!(row.2.as_deref(), Some("2026-04-20T09:00:00Z"));
        assert_eq!(row.3, None);
    }
}
