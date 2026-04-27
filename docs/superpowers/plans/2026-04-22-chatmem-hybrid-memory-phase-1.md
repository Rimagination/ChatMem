# ChatMem Hybrid Memory Phase 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the reliable local-history foundation for ChatMem: project scans, chunk-level indexing, repo attribution diagnostics, and a recall-aware `get_project_context` entry point.

**Architecture:** This phase is additive. It keeps existing approved memory, candidates, episodes, wiki pages, handoffs, checkpoints, runs, and artifacts intact while adding chunk-level search documents and repo alias/link diagnostics. The new `get_project_context` tool composes approved memory with historical evidence without treating unapproved history as durable policy.

**Tech Stack:** Rust, Tauri commands, rusqlite, RMCP, React, TypeScript, Vitest, Testing Library.

---

## Scope

This plan implements Phase 1 from `docs/superpowers/specs/2026-04-22-chatmem-hybrid-memory-architecture-design.md`.

Included:

- Add chunk and repo-attribution schema.
- Backfill/search conversation chunks during conversation snapshot sync.
- Add local scan command for a repo.
- Add recall-aware `get_project_context` in store, MCP, and Tauri command surfaces.
- Expand memory health/status data for project home.
- Add a small frontend project index status surface.
- Update repo ChatMem skill guidance to prefer `get_project_context` for recall.

Not included in this plan:

- Agent-assisted memory seeding UI.
- Extraction job queue.
- Optional background LLM extraction.
- Full project-home redesign beyond scan/index status.
- Automatic approval of memory candidates.

## File Structure

- `src-tauri/src/chatmem_memory/db.rs`
  - Add tables and indexes for `conversation_chunks`, `repo_aliases`, and `conversation_repo_links`.
- `src-tauri/src/chatmem_memory/models.rs`
  - Add response/input models for chunks, project context, and index health.
- `src-tauri/src/chatmem_memory/chunks.rs`
  - New focused module that turns normalized conversations into deterministic searchable chunks.
- `src-tauri/src/chatmem_memory/store.rs`
  - Persist chunks during `upsert_conversation_snapshot`, store repo aliases and links, and implement `get_project_context`.
- `src-tauri/src/chatmem_memory/sync.rs`
  - Expose scan summary and record attribution links during repo sync.
- `src-tauri/src/chatmem_memory/mcp.rs`
  - Add MCP route for `get_project_context`.
- `src-tauri/src/main.rs`
  - Add Tauri commands `scan_repo_conversations` and `get_project_context`.
- `src/chatmem-memory/types.ts`
  - Add matching frontend types.
- `src/chatmem-memory/api.ts`
  - Add wrappers for scan and project context commands.
- `src/components/ProjectIndexStatus.tsx`
  - New component for scan/index status and manual rescan action.
- `src/App.tsx`
  - Load index health and render `ProjectIndexStatus` when a project is active.
- `src/__tests__/ProjectIndexStatus.test.tsx`
  - Component tests for status and scan action.
- `src/__tests__/MemoryWorkspace.test.tsx`
  - Integration test that project memory load also asks for index health.
- `skills/chatmem/SKILL.md`
  - Update agent workflow to use `get_project_context` for recall.
- `src/__tests__/integrationManifests.test.ts`
  - Assert the repo skill mentions `get_project_context`.

## Task 1: Add Chunk And Repo Attribution Schema

**Files:**
- Modify: `src-tauri/src/chatmem_memory/db.rs`

- [ ] **Step 1: Write schema migration tests**

Append these tests inside the existing `#[cfg(test)] mod tests` in `src-tauri/src/chatmem_memory/db.rs`:

```rust
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
```

- [ ] **Step 2: Run the focused failing tests**

Run:

```powershell
cd D:\VSP\agentswap-gui\src-tauri
cargo test chatmem_memory::db::tests::migration_creates_conversation_chunk_tables chatmem_memory::db::tests::migration_creates_chunk_search_indexes
```

Expected: FAIL because the new tables and indexes do not exist.

- [ ] **Step 3: Add schema**

In `src-tauri/src/chatmem_memory/db.rs`, inside the main `conn.execute_batch` schema block after the `messages` table, add:

```rust
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
```

Inside the same schema block after `repos`, add:

```rust
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
```

Inside the index section before the FTS table, add:

```rust
        CREATE INDEX IF NOT EXISTS idx_conversation_chunks_repo_updated
        ON conversation_chunks(repo_id, updated_at);

        CREATE INDEX IF NOT EXISTS idx_conversation_chunks_conversation
        ON conversation_chunks(conversation_id, ordinal);

        CREATE INDEX IF NOT EXISTS idx_repo_aliases_alias_root
        ON repo_aliases(alias_root);

        CREATE INDEX IF NOT EXISTS idx_conversation_repo_links_repo
        ON conversation_repo_links(repo_id, confidence);
```

- [ ] **Step 4: Run schema tests**

Run:

```powershell
cd D:\VSP\agentswap-gui\src-tauri
cargo test chatmem_memory::db::tests::migration_creates_conversation_chunk_tables chatmem_memory::db::tests::migration_creates_chunk_search_indexes
```

Expected: PASS.

- [ ] **Step 5: Commit schema**

```powershell
git add src-tauri/src/chatmem_memory/db.rs
git commit -m "feat: add ChatMem history index schema"
```

## Task 2: Add Deterministic Conversation Chunking

**Files:**
- Create: `src-tauri/src/chatmem_memory/chunks.rs`
- Modify: `src-tauri/src/chatmem_memory/mod.rs`

- [ ] **Step 1: Create chunking tests**

Create `src-tauri/src/chatmem_memory/chunks.rs` with this initial test module and public API shell:

```rust
use agentswap_core::types::{ChangeType, Conversation};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConversationChunk {
    pub chunk_id_suffix: String,
    pub chunk_type: String,
    pub title: String,
    pub body: String,
    pub message_ids: Vec<String>,
    pub ordinal: usize,
    pub token_estimate: usize,
}

pub fn build_conversation_chunks(_conversation_id: &str, _conversation: &Conversation) -> Vec<ConversationChunk> {
    Vec::new()
}

fn estimate_tokens(text: &str) -> usize {
    text.chars().count().div_ceil(4).max(1)
}

#[cfg(test)]
mod tests {
    use super::build_conversation_chunks;
    use agentswap_core::types::{AgentKind, Conversation, Message, Role};
    use chrono::Utc;
    use std::collections::HashMap;
    use uuid::Uuid;

    fn message(role: Role, content: &str) -> Message {
        Message {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            role,
            content: content.to_string(),
            tool_calls: vec![],
            metadata: HashMap::new(),
        }
    }

    fn conversation(messages: Vec<Message>) -> Conversation {
        Conversation {
            id: "conv-chunks".to_string(),
            source_agent: AgentKind::Codex,
            project_dir: "d:/vsp/agentswap-gui".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            summary: Some("Chunking test".to_string()),
            messages,
            file_changes: vec![],
        }
    }

    #[test]
    fn chunks_include_late_conversation_messages() {
        let mut messages = Vec::new();
        for index in 0..16 {
            messages.push(message(Role::User, &format!("ordinary setup message {index}")));
        }
        messages.push(message(
            Role::Assistant,
            "Late important release signing detail: TAURI_PRIVATE_KEY must be configured.",
        ));

        let chunks = build_conversation_chunks("codex:conv-chunks", &conversation(messages));

        assert!(chunks.iter().any(|chunk| chunk
            .body
            .contains("TAURI_PRIVATE_KEY must be configured")));
        assert!(chunks.iter().all(|chunk| !chunk.message_ids.is_empty()));
    }

    #[test]
    fn chunks_classify_user_requests_and_assistant_summaries() {
        let conv = conversation(vec![
            message(Role::User, "Can you fix the memory search recall bug?"),
            message(Role::Assistant, "Implemented chunk-level recall search for ChatMem."),
        ]);

        let chunks = build_conversation_chunks("codex:conv-chunks", &conv);

        assert!(chunks.iter().any(|chunk| chunk.chunk_type == "user_request"));
        assert!(chunks.iter().any(|chunk| chunk.chunk_type == "assistant_summary"));
        assert_eq!(chunks[0].ordinal, 0);
    }
}
```

- [ ] **Step 2: Run failing chunk tests**

Run:

```powershell
cd D:\VSP\agentswap-gui\src-tauri
cargo test chatmem_memory::chunks::tests
```

Expected: FAIL because `mod.rs` does not expose `chunks` and the shell returns no chunks.

- [ ] **Step 3: Expose module**

In `src-tauri/src/chatmem_memory/mod.rs`, add:

```rust
pub mod chunks;
```

- [ ] **Step 4: Implement chunk builder**

Replace `build_conversation_chunks` and add helpers in `src-tauri/src/chatmem_memory/chunks.rs`:

```rust
pub fn build_conversation_chunks(conversation_id: &str, conversation: &Conversation) -> Vec<ConversationChunk> {
    let mut chunks = Vec::new();
    let mut ordinal = 0usize;

    if let Some(summary) = conversation.summary.as_deref().filter(|value| !value.trim().is_empty()) {
        chunks.push(ConversationChunk {
            chunk_id_suffix: "summary".to_string(),
            chunk_type: "assistant_summary".to_string(),
            title: format!("Conversation summary: {summary}"),
            body: summary.trim().to_string(),
            message_ids: vec![],
            ordinal,
            token_estimate: estimate_tokens(summary),
        });
        ordinal += 1;
    }

    for message in &conversation.messages {
        let body = message.content.trim();
        if body.is_empty() {
            continue;
        }

        let chunk_type = match message.role {
            agentswap_core::types::Role::User => "user_request",
            agentswap_core::types::Role::Assistant => "assistant_summary",
            agentswap_core::types::Role::System => "implementation_detail",
        };
        let message_id = message.id.to_string();
        let title = format!("{} message in {conversation_id}", chunk_type.replace('_', " "));
        let body = compact_chunk_body(body);

        chunks.push(ConversationChunk {
            chunk_id_suffix: format!("message:{message_id}"),
            chunk_type: chunk_type.to_string(),
            title,
            token_estimate: estimate_tokens(&body),
            body,
            message_ids: vec![message_id],
            ordinal,
        });
        ordinal += 1;
    }

    for (index, file_change) in conversation.file_changes.iter().enumerate() {
        let change_type = match file_change.change_type {
            ChangeType::Created => "created",
            ChangeType::Modified => "modified",
            ChangeType::Deleted => "deleted",
        };
        let body = format!("{change_type}: {}", file_change.path);
        chunks.push(ConversationChunk {
            chunk_id_suffix: format!("file:{index}"),
            chunk_type: "file_change".to_string(),
            title: format!("File change in {conversation_id}"),
            token_estimate: estimate_tokens(&body),
            body,
            message_ids: vec![file_change.message_id.to_string()],
            ordinal,
        });
        ordinal += 1;
    }

    chunks
}

fn compact_chunk_body(text: &str) -> String {
    const MAX_CHARS: usize = 2_400;
    let compact = text
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string();

    if compact.chars().count() <= MAX_CHARS {
        compact
    } else {
        let prefix = compact.chars().take(MAX_CHARS).collect::<String>();
        format!("{prefix}\n[truncated]")
    }
}
```

- [ ] **Step 5: Run chunk tests**

Run:

```powershell
cd D:\VSP\agentswap-gui\src-tauri
cargo test chatmem_memory::chunks::tests
```

Expected: PASS.

- [ ] **Step 6: Commit chunk builder**

```powershell
git add src-tauri/src/chatmem_memory/chunks.rs src-tauri/src/chatmem_memory/mod.rs
git commit -m "feat: add conversation chunk builder"
```

## Task 3: Persist Chunk Search Documents During Snapshot Sync

**Files:**
- Modify: `src-tauri/src/chatmem_memory/store.rs`

- [ ] **Step 1: Write failing store test**

Append this test to the existing `#[cfg(test)] mod tests` in `src-tauri/src/chatmem_memory/store.rs`:

```rust
#[test]
fn conversation_snapshot_indexes_late_message_chunks() {
    let store = new_store();
    let repo_root = "d:/vsp/agentswap-gui";
    let now = Utc::now();
    let mut messages = Vec::new();

    for index in 0..16 {
        messages.push(Message {
            id: Uuid::new_v4(),
            timestamp: now,
            role: Role::User,
            content: format!("ordinary setup message {index}"),
            tool_calls: vec![],
            metadata: HashMap::new(),
        });
    }
    messages.push(Message {
        id: Uuid::new_v4(),
        timestamp: now,
        role: Role::Assistant,
        content: "Late recall marker: configure TAURI_PRIVATE_KEY before release packaging.".to_string(),
        tool_calls: vec![],
        metadata: HashMap::new(),
    });

    let conversation = Conversation {
        id: "conv-late-chunk".to_string(),
        source_agent: AgentKind::Codex,
        project_dir: repo_root.to_string(),
        created_at: now,
        updated_at: now,
        summary: Some("Late chunk indexing".to_string()),
        messages,
        file_changes: vec![],
    };

    store.upsert_conversation_snapshot("codex", &conversation, None).unwrap();

    let matches = store
        .search_history(repo_root, "TAURI_PRIVATE_KEY release packaging", 5)
        .unwrap();

    assert!(matches.iter().any(|item| {
        item.r#type == "chunk" && item.summary.contains("TAURI_PRIVATE_KEY")
    }));
}
```

- [ ] **Step 2: Run failing store test**

Run:

```powershell
cd D:\VSP\agentswap-gui\src-tauri
cargo test chatmem_memory::store::tests::conversation_snapshot_indexes_late_message_chunks
```

Expected: FAIL because chunk documents are not inserted.

- [ ] **Step 3: Import chunk builder**

At the top of `src-tauri/src/chatmem_memory/store.rs`, add `chunks` to the module imports:

```rust
use super::{
    chunks,
    embedding,
    handoff,
    models::{
        AgentConversationCount, ApprovedMemoryResponse, CreateMemoryCandidateInput,
        CreateMemoryMergeProposalInput,
        EntityGraphPayload, EntityLinkResponse, EmbeddingRebuildReport, EntityNodeResponse,
        EpisodeResponse, EvidenceRef, HandoffPacketResponse, MemoryCandidateResponse,
        MemoryConflictResponse, MemoryMergeSuggestion, RepoMemoryHealthResponse,
        SearchHistoryMatch, WikiPageResponse,
    },
    repo_identity,
};
```

- [ ] **Step 4: Delete stale chunks before message replacement**

In `upsert_conversation_snapshot`, after deleting stale `file_changes`, add:

```rust
        tx.execute(
            "DELETE FROM conversation_chunks WHERE conversation_id = ?1",
            [conversation_id.clone()],
        )?;
        tx.execute(
            "DELETE FROM search_documents_fts
             WHERE doc_id IN (
                SELECT doc_id FROM search_documents
                WHERE doc_type = 'chunk' AND doc_ref_id LIKE ?1
             )",
            [format!("{conversation_id}:%")],
        )?;
        tx.execute(
            "DELETE FROM search_documents
             WHERE repo_id = ?1 AND doc_type = 'chunk' AND doc_ref_id LIKE ?2",
            params![repo_id.clone(), format!("{conversation_id}:%")],
        )?;
```

- [ ] **Step 5: Insert chunks after conversation summary search document**

In `upsert_conversation_snapshot`, after the existing `upsert_search_document_tx` call for `doc_type` `"conversation"`, add:

```rust
        let now = chrono::Utc::now().to_rfc3339();
        for chunk in chunks::build_conversation_chunks(&conversation_id, conversation) {
            let chunk_ref_id = format!("{conversation_id}:{}", chunk.chunk_id_suffix);
            let chunk_id = format!("chunk:{chunk_ref_id}");
            let message_ids_json = serde_json::to_string(&chunk.message_ids)?;

            tx.execute(
                "INSERT INTO conversation_chunks (
                    chunk_id, repo_id, conversation_id, chunk_type, title, body,
                    message_ids_json, ordinal, token_estimate, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
                params![
                    chunk_id,
                    repo_id,
                    conversation_id,
                    chunk.chunk_type,
                    chunk.title,
                    chunk.body,
                    message_ids_json,
                    chunk.ordinal as i64,
                    chunk.token_estimate as i64,
                    now,
                ],
            )?;

            upsert_search_document_tx(
                &tx,
                &format!("chunk:{chunk_ref_id}"),
                &repo_id,
                "chunk",
                &chunk_ref_id,
                &chunk.title,
                &chunk.body,
            )?;
        }
```

- [ ] **Step 6: Teach evidence owner mapping about chunks**

Update `evidence_owner_for_doc_type` in `src-tauri/src/chatmem_memory/store.rs`:

```rust
fn evidence_owner_for_doc_type(doc_type: &str) -> &'static str {
    match doc_type {
        "memory" => "memory",
        "episode" => "episode",
        "wiki" => "wiki_page",
        "chunk" => "chunk",
        _ => "conversation",
    }
}
```

- [ ] **Step 7: Add chunk evidence loading fallback**

In `load_evidence_refs_from_conn`, before returning rows, keep existing behavior. Then add a helper for chunks and call it from `search_history_in_repo_id_with_embedding_config` when `candidate.doc_type == "chunk"`:

```rust
fn load_chunk_evidence_refs_from_conn(conn: &Connection, chunk_ref_id: &str) -> Result<Vec<EvidenceRef>> {
    let mut stmt = conn.prepare(
        "SELECT conversation_id, message_ids_json, body
         FROM conversation_chunks
         WHERE chunk_id = ?1",
    )?;
    let chunk_id = format!("chunk:{chunk_ref_id}");
    let row = stmt
        .query_row([chunk_id], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })
        .optional()?;

    let Some((conversation_id, message_ids_json, body)) = row else {
        return Ok(vec![]);
    };
    let message_ids = serde_json::from_str::<Vec<String>>(&message_ids_json).unwrap_or_default();

    Ok(vec![EvidenceRef {
        evidence_id: None,
        conversation_id: Some(conversation_id),
        message_id: message_ids.first().cloned(),
        tool_call_id: None,
        file_change_id: None,
        excerpt: truncate_text(&body, 240),
    }])
}
```

Then inside match construction:

```rust
            evidence_refs: if candidate.doc_type == "chunk" {
                load_chunk_evidence_refs_from_conn(conn, &candidate.doc_ref_id)?
            } else {
                load_evidence_refs_from_conn(
                    conn,
                    evidence_owner_for_doc_type(&candidate.doc_type),
                    &candidate.doc_ref_id,
                )?
            },
```

- [ ] **Step 8: Run store test**

Run:

```powershell
cd D:\VSP\agentswap-gui\src-tauri
cargo test chatmem_memory::store::tests::conversation_snapshot_indexes_late_message_chunks
```

Expected: PASS.

- [ ] **Step 9: Commit chunk persistence**

```powershell
git add src-tauri/src/chatmem_memory/store.rs
git commit -m "feat: index conversation chunks in ChatMem search"
```

## Task 4: Add Repo Aliases, Scan Summary, And Health Counts

**Files:**
- Modify: `src-tauri/src/chatmem_memory/models.rs`
- Modify: `src-tauri/src/chatmem_memory/store.rs`
- Modify: `src-tauri/src/chatmem_memory/sync.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Add model types**

In `src-tauri/src/chatmem_memory/models.rs`, after `AgentConversationCount`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RepoAliasResponse {
    pub alias_root: String,
    pub alias_kind: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct RepoScanReport {
    pub repo_root: String,
    pub canonical_repo_root: String,
    pub scanned_conversation_count: usize,
    pub linked_conversation_count: usize,
    pub skipped_conversation_count: usize,
    pub source_agents: Vec<AgentConversationCount>,
    pub warnings: Vec<String>,
}
```

Extend `RepoMemoryHealthResponse`:

```rust
    pub indexed_chunk_count: usize,
    pub repo_aliases: Vec<RepoAliasResponse>,
```

- [ ] **Step 2: Add failing store test for health aliases**

Append this test to `src-tauri/src/chatmem_memory/store.rs` tests:

```rust
#[test]
fn repo_memory_health_reports_indexed_chunks_and_aliases() {
    let store = new_store();
    let repo_root = "d:/vsp/agentswap-gui";
    let repo_id = store.ensure_repo(repo_root).unwrap();
    store
        .upsert_repo_alias_for_repo_id(&repo_id, "d:/vsp", "ancestor", 0.61)
        .unwrap();

    let health = store.repo_memory_health(repo_root).unwrap();

    assert_eq!(health.indexed_chunk_count, 0);
    assert!(health
        .repo_aliases
        .iter()
        .any(|alias| alias.alias_root == "d:/vsp" && alias.alias_kind == "ancestor"));
}
```

- [ ] **Step 3: Run failing health test**

Run:

```powershell
cd D:\VSP\agentswap-gui\src-tauri
cargo test chatmem_memory::store::tests::repo_memory_health_reports_indexed_chunks_and_aliases
```

Expected: FAIL because alias helpers and health fields do not exist.

- [ ] **Step 4: Add alias helper methods**

In the `impl MemoryStore` block in `src-tauri/src/chatmem_memory/store.rs`, add:

```rust
    pub(crate) fn upsert_repo_alias_for_repo_id(
        &self,
        repo_id: &str,
        alias_root: &str,
        alias_kind: &str,
        confidence: f64,
    ) -> Result<()> {
        let alias_root = repo_identity::normalize_repo_root(alias_root);
        if alias_root.is_empty() {
            return Ok(());
        }
        let now = chrono::Utc::now().to_rfc3339();
        let alias_id = uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_URL,
            format!("chatmem:repo-alias:{repo_id}:{alias_root}").as_bytes(),
        )
        .to_string();
        let conn = self.conn()?;
        conn.execute(
            "INSERT INTO repo_aliases (
                alias_id, repo_id, alias_root, alias_kind, confidence, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
             ON CONFLICT(repo_id, alias_root) DO UPDATE SET
                alias_kind = excluded.alias_kind,
                confidence = excluded.confidence,
                updated_at = excluded.updated_at",
            params![alias_id, repo_id, alias_root, alias_kind, confidence, now],
        )?;
        Ok(())
    }

    fn list_repo_aliases_by_repo_id(
        &self,
        conn: &Connection,
        repo_id: &str,
    ) -> Result<Vec<RepoAliasResponse>> {
        let mut stmt = conn.prepare(
            "SELECT alias_root, alias_kind, confidence
             FROM repo_aliases
             WHERE repo_id = ?1
             ORDER BY confidence DESC, alias_root ASC",
        )?;
        let aliases = stmt
            .query_map([repo_id], |row| {
                Ok(RepoAliasResponse {
                    alias_root: row.get(0)?,
                    alias_kind: row.get(1)?,
                    confidence: row.get(2)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(aliases)
    }
```

Add `RepoAliasResponse` to the `models::{...}` import list in `store.rs`.

- [ ] **Step 5: Update health counts**

In `repo_memory_health`, add:

```rust
        let indexed_chunk_count = count_table_rows(&conn, "conversation_chunks", &repo_id, None)?;
        let repo_aliases = self.list_repo_aliases_by_repo_id(&conn, &repo_id)?;
```

Then add these fields to the returned `RepoMemoryHealthResponse`:

```rust
            indexed_chunk_count,
            repo_aliases,
```

- [ ] **Step 6: Update sync report**

In `src-tauri/src/chatmem_memory/sync.rs`, import `AgentConversationCount` and `RepoScanReport`:

```rust
use super::{
    models::{AgentConversationCount, RepoScanReport},
    store::MemoryStore,
};
```

Replace the current `sync_repo_conversations` signature and body with:

```rust
pub fn sync_repo_conversations(store: &MemoryStore, repo_root: &str) -> anyhow::Result<usize> {
    Ok(scan_repo_conversations(store, repo_root)?.linked_conversation_count)
}

pub fn scan_repo_conversations(store: &MemoryStore, repo_root: &str) -> anyhow::Result<RepoScanReport> {
    let normalized_repo = crate::chatmem_memory::repo_identity::canonical_repo_root(repo_root);
    let repo_id = store.ensure_repo(&normalized_repo)?;
    let mut scanned = 0usize;
    let mut linked = 0usize;
    let mut skipped = 0usize;
    let mut source_agents = Vec::new();
    let mut warnings = Vec::new();

    store.upsert_repo_alias_for_repo_id(&repo_id, repo_root, "requested", 1.0)?;
    store.upsert_repo_alias_for_repo_id(&repo_id, &normalized_repo, "canonical", 1.0)?;

    for agent in ["claude", "codex", "gemini"] {
        let Some(adapter) = get_adapter(agent) else {
            continue;
        };

        if !adapter.is_available() {
            continue;
        }

        let summaries = adapter.list_conversations()?;
        let mut agent_count = 0usize;
        for summary in summaries {
            scanned += 1;
            if !summary_project_matches_repo(agent, &summary.project_dir, &normalized_repo) {
                skipped += 1;
                continue;
            }

            let mut conversation = adapter.read_conversation(&summary.id)?;
            if agent == "gemini"
                && crate::chatmem_memory::repo_identity::normalize_repo_root(&conversation.project_dir)
                    != normalized_repo
            {
                conversation.project_dir = normalized_repo.clone();
            }
            sync_conversation_into_store(store, agent, &conversation)?;
            linked += 1;
            agent_count += 1;

            let observed_project =
                crate::chatmem_memory::repo_identity::normalize_repo_root(&summary.project_dir);
            if observed_project != normalized_repo {
                store.upsert_repo_alias_for_repo_id(&repo_id, &observed_project, "observed", 0.72)?;
            }
        }

        if agent_count > 0 {
            source_agents.push(AgentConversationCount {
                source_agent: agent.to_string(),
                conversation_count: agent_count,
            });
        }
    }

    if linked == 0 && scanned > 0 {
        warnings.push(format!(
            "Scanned {scanned} local conversations but none matched {normalized_repo}."
        ));
    }

    Ok(RepoScanReport {
        repo_root: crate::chatmem_memory::repo_identity::normalize_repo_root(repo_root),
        canonical_repo_root: normalized_repo,
        scanned_conversation_count: scanned,
        linked_conversation_count: linked,
        skipped_conversation_count: skipped,
        source_agents,
        warnings,
    })
}
```

- [ ] **Step 7: Add Tauri scan command**

In `src-tauri/src/main.rs`, import `RepoScanReport` and `scan_repo_conversations`, then add:

```rust
#[command]
async fn scan_repo_conversations(repo_root: String) -> Result<RepoScanReport, String> {
    let store = open_memory_store()?;
    chatmem_memory::sync::scan_repo_conversations(&store, &repo_root)
        .map_err(|error| error.to_string())
}
```

Add `scan_repo_conversations` to `tauri::generate_handler![...]`.

- [ ] **Step 8: Run Rust tests**

Run:

```powershell
cd D:\VSP\agentswap-gui\src-tauri
cargo test chatmem_memory::store::tests::repo_memory_health_reports_indexed_chunks_and_aliases
```

Expected: PASS.

- [ ] **Step 9: Commit scan status**

```powershell
git add src-tauri/src/chatmem_memory/models.rs src-tauri/src/chatmem_memory/store.rs src-tauri/src/chatmem_memory/sync.rs src-tauri/src/main.rs
git commit -m "feat: report ChatMem history scan status"
```

## Task 5: Add Recall-Aware Project Context

**Files:**
- Modify: `src-tauri/src/chatmem_memory/models.rs`
- Modify: `src-tauri/src/chatmem_memory/search.rs`
- Modify: `src-tauri/src/chatmem_memory/store.rs`

- [ ] **Step 1: Add project context models**

In `src-tauri/src/chatmem_memory/models.rs`, after `SearchRepoHistoryInput`, add:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct GetProjectContextInput {
    pub repo_root: String,
    pub query: String,
    pub intent: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ProjectContextPayload {
    pub repo_summary: String,
    pub intent: String,
    pub approved_memories: Vec<ApprovedMemoryResponse>,
    pub priority_gotchas: Vec<ApprovedMemoryResponse>,
    pub recent_handoff: Option<HandoffPacketResponse>,
    pub relevant_history: Vec<SearchHistoryMatch>,
    pub pending_candidates: Vec<MemoryCandidateResponse>,
    pub repo_diagnostics: RepoMemoryHealthResponse,
}
```

- [ ] **Step 2: Write failing project context tests**

Append these tests to `src-tauri/src/chatmem_memory/store.rs` tests:

```rust
#[test]
fn project_context_recall_returns_history_when_memory_is_empty() {
    let store = new_store();
    let repo_root = "d:/vsp/agentswap-gui";
    let now = Utc::now();
    let conversation = Conversation {
        id: "conv-recall-context".to_string(),
        source_agent: AgentKind::Codex,
        project_dir: repo_root.to_string(),
        created_at: now,
        updated_at: now,
        summary: Some("Recall context".to_string()),
        messages: vec![Message {
            id: Uuid::new_v4(),
            timestamp: now,
            role: Role::Assistant,
            content: "We decided that history evidence must be labeled as not approved memory."
                .to_string(),
            tool_calls: vec![],
            metadata: HashMap::new(),
        }],
        file_changes: vec![],
    };

    store.upsert_conversation_snapshot("codex", &conversation, None).unwrap();

    let context = store
        .get_project_context(
            repo_root,
            "did we discuss history evidence labeling",
            Some("recall"),
            Some(5),
        )
        .unwrap();

    assert!(context.approved_memories.is_empty());
    assert!(context
        .relevant_history
        .iter()
        .any(|item| item.summary.contains("not approved memory")));
}

#[test]
fn project_context_includes_related_pending_candidates_as_unapproved() {
    let store = new_store();
    let repo_root = "d:/vsp/agentswap-gui";
    store
        .create_candidate(&CreateMemoryCandidateInput {
            repo_root: repo_root.to_string(),
            kind: "gotcha".to_string(),
            summary: "History evidence is not policy".to_string(),
            value: "Historical matches must be labeled as evidence until approved.".to_string(),
            why_it_matters: "Prevents unreviewed transcript details from becoming policy.".to_string(),
            evidence_refs: vec![],
            confidence: 0.81,
            proposed_by: "codex".to_string(),
        })
        .unwrap();

    let context = store
        .get_project_context(
            repo_root,
            "history evidence policy",
            Some("recall"),
            Some(5),
        )
        .unwrap();

    assert_eq!(context.pending_candidates.len(), 1);
    assert_eq!(context.pending_candidates[0].status, "pending_review");
}
```

- [ ] **Step 3: Run failing context tests**

Run:

```powershell
cd D:\VSP\agentswap-gui\src-tauri
cargo test chatmem_memory::store::tests::project_context
```

Expected: FAIL because `get_project_context` does not exist.

- [ ] **Step 4: Add helper in search module**

In `src-tauri/src/chatmem_memory/search.rs`, expose the memory payload builder pieces by adding:

```rust
pub fn compact_repo_memory(
    store: &MemoryStore,
    repo_root: &str,
    task_hint: Option<&str>,
) -> Result<RepoMemoryPayload> {
    build_repo_memory_payload(store, repo_root, task_hint)
}
```

- [ ] **Step 5: Implement store method**

In `src-tauri/src/chatmem_memory/store.rs`, add `ProjectContextPayload` and `GetProjectContextInput` to imports if needed, then add this method in `impl MemoryStore`:

```rust
    pub fn get_project_context(
        &self,
        repo_root: &str,
        query: &str,
        intent: Option<&str>,
        limit: Option<usize>,
    ) -> Result<ProjectContextPayload> {
        let intent = intent.unwrap_or("auto").trim().to_string();
        let result_limit = limit.unwrap_or(5).max(1);
        let memory_payload = crate::chatmem_memory::search::compact_repo_memory(
            self,
            repo_root,
            Some(query),
        )?;
        let health = self.repo_memory_health(repo_root)?;

        let relevant_history = if should_search_history(&intent, query) {
            self.search_history(repo_root, query, result_limit)?
        } else {
            Vec::new()
        };

        let pending_candidates = self
            .list_candidates_with_status(repo_root, Some("pending_review"))?
            .into_iter()
            .filter(|candidate| candidate_matches_query(candidate, query))
            .take(result_limit)
            .collect::<Vec<_>>();

        Ok(ProjectContextPayload {
            repo_summary: format!("Project context for {repo_root}"),
            intent,
            approved_memories: memory_payload.approved_memories,
            priority_gotchas: memory_payload.priority_gotchas,
            recent_handoff: memory_payload.recent_handoff,
            relevant_history,
            pending_candidates,
            repo_diagnostics: health,
        })
    }
```

Add these helper functions near other search helper functions:

```rust
fn should_search_history(intent: &str, query: &str) -> bool {
    let normalized_intent = intent.trim().to_lowercase();
    if matches!(
        normalized_intent.as_str(),
        "recall" | "continue_work" | "debug" | "release" | "memory_review"
    ) {
        return true;
    }

    let query = query.to_lowercase();
    query.contains("remember")
        || query.contains("discuss")
        || query.contains("history")
        || query.contains("before")
        || query.contains("之前")
        || query.contains("讨论")
        || query.contains("记得")
}

fn candidate_matches_query(candidate: &MemoryCandidateResponse, query: &str) -> bool {
    let query = normalize_text(query);
    if query.is_empty() {
        return true;
    }
    let haystack = normalize_text(&format!(
        "{} {} {}",
        candidate.summary, candidate.value, candidate.why_it_matters
    ));
    query
        .split_whitespace()
        .any(|token| token.len() >= 3 && haystack.contains(token))
}
```

- [ ] **Step 6: Run project context tests**

Run:

```powershell
cd D:\VSP\agentswap-gui\src-tauri
cargo test chatmem_memory::store::tests::project_context
```

Expected: PASS.

- [ ] **Step 7: Commit project context store**

```powershell
git add src-tauri/src/chatmem_memory/models.rs src-tauri/src/chatmem_memory/search.rs src-tauri/src/chatmem_memory/store.rs
git commit -m "feat: compose recall-aware project context"
```

## Task 6: Expose Project Context Through MCP And Tauri

**Files:**
- Modify: `src-tauri/src/chatmem_memory/mcp.rs`
- Modify: `src-tauri/src/main.rs`

- [ ] **Step 1: Add MCP route test**

In `src-tauri/src/chatmem_memory/mcp.rs`, update `mcp_tools_include_core_memory_surface` expected names to include:

```rust
ChatMemMcpService::get_project_context_tool_attr().name,
```

- [ ] **Step 2: Run failing MCP tool test**

Run:

```powershell
cd D:\VSP\agentswap-gui\src-tauri
cargo test chatmem_memory::mcp::tests::mcp_tools_include_core_memory_surface
```

Expected: FAIL because `get_project_context` route does not exist.

- [ ] **Step 3: Add MCP imports**

In `src-tauri/src/chatmem_memory/mcp.rs`, import:

```rust
GetProjectContextInput, ProjectContextPayload,
```

inside the existing `models::{...}` list.

- [ ] **Step 4: Add MCP route**

In `ChatMemMcpService::tool_router`, after `get_repo_memory`, add:

```rust
.with_route((Self::get_project_context_tool_attr(), Self::get_project_context))
```

Then add the tool method:

```rust
    #[tool(name = "get_project_context", description = "Return approved memory plus recall-aware local history evidence for a repository query")]
    async fn get_project_context(
        &self,
        Parameters(input): Parameters<GetProjectContextInput>,
    ) -> Result<Json<ProjectContextPayload>, McpError> {
        let _ = sync::sync_repo_conversations(&self.store, &input.repo_root);
        self.store
            .get_project_context(
                &input.repo_root,
                &input.query,
                input.intent.as_deref(),
                input.limit,
            )
            .map(Json)
            .map_err(|error| internal_error(error.to_string()))
    }
```

- [ ] **Step 5: Add Tauri command**

In `src-tauri/src/main.rs`, import `GetProjectContextInput` and `ProjectContextPayload`, then add:

```rust
#[command]
async fn get_project_context(
    repo_root: String,
    query: String,
    intent: Option<String>,
    limit: Option<usize>,
) -> Result<ProjectContextPayload, String> {
    let store = open_memory_store()?;
    store
        .get_project_context(&repo_root, &query, intent.as_deref(), limit)
        .map_err(|error| error.to_string())
}
```

Add `get_project_context` to `tauri::generate_handler![...]`.

- [ ] **Step 6: Run MCP tests**

Run:

```powershell
cd D:\VSP\agentswap-gui\src-tauri
cargo test chatmem_memory::mcp::tests::mcp_tools_include_core_memory_surface
```

Expected: PASS.

- [ ] **Step 7: Commit tool surface**

```powershell
git add src-tauri/src/chatmem_memory/mcp.rs src-tauri/src/main.rs
git commit -m "feat: expose project context retrieval"
```

## Task 7: Add Frontend Types, API, And Status Component

**Files:**
- Modify: `src/chatmem-memory/types.ts`
- Modify: `src/chatmem-memory/api.ts`
- Create: `src/components/ProjectIndexStatus.tsx`
- Create: `src/__tests__/ProjectIndexStatus.test.tsx`

- [ ] **Step 1: Add frontend types**

In `src/chatmem-memory/types.ts`, extend `RepoMemoryHealth`:

```ts
  indexed_chunk_count: number;
  repo_aliases: RepoAlias[];
```

Add these types after `RepoMemoryHealth`:

```ts
export type RepoAlias = {
  alias_root: string;
  alias_kind: string;
  confidence: number;
};

export type RepoScanReport = {
  repo_root: string;
  canonical_repo_root: string;
  scanned_conversation_count: number;
  linked_conversation_count: number;
  skipped_conversation_count: number;
  source_agents: AgentConversationCount[];
  warnings: string[];
};

export type ProjectContextPayload = {
  repo_summary: string;
  intent: string;
  approved_memories: ApprovedMemory[];
  priority_gotchas: ApprovedMemory[];
  recent_handoff: HandoffPacket | null;
  relevant_history: Array<{
    type: string;
    title: string;
    summary: string;
    why_matched: string;
    score: number;
    evidence_refs: EvidenceRef[];
  }>;
  pending_candidates: MemoryCandidate[];
  repo_diagnostics: RepoMemoryHealth;
};
```

- [ ] **Step 2: Add API wrappers**

In `src/chatmem-memory/api.ts`, import `ProjectContextPayload` and `RepoScanReport`, then add:

```ts
export function scanRepoConversations(repoRoot: string) {
  return invoke<RepoScanReport>("scan_repo_conversations", { repoRoot });
}

export function getProjectContext(payload: {
  repoRoot: string;
  query: string;
  intent?: string;
  limit?: number;
}) {
  return invoke<ProjectContextPayload>("get_project_context", payload);
}
```

- [ ] **Step 3: Create component test**

Create `src/__tests__/ProjectIndexStatus.test.tsx`:

```tsx
import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ProjectIndexStatus from "../components/ProjectIndexStatus";
import type { RepoMemoryHealth } from "../chatmem-memory/types";

const health: RepoMemoryHealth = {
  repo_root: "d:/vsp/agentswap-gui",
  canonical_repo_root: "d:/vsp/agentswap-gui",
  approved_memory_count: 2,
  pending_candidate_count: 3,
  search_document_count: 40,
  indexed_chunk_count: 31,
  inherited_repo_roots: ["d:/vsp"],
  conversation_counts_by_agent: [
    { source_agent: "codex", conversation_count: 7 },
    { source_agent: "claude", conversation_count: 2 },
  ],
  repo_aliases: [
    { alias_root: "d:/vsp", alias_kind: "ancestor", confidence: 0.61 },
  ],
  warnings: ["Some conversations are linked through an ancestor repo."],
};

describe("ProjectIndexStatus", () => {
  it("shows index counts and warnings", () => {
    render(
      <ProjectIndexStatus
        health={health}
        loading={false}
        scanning={false}
        locale="en"
        onScan={() => undefined}
      />,
    );

    expect(screen.getByText("Local history")).toBeTruthy();
    expect(screen.getByText("9")).toBeTruthy();
    expect(screen.getByText("31")).toBeTruthy();
    expect(screen.getByText("3")).toBeTruthy();
    expect(screen.getByText(/ancestor repo/i)).toBeTruthy();
  });

  it("calls scan action", () => {
    const onScan = vi.fn();
    render(
      <ProjectIndexStatus
        health={health}
        loading={false}
        scanning={false}
        locale="en"
        onScan={onScan}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "Rescan local history" }));
    expect(onScan).toHaveBeenCalledTimes(1);
  });
});
```

- [ ] **Step 4: Run failing component test**

Run:

```powershell
cd D:\VSP\agentswap-gui
npm.cmd run test:run -- src/__tests__/ProjectIndexStatus.test.tsx
```

Expected: FAIL because the component does not exist.

- [ ] **Step 5: Create component**

Create `src/components/ProjectIndexStatus.tsx`:

```tsx
import type { RepoMemoryHealth } from "../chatmem-memory/types";
import type { Locale } from "../i18n/strings";

type ProjectIndexStatusProps = {
  health: RepoMemoryHealth | null;
  loading: boolean;
  scanning: boolean;
  locale: Locale;
  onScan: () => void;
};

function totalConversations(health: RepoMemoryHealth | null) {
  return (
    health?.conversation_counts_by_agent.reduce(
      (total, item) => total + item.conversation_count,
      0,
    ) ?? 0
  );
}

export default function ProjectIndexStatus({
  health,
  loading,
  scanning,
  locale,
  onScan,
}: ProjectIndexStatusProps) {
  const isEnglish = locale === "en";
  const labels = {
    title: isEnglish ? "Local history" : "本地历史",
    conversations: isEnglish ? "Conversations" : "对话",
    chunks: isEnglish ? "Chunks" : "索引片段",
    pending: isEnglish ? "Pending memory" : "待确认记忆",
    approved: isEnglish ? "Approved memory" : "已确认记忆",
    rescan: isEnglish ? "Rescan local history" : "重新扫描本地历史",
    loading: isEnglish ? "Loading history status..." : "正在读取历史状态...",
    scanning: isEnglish ? "Scanning..." : "正在扫描...",
  };

  if (loading && !health) {
    return (
      <section className="project-index-status task-panel">
        <div className="task-panel-header">
          <div>
            <span className="task-panel-label">{labels.title}</span>
            <h2>{labels.loading}</h2>
          </div>
        </div>
      </section>
    );
  }

  return (
    <section className="project-index-status task-panel">
      <div className="task-panel-header">
        <div>
          <span className="task-panel-label">{labels.title}</span>
          <h2>{health?.canonical_repo_root ?? "--"}</h2>
        </div>
        <button type="button" className="btn btn-secondary" disabled={scanning} onClick={onScan}>
          {scanning ? labels.scanning : labels.rescan}
        </button>
      </div>

      <div className="project-index-grid">
        <div className="meta-block">
          <span className="meta-label">{labels.conversations}</span>
          <span className="meta-value">{totalConversations(health)}</span>
        </div>
        <div className="meta-block">
          <span className="meta-label">{labels.chunks}</span>
          <span className="meta-value">{health?.indexed_chunk_count ?? 0}</span>
        </div>
        <div className="meta-block">
          <span className="meta-label">{labels.pending}</span>
          <span className="meta-value">{health?.pending_candidate_count ?? 0}</span>
        </div>
        <div className="meta-block">
          <span className="meta-label">{labels.approved}</span>
          <span className="meta-value">{health?.approved_memory_count ?? 0}</span>
        </div>
      </div>

      {health?.warnings.length ? (
        <div className="project-index-warnings">
          {health.warnings.map((warning) => (
            <p key={warning}>{warning}</p>
          ))}
        </div>
      ) : null}
    </section>
  );
}
```

- [ ] **Step 6: Add CSS**

In `src/styles.css`, add:

```css
.project-index-status {
  gap: 16px;
}

.project-index-grid {
  display: grid;
  grid-template-columns: repeat(4, minmax(0, 1fr));
  gap: 12px;
}

.project-index-warnings {
  display: grid;
  gap: 8px;
  color: var(--text-muted);
  font-size: 0.9rem;
}

.project-index-warnings p {
  margin: 0;
}

@media (max-width: 760px) {
  .project-index-grid {
    grid-template-columns: repeat(2, minmax(0, 1fr));
  }
}
```

- [ ] **Step 7: Run component test**

Run:

```powershell
cd D:\VSP\agentswap-gui
npm.cmd run test:run -- src/__tests__/ProjectIndexStatus.test.tsx
```

Expected: PASS.

- [ ] **Step 8: Commit frontend component**

```powershell
git add src/chatmem-memory/types.ts src/chatmem-memory/api.ts src/components/ProjectIndexStatus.tsx src/__tests__/ProjectIndexStatus.test.tsx src/styles.css
git commit -m "feat: show ChatMem local history status"
```

## Task 8: Wire Project Status Into App

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/__tests__/MemoryWorkspace.test.tsx`

- [ ] **Step 1: Add failing integration test**

In `src/__tests__/MemoryWorkspace.test.tsx`, add a test using the existing mock setup style:

```tsx
it("loads project index health and triggers a local history scan", async () => {
  const user = userEvent.setup();

  render(<App />);

  await screen.findByText("Project Memory");
  expect(mockInvoke).toHaveBeenCalledWith("get_repo_memory_health", {
    repoRoot: "D:/VSP/agentswap-gui",
  });

  await user.click(screen.getByRole("button", { name: "Rescan local history" }));

  expect(mockInvoke).toHaveBeenCalledWith("scan_repo_conversations", {
    repoRoot: "D:/VSP/agentswap-gui",
  });
});
```

In the test mock, add command responses:

```ts
if (command === "get_repo_memory_health") {
  return {
    repo_root: "d:/vsp/agentswap-gui",
    canonical_repo_root: "d:/vsp/agentswap-gui",
    approved_memory_count: 1,
    pending_candidate_count: 2,
    search_document_count: 12,
    indexed_chunk_count: 8,
    inherited_repo_roots: [],
    conversation_counts_by_agent: [{ source_agent: "codex", conversation_count: 4 }],
    repo_aliases: [],
    warnings: [],
  };
}

if (command === "scan_repo_conversations") {
  return {
    repo_root: "d:/vsp/agentswap-gui",
    canonical_repo_root: "d:/vsp/agentswap-gui",
    scanned_conversation_count: 4,
    linked_conversation_count: 4,
    skipped_conversation_count: 0,
    source_agents: [{ source_agent: "codex", conversation_count: 4 }],
    warnings: [],
  };
}
```

- [ ] **Step 2: Run failing integration test**

Run:

```powershell
cd D:\VSP\agentswap-gui
npm.cmd run test:run -- src/__tests__/MemoryWorkspace.test.tsx
```

Expected: FAIL because App does not render the new status component.

- [ ] **Step 3: Import API and component**

In `src/App.tsx`, add:

```ts
import ProjectIndexStatus from "./components/ProjectIndexStatus";
```

Update the memory API import to include:

```ts
getRepoMemoryHealth,
scanRepoConversations,
```

Import type:

```ts
import type { RepoMemoryHealth } from "./chatmem-memory/types";
```

- [ ] **Step 4: Add state**

Near the existing memory state in `src/App.tsx`, add:

```ts
const [repoMemoryHealth, setRepoMemoryHealth] = useState<RepoMemoryHealth | null>(null);
const [repoHealthLoading, setRepoHealthLoading] = useState(false);
const [repoScanRunning, setRepoScanRunning] = useState(false);
```

- [ ] **Step 5: Load health with memory**

In the `useEffect` that loads project memory, replace the `Promise.all` call with:

```ts
const [nextMemories, nextCandidates, nextWikiPages, nextHealth] = await Promise.all([
  listRepoMemories(activeRepoRoot),
  listMemoryCandidates(activeRepoRoot, "pending_review"),
  rebuildRepoWiki(activeRepoRoot),
  getRepoMemoryHealth(activeRepoRoot),
]);
```

Then set:

```ts
setRepoMemoryHealth(nextHealth);
```

Set `repoHealthLoading` alongside `memoryLoading`:

```ts
setRepoHealthLoading(true);
```

and in `finally`:

```ts
setRepoHealthLoading(false);
```

When `!activeRepoRoot`, add:

```ts
setRepoMemoryHealth(null);
```

- [ ] **Step 6: Add scan handler**

Add this handler in `src/App.tsx`:

```ts
const handleScanRepoConversations = async () => {
  if (!activeRepoRoot) {
    return;
  }

  setRepoScanRunning(true);
  try {
    await scanRepoConversations(activeRepoRoot);
    const nextHealth = await getRepoMemoryHealth(activeRepoRoot);
    setRepoMemoryHealth(nextHealth);
  } catch (error) {
    console.error("Failed to scan repo conversations:", error);
  } finally {
    setRepoScanRunning(false);
  }
};
```

- [ ] **Step 7: Render status**

Render `ProjectIndexStatus` near the project context/memory surface:

```tsx
{activeRepoRoot ? (
  <ProjectIndexStatus
    health={repoMemoryHealth}
    loading={repoHealthLoading}
    scanning={repoScanRunning}
    locale={locale}
    onScan={() => void handleScanRepoConversations()}
  />
) : null}
```

- [ ] **Step 8: Run frontend test**

Run:

```powershell
cd D:\VSP\agentswap-gui
npm.cmd run test:run -- src/__tests__/MemoryWorkspace.test.tsx
```

Expected: PASS.

- [ ] **Step 9: Commit App wiring**

```powershell
git add src/App.tsx src/__tests__/MemoryWorkspace.test.tsx
git commit -m "feat: wire project history status"
```

## Task 9: Update ChatMem Skill Guidance

**Files:**
- Modify: `skills/chatmem/SKILL.md`
- Modify: `src/__tests__/integrationManifests.test.ts`

- [ ] **Step 1: Add failing manifest test expectation**

In `src/__tests__/integrationManifests.test.ts`, add:

```ts
expect(skillDoc).toContain("get_project_context");
expect(skillDoc).toContain("history evidence");
expect(skillDoc).toContain("approved memory");
```

- [ ] **Step 2: Run failing manifest test**

Run:

```powershell
cd D:\VSP\agentswap-gui
npm.cmd run test:run -- src/__tests__/integrationManifests.test.ts
```

Expected: FAIL because the skill does not mention `get_project_context`.

- [ ] **Step 3: Update MCP-first workflow**

In `skills/chatmem/SKILL.md`, replace workflow steps 2 to 5 with:

```markdown
2. Call `get_project_context` for substantial repo work when available. Use `intent="startup"` for compact startup context, `intent="recall"` when the user asks whether something was discussed before, and `intent="continue_work"` when resuming.
3. Treat approved memory as durable project guidance. Treat history evidence as local evidence that may be stale or unapproved.
4. If `get_project_context` is unavailable, fall back to `get_repo_memory` and then call `search_repo_history` for specific gaps: prior decisions, commands, key files, errors, earlier attempts, or recall questions.
5. Use the smallest useful context. Prefer approved memory, generated wiki pages, checkpoints, handoffs, targeted history evidence, and pending candidate summaries over replaying raw conversation logs.
```

Add this rule under Retrieval Rule:

```markdown
For recall questions, never answer from `get_repo_memory` alone. If approved memory does not contain the answer, search history and clearly label matches as history evidence rather than approved memory.
```

- [ ] **Step 4: Run manifest test**

Run:

```powershell
cd D:\VSP\agentswap-gui
npm.cmd run test:run -- src/__tests__/integrationManifests.test.ts
```

Expected: PASS.

- [ ] **Step 5: Commit skill guidance**

```powershell
git add skills/chatmem/SKILL.md src/__tests__/integrationManifests.test.ts
git commit -m "docs: guide ChatMem agents to project context"
```

## Task 10: Run Phase 1 Verification

**Files:**
- No new files.

- [ ] **Step 1: Run Rust memory tests**

Run:

```powershell
cd D:\VSP\agentswap-gui\src-tauri
cargo test chatmem_memory::
```

Expected: PASS.

- [ ] **Step 2: Run frontend tests touched by this plan**

Run:

```powershell
cd D:\VSP\agentswap-gui
npm.cmd run test:run -- src/__tests__/ProjectIndexStatus.test.tsx src/__tests__/MemoryWorkspace.test.tsx src/__tests__/integrationManifests.test.ts
```

Expected: PASS.

- [ ] **Step 3: Run full frontend test suite if targeted tests pass**

Run:

```powershell
cd D:\VSP\agentswap-gui
npm.cmd run test:run
```

Expected: PASS.

- [ ] **Step 4: Inspect final diff**

Run:

```powershell
git status --short
git log --oneline -10
```

Expected: `git status --short` is empty after all task commits. The last commits should match the task commits in this plan.

## Phase 1 Acceptance Criteria

- `read_conversation` and repo scans persist chunk-level search documents.
- `search_history` can find important late-conversation content that appears after the first 12 messages.
- `get_project_context` returns approved memory, relevant history, pending candidates, and diagnostics.
- Recall intent searches history even when approved memory is empty.
- Tauri and MCP both expose `get_project_context`.
- Project UI displays local history status and can trigger a scan.
- ChatMem skill tells future agents not to answer recall questions from approved memory alone.
