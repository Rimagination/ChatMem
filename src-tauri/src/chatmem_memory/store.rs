use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use agentswap_core::types::{ChangeType, Conversation, Role, ToolStatus};

use super::{
    checkpoints::{CheckpointRecord, CreateCheckpointInput},
    chunks,
    db,
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

#[derive(Debug, Clone)]
pub struct MemoryStore {
    db_path: PathBuf,
}

#[derive(Debug, Clone)]
pub enum ReviewAction {
    Approve { title: String, usage_hint: String },
    ApproveWithEdit { title: String, value: String, usage_hint: String },
    ApproveMerge {
        memory_id: String,
        title: String,
        value: String,
        usage_hint: String,
    },
    Reject,
    Snooze,
}

impl MemoryStore {
    pub fn open_app() -> Result<Self> {
        let path = db::default_db_path()?;
        Self::new(path)
    }

    pub fn new(db_path: PathBuf) -> Result<Self> {
        let _ = db::open_connection(&db_path)?;
        Ok(Self { db_path })
    }

    pub(crate) fn conn(&self) -> Result<Connection> {
        db::open_connection(&self.db_path)
    }

    pub fn ensure_repo(&self, repo_root: &str) -> Result<String> {
        let repo_root = repo_identity::canonical_repo_root(repo_root);
        let repo_id = repo_identity::fingerprint_repo(&repo_root, None, None);
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn()?;

        conn.execute(
            "INSERT INTO repos (
                repo_id, repo_root, repo_fingerprint, git_remote, default_branch, created_at, updated_at
             ) VALUES (?1, ?2, ?3, NULL, NULL, ?4, ?4)
             ON CONFLICT(repo_id) DO UPDATE SET repo_root = excluded.repo_root, updated_at = excluded.updated_at",
            params![repo_id, repo_root, repo_id, now],
        )?;

        Ok(repo_id)
    }

    pub fn repo_root_for_id(&self, repo_id: &str) -> Result<String> {
        let conn = self.conn()?;
        conn.query_row(
            "SELECT repo_root FROM repos WHERE repo_id = ?1",
            [repo_id],
            |row| row.get::<_, String>(0),
        )
        .context("repository not found")
    }

    pub fn upsert_conversation_snapshot(
        &self,
        agent: &str,
        conversation: &Conversation,
        storage_path: Option<String>,
    ) -> Result<String> {
        let repo_id = self.ensure_repo(&conversation.project_dir)?;
        let conversation_id = format!("{agent}:{}", conversation.id);
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;

        tx.execute(
            "INSERT INTO conversations (
                conversation_id, repo_id, source_agent, source_conversation_id, summary,
                started_at, updated_at, storage_path
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
             ON CONFLICT(conversation_id) DO UPDATE SET
                repo_id = excluded.repo_id,
                summary = excluded.summary,
                updated_at = excluded.updated_at,
                storage_path = excluded.storage_path",
            params![
                conversation_id,
                repo_id,
                agent,
                conversation.id,
                conversation.summary,
                conversation.created_at.to_rfc3339(),
                conversation.updated_at.to_rfc3339(),
                storage_path,
            ],
        )?;

        tx.execute("DELETE FROM tool_calls WHERE message_id IN (SELECT message_id FROM messages WHERE conversation_id = ?1)", [conversation_id.clone()])?;
        tx.execute("DELETE FROM messages WHERE conversation_id = ?1", [conversation_id.clone()])?;
        tx.execute("DELETE FROM file_changes WHERE conversation_id = ?1", [conversation_id.clone()])?;
        tx.execute(
            "DELETE FROM conversation_chunks WHERE conversation_id = ?1",
            [conversation_id.clone()],
        )?;
        tx.execute(
            "DELETE FROM search_documents_fts
             WHERE doc_id IN (
                SELECT doc_id FROM search_documents
                WHERE repo_id = ?1
                  AND doc_type = 'chunk'
                  AND doc_ref_id LIKE ?2
             )",
            params![repo_id.clone(), format!("{conversation_id}:%")],
        )?;
        tx.execute(
            "DELETE FROM search_documents
             WHERE repo_id = ?1
               AND doc_type = 'chunk'
               AND doc_ref_id LIKE ?2",
            params![repo_id.clone(), format!("{conversation_id}:%")],
        )?;

        for message in &conversation.messages {
            let message_id = format!("{conversation_id}:{}", message.id);
            let role = match message.role {
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::System => "system",
            };

            tx.execute(
                "INSERT INTO messages (message_id, conversation_id, role, content, timestamp)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    message_id,
                    conversation_id,
                    role,
                    message.content,
                    message.timestamp.to_rfc3339(),
                ],
            )?;

            for (index, tool_call) in message.tool_calls.iter().enumerate() {
                let tool_call_id = format!("{message_id}:tool:{index}");
                let status = match tool_call.status {
                    ToolStatus::Success => "success",
                    ToolStatus::Error => "error",
                };
                tx.execute(
                    "INSERT INTO tool_calls (tool_call_id, message_id, name, input_json, output_text, status)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                    params![
                        tool_call_id,
                        message_id,
                        tool_call.name,
                        serde_json::to_string(&tool_call.input).unwrap_or_else(|_| "{}".to_string()),
                        tool_call.output,
                        status,
                    ],
                )?;
            }
        }

        for (index, file_change) in conversation.file_changes.iter().enumerate() {
            let file_change_id = format!("{conversation_id}:change:{index}");
            let message_id = format!("{conversation_id}:{}", file_change.message_id);
            let change_type = match file_change.change_type {
                ChangeType::Created => "created",
                ChangeType::Modified => "modified",
                ChangeType::Deleted => "deleted",
            };

            tx.execute(
                "INSERT INTO file_changes (file_change_id, conversation_id, message_id, path, change_type, timestamp)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![
                    file_change_id,
                    conversation_id,
                    message_id,
                    file_change.path,
                    change_type,
                    file_change.timestamp.to_rfc3339(),
                ],
            )?;
        }

        let title = conversation
            .summary
            .clone()
            .unwrap_or_else(|| conversation.id.clone());
        let search_body = build_conversation_search_body(conversation);
        let search_doc_id = format!("conversation:{conversation_id}");
        upsert_search_document_tx(
            &tx,
            &search_doc_id,
            &repo_id,
            "conversation",
            &conversation_id,
            &title,
            &search_body,
        )?;
        let chunk_rows = chunks::build_conversation_chunks(&conversation_id, conversation);
        let chunk_now = chrono::Utc::now().to_rfc3339();
        for chunk in chunk_rows {
            let chunk_ref_id = format!("{conversation_id}:{}", chunk.chunk_id_suffix);
            let chunk_id = format!("chunk:{chunk_ref_id}");
            let message_ids_json = serde_json::to_string(&chunk.message_ids)?;
            let chunk_title = chunk.title;
            let chunk_body = chunk.body;
            tx.execute(
                "INSERT INTO conversation_chunks (
                    chunk_id, repo_id, conversation_id, chunk_type, title, body,
                    message_ids_json, ordinal, token_estimate, created_at, updated_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
                params![
                    &chunk_id,
                    &repo_id,
                    &conversation_id,
                    chunk.chunk_type,
                    &chunk_title,
                    &chunk_body,
                    message_ids_json,
                    chunk.ordinal as i64,
                    chunk.token_estimate as i64,
                    &chunk_now,
                ],
            )?;
            upsert_search_document_tx(
                &tx,
                &chunk_id,
                &repo_id,
                "chunk",
                &chunk_ref_id,
                &chunk_title,
                &chunk_body,
            )?;
        }

        let episode_id = format!("episode:{conversation_id}");
        let episode_summary = summarize_conversation(conversation);
        tx.execute(
            "INSERT INTO episodes (
                episode_id, repo_id, title, summary, outcome, created_at, source_conversation_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
             ON CONFLICT(episode_id) DO UPDATE SET
                title = excluded.title,
                summary = excluded.summary,
                outcome = excluded.outcome,
                created_at = excluded.created_at",
            params![
                episode_id,
                repo_id,
                title,
                episode_summary,
                "captured",
                conversation.updated_at.to_rfc3339(),
                conversation_id,
            ],
        )?;

        upsert_search_document_tx(
            &tx,
            &format!("episode:{conversation_id}"),
            &repo_id,
            "episode",
            &episode_id,
            &title,
            &episode_summary,
        )?;

        let excerpt = search_body.chars().take(240).collect::<String>();
        tx.execute("DELETE FROM evidence_refs WHERE owner_type = 'episode' AND owner_id = ?1", [episode_id.clone()])?;
        tx.execute(
            "INSERT INTO evidence_refs (
                evidence_id, owner_type, owner_id, conversation_id, message_id, tool_call_id, file_change_id, excerpt, created_at
             ) VALUES (?1, 'episode', ?2, ?3, NULL, NULL, NULL, ?4, ?5)",
            params![
                uuid::Uuid::new_v4().to_string(),
                episode_id,
                conversation_id,
                excerpt,
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;
        extract_memory_candidates_from_conversation_tx(
            &tx,
            &repo_id,
            &conversation_id,
            conversation,
        )?;

        tx.commit()?;

        Ok(repo_id)
    }

    pub fn create_candidate(&self, input: &CreateMemoryCandidateInput) -> Result<String> {
        let repo_id = self.ensure_repo(&input.repo_root)?;
        let candidate_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;

        tx.execute(
            "INSERT INTO memory_candidates (
                candidate_id, repo_id, kind, summary, value, why_it_matters,
                confidence, proposed_by, status, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pending_review', ?9)",
            params![
                candidate_id,
                repo_id,
                input.kind,
                input.summary,
                input.value,
                input.why_it_matters,
                input.confidence,
                input.proposed_by,
                now,
            ],
        )?;

        replace_evidence_refs_tx(&tx, "candidate", &candidate_id, &input.evidence_refs)?;
        record_candidate_conflicts_tx(
            &tx,
            &repo_id,
            &candidate_id,
            &input.kind,
            &input.summary,
            &input.value,
            &input.why_it_matters,
        )?;
        tx.commit()?;

        Ok(candidate_id)
    }

    pub fn propose_memory_merge(&self, input: &CreateMemoryMergeProposalInput) -> Result<String> {
        let repo_id = self.ensure_repo(&input.repo_root)?;
        let proposal_id = uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_URL,
            format!(
                "chatmem:merge-proposal:{}:{}",
                input.candidate_id, input.target_memory_id
            )
            .as_bytes(),
        )
        .to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;

        let (candidate_repo_id, candidate_kind, candidate_status) = tx
            .query_row(
                "SELECT repo_id, kind, status FROM memory_candidates WHERE candidate_id = ?1",
                [&input.candidate_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .with_context(|| format!("candidate {} not found", input.candidate_id))?;
        if candidate_repo_id != repo_id {
            anyhow::bail!("candidate belongs to a different repository");
        }
        if candidate_status != "pending_review" {
            anyhow::bail!("candidate must be pending_review before a merge can be proposed");
        }

        let (memory_repo_id, memory_kind) = tx
            .query_row(
                "SELECT repo_id, kind FROM approved_memories WHERE memory_id = ?1 AND status = 'active'",
                [&input.target_memory_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)),
            )
            .with_context(|| format!("active memory {} not found", input.target_memory_id))?;
        if memory_repo_id != repo_id {
            anyhow::bail!("target memory belongs to a different repository");
        }
        if memory_kind != candidate_kind {
            anyhow::bail!("candidate and target memory kinds do not match");
        }

        tx.execute(
            "INSERT INTO memory_merge_proposals (
                proposal_id, repo_id, candidate_id, target_memory_id,
                proposed_title, proposed_value, proposed_usage_hint, risk_note,
                proposed_by, status, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 'pending_review', ?10, ?10)
             ON CONFLICT(candidate_id, target_memory_id) DO UPDATE SET
                proposed_title = excluded.proposed_title,
                proposed_value = excluded.proposed_value,
                proposed_usage_hint = excluded.proposed_usage_hint,
                risk_note = excluded.risk_note,
                proposed_by = excluded.proposed_by,
                status = 'pending_review',
                updated_at = excluded.updated_at",
            params![
                proposal_id,
                repo_id,
                input.candidate_id,
                input.target_memory_id,
                input.proposed_title,
                input.proposed_value,
                input.proposed_usage_hint,
                input.risk_note,
                input.proposed_by,
                now,
            ],
        )?;
        replace_evidence_refs_tx(
            &tx,
            "memory_merge_proposal",
            &proposal_id,
            &input.evidence_refs,
        )?;
        tx.commit()?;

        Ok(proposal_id)
    }

    pub fn review_candidate(&self, candidate_id: &str, review: ReviewAction) -> Result<()> {
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;

        let candidate = tx
            .query_row(
                "SELECT repo_id, kind, summary, value, why_it_matters FROM memory_candidates WHERE candidate_id = ?1",
                [candidate_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .with_context(|| format!("candidate {candidate_id} not found"))?;

        let now = chrono::Utc::now().to_rfc3339();
        match review {
            ReviewAction::Approve { title, usage_hint } => {
                let memory_id = uuid::Uuid::new_v4().to_string();
                tx.execute(
                    "INSERT INTO approved_memories (
                        memory_id, repo_id, kind, title, value, usage_hint, status,
                        last_verified_at, freshness_status, freshness_score,
                        verified_at, verified_by, created_from_candidate_id, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'active', ?7, 'fresh', 1.0, ?7, NULL, ?8, ?7, ?7)",
                    params![
                        memory_id,
                        candidate.0,
                        candidate.1,
                        title,
                        candidate.3,
                        usage_hint,
                        now,
                        candidate_id,
                    ],
                )?;
                tx.execute(
                    "UPDATE memory_candidates SET status = 'approved', reviewed_at = ?2 WHERE candidate_id = ?1",
                    params![candidate_id, now],
                )?;
                tx.execute(
                    "UPDATE memory_merge_proposals
                     SET status = 'superseded', updated_at = ?2
                     WHERE candidate_id = ?1 AND status = 'pending_review'",
                    params![candidate_id, now],
                )?;
                let evidence = load_evidence_refs_from_conn(&tx, "candidate", candidate_id)?;
                replace_evidence_refs_tx(&tx, "memory", &memory_id, &evidence)?;
                upsert_search_document_tx(
                    &tx,
                    &format!("memory:{memory_id}"),
                    &candidate.0,
                    "memory",
                    &memory_id,
                    &title,
                    &format!("{}\n\n{}", candidate.3, usage_hint),
                )?;
            }
            ReviewAction::ApproveWithEdit {
                title,
                value,
                usage_hint,
            } => {
                let memory_id = uuid::Uuid::new_v4().to_string();
                tx.execute(
                    "INSERT INTO approved_memories (
                        memory_id, repo_id, kind, title, value, usage_hint, status,
                        last_verified_at, freshness_status, freshness_score,
                        verified_at, verified_by, created_from_candidate_id, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'active', ?7, 'fresh', 1.0, ?7, NULL, ?8, ?7, ?7)",
                    params![
                        memory_id,
                        candidate.0,
                        candidate.1,
                        title,
                        value,
                        usage_hint,
                        now,
                        candidate_id,
                    ],
                )?;
                tx.execute(
                    "UPDATE memory_candidates SET status = 'approved', reviewed_at = ?2 WHERE candidate_id = ?1",
                    params![candidate_id, now],
                )?;
                tx.execute(
                    "UPDATE memory_merge_proposals
                     SET status = 'superseded', updated_at = ?2
                     WHERE candidate_id = ?1 AND status = 'pending_review'",
                    params![candidate_id, now],
                )?;
                let evidence = load_evidence_refs_from_conn(&tx, "candidate", candidate_id)?;
                replace_evidence_refs_tx(&tx, "memory", &memory_id, &evidence)?;
                upsert_search_document_tx(
                    &tx,
                    &format!("memory:{memory_id}"),
                    &candidate.0,
                    "memory",
                    &memory_id,
                    &title,
                    &format!("{value}\n\n{usage_hint}"),
                )?;
            }
            ReviewAction::ApproveMerge {
                memory_id,
                title,
                value,
                usage_hint,
            } => {
                let memory_repo_id = tx
                    .query_row(
                        "SELECT repo_id FROM approved_memories WHERE memory_id = ?1",
                        [&memory_id],
                        |row| row.get::<_, String>(0),
                    )
                    .with_context(|| format!("memory {memory_id} not found"))?;
                if memory_repo_id != candidate.0 {
                    anyhow::bail!("candidate and memory belong to different repositories");
                }

                tx.execute(
                    "UPDATE approved_memories
                     SET title = ?2,
                         value = ?3,
                         usage_hint = ?4,
                         last_verified_at = ?5,
                         freshness_status = 'fresh',
                         freshness_score = 1.0,
                         verified_at = ?5,
                         verified_by = 'merge_review',
                         updated_at = ?5
                     WHERE memory_id = ?1",
                    params![memory_id, title, value, usage_hint, now],
                )?;
                tx.execute(
                    "UPDATE memory_candidates SET status = 'approved', reviewed_at = ?2 WHERE candidate_id = ?1",
                    params![candidate_id, now],
                )?;
                tx.execute(
                    "UPDATE memory_conflicts
                     SET status = 'resolved', resolved_at = ?3
                     WHERE candidate_id = ?1 AND memory_id = ?2 AND status = 'open'",
                    params![candidate_id, memory_id, now],
                )?;
                tx.execute(
                    "UPDATE memory_merge_proposals
                     SET status = CASE WHEN target_memory_id = ?2 THEN 'approved' ELSE 'superseded' END,
                         updated_at = ?3
                     WHERE candidate_id = ?1 AND status = 'pending_review'",
                    params![candidate_id, memory_id, now],
                )?;
                let candidate_evidence = load_evidence_refs_from_conn(&tx, "candidate", candidate_id)?;
                append_evidence_refs_tx(&tx, "memory", &memory_id, &candidate_evidence)?;
                upsert_search_document_tx(
                    &tx,
                    &format!("memory:{memory_id}"),
                    &candidate.0,
                    "memory",
                    &memory_id,
                    &title,
                    &format!("{value}\n\n{usage_hint}"),
                )?;
            }
            ReviewAction::Reject => {
                tx.execute(
                    "UPDATE memory_candidates SET status = 'rejected', reviewed_at = ?2 WHERE candidate_id = ?1",
                    params![candidate_id, now],
                )?;
                tx.execute(
                    "UPDATE memory_merge_proposals
                     SET status = 'rejected', updated_at = ?2
                     WHERE candidate_id = ?1 AND status = 'pending_review'",
                    params![candidate_id, now],
                )?;
            }
            ReviewAction::Snooze => {
                tx.execute(
                    "UPDATE memory_candidates SET status = 'snoozed', reviewed_at = ?2 WHERE candidate_id = ?1",
                    params![candidate_id, now],
                )?;
            }
        }

        tx.commit()?;
        Ok(())
    }

    pub fn reverify_memory(&self, memory_id: &str, verified_by: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        let memory = tx
            .query_row(
                "SELECT repo_id, title, value
                 FROM approved_memories
                 WHERE memory_id = ?1",
                [memory_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;

        let Some((repo_id, title, value)) = memory else {
            return Err(anyhow::anyhow!("memory {memory_id} not found"));
        };

        tx.execute(
            "UPDATE approved_memories
             SET last_verified_at = ?2,
                 freshness_status = 'fresh',
                 freshness_score = 1.0,
                 verified_at = ?2,
                 verified_by = ?3,
                 updated_at = ?2
             WHERE memory_id = ?1",
            params![memory_id, now, verified_by],
        )?;

        upsert_search_document_tx(
            &tx,
            &format!("memory:{memory_id}"),
            &repo_id,
            "memory",
            memory_id,
            &title,
            &value,
        )?;

        tx.commit()?;
        Ok(())
    }

    pub fn suggest_memory_merges(&self, repo_root: &str) -> Result<Vec<MemoryMergeSuggestion>> {
        let repo_id = self.ensure_repo(repo_root)?;
        self.suggest_memory_merges_by_repo_id(&repo_id)
    }

    pub fn list_memory_conflicts(
        &self,
        repo_root: &str,
        status: Option<&str>,
    ) -> Result<Vec<MemoryConflictResponse>> {
        let repo_id = self.ensure_repo(repo_root)?;
        self.list_memory_conflicts_by_repo_id(&repo_id, status)
    }

    pub fn list_entity_graph(&self, repo_root: &str, limit: usize) -> Result<EntityGraphPayload> {
        let repo_id = self.ensure_repo(repo_root)?;
        let conn = self.conn()?;
        let limit = limit.max(1);

        let mut entity_stmt = conn.prepare(
            "SELECT e.entity_id, e.name, e.kind, COUNT(l.link_id) AS mention_count
             FROM memory_entities e
             LEFT JOIN memory_entity_links l ON l.entity_id = e.entity_id
             WHERE e.repo_id = ?1
             GROUP BY e.entity_id, e.name, e.kind
             ORDER BY mention_count DESC, e.updated_at DESC
             LIMIT ?2",
        )?;
        let entities = entity_stmt
            .query_map(params![repo_id.clone(), limit as i64], |row| {
                Ok(EntityNodeResponse {
                    entity_id: row.get(0)?,
                    name: row.get(1)?,
                    kind: row.get(2)?,
                    mention_count: row.get::<_, i64>(3)? as usize,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let entity_ids = entities
            .iter()
            .map(|entity| entity.entity_id.clone())
            .collect::<HashSet<_>>();

        let mut link_stmt = conn.prepare(
            "SELECT l.entity_id, e.name, l.owner_type, l.owner_id, l.relationship,
                    COALESCE(sd.title, l.owner_id) AS source_title
             FROM memory_entity_links l
             JOIN memory_entities e ON e.entity_id = l.entity_id
             LEFT JOIN search_documents sd
               ON sd.repo_id = l.repo_id
              AND sd.doc_ref_id = l.owner_id
              AND (
                    (l.owner_type = 'memory' AND sd.doc_type = 'memory')
                 OR (l.owner_type = 'episode' AND sd.doc_type = 'episode')
                 OR (l.owner_type = 'wiki_page' AND sd.doc_type = 'wiki')
                 OR (l.owner_type = 'conversation' AND sd.doc_type = 'conversation')
              )
             WHERE l.repo_id = ?1
             ORDER BY l.created_at DESC
             LIMIT ?2",
        )?;
        let links = link_stmt
            .query_map(params![repo_id, (limit * 4) as i64], |row| {
                Ok(EntityLinkResponse {
                    entity_id: row.get(0)?,
                    entity_name: row.get(1)?,
                    owner_type: row.get(2)?,
                    owner_id: row.get(3)?,
                    relationship: row.get(4)?,
                    source_title: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .filter(|link| entity_ids.contains(&link.entity_id))
            .collect::<Vec<_>>();

        Ok(EntityGraphPayload { entities, links })
    }

    pub fn list_repo_memories(&self, repo_root: &str) -> Result<Vec<ApprovedMemoryResponse>> {
        let repo_id = self.ensure_repo(repo_root)?;
        let conn = self.conn()?;
        let memories = self.list_repo_memories_by_repo_id(&conn, &repo_id)?;
        if !memories.is_empty() {
            return Ok(memories);
        }

        for (ancestor_repo_id, ancestor_repo_root) in ancestor_repo_roots_from_conn(&conn, repo_root)? {
            let mut inherited = self.list_repo_memories_by_repo_id(&conn, &ancestor_repo_id)?;
            if inherited.is_empty() {
                continue;
            }
            for memory in &mut inherited {
                memory.selected_because =
                    Some(format!("Inherited from ancestor repo {ancestor_repo_root}"));
            }
            return Ok(inherited);
        }

        Ok(memories)
    }

    fn list_repo_memories_by_repo_id(
        &self,
        conn: &Connection,
        repo_id: &str,
    ) -> Result<Vec<ApprovedMemoryResponse>> {
        let mut stmt = conn.prepare(
            "SELECT memory_id, kind, title, value, usage_hint, status, last_verified_at,
                    freshness_status, freshness_score, verified_at, verified_by
             FROM approved_memories
             WHERE repo_id = ?1
             ORDER BY updated_at DESC",
        )?;

        let rows = stmt.query_map([repo_id], |row| {
            let last_verified_at = row.get::<_, Option<String>>(6)?;
            let verified_at = row.get::<_, Option<String>>(9)?;
            let (freshness_status, freshness_score) =
                evaluate_memory_freshness(last_verified_at.as_deref(), verified_at.as_deref());

            Ok(ApprovedMemoryResponse {
                memory_id: row.get(0)?,
                kind: row.get(1)?,
                title: row.get(2)?,
                value: row.get(3)?,
                usage_hint: row.get(4)?,
                status: row.get(5)?,
                last_verified_at,
                freshness_status,
                freshness_score,
                verified_at,
                verified_by: row.get(10)?,
                selected_because: None,
                evidence_refs: vec![],
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
            .and_then(|mut memories| {
                for memory in &mut memories {
                    memory.evidence_refs = self.evidence_refs("memory", &memory.memory_id)?;
                }
                Ok(memories)
            })
    }

    pub fn repo_memory_health(&self, repo_root: &str) -> Result<RepoMemoryHealthResponse> {
        let repo_id = self.ensure_repo(repo_root)?;
        let canonical_repo_root = self.repo_root_for_id(&repo_id)?;
        let conn = self.conn()?;

        let approved_memory_count = count_table_rows(
            &conn,
            "approved_memories",
            &repo_id,
            Some(("status", "active")),
        )?;
        let pending_candidate_count = count_table_rows(
            &conn,
            "memory_candidates",
            &repo_id,
            Some(("status", "pending_review")),
        )?;
        let search_document_count = count_table_rows(&conn, "search_documents", &repo_id, None)?;

        let mut inherited_repo_roots = Vec::new();
        for (ancestor_repo_id, ancestor_repo_root) in
            ancestor_repo_roots_from_conn(&conn, &canonical_repo_root)?
        {
            let ancestor_memory_count = count_table_rows(
                &conn,
                "approved_memories",
                &ancestor_repo_id,
                Some(("status", "active")),
            )?;
            if ancestor_memory_count > 0 {
                inherited_repo_roots.push(ancestor_repo_root);
            }
        }

        let mut stmt = conn.prepare(
            "SELECT source_agent, COUNT(*)
             FROM conversations
             WHERE repo_id = ?1
             GROUP BY source_agent
             ORDER BY source_agent ASC",
        )?;
        let conversation_counts_by_agent = stmt
            .query_map([repo_id.clone()], |row| {
                Ok(AgentConversationCount {
                    source_agent: row.get(0)?,
                    conversation_count: row.get::<_, i64>(1)? as usize,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut warnings = Vec::new();
        if approved_memory_count == 0 && !inherited_repo_roots.is_empty() {
            warnings.push(format!(
                "No approved memories exist for this repo, but ancestor repo memory exists in {}.",
                inherited_repo_roots.join(", ")
            ));
        }
        if pending_candidate_count > 0 {
            warnings.push(format!(
                "{pending_candidate_count} pending memory candidate(s) need review before they become startup memory."
            ));
        }
        if search_document_count == 0 && inherited_repo_roots.is_empty() {
            warnings.push("No searchable ChatMem documents are indexed for this repo yet.".to_string());
        }

        Ok(RepoMemoryHealthResponse {
            repo_root: repo_identity::normalize_repo_root(repo_root),
            canonical_repo_root,
            approved_memory_count,
            pending_candidate_count,
            search_document_count,
            inherited_repo_roots,
            conversation_counts_by_agent,
            warnings,
        })
    }

    pub fn list_candidates(&self, repo_root: &str) -> Result<Vec<MemoryCandidateResponse>> {
        let repo_id = self.ensure_repo(repo_root)?;
        self.list_candidates_by_repo_id(&repo_id, None)
    }

    pub fn list_candidates_with_status(
        &self,
        repo_root: &str,
        status: Option<&str>,
    ) -> Result<Vec<MemoryCandidateResponse>> {
        let repo_id = self.ensure_repo(repo_root)?;
        self.list_candidates_by_repo_id(&repo_id, status)
    }

    fn list_candidates_by_repo_id(
        &self,
        repo_id: &str,
        status: Option<&str>,
    ) -> Result<Vec<MemoryCandidateResponse>> {
        let conn = self.conn()?;
        let mut sql = String::from(
            "SELECT candidate_id, kind, summary, value, why_it_matters, confidence, proposed_by, status, created_at
             FROM memory_candidates
             WHERE repo_id = ?1",
        );
        if status.is_some() {
            sql.push_str(" AND status = ?2");
        }
        sql.push_str(" ORDER BY created_at DESC");

        let mut stmt = conn.prepare(&sql)?;
        let rows = if let Some(status) = status {
            stmt.query_map(params![repo_id, status], |row| {
                Ok(MemoryCandidateResponse {
                    candidate_id: row.get(0)?,
                    kind: row.get(1)?,
                    summary: row.get(2)?,
                    value: row.get(3)?,
                    why_it_matters: row.get(4)?,
                    confidence: row.get(5)?,
                    proposed_by: row.get(6)?,
                    status: row.get(7)?,
                    created_at: row.get(8)?,
                    evidence_refs: vec![],
                    merge_suggestion: None,
                    conflict_suggestion: None,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?
        } else {
            stmt.query_map(params![repo_id], |row| {
                Ok(MemoryCandidateResponse {
                    candidate_id: row.get(0)?,
                    kind: row.get(1)?,
                    summary: row.get(2)?,
                    value: row.get(3)?,
                    why_it_matters: row.get(4)?,
                    confidence: row.get(5)?,
                    proposed_by: row.get(6)?,
                    status: row.get(7)?,
                    created_at: row.get(8)?,
                    evidence_refs: vec![],
                    merge_suggestion: None,
                    conflict_suggestion: None,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?
        };

        let merge_suggestions = self
            .suggest_memory_merges_by_repo_id(repo_id)?
            .into_iter()
            .map(|suggestion| (suggestion.candidate_id.clone(), suggestion))
            .collect::<HashMap<_, _>>();
        let agent_merge_proposals = self
            .list_pending_memory_merge_proposals_by_repo_id(repo_id)?
            .into_iter()
            .map(|proposal| (proposal.candidate_id.clone(), proposal))
            .collect::<HashMap<_, _>>();
        let conflict_suggestions = self
            .list_memory_conflicts_by_repo_id(repo_id, Some("open"))?
            .into_iter()
            .map(|conflict| (conflict.candidate_id.clone(), conflict))
            .collect::<HashMap<_, _>>();
        let mut candidates = rows;
        for candidate in &mut candidates {
            candidate.evidence_refs = self.evidence_refs("candidate", &candidate.candidate_id)?;
            candidate.merge_suggestion = agent_merge_proposals
                .get(&candidate.candidate_id)
                .or_else(|| merge_suggestions.get(&candidate.candidate_id))
                .cloned();
            candidate.conflict_suggestion =
                conflict_suggestions.get(&candidate.candidate_id).cloned();
        }
        Ok(candidates)
    }

    fn list_pending_memory_merge_proposals_by_repo_id(
        &self,
        repo_id: &str,
    ) -> Result<Vec<MemoryMergeSuggestion>> {
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT p.proposal_id, p.candidate_id, p.target_memory_id, m.title,
                    p.proposed_title, p.proposed_value, p.proposed_usage_hint,
                    p.risk_note, p.proposed_by, p.created_at
             FROM memory_merge_proposals p
             JOIN approved_memories m ON m.memory_id = p.target_memory_id
             WHERE p.repo_id = ?1
               AND p.status = 'pending_review'
               AND m.status = 'active'
             ORDER BY p.updated_at DESC",
        )?;
        let rows = stmt.query_map([repo_id], |row| {
            let proposed_by = row.get::<_, String>(8)?;
            Ok(MemoryMergeSuggestion {
                proposal_id: Some(row.get(0)?),
                candidate_id: row.get(1)?,
                memory_id: row.get(2)?,
                memory_title: row.get(3)?,
                reason: format!("来自 {proposed_by} 的 agent-authored merge proposal；批准前请先复核。"),
                proposed_title: Some(row.get(4)?),
                proposed_value: Some(row.get(5)?),
                proposed_usage_hint: Some(row.get(6)?),
                risk_note: row.get(7)?,
                proposed_by: Some(proposed_by),
                created_at: Some(row.get(9)?),
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    fn suggest_memory_merges_by_repo_id(&self, repo_id: &str) -> Result<Vec<MemoryMergeSuggestion>> {
        let conn = self.conn()?;
        let active_memories = {
            let mut stmt = conn.prepare(
                "SELECT memory_id, kind, title, value, usage_hint
                 FROM approved_memories
                 WHERE repo_id = ?1 AND status = 'active'",
            )?;
            let rows = stmt.query_map([repo_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        let candidate_rows = {
            let mut stmt = conn.prepare(
                "SELECT candidate_id, kind, summary, value, why_it_matters
                 FROM memory_candidates
                 WHERE repo_id = ?1 AND status = 'pending_review'
                 ORDER BY created_at DESC",
            )?;
            let rows = stmt.query_map([repo_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                ))
            })?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        let mut suggestions = Vec::new();
        for (candidate_id, kind, summary, value, why_it_matters) in candidate_rows {
            let best_match = active_memories
                .iter()
                .filter(|(_, memory_kind, _, _, _)| memory_kind == &kind)
                .filter_map(|(memory_id, _, title, memory_value, usage_hint)| {
                    let score =
                        merge_similarity(&summary, &value, &why_it_matters, title, memory_value, usage_hint);
                    let value_overlap = token_overlap(&value, memory_value);
                    if score >= 0.55 || value_overlap >= 0.65 {
                        Some((score, memory_id, title, memory_value, usage_hint))
                    } else {
                        None
                    }
                })
                .max_by(|left, right| left.0.partial_cmp(&right.0).unwrap_or(std::cmp::Ordering::Equal));

            if let Some((score, memory_id, title, memory_value, usage_hint)) = best_match {
                let reason = if normalize_text(&value) == normalize_text(memory_value) {
                    format!("该候选记忆与已批准记忆内容一致，应走合并复核，避免重复存储（score {:.2}）。", score)
                } else {
                    format!("该候选记忆与已批准记忆重叠，建议进行 merge-aware review（score {:.2}）。", score)
                };

                suggestions.push(MemoryMergeSuggestion {
                    proposal_id: None,
                    candidate_id,
                    memory_id: memory_id.clone(),
                    memory_title: title.clone(),
                    reason,
                    proposed_title: Some(title.clone()),
                    proposed_value: Some(merge_memory_text(memory_value, &value)),
                    proposed_usage_hint: Some(merge_memory_text(usage_hint, &why_it_matters)),
                    risk_note: Some("批准前请复核：该提议会改写现有 approved memory，而不是创建重复记忆。".to_string()),
                    proposed_by: None,
                    created_at: None,
                });
            }
        }

        Ok(suggestions)
    }

    fn list_memory_conflicts_by_repo_id(
        &self,
        repo_id: &str,
        status: Option<&str>,
    ) -> Result<Vec<MemoryConflictResponse>> {
        let conn = self.conn()?;
        let mut sql = String::from(
            "SELECT mc.conflict_id, mc.candidate_id, mc.memory_id, am.title,
                    mc.reason, mc.status, mc.created_at
             FROM memory_conflicts mc
             JOIN approved_memories am ON am.memory_id = mc.memory_id
             WHERE mc.repo_id = ?1",
        );
        if status.is_some() {
            sql.push_str(" AND mc.status = ?2");
        }
        sql.push_str(" ORDER BY mc.created_at DESC");

        let mut stmt = conn.prepare(&sql)?;
        let rows = if let Some(status) = status {
            stmt.query_map(params![repo_id, status], |row| {
                Ok(MemoryConflictResponse {
                    conflict_id: row.get(0)?,
                    candidate_id: row.get(1)?,
                    memory_id: row.get(2)?,
                    memory_title: row.get(3)?,
                    reason: row.get(4)?,
                    status: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?
        } else {
            stmt.query_map(params![repo_id], |row| {
                Ok(MemoryConflictResponse {
                    conflict_id: row.get(0)?,
                    candidate_id: row.get(1)?,
                    memory_id: row.get(2)?,
                    memory_title: row.get(3)?,
                    reason: row.get(4)?,
                    status: row.get(5)?,
                    created_at: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?
        };

        Ok(rows)
    }

    pub fn list_episodes(&self, repo_root: &str) -> Result<Vec<EpisodeResponse>> {
        let repo_id = self.ensure_repo(repo_root)?;
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT episode_id, title, summary, outcome, created_at, source_conversation_id
             FROM episodes
             WHERE repo_id = ?1
             ORDER BY created_at DESC",
        )?;

        let rows = stmt
            .query_map([repo_id], |row| {
                Ok(EpisodeResponse {
                    episode_id: row.get(0)?,
                    title: row.get(1)?,
                    summary: row.get(2)?,
                    outcome: row.get(3)?,
                    created_at: row.get(4)?,
                    source_conversation_id: row.get(5)?,
                    evidence_refs: vec![],
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        let mut episodes = rows;
        for episode in &mut episodes {
            episode.evidence_refs = self.evidence_refs("episode", &episode.episode_id)?;
        }
        Ok(episodes)
    }

    pub fn list_wiki_pages(&self, repo_root: &str) -> Result<Vec<WikiPageResponse>> {
        let repo_id = self.ensure_repo(repo_root)?;
        let repo_root = self.repo_root_for_id(&repo_id)?;
        let conn = self.conn()?;
        load_wiki_pages_from_conn(&conn, &repo_id, &repo_root)
    }

    pub fn rebuild_repo_wiki(&self, repo_root: &str) -> Result<Vec<WikiPageResponse>> {
        let repo_id = self.ensure_repo(repo_root)?;
        let repo_root = self.repo_root_for_id(&repo_id)?;
        let memories = self
            .list_repo_memories(&repo_root)?
            .into_iter()
            .filter(|memory| memory.status == "active")
            .collect::<Vec<_>>();
        let episodes = self.list_episodes(&repo_root)?;
        let page_specs = build_wiki_page_specs(&repo_root, &memories, &episodes);
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;

        for spec in page_specs {
            upsert_wiki_page_tx(&tx, &repo_id, &spec)?;
        }

        tx.commit()?;
        let conn = self.conn()?;
        load_wiki_pages_from_conn(&conn, &repo_id, &repo_root)
    }

    pub fn create_checkpoint(&self, input: &CreateCheckpointInput) -> Result<CheckpointRecord> {
        let repo_id = self.ensure_repo(&input.repo_root)?;
        let repo_root = self.repo_root_for_id(&repo_id)?;
        let checkpoint = CheckpointRecord {
            checkpoint_id: uuid::Uuid::new_v4().to_string(),
            repo_root,
            conversation_id: input.conversation_id.clone(),
            source_agent: input.source_agent.clone(),
            status: "active".to_string(),
            summary: input.summary.clone(),
            resume_command: input.resume_command.clone(),
            metadata_json: input
                .metadata_json
                .clone()
                .filter(|value| !value.trim().is_empty())
                .unwrap_or_else(|| "{}".to_string()),
            handoff_id: None,
            created_at: chrono::Utc::now().to_rfc3339(),
        };
        let conn = self.conn()?;

        conn.execute(
            "INSERT INTO checkpoints (
                checkpoint_id, repo_id, conversation_id, source_agent, status, summary,
                resume_command, metadata_json, handoff_id, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                checkpoint.checkpoint_id,
                repo_id,
                checkpoint.conversation_id,
                checkpoint.source_agent,
                checkpoint.status,
                checkpoint.summary,
                checkpoint.resume_command,
                checkpoint.metadata_json,
                checkpoint.handoff_id,
                checkpoint.created_at,
            ],
        )?;

        Ok(checkpoint)
    }

    pub fn list_checkpoints(&self, repo_root: &str) -> Result<Vec<CheckpointRecord>> {
        let repo_id = self.ensure_repo(repo_root)?;
        let repo_root = self.repo_root_for_id(&repo_id)?;
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT checkpoint_id, conversation_id, source_agent, status, summary,
                    resume_command, metadata_json, handoff_id, created_at
             FROM checkpoints
             WHERE repo_id = ?1
             ORDER BY created_at DESC",
        )?;

        let rows = stmt.query_map([repo_id], |row| {
            Ok(CheckpointRecord {
                checkpoint_id: row.get(0)?,
                repo_root: repo_root.clone(),
                conversation_id: row.get(1)?,
                source_agent: row.get(2)?,
                status: row.get(3)?,
                summary: row.get(4)?,
                resume_command: row.get(5)?,
                metadata_json: row.get(6)?,
                handoff_id: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;

        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn list_handoffs(&self, repo_root: &str) -> Result<Vec<HandoffPacketResponse>> {
        let repo_id = self.ensure_repo(repo_root)?;
        let repo_root = self.repo_root_for_id(&repo_id)?;
        let conn = self.conn()?;
        let mut stmt = conn.prepare(
            "SELECT handoff_id, from_agent, to_agent, status, checkpoint_id, target_profile,
                    compression_strategy, current_goal, done_json, next_json,
                    key_files_json, commands_json, related_memories_json, related_episodes_json,
                    consumed_at, consumed_by, created_at
             FROM handoff_packets
             WHERE repo_id = ?1
             ORDER BY created_at DESC",
        )?;

        let rows = stmt
            .query_map([repo_id], |row| {
                Ok(HandoffPacketResponse {
                    handoff_id: row.get(0)?,
                    repo_root: repo_root.clone(),
                    from_agent: row.get(1)?,
                    to_agent: row.get(2)?,
                    status: row.get(3)?,
                    checkpoint_id: row.get(4)?,
                    target_profile: row.get(5)?,
                    compression_strategy: row.get(6)?,
                    current_goal: row.get(7)?,
                    done_items: serde_json::from_str::<Vec<String>>(&row.get::<_, String>(8)?).unwrap_or_default(),
                    next_items: serde_json::from_str::<Vec<String>>(&row.get::<_, String>(9)?).unwrap_or_default(),
                    key_files: serde_json::from_str::<Vec<String>>(&row.get::<_, String>(10)?).unwrap_or_default(),
                    useful_commands: serde_json::from_str::<Vec<String>>(&row.get::<_, String>(11)?).unwrap_or_default(),
                    related_memories: serde_json::from_str::<Vec<ApprovedMemoryResponse>>(&row.get::<_, String>(12)?).unwrap_or_default(),
                    related_episodes: serde_json::from_str::<Vec<EpisodeResponse>>(&row.get::<_, String>(13)?).unwrap_or_default(),
                    consumed_at: row.get(14)?,
                    consumed_by: row.get(15)?,
                    created_at: row.get(16)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(rows)
    }

    pub fn latest_handoff(&self, repo_root: &str) -> Result<Option<HandoffPacketResponse>> {
        Ok(self.list_handoffs(repo_root)?.into_iter().next())
    }

    pub fn build_and_store_handoff(
        &self,
        repo_root: &str,
        from_agent: &str,
        to_agent: &str,
        goal_hint: Option<&str>,
    ) -> Result<HandoffPacketResponse> {
        self.build_and_store_handoff_for_target_profile(
            repo_root,
            from_agent,
            to_agent,
            goal_hint,
            None,
        )
    }

    pub fn build_and_store_handoff_for_target_profile(
        &self,
        repo_root: &str,
        from_agent: &str,
        to_agent: &str,
        goal_hint: Option<&str>,
        target_profile: Option<&str>,
    ) -> Result<HandoffPacketResponse> {
        let repo_id = self.ensure_repo(repo_root)?;
        let repo_root = self.repo_root_for_id(&repo_id)?;
        let conn = self.conn()?;
        let latest_summary = conn
            .query_row(
                "SELECT summary FROM conversations WHERE repo_id = ?1 ORDER BY updated_at DESC LIMIT 1",
                [repo_id.clone()],
                |row| row.get::<_, Option<String>>(0),
            )
            .optional()?
            .flatten();

        let key_files = {
            let mut stmt = conn.prepare(
                "SELECT DISTINCT path FROM file_changes
                 WHERE conversation_id IN (
                    SELECT conversation_id FROM conversations WHERE repo_id = ?1 ORDER BY updated_at DESC LIMIT 3
                 )
                 ORDER BY timestamp DESC
                 LIMIT 5",
            )?;
            let rows = stmt
                .query_map([repo_id.clone()], |row| row.get::<_, String>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            rows
        };

        let related_memories = self
            .list_repo_memories(&repo_root)?
            .into_iter()
            .filter(|memory| memory.status == "active" && memory.freshness_status == "fresh")
            .take(3)
            .collect::<Vec<_>>();
        let useful_commands = related_memories
            .iter()
            .filter(|memory| memory.kind == "command")
            .map(|memory| memory.value.clone())
            .take(3)
            .collect::<Vec<_>>();
        let related_episodes = self.list_episodes(&repo_root)?.into_iter().take(2).collect::<Vec<_>>();
        let current_goal = handoff::derive_goal(goal_hint, latest_summary.as_deref());
        let done_items = handoff::summarize_done_item(latest_summary.as_deref());
        let next_items = vec![current_goal.clone()];
        let packet = handoff::build_handoff_packet(
            &repo_root,
            from_agent,
            to_agent,
            current_goal,
            done_items,
            next_items,
            key_files,
            useful_commands,
            related_memories,
            related_episodes,
            target_profile,
        );

        conn.execute(
            "INSERT INTO handoff_packets (
                handoff_id, repo_id, from_agent, to_agent, status, target_profile, checkpoint_id,
                compression_strategy, current_goal,
                done_json, next_json, key_files_json, commands_json,
                related_memories_json, related_episodes_json, consumed_at, consumed_by, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                packet.handoff_id,
                repo_id,
                packet.from_agent,
                packet.to_agent,
                packet.status,
                packet.target_profile,
                packet.checkpoint_id,
                packet.compression_strategy,
                packet.current_goal,
                serde_json::to_string(&packet.done_items)?,
                serde_json::to_string(&packet.next_items)?,
                serde_json::to_string(&packet.key_files)?,
                serde_json::to_string(&packet.useful_commands)?,
                serde_json::to_string(&packet.related_memories)?,
                serde_json::to_string(&packet.related_episodes)?,
                packet.consumed_at,
                packet.consumed_by,
                packet.created_at,
            ],
        )?;

        Ok(packet)
    }

    pub fn build_and_store_handoff_from_checkpoint(
        &self,
        checkpoint_id: &str,
        _from_agent: &str,
        to_agent: &str,
        _goal_hint: Option<&str>,
        target_profile: Option<&str>,
    ) -> Result<HandoffPacketResponse> {
        let mut conn = self.conn()?;
        let checkpoint = conn
            .query_row(
                "SELECT repo_id, conversation_id, source_agent, status, summary, resume_command, handoff_id
                 FROM checkpoints
                 WHERE checkpoint_id = ?1",
                [checkpoint_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                    ))
                },
            )
            .optional()?;

        let Some((repo_id, conversation_id, source_agent, status, summary, resume_command, handoff_id)) = checkpoint else {
            return Err(anyhow::anyhow!("checkpoint {checkpoint_id} not found"));
        };

        if status != "active" || handoff_id.is_some() {
            return Err(anyhow::anyhow!(
                "checkpoint {checkpoint_id} was already promoted and cannot be promoted again"
            ));
        }

        let repo_root = conn.query_row(
            "SELECT repo_root FROM repos WHERE repo_id = ?1",
            [repo_id.clone()],
            |row| row.get::<_, String>(0),
        )?;
        let key_files = {
            let mut stmt = conn.prepare(
                "SELECT DISTINCT path
                 FROM file_changes
                 WHERE conversation_id = ?1
                 ORDER BY timestamp DESC
                 LIMIT 5",
            )?;
            let rows = stmt.query_map([conversation_id], |row| row.get::<_, String>(0))?;
            rows.collect::<Result<Vec<_>, _>>()?
        };

        let related_memories = self
            .list_repo_memories(&repo_root)?
            .into_iter()
            .filter(|memory| memory.status == "active" && memory.freshness_status == "fresh")
            .take(3)
            .collect::<Vec<_>>();
        let mut useful_commands = related_memories
            .iter()
            .filter(|memory| memory.kind == "command")
            .map(|memory| memory.value.clone())
            .take(3)
            .collect::<Vec<_>>();
        if let Some(command) = &resume_command {
            if !useful_commands.iter().any(|value| value == command) {
                useful_commands.insert(0, command.clone());
            }
        }
        let related_episodes = self
            .list_episodes(&repo_root)?
            .into_iter()
            .take(2)
            .collect::<Vec<_>>();
        let current_goal = handoff::derive_goal(None, Some(&summary));
        let done_items = vec![format!("已从 {source_agent} checkpoint 固化上下文：{summary}")];
        let mut next_items = vec![current_goal.clone()];
        if let Some(command) = &resume_command {
            next_items.push(format!("Resume with: {command}"));
        }

        let mut packet = handoff::build_handoff_packet(
            &repo_root,
            &source_agent,
            to_agent,
            current_goal,
            done_items,
            next_items,
            key_files,
            useful_commands,
            related_memories,
            related_episodes,
            target_profile,
        );
        packet.checkpoint_id = Some(checkpoint_id.to_string());

        let tx = conn.transaction()?;

        tx.execute(
            "INSERT INTO handoff_packets (
                handoff_id, repo_id, from_agent, to_agent, status, target_profile, checkpoint_id,
                compression_strategy, current_goal,
                done_json, next_json, key_files_json, commands_json,
                related_memories_json, related_episodes_json, consumed_at, consumed_by, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
            params![
                packet.handoff_id,
                repo_id,
                packet.from_agent,
                packet.to_agent,
                packet.status,
                packet.target_profile,
                packet.checkpoint_id,
                packet.compression_strategy,
                packet.current_goal,
                serde_json::to_string(&packet.done_items)?,
                serde_json::to_string(&packet.next_items)?,
                serde_json::to_string(&packet.key_files)?,
                serde_json::to_string(&packet.useful_commands)?,
                serde_json::to_string(&packet.related_memories)?,
                serde_json::to_string(&packet.related_episodes)?,
                packet.consumed_at,
                packet.consumed_by,
                packet.created_at,
            ],
        )
        .map_err(|error| {
            if matches!(
                error.sqlite_error_code(),
                Some(rusqlite::ErrorCode::ConstraintViolation)
            ) {
                anyhow::anyhow!(
                    "checkpoint {checkpoint_id} was already promoted and cannot be promoted again"
                )
            } else {
                error.into()
            }
        })?;

        let updated = tx.execute(
            "UPDATE checkpoints
             SET status = 'promoted',
                 handoff_id = ?2
             WHERE checkpoint_id = ?1
               AND status = 'active'
               AND handoff_id IS NULL",
            params![checkpoint_id, packet.handoff_id],
        )?;

        if updated != 1 {
            return Err(anyhow::anyhow!(
                "checkpoint {checkpoint_id} was already promoted and cannot be promoted again"
            ));
        }

        tx.commit()?;

        Ok(packet)
    }

    pub fn mark_handoff_consumed(&self, handoff_id: &str, consumed_by: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let conn = self.conn()?;
        let handoff = conn
            .query_row(
                "SELECT to_agent, status, consumed_at, consumed_by
                 FROM handoff_packets
                 WHERE handoff_id = ?1",
                [handoff_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .optional()?;

        let Some((to_agent, status, consumed_at, existing_consumed_by)) = handoff else {
            return Err(anyhow::anyhow!("handoff {handoff_id} not found"));
        };

        if consumed_at.is_some() || status == "consumed" {
            return Err(anyhow::anyhow!(
                "handoff {handoff_id} is already consumed by {}",
                existing_consumed_by.unwrap_or_else(|| "unknown".to_string())
            ));
        }

        if to_agent != consumed_by {
            return Err(anyhow::anyhow!(
                "handoff {handoff_id} cannot be consumed by {consumed_by}; target agent is {to_agent}"
            ));
        }

        let updated = conn.execute(
            "UPDATE handoff_packets
             SET status = 'consumed', consumed_at = ?2, consumed_by = ?3
             WHERE handoff_id = ?1 AND consumed_at IS NULL AND status != 'consumed'",
            params![handoff_id, now, consumed_by],
        )?;

        if updated == 0 {
            return Err(anyhow::anyhow!("handoff {handoff_id} cannot be consumed"));
        }

        Ok(())
    }

    pub fn search_history(&self, repo_root: &str, query: &str, limit: usize) -> Result<Vec<SearchHistoryMatch>> {
        let config = embedding::EmbeddingConfig::from_env();
        match self.search_history_with_embedding_config(repo_root, query, limit, &config) {
            Ok(matches) => Ok(matches),
            Err(error) if config != embedding::EmbeddingConfig::LocalHash => {
                eprintln!(
                    "ChatMem embedding provider failed; falling back to local hash embeddings: {error}"
                );
                self.search_history_with_embedding_config(
                    repo_root,
                    query,
                    limit,
                    &embedding::EmbeddingConfig::LocalHash,
                )
            }
            Err(error) => Err(error),
        }
    }

    pub fn rebuild_repo_embeddings(&self, repo_root: &str) -> Result<EmbeddingRebuildReport> {
        let config = embedding::EmbeddingConfig::from_env();
        match self.rebuild_repo_embeddings_with_config(repo_root, &config) {
            Ok(report) => Ok(report),
            Err(error) if config != embedding::EmbeddingConfig::LocalHash => {
                eprintln!(
                    "ChatMem embedding provider failed during rebuild; falling back to local hash embeddings: {error}"
                );
                self.rebuild_repo_embeddings_with_config(
                    repo_root,
                    &embedding::EmbeddingConfig::LocalHash,
                )
            }
            Err(error) => Err(error),
        }
    }

    pub(crate) fn rebuild_repo_embeddings_with_config(
        &self,
        repo_root: &str,
        config: &embedding::EmbeddingConfig,
    ) -> Result<EmbeddingRebuildReport> {
        let repo_id = self.ensure_repo(repo_root)?;
        let conn = self.conn()?;
        backfill_missing_document_embeddings_with_config(
            &conn,
            &repo_id,
            &embedding::EmbeddingConfig::LocalHash,
        )?;
        if config != &embedding::EmbeddingConfig::LocalHash {
            backfill_missing_document_embeddings_with_config(&conn, &repo_id, config)?;
        }

        let active_count = count_document_embeddings(&conn, &repo_id, &config.model_id())?;
        let fallback_count = count_document_embeddings(
            &conn,
            &repo_id,
            embedding::LOCAL_EMBEDDING_MODEL,
        )?;

        Ok(EmbeddingRebuildReport {
            provider: config.provider_label().to_string(),
            embedding_model: config.model_id(),
            dimensions: config.dimensions(),
            indexed_documents: active_count,
            fallback_indexed_documents: fallback_count,
        })
    }

    pub(crate) fn search_history_with_embedding_config(
        &self,
        repo_root: &str,
        query: &str,
        limit: usize,
        config: &embedding::EmbeddingConfig,
    ) -> Result<Vec<SearchHistoryMatch>> {
        if query.trim().is_empty() || limit == 0 {
            return Ok(vec![]);
        }

        let repo_id = self.ensure_repo(repo_root)?;
        let conn = self.conn()?;
        let matches =
            search_history_in_repo_id_with_embedding_config(&conn, &repo_id, query, limit, config)?;
        if !matches.is_empty() {
            return Ok(matches);
        }

        for (ancestor_repo_id, _) in ancestor_repo_roots_from_conn(&conn, repo_root)? {
            let matches = search_history_in_repo_id_with_embedding_config(
                &conn,
                &ancestor_repo_id,
                query,
                limit,
                config,
            )?;
            if !matches.is_empty() {
                return Ok(matches);
            }
        }

        Ok(vec![])
    }

    pub fn evidence_refs(&self, owner_type: &str, owner_id: &str) -> Result<Vec<EvidenceRef>> {
        let conn = self.conn()?;
        load_evidence_refs_from_conn(&conn, owner_type, owner_id)
    }
}

fn search_history_in_repo_id_with_embedding_config(
    conn: &Connection,
    repo_id: &str,
    query: &str,
    limit: usize,
    config: &embedding::EmbeddingConfig,
) -> Result<Vec<SearchHistoryMatch>> {
    backfill_missing_document_embeddings_with_config(
        conn,
        repo_id,
        &embedding::EmbeddingConfig::LocalHash,
    )?;
    if config != &embedding::EmbeddingConfig::LocalHash {
        backfill_missing_document_embeddings_with_config(conn, repo_id, config)?;
    }

    let candidate_limit = (limit * 4).max(16);
    let mut candidates = HashMap::new();

    collect_fts_search_candidates(conn, repo_id, query, candidate_limit, &mut candidates)?;
    collect_like_search_candidates(conn, repo_id, query, candidate_limit, &mut candidates)?;
    collect_vector_search_candidates_with_config(
        conn,
        repo_id,
        query,
        candidate_limit,
        config,
        &mut candidates,
    )?;
    if config != &embedding::EmbeddingConfig::LocalHash {
        collect_vector_search_candidates_with_config(
            conn,
            repo_id,
            query,
            candidate_limit,
            &embedding::EmbeddingConfig::LocalHash,
            &mut candidates,
        )?;
    }

    let mut scored = candidates
        .into_values()
        .filter_map(|mut candidate| {
            if !candidate.has_retrieval_signal() {
                return None;
            }

            candidate.score = score_search_candidate(&candidate, query);
            Some(candidate)
        })
        .collect::<Vec<_>>();

    scored.sort_by(|left, right| {
        right
            .score
            .partial_cmp(&left.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| right.updated_at.cmp(&left.updated_at))
    });

    let mut matches = Vec::new();
    for candidate in scored.into_iter().take(limit) {
        let why_matched = candidate.match_reason();
        let evidence_refs = if candidate.doc_type == "chunk" {
            load_chunk_evidence_refs_from_conn(conn, &candidate.doc_ref_id)?
        } else {
            load_evidence_refs_from_conn(
                conn,
                evidence_owner_for_doc_type(&candidate.doc_type),
                &candidate.doc_ref_id,
            )?
        };
        matches.push(SearchHistoryMatch {
            r#type: candidate.doc_type.clone(),
            title: candidate.title,
            summary: truncate_text(&candidate.body, 280),
            why_matched,
            score: candidate.score,
            evidence_refs,
        });
    }

    Ok(matches)
}

fn ancestor_repo_roots_from_conn(
    conn: &Connection,
    repo_root: &str,
) -> Result<Vec<(String, String)>> {
    let normalized = repo_identity::normalize_repo_root(repo_root);
    let mut stmt = conn.prepare(
        "SELECT repo_id, repo_root
         FROM repos
         WHERE ?1 LIKE repo_root || '/%'
         ORDER BY length(repo_root) DESC, updated_at DESC",
    )?;
    let rows = stmt
        .query_map([normalized], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn count_table_rows(
    conn: &Connection,
    table: &str,
    repo_id: &str,
    status_filter: Option<(&str, &str)>,
) -> Result<usize> {
    let sql = match status_filter {
        Some((status_column, _)) => {
            format!("SELECT COUNT(*) FROM {table} WHERE repo_id = ?1 AND {status_column} = ?2")
        }
        None => format!("SELECT COUNT(*) FROM {table} WHERE repo_id = ?1"),
    };

    let count = match status_filter {
        Some((_, status_value)) => conn.query_row(
            &sql,
            params![repo_id, status_value],
            |row| row.get::<_, i64>(0),
        )?,
        None => conn.query_row(&sql, [repo_id], |row| row.get::<_, i64>(0))?,
    };

    Ok(count as usize)
}

fn evidence_owner_for_doc_type(doc_type: &str) -> &'static str {
    match doc_type {
        "memory" => "memory",
        "episode" => "episode",
        "chunk" => "chunk",
        "wiki" => "wiki_page",
        _ => "conversation",
    }
}

const VECTOR_MATCH_THRESHOLD: f64 = 0.18;

#[derive(Debug, Clone)]
struct SearchCandidate {
    doc_type: String,
    doc_ref_id: String,
    title: String,
    body: String,
    updated_at: String,
    keyword_rank: Option<usize>,
    like_match: bool,
    vector_score: Option<f64>,
    vector_provider: Option<String>,
    score: f64,
}

impl SearchCandidate {
    fn has_retrieval_signal(&self) -> bool {
        self.keyword_rank.is_some()
            || self.like_match
            || self.vector_score.unwrap_or(0.0) >= VECTOR_MATCH_THRESHOLD
    }

    fn match_reason(&self) -> String {
        let vector_score = self.vector_score.unwrap_or(0.0);
        let provider = self.vector_provider.as_deref().unwrap_or("local-hash");
        if self.keyword_rank.is_some() && vector_score >= VECTOR_MATCH_THRESHOLD {
            format!("hybrid repository history match (keyword + {provider} vector rerank)")
        } else if vector_score >= VECTOR_MATCH_THRESHOLD {
            format!("vector similarity match from {provider} embeddings")
        } else if self.keyword_rank.is_some() {
            "keyword repository history match".to_string()
        } else {
            "repository history substring fallback match".to_string()
        }
    }
}

fn collect_fts_search_candidates(
    conn: &Connection,
    repo_id: &str,
    query: &str,
    limit: usize,
    candidates: &mut HashMap<String, SearchCandidate>,
) -> Result<()> {
    let Some(fts_query) = build_fts_match_query(query) else {
        return Ok(());
    };
    let fts_result = conn.prepare(
        "SELECT sd.doc_id, sd.doc_type, sd.doc_ref_id, sd.title, sd.body, sd.updated_at
         FROM search_documents_fts
         JOIN search_documents sd ON sd.doc_id = search_documents_fts.doc_id
         WHERE sd.repo_id = ?1 AND search_documents_fts MATCH ?2
         ORDER BY bm25(search_documents_fts)
         LIMIT ?3",
    );

    let Ok(mut stmt) = fts_result else {
        return Ok(());
    };

    let rows = stmt.query_map(params![repo_id, fts_query, limit as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
        ))
    });

    let Ok(rows) = rows else {
        return Ok(());
    };

    for (rank, row) in rows.enumerate() {
        let (doc_id, doc_type, doc_ref_id, title, body, updated_at) = row?;
        let candidate = search_candidate_entry(
            candidates, doc_id, doc_type, doc_ref_id, title, body, updated_at,
        );
        candidate.keyword_rank = Some(candidate.keyword_rank.map_or(rank, |existing| existing.min(rank)));
    }

    Ok(())
}

fn build_fts_match_query(query: &str) -> Option<String> {
    let terms = query
        .split_whitespace()
        .map(|term| term.trim())
        .filter(|term| !term.is_empty())
        .map(|term| format!("\"{}\"", term.replace('"', "\"\"")))
        .collect::<Vec<_>>();

    if terms.is_empty() {
        None
    } else {
        Some(terms.join(" "))
    }
}

fn collect_like_search_candidates(
    conn: &Connection,
    repo_id: &str,
    query: &str,
    limit: usize,
    candidates: &mut HashMap<String, SearchCandidate>,
) -> Result<()> {
    let like = format!("%{}%", query.to_lowercase());
    let mut stmt = conn.prepare(
        "SELECT doc_id, doc_type, doc_ref_id, title, body, updated_at
         FROM search_documents
         WHERE repo_id = ?1 AND lower(title || ' ' || body) LIKE ?2
         ORDER BY updated_at DESC
         LIMIT ?3",
    )?;
    let rows = stmt.query_map(params![repo_id, like, limit as i64], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
            row.get::<_, String>(4)?,
            row.get::<_, String>(5)?,
        ))
    })?;

    for row in rows {
        let (doc_id, doc_type, doc_ref_id, title, body, updated_at) = row?;
        let candidate = search_candidate_entry(
            candidates, doc_id, doc_type, doc_ref_id, title, body, updated_at,
        );
        candidate.like_match = true;
    }

    Ok(())
}

fn collect_vector_search_candidates_with_config(
    conn: &Connection,
    repo_id: &str,
    query: &str,
    limit: usize,
    config: &embedding::EmbeddingConfig,
    candidates: &mut HashMap<String, SearchCandidate>,
) -> Result<()> {
    let query_embedding = embedding::embed_query_with_config(config, query)?;
    let query_vector = query_embedding.vector;
    if query_vector.iter().all(|value| *value == 0.0) {
        return Ok(());
    }

    let mut stmt = conn.prepare(
        "SELECT sd.doc_id, sd.doc_type, sd.doc_ref_id, sd.title, sd.body, sd.updated_at, de.vector_json
         FROM document_embeddings de
         JOIN search_documents sd ON sd.doc_id = de.doc_id
         WHERE sd.repo_id = ?1
           AND de.repo_id = ?1
           AND de.embedding_model = ?2
           AND de.dimensions = ?3",
    )?;

    let rows = stmt.query_map(
        params![
            repo_id,
            query_embedding.model_id,
            query_embedding.dimensions as i64,
        ],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
            ))
        },
    )?;

    let mut scored_rows = Vec::new();
    for row in rows {
        let (doc_id, doc_type, doc_ref_id, title, body, updated_at, vector_json) = row?;
        let document_vector = serde_json::from_str::<Vec<f32>>(&vector_json).unwrap_or_default();
        let vector_score = embedding::cosine_similarity(&query_vector, &document_vector);
        if vector_score >= VECTOR_MATCH_THRESHOLD {
            scored_rows.push((
                vector_score,
                doc_id,
                doc_type,
                doc_ref_id,
                title,
                body,
                updated_at,
            ));
        }
    }

    scored_rows.sort_by(|left, right| {
        right
            .0
            .partial_cmp(&left.0)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for (vector_score, doc_id, doc_type, doc_ref_id, title, body, updated_at) in
        scored_rows.into_iter().take(limit)
    {
        let candidate = search_candidate_entry(
            candidates, doc_id, doc_type, doc_ref_id, title, body, updated_at,
        );
        if vector_score > candidate.vector_score.unwrap_or(0.0) {
            candidate.vector_score = Some(vector_score);
            candidate.vector_provider = Some(config.provider_label().to_string());
        }
    }

    Ok(())
}

fn search_candidate_entry<'a>(
    candidates: &'a mut HashMap<String, SearchCandidate>,
    doc_id: String,
    doc_type: String,
    doc_ref_id: String,
    title: String,
    body: String,
    updated_at: String,
) -> &'a mut SearchCandidate {
    let key = doc_id.clone();
    candidates.entry(key).or_insert_with(|| SearchCandidate {
        doc_type,
        doc_ref_id,
        title,
        body,
        updated_at,
        keyword_rank: None,
        like_match: false,
        vector_score: None,
        vector_provider: None,
        score: 0.0,
    })
}

fn score_search_candidate(candidate: &SearchCandidate, query: &str) -> f64 {
    let keyword_score = candidate
        .keyword_rank
        .map(|rank| 1.0 / (rank as f64 + 1.0))
        .unwrap_or(0.0);
    let like_score = if candidate.like_match { 0.45 } else { 0.0 };
    let vector_score = candidate.vector_score.unwrap_or(0.0).max(0.0);
    let coverage = token_overlap(query, &format!("{} {}", candidate.title, candidate.body));
    let type_bonus = match candidate.doc_type.as_str() {
        "memory" => 0.08,
        "wiki" => 0.05,
        "episode" => 0.03,
        _ => 0.0,
    };

    (keyword_score * 0.48)
        + (like_score * 0.18)
        + (vector_score * 0.42)
        + (coverage * 0.12)
        + type_bonus
}

fn backfill_missing_document_embeddings_with_config(
    conn: &Connection,
    repo_id: &str,
    config: &embedding::EmbeddingConfig,
) -> Result<()> {
    let embedding_model = config.model_id();
    let dimensions = config.dimensions();
    let mut stmt = conn.prepare(
        "SELECT sd.doc_id, sd.title, sd.body
         FROM search_documents sd
         LEFT JOIN document_embeddings de
           ON de.doc_id = sd.doc_id
          AND de.embedding_model = ?2
         WHERE sd.repo_id = ?1
           AND (
               de.doc_id IS NULL
               OR de.dimensions != ?3
               OR de.updated_at < sd.updated_at
           )",
    )?;

    let rows = stmt.query_map(
        params![
            repo_id,
            embedding_model,
            dimensions as i64,
        ],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        },
    )?;

    let missing = rows.collect::<Result<Vec<_>, _>>()?;
    drop(stmt);

    for (doc_id, title, body) in missing {
        upsert_document_embedding_for_config_tx(conn, &doc_id, repo_id, &title, &body, config)?;
    }

    Ok(())
}

fn count_document_embeddings(conn: &Connection, repo_id: &str, embedding_model: &str) -> Result<usize> {
    let count = conn.query_row(
        "SELECT COUNT(*)
         FROM document_embeddings
         WHERE repo_id = ?1
           AND embedding_model = ?2",
        params![repo_id, embedding_model],
        |row| row.get::<_, i64>(0),
    )?;

    Ok(count as usize)
}

fn load_evidence_refs_from_conn(conn: &Connection, owner_type: &str, owner_id: &str) -> Result<Vec<EvidenceRef>> {
    let mut stmt = conn.prepare(
        "SELECT evidence_id, conversation_id, message_id, tool_call_id, file_change_id, excerpt
         FROM evidence_refs
         WHERE owner_type = ?1 AND owner_id = ?2
         ORDER BY created_at ASC",
    )?;
    let rows = stmt
        .query_map(params![owner_type, owner_id], |row| {
            Ok(EvidenceRef {
                evidence_id: Some(row.get(0)?),
                conversation_id: row.get(1)?,
                message_id: row.get(2)?,
                tool_call_id: row.get(3)?,
                file_change_id: row.get(4)?,
                excerpt: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

fn load_chunk_evidence_refs_from_conn(conn: &Connection, chunk_ref_id: &str) -> Result<Vec<EvidenceRef>> {
    let chunk_id = format!("chunk:{chunk_ref_id}");
    let row = conn
        .query_row(
            "SELECT conversation_id, message_ids_json, body
             FROM conversation_chunks
             WHERE chunk_id = ?1",
            [chunk_id],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            },
        )
        .optional()?;

    let Some((conversation_id, message_ids_json, body)) = row else {
        return Ok(vec![]);
    };

    let message_ids = serde_json::from_str::<Vec<String>>(&message_ids_json).unwrap_or_default();
    let message_id = message_ids
        .into_iter()
        .next()
        .map(|id| format!("{conversation_id}:{id}"));

    Ok(vec![EvidenceRef {
        evidence_id: None,
        conversation_id: Some(conversation_id),
        message_id,
        tool_call_id: None,
        file_change_id: None,
        excerpt: truncate_text(&body, 240),
    }])
}

fn replace_evidence_refs_tx(
    conn: &Connection,
    owner_type: &str,
    owner_id: &str,
    evidence_refs: &[EvidenceRef],
) -> Result<()> {
    conn.execute(
        "DELETE FROM evidence_refs WHERE owner_type = ?1 AND owner_id = ?2",
        params![owner_type, owner_id],
    )?;

    for evidence in evidence_refs {
        conn.execute(
            "INSERT INTO evidence_refs (
                evidence_id, owner_type, owner_id, conversation_id, message_id, tool_call_id, file_change_id, excerpt, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                uuid::Uuid::new_v4().to_string(),
                owner_type,
                owner_id,
                evidence.conversation_id,
                evidence.message_id,
                evidence.tool_call_id,
                evidence.file_change_id,
                evidence.excerpt,
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;
    }

    Ok(())
}

fn upsert_search_document_tx(
    conn: &Connection,
    doc_id: &str,
    repo_id: &str,
    doc_type: &str,
    doc_ref_id: &str,
    title: &str,
    body: &str,
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO search_documents (doc_id, repo_id, doc_type, doc_ref_id, title, body, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
         ON CONFLICT(doc_id) DO UPDATE SET
            repo_id = excluded.repo_id,
            doc_type = excluded.doc_type,
            doc_ref_id = excluded.doc_ref_id,
            title = excluded.title,
            body = excluded.body,
            updated_at = excluded.updated_at",
        params![doc_id, repo_id, doc_type, doc_ref_id, title, body, now],
    )?;
    conn.execute("DELETE FROM search_documents_fts WHERE doc_id = ?1", [doc_id])?;
    conn.execute(
        "INSERT INTO search_documents_fts (doc_id, title, body) VALUES (?1, ?2, ?3)",
        params![doc_id, title, body],
    )?;
    upsert_document_embedding_tx(conn, doc_id, repo_id, title, body)?;
    replace_entity_links_for_document_tx(conn, repo_id, doc_type, doc_ref_id, title, body)?;
    Ok(())
}

fn append_evidence_refs_tx(
    conn: &Connection,
    owner_type: &str,
    owner_id: &str,
    evidence_refs: &[EvidenceRef],
) -> Result<()> {
    for evidence in evidence_refs {
        conn.execute(
            "INSERT INTO evidence_refs (
                evidence_id, owner_type, owner_id, conversation_id, message_id, tool_call_id, file_change_id, excerpt, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                uuid::Uuid::new_v4().to_string(),
                owner_type,
                owner_id,
                evidence.conversation_id,
                evidence.message_id,
                evidence.tool_call_id,
                evidence.file_change_id,
                evidence.excerpt,
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;
    }

    Ok(())
}

fn upsert_document_embedding_tx(
    conn: &Connection,
    doc_id: &str,
    repo_id: &str,
    title: &str,
    body: &str,
) -> Result<()> {
    upsert_document_embedding_for_config_tx(
        conn,
        doc_id,
        repo_id,
        title,
        body,
        &embedding::EmbeddingConfig::LocalHash,
    )
}

fn upsert_document_embedding_for_config_tx(
    conn: &Connection,
    doc_id: &str,
    repo_id: &str,
    title: &str,
    body: &str,
    config: &embedding::EmbeddingConfig,
) -> Result<()> {
    let now = chrono::Utc::now().to_rfc3339();
    let embedding = embedding::embed_search_document_with_config(config, title, body)?;
    let vector_json = serde_json::to_string(&embedding.vector)?;
    conn.execute(
        "INSERT INTO document_embeddings (
            doc_id, repo_id, embedding_model, dimensions, vector_json, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(doc_id, embedding_model) DO UPDATE SET
            repo_id = excluded.repo_id,
            dimensions = excluded.dimensions,
            vector_json = excluded.vector_json,
            updated_at = excluded.updated_at",
        params![
            doc_id,
            repo_id,
            embedding.model_id,
            embedding.dimensions as i64,
            vector_json,
            now,
        ],
    )?;
    Ok(())
}

#[derive(Debug, Clone)]
struct ExtractedEntity {
    name: String,
    normalized_name: String,
    kind: String,
}

fn replace_entity_links_for_document_tx(
    conn: &Connection,
    repo_id: &str,
    doc_type: &str,
    doc_ref_id: &str,
    title: &str,
    body: &str,
) -> Result<()> {
    let owner_type = evidence_owner_for_doc_type(doc_type);
    conn.execute(
        "DELETE FROM memory_entity_links
         WHERE repo_id = ?1 AND owner_type = ?2 AND owner_id = ?3",
        params![repo_id, owner_type, doc_ref_id],
    )?;

    let now = chrono::Utc::now().to_rfc3339();
    for entity in extract_document_entities(title, body) {
        let entity_id = uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_URL,
            format!("chatmem:entity:{repo_id}:{}", entity.normalized_name).as_bytes(),
        )
        .to_string();
        conn.execute(
            "INSERT INTO memory_entities (
                entity_id, repo_id, name, normalized_name, kind, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?6)
             ON CONFLICT(repo_id, normalized_name) DO UPDATE SET
                name = excluded.name,
                kind = excluded.kind,
                updated_at = excluded.updated_at",
            params![
                entity_id,
                repo_id,
                entity.name,
                entity.normalized_name,
                entity.kind,
                now,
            ],
        )?;

        let link_id = uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_URL,
            format!("chatmem:entity-link:{repo_id}:{owner_type}:{doc_ref_id}:{entity_id}:mentions")
                .as_bytes(),
        )
        .to_string();
        conn.execute(
            "INSERT OR IGNORE INTO memory_entity_links (
                link_id, repo_id, entity_id, owner_type, owner_id, relationship, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, 'mentions', ?6)",
            params![link_id, repo_id, entity_id, owner_type, doc_ref_id, now],
        )?;
    }

    Ok(())
}

fn extract_document_entities(title: &str, body: &str) -> Vec<ExtractedEntity> {
    let mut entities = Vec::new();
    let mut seen = HashSet::new();
    let text = format!("{title}\n{body}");

    for (name, kind) in known_entity_terms(&text) {
        push_entity(&mut entities, &mut seen, name, kind);
    }

    for raw in text.split_whitespace() {
        let Some(name) = clean_entity_token(raw) else {
            continue;
        };
        if !is_entity_candidate(&name) {
            continue;
        }
        let kind = infer_entity_kind(&name);
        push_entity(&mut entities, &mut seen, &name, &kind);
        if entities.len() >= 16 {
            break;
        }
    }

    entities
}

fn known_entity_terms(text: &str) -> Vec<(&'static str, &'static str)> {
    let lower = text.to_lowercase();
    let known = [
        ("chatmem", "project", "ChatMem"),
        ("webdav", "protocol", "WebDAV"),
        ("mcp", "protocol", "MCP"),
        ("tauri", "framework", "Tauri"),
        ("codex", "agent", "Codex"),
        ("claude", "agent", "Claude"),
        ("gemini", "agent", "Gemini"),
        ("sqlite", "database", "SQLite"),
        ("fts5", "index", "FTS5"),
        ("github actions", "ci", "GitHub Actions"),
        ("tauri_private_key", "symbol", "TAURI_PRIVATE_KEY"),
    ];

    known
        .iter()
        .filter_map(|(needle, kind, name)| lower.contains(needle).then_some((*name, *kind)))
        .collect()
}

fn push_entity(
    entities: &mut Vec<ExtractedEntity>,
    seen: &mut HashSet<String>,
    name: &str,
    kind: &str,
) {
    let normalized_name = normalize_entity_name(name);
    if normalized_name.is_empty() || !seen.insert(normalized_name.clone()) {
        return;
    }

    entities.push(ExtractedEntity {
        name: name.to_string(),
        normalized_name,
        kind: kind.to_string(),
    });
}

fn clean_entity_token(raw: &str) -> Option<String> {
    let cleaned = raw
        .trim_matches(|ch: char| !(ch.is_alphanumeric() || ch == '_' || ch == '-' || ch == '.'))
        .trim_matches('.')
        .to_string();

    if cleaned.len() >= 3 {
        Some(cleaned)
    } else {
        None
    }
}

fn is_entity_candidate(token: &str) -> bool {
    let lower = token.to_lowercase();
    if matches!(
        lower.as_str(),
        "the" | "and" | "for" | "with" | "from" | "this" | "that" | "when" | "before"
    ) {
        return false;
    }

    let has_upper = token.chars().any(|ch| ch.is_ascii_uppercase());
    let has_lower = token.chars().any(|ch| ch.is_ascii_lowercase());
    let has_digit = token.chars().any(|ch| ch.is_ascii_digit());
    let has_symbol = token.contains('_') || token.contains('-') || token.contains('.');
    let all_caps = token
        .chars()
        .filter(|ch| ch.is_ascii_alphabetic())
        .all(|ch| ch.is_ascii_uppercase());

    (has_symbol && (has_upper || has_digit))
        || (all_caps && has_upper && token.len() >= 3)
        || (has_upper && has_lower && token.chars().next().is_some_and(|ch| ch.is_ascii_uppercase()))
}

fn infer_entity_kind(name: &str) -> String {
    if name.contains('_') || name.chars().all(|ch| ch.is_ascii_uppercase() || !ch.is_ascii_alphabetic()) {
        "symbol".to_string()
    } else if name.contains('.') || name.contains('-') {
        "artifact".to_string()
    } else {
        "term".to_string()
    }
}

fn normalize_entity_name(name: &str) -> String {
    normalize_text(name)
}

#[derive(Debug, Clone)]
struct AutoMemoryCandidateDraft {
    kind: String,
    summary: String,
    value: String,
    why_it_matters: String,
}

fn extract_memory_candidates_from_conversation_tx(
    conn: &Connection,
    repo_id: &str,
    conversation_id: &str,
    conversation: &Conversation,
) -> Result<()> {
    let mut seen_values = HashSet::new();
    for message in &conversation.messages {
        let message_id = format!("{conversation_id}:{}", message.id);
        for draft in auto_memory_candidate_drafts(&message.content) {
            let normalized_value = normalize_text(&draft.value);
            if normalized_value.is_empty() || !seen_values.insert(normalized_value.clone()) {
                continue;
            }
            if auto_candidate_exists_tx(conn, repo_id, &normalized_value)? {
                continue;
            }

            let candidate_id = uuid::Uuid::new_v4().to_string();
            let now = chrono::Utc::now().to_rfc3339();
            conn.execute(
                "INSERT INTO memory_candidates (
                    candidate_id, repo_id, kind, summary, value, why_it_matters,
                    confidence, proposed_by, status, created_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0.62, 'auto_extractor', 'pending_review', ?7)",
                params![
                    candidate_id,
                    repo_id,
                    draft.kind,
                    draft.summary,
                    draft.value,
                    draft.why_it_matters,
                    now,
                ],
            )?;

            replace_evidence_refs_tx(
                conn,
                "candidate",
                &candidate_id,
                &[EvidenceRef {
                    evidence_id: None,
                    conversation_id: Some(conversation_id.to_string()),
                    message_id: Some(message_id.clone()),
                    tool_call_id: None,
                    file_change_id: None,
                    excerpt: truncate_text(&message.content, 240),
                }],
            )?;
            record_candidate_conflicts_tx(
                conn,
                repo_id,
                &candidate_id,
                &draft.kind,
                &draft.summary,
                &draft.value,
                &draft.why_it_matters,
            )?;
        }
    }

    Ok(())
}

fn auto_memory_candidate_drafts(content: &str) -> Vec<AutoMemoryCandidateDraft> {
    content
        .lines()
        .filter_map(auto_memory_candidate_from_line)
        .collect()
}

fn auto_memory_candidate_from_line(line: &str) -> Option<AutoMemoryCandidateDraft> {
    let trimmed = line
        .trim()
        .trim_start_matches(|ch| ch == '-' || ch == '*' || ch == ' ')
        .trim();
    if trimmed.len() < 12 {
        return None;
    }

    let lower = trimmed.to_lowercase();
    let (kind, value) = if let Some(value) = strip_marker(trimmed, &lower, "remember:") {
        ("preference", value)
    } else if let Some(value) = strip_marker(trimmed, &lower, "remember that ") {
        ("preference", value)
    } else if let Some(value) = strip_marker(trimmed, &lower, "rule:") {
        ("convention", value)
    } else if let Some(value) = strip_marker(trimmed, &lower, "gotcha:") {
        ("gotcha", value)
    } else if let Some(value) = strip_marker(trimmed, &lower, "note:") {
        ("gotcha", value)
    } else if let Some(value) = strip_marker(trimmed, &lower, "\u{8bb0}\u{4f4f}:") {
        ("preference", value)
    } else if let Some(value) = strip_marker(trimmed, &lower, "\u{89c4}\u{5219}:") {
        ("convention", value)
    } else if let Some(value) = strip_marker(trimmed, &lower, "\u{6ce8}\u{610f}:") {
        ("gotcha", value)
    } else if lower.starts_with("always ")
        || lower.starts_with("must ")
        || lower.starts_with("do not ")
        || lower.starts_with("never ")
    {
        ("gotcha", trimmed)
    } else {
        return None;
    };

    let value = truncate_text(value.trim(), 500);
    if value.len() < 8 {
        return None;
    }

    Some(AutoMemoryCandidateDraft {
        kind: kind.to_string(),
        summary: truncate_text(&value, 96),
        value,
        why_it_matters:
            "从明确的 durable-memory wording 自动提取；请在批准前复核中文表述和技术 token 是否准确。"
                .to_string(),
    })
}

fn strip_marker<'a>(trimmed: &'a str, lower: &str, marker: &str) -> Option<&'a str> {
    lower
        .starts_with(marker)
        .then_some(trimmed.get(marker.len()..).unwrap_or_default())
}

fn auto_candidate_exists_tx(conn: &Connection, repo_id: &str, normalized_value: &str) -> Result<bool> {
    let mut stmt = conn.prepare(
        "SELECT value
         FROM memory_candidates
         WHERE repo_id = ?1",
    )?;
    let rows = stmt.query_map([repo_id], |row| row.get::<_, String>(0))?;
    for row in rows {
        if normalize_text(&row?) == normalized_value {
            return Ok(true);
        }
    }

    Ok(false)
}

fn record_candidate_conflicts_tx(
    conn: &Connection,
    repo_id: &str,
    candidate_id: &str,
    kind: &str,
    summary: &str,
    value: &str,
    why_it_matters: &str,
) -> Result<()> {
    let mut stmt = conn.prepare(
        "SELECT memory_id, title, value, usage_hint
         FROM approved_memories
         WHERE repo_id = ?1 AND status = 'active' AND kind = ?2",
    )?;
    let rows = stmt.query_map(params![repo_id, kind], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, String>(3)?,
        ))
    })?;
    let memories = rows.collect::<Result<Vec<_>, _>>()?;
    drop(stmt);

    for (memory_id, title, memory_value, usage_hint) in memories {
        let similarity = merge_similarity(summary, value, why_it_matters, &title, &memory_value, &usage_hint)
            .max(token_overlap(value, &memory_value));
        if similarity < 0.42 || !has_negation_flip(value, &memory_value) {
            continue;
        }

        let conflict_id = uuid::Uuid::new_v5(
            &uuid::Uuid::NAMESPACE_URL,
            format!("chatmem:conflict:{candidate_id}:{memory_id}").as_bytes(),
        )
        .to_string();
        let reason = format!(
            "该候选记忆可能与已批准记忆“{title}”冲突（overlap {:.2}）；批准任一版本前请先复核。",
            similarity
        );
        conn.execute(
            "INSERT INTO memory_conflicts (
                conflict_id, repo_id, candidate_id, memory_id, reason, status, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, 'open', ?6)
             ON CONFLICT(candidate_id, memory_id) DO UPDATE SET
                reason = excluded.reason,
                status = 'open'",
            params![
                conflict_id,
                repo_id,
                candidate_id,
                memory_id,
                reason,
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;
    }

    Ok(())
}

fn has_negation_flip(left: &str, right: &str) -> bool {
    has_negation(left) != has_negation(right)
}

fn has_negation(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("do not ")
        || lower.contains("don't ")
        || lower.contains("never ")
        || lower.contains("not ")
        || lower.contains("avoid ")
        || lower.contains("without ")
        || lower.contains("instead of ")
        || lower.contains("no longer ")
        || lower.contains("\u{4e0d}\u{8981}")
        || lower.contains("\u{7981}\u{6b62}")
}

#[derive(Debug, Clone)]
struct WikiPageSpec {
    slug: String,
    title: String,
    body: String,
    status: String,
    source_memory_ids: Vec<String>,
    source_episode_ids: Vec<String>,
    last_verified_at: Option<String>,
}

fn load_wiki_pages_from_conn(
    conn: &Connection,
    repo_id: &str,
    repo_root: &str,
) -> Result<Vec<WikiPageResponse>> {
    let mut stmt = conn.prepare(
        "SELECT page_id, slug, title, body, status, source_memory_ids_json,
                source_episode_ids_json, last_built_at, last_verified_at, updated_at
         FROM wiki_pages
         WHERE repo_id = ?1
         ORDER BY title ASC",
    )?;

    let rows = stmt.query_map([repo_id], |row| {
        let source_memory_ids_json: String = row.get(5)?;
        let source_episode_ids_json: String = row.get(6)?;
        Ok(WikiPageResponse {
            page_id: row.get(0)?,
            repo_root: repo_root.to_string(),
            slug: row.get(1)?,
            title: row.get(2)?,
            body: row.get(3)?,
            status: row.get(4)?,
            source_memory_ids: parse_string_vec(&source_memory_ids_json),
            source_episode_ids: parse_string_vec(&source_episode_ids_json),
            last_built_at: row.get(7)?,
            last_verified_at: row.get(8)?,
            updated_at: row.get(9)?,
        })
    })?;

    rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
}

fn upsert_wiki_page_tx(conn: &Connection, repo_id: &str, spec: &WikiPageSpec) -> Result<()> {
    let page_id = format!("wiki:{repo_id}:{}", spec.slug);
    let now = chrono::Utc::now().to_rfc3339();
    let source_memory_ids_json = serde_json::to_string(&spec.source_memory_ids)?;
    let source_episode_ids_json = serde_json::to_string(&spec.source_episode_ids)?;

    conn.execute(
        "INSERT INTO wiki_pages (
            page_id, repo_id, slug, title, body, status, source_memory_ids_json,
            source_episode_ids_json, last_built_at, last_verified_at, created_at, updated_at
         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?9, ?9)
         ON CONFLICT(repo_id, slug) DO UPDATE SET
            title = excluded.title,
            body = excluded.body,
            status = excluded.status,
            source_memory_ids_json = excluded.source_memory_ids_json,
            source_episode_ids_json = excluded.source_episode_ids_json,
            last_built_at = excluded.last_built_at,
            last_verified_at = excluded.last_verified_at,
            updated_at = excluded.updated_at",
        params![
            page_id,
            repo_id,
            spec.slug,
            spec.title,
            spec.body,
            spec.status,
            source_memory_ids_json,
            source_episode_ids_json,
            now,
            spec.last_verified_at,
        ],
    )?;

    upsert_search_document_tx(
        conn,
        &format!("wiki:{page_id}"),
        repo_id,
        "wiki",
        &page_id,
        &spec.title,
        &spec.body,
    )
}

fn build_wiki_page_specs(
    repo_root: &str,
    memories: &[ApprovedMemoryResponse],
    episodes: &[EpisodeResponse],
) -> Vec<WikiPageSpec> {
    let commands = memories
        .iter()
        .filter(|memory| memory.kind == "command")
        .cloned()
        .collect::<Vec<_>>();
    let gotchas = memories
        .iter()
        .filter(|memory| memory.kind == "gotcha")
        .cloned()
        .collect::<Vec<_>>();
    let decisions = memories
        .iter()
        .filter(|memory| {
            matches!(
                memory.kind.as_str(),
                "decision" | "convention" | "strategy" | "preference" | "architecture"
            )
        })
        .cloned()
        .collect::<Vec<_>>();

    vec![
        WikiPageSpec {
            slug: "project-overview".to_string(),
            title: "项目概览".to_string(),
            body: build_project_overview_wiki_body(repo_root, memories, episodes),
            status: wiki_status(memories),
            source_memory_ids: memories.iter().map(|memory| memory.memory_id.clone()).collect(),
            source_episode_ids: episodes.iter().map(|episode| episode.episode_id.clone()).collect(),
            last_verified_at: newest_memory_verification(memories),
        },
        build_memory_wiki_page(
            "commands",
            "命令",
            "从 approved repository memory 保留的命令。",
            &commands,
        ),
        build_memory_wiki_page(
            "gotchas",
            "注意事项",
            "从 approved repository memory 保留的操作陷阱和约束。",
            &gotchas,
        ),
        build_memory_wiki_page(
            "decisions-and-conventions",
            "决策与约定",
            "稳定的决策、约定、策略和偏好。",
            &decisions,
        ),
        WikiPageSpec {
            slug: "recent-work".to_string(),
            title: "最近工作".to_string(),
            body: build_recent_work_wiki_body(episodes),
            status: if episodes.is_empty() { "empty" } else { "fresh" }.to_string(),
            source_memory_ids: vec![],
            source_episode_ids: episodes.iter().map(|episode| episode.episode_id.clone()).collect(),
            last_verified_at: None,
        },
    ]
}

fn build_memory_wiki_page(
    slug: &str,
    title: &str,
    intro: &str,
    memories: &[ApprovedMemoryResponse],
) -> WikiPageSpec {
    let body = if memories.is_empty() {
        format!("# {title}\n\n{intro}\n\n暂无已批准条目。\n")
    } else {
        format!(
            "# {title}\n\n{intro}\n\n{}",
            memories
                .iter()
                .map(format_memory_wiki_item)
                .collect::<Vec<_>>()
                .join("\n")
        )
    };

    WikiPageSpec {
        slug: slug.to_string(),
        title: title.to_string(),
        body,
        status: wiki_status(memories),
        source_memory_ids: memories.iter().map(|memory| memory.memory_id.clone()).collect(),
        source_episode_ids: vec![],
        last_verified_at: newest_memory_verification(memories),
    }
}

fn build_project_overview_wiki_body(
    repo_root: &str,
    memories: &[ApprovedMemoryResponse],
    episodes: &[EpisodeResponse],
) -> String {
    let mut body = format!(
        "# 项目概览\n\n仓库：`{repo_root}`\n\nChatMem approved memory 仍然是事实来源；本页是为用户和 agent onboarding 生成的 wiki projection。\n\n"
    );

    body.push_str("## 关键记忆\n\n");
    if memories.is_empty() {
        body.push_str("暂无已批准记忆。\n\n");
    } else {
        for memory in memories.iter().take(8) {
            body.push_str(&format_memory_wiki_item(memory));
            body.push('\n');
        }
    }

    body.push_str("## 最近工作\n\n");
    if episodes.is_empty() {
        body.push_str("暂无已捕获 episode。\n");
    } else {
        for episode in episodes.iter().take(6) {
            body.push_str(&format_episode_wiki_item(episode));
            body.push('\n');
        }
    }

    body
}

fn build_recent_work_wiki_body(episodes: &[EpisodeResponse]) -> String {
    let mut body = "# 最近工作\n\n从本地 agent 对话捕获的精简 repository episodes。\n\n".to_string();

    if episodes.is_empty() {
        body.push_str("暂无已捕获 episode。\n");
    } else {
        for episode in episodes.iter().take(12) {
            body.push_str(&format_episode_wiki_item(episode));
            body.push('\n');
        }
    }

    body
}

fn format_memory_wiki_item(memory: &ApprovedMemoryResponse) -> String {
    let mut item = format!(
        "- **{}** (`{}` / `{}`): {}\n  - 使用方式：{}\n  - 来源 memory：`{}`",
        memory.title,
        memory.kind,
        memory.freshness_status,
        memory.value,
        memory.usage_hint,
        memory.memory_id
    );

    if let Some(verified_at) = memory.verified_at.as_deref().or(memory.last_verified_at.as_deref()) {
        item.push_str(&format!("\n  - 最近验证：{verified_at}"));
    }

    item
}

fn format_episode_wiki_item(episode: &EpisodeResponse) -> String {
    format!(
        "- **{}** (`{}`): {}\n  - 来源 episode：`{}`",
        episode.title, episode.outcome, episode.summary, episode.episode_id
    )
}

fn wiki_status(memories: &[ApprovedMemoryResponse]) -> String {
    if memories.is_empty() {
        return "empty".to_string();
    }

    if memories.iter().any(|memory| memory.freshness_status == "stale") {
        return "stale".to_string();
    }

    if memories
        .iter()
        .any(|memory| memory.freshness_status == "needs_review")
    {
        return "needs_review".to_string();
    }

    if memories.iter().any(|memory| memory.freshness_status == "unknown") {
        return "unknown".to_string();
    }

    "fresh".to_string()
}

fn newest_memory_verification(memories: &[ApprovedMemoryResponse]) -> Option<String> {
    memories
        .iter()
        .filter_map(|memory| {
            memory
                .verified_at
                .as_ref()
                .or(memory.last_verified_at.as_ref())
                .cloned()
        })
        .max()
}

fn parse_string_vec(json: &str) -> Vec<String> {
    serde_json::from_str(json).unwrap_or_default()
}

fn summarize_conversation(conversation: &Conversation) -> String {
    if let Some(summary) = &conversation.summary {
        if !summary.trim().is_empty() {
            return summary.clone();
        }
    }

    conversation
        .messages
        .iter()
        .find(|message| !message.content.trim().is_empty())
        .map(|message| truncate_text(&message.content, 240))
        .unwrap_or_else(|| conversation.id.clone())
}

fn build_conversation_search_body(conversation: &Conversation) -> String {
    let mut sections = Vec::new();
    if let Some(summary) = &conversation.summary {
        sections.push(summary.clone());
    }

    for message in conversation.messages.iter().take(12) {
        if !message.content.trim().is_empty() {
            sections.push(message.content.clone());
        }
    }

    for file_change in conversation.file_changes.iter().take(8) {
        sections.push(file_change.path.clone());
    }

    truncate_text(&sections.join("\n"), 4_000)
}

fn normalize_text(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|ch| if ch.is_alphanumeric() { ch } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn token_overlap(left: &str, right: &str) -> f64 {
    let left_tokens = normalize_text(left)
        .split_whitespace()
        .map(ToString::to_string)
        .collect::<HashSet<_>>();
    let right_tokens = normalize_text(right)
        .split_whitespace()
        .map(ToString::to_string)
        .collect::<HashSet<_>>();

    if left_tokens.is_empty() || right_tokens.is_empty() {
        return 0.0;
    }

    let overlap = left_tokens.intersection(&right_tokens).count() as f64;
    overlap / left_tokens.len().max(right_tokens.len()) as f64
}

fn merge_similarity(
    candidate_summary: &str,
    candidate_value: &str,
    why_it_matters: &str,
    memory_title: &str,
    memory_value: &str,
    usage_hint: &str,
) -> f64 {
    if !candidate_value.trim().is_empty() && normalize_text(candidate_value) == normalize_text(memory_value) {
        return 1.0;
    }

    let title_overlap = token_overlap(candidate_summary, memory_title);
    let value_overlap = token_overlap(candidate_value, memory_value);
    let why_overlap = token_overlap(why_it_matters, usage_hint);

    (title_overlap * 0.45) + (value_overlap * 0.45) + (why_overlap * 0.10)
}

fn merge_memory_text(existing: &str, incoming: &str) -> String {
    let existing = existing.trim();
    let incoming = incoming.trim();

    if incoming.is_empty() || normalize_text(existing) == normalize_text(incoming) {
        return existing.to_string();
    }

    if existing.is_empty() {
        return incoming.to_string();
    }

    format!("{existing}\n\n更新：{incoming}")
}

fn evaluate_memory_freshness(
    last_verified_at: Option<&str>,
    verified_at: Option<&str>,
) -> (String, f64) {
    let verification_timestamp = last_verified_at.or(verified_at);
    let Some(verification_timestamp) = verification_timestamp else {
        return ("unknown".to_string(), 0.0);
    };

    let Ok(parsed_at) = chrono::DateTime::parse_from_rfc3339(verification_timestamp) else {
        return ("unknown".to_string(), 0.0);
    };

    let age = chrono::Utc::now().signed_duration_since(parsed_at.with_timezone(&chrono::Utc));

    if age <= chrono::Duration::days(7) {
        ("fresh".to_string(), 1.0)
    } else if age <= chrono::Duration::days(30) {
        ("needs_review".to_string(), 0.55)
    } else {
        ("stale".to_string(), 0.2)
    }
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
    use super::{MemoryStore, ReviewAction};
    use crate::chatmem_memory::{
        checkpoints::CreateCheckpointInput,
        models::{CreateMemoryCandidateInput, CreateMemoryMergeProposalInput},
    };
    use agentswap_core::types::{AgentKind, Conversation, Message, Role};
    use chrono::Utc;
    use rusqlite::params;
    use std::collections::HashMap;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::{thread, time::Duration};
    use uuid::Uuid;

    fn new_store() -> MemoryStore {
        let path = std::env::temp_dir().join(format!("chatmem-store-test-{}.sqlite", uuid::Uuid::new_v4()));
        MemoryStore::new(path).unwrap()
    }

    fn approve_test_memory(
        store: &MemoryStore,
        repo_root: &str,
        title: &str,
        value: &str,
    ) -> String {
        let candidate_id = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "strategy".to_string(),
                summary: title.to_string(),
                value: value.to_string(),
                why_it_matters: "Keeps cross-agent memory retrieval stable".to_string(),
                evidence_refs: vec![],
                confidence: 0.91,
                proposed_by: "codex".to_string(),
            })
            .unwrap();
        store
            .review_candidate(
                &candidate_id,
                ReviewAction::Approve {
                    title: title.to_string(),
                    usage_hint: "Use when repo-specific memory appears empty".to_string(),
                },
            )
            .unwrap();
        candidate_id
    }

    #[test]
    fn list_repo_memories_falls_back_to_existing_ancestor_repo_when_child_is_empty() {
        let store = new_store();
        approve_test_memory(
            &store,
            "d:/vsp",
            "ChatMem parent-root memory",
            "chatmem-parent-root-unique-token",
        );

        let memories = store.list_repo_memories("d:/vsp/agentswap-gui").unwrap();

        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].title, "ChatMem parent-root memory");
        assert_eq!(
            memories[0].selected_because.as_deref(),
            Some("Inherited from ancestor repo d:/vsp")
        );
    }

    #[test]
    fn search_history_falls_back_to_existing_ancestor_repo_when_child_has_no_matches() {
        let store = new_store();
        approve_test_memory(
            &store,
            "d:/vsp",
            "ChatMem parent search memory",
            "ancestor-search-unique-token",
        );

        let matches = store
            .search_history("d:/vsp/agentswap-gui", "ancestor-search-unique-token", 5)
            .unwrap();

        assert!(matches
            .iter()
            .any(|item| item.title == "ChatMem parent search memory"));
    }

    #[test]
    fn repo_memory_health_reports_pending_candidates_and_ancestor_drift() {
        let store = new_store();
        approve_test_memory(
            &store,
            "d:/vsp",
            "Parent memory",
            "parent-memory-health-token",
        );
        store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: "d:/vsp/agentswap-gui".to_string(),
                kind: "product_decision".to_string(),
                summary: "Keep conversation view full width".to_string(),
                value: "Project memory review belongs on project home.".to_string(),
                why_it_matters: "Pending candidates must be visible before startup memory works.".to_string(),
                evidence_refs: vec![],
                confidence: 0.95,
                proposed_by: "codex".to_string(),
            })
            .unwrap();

        let health = store.repo_memory_health("d:/vsp/agentswap-gui").unwrap();

        assert_eq!(health.approved_memory_count, 0);
        assert_eq!(health.pending_candidate_count, 1);
        assert_eq!(health.inherited_repo_roots, vec!["d:/vsp".to_string()]);
        assert!(health
            .warnings
            .iter()
            .any(|warning| warning.contains("ancestor repo")));
        assert!(health
            .warnings
            .iter()
            .any(|warning| warning.contains("pending memory candidate")));
    }

    #[test]
    fn approving_a_candidate_promotes_it_to_approved_memory() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";
        let candidate_id = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "command".to_string(),
                summary: "Run tests".to_string(),
                value: "npm run test:run".to_string(),
                why_it_matters: "Needed before merge".to_string(),
                evidence_refs: vec![],
                confidence: 0.92,
                proposed_by: "codex".to_string(),
            })
            .unwrap();

        store
            .review_candidate(
                &candidate_id,
                ReviewAction::Approve {
                    title: "Primary test command".into(),
                    usage_hint: "Inject on startup when the task hint mentions tests".into(),
                },
            )
            .unwrap();

        let memories = store.list_repo_memories(repo_root).unwrap();
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].value, "npm run test:run");
    }

    #[test]
    fn rebuilding_repo_wiki_projects_memory_and_episodes_into_pages() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";
        let command_candidate_id = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "command".to_string(),
                summary: "Run test suite".to_string(),
                value: "npm run test:run".to_string(),
                why_it_matters: "Use before shipping UI changes".to_string(),
                evidence_refs: vec![],
                confidence: 0.92,
                proposed_by: "codex".to_string(),
            })
            .unwrap();

        store
            .review_candidate(
                &command_candidate_id,
                ReviewAction::Approve {
                    title: "Primary frontend verification".into(),
                    usage_hint: "Use before handing off frontend work".into(),
                },
            )
            .unwrap();

        let repo_id = store.ensure_repo(repo_root).unwrap();
        let conn = store.conn().unwrap();
        conn.execute(
            "INSERT INTO episodes (
                episode_id, repo_id, title, summary, outcome, created_at, source_conversation_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                "episode:codex:conv-001",
                repo_id,
                "Memory architecture discussion",
                "Decided to keep ChatMem as source of truth and generate a wiki projection.",
                "captured",
                "2026-04-21T10:00:00Z",
                "codex:conv-001",
            ],
        )
        .unwrap();

        let pages = store.rebuild_repo_wiki(repo_root).unwrap();

        let commands = pages
            .iter()
            .find(|page| page.slug == "commands")
            .expect("commands page should be generated");
        assert!(commands.body.contains("npm run test:run"));
        assert_eq!(commands.source_memory_ids.len(), 1);

        let recent_work = pages
            .iter()
            .find(|page| page.slug == "recent-work")
            .expect("recent-work page should be generated");
        assert!(recent_work.body.contains("Memory architecture discussion"));
        assert_eq!(recent_work.source_episode_ids.len(), 1);
    }

    #[test]
    fn search_history_returns_wiki_projection_matches() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";
        let candidate_id = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "gotcha".to_string(),
                summary: "Remember wiki projection".to_string(),
                value: "The wiki is a projection, not the source of truth.".to_string(),
                why_it_matters: "Prevents stale wiki pages from overriding approved memory.".to_string(),
                evidence_refs: vec![],
                confidence: 0.91,
                proposed_by: "codex".to_string(),
            })
            .unwrap();

        store
            .review_candidate(
                &candidate_id,
                ReviewAction::Approve {
                    title: "Wiki projection boundary".into(),
                    usage_hint: "Keep database memory authoritative".into(),
                },
            )
            .unwrap();

        store.rebuild_repo_wiki(repo_root).unwrap();
        let matches = store.search_history(repo_root, "projection", 5).unwrap();

        assert!(matches.iter().any(|item| item.r#type == "wiki"));
    }

    #[test]
    fn search_history_accepts_hyphenated_query_terms() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";
        let candidate_id = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "gotcha".to_string(),
                summary: "Remember codex-smoke-release".to_string(),
                value: "codex-smoke-release queries should not be parsed as raw FTS syntax."
                    .to_string(),
                why_it_matters: "Repository names, package versions, and ids often contain hyphens."
                    .to_string(),
                evidence_refs: vec![],
                confidence: 0.91,
                proposed_by: "codex".to_string(),
            })
            .unwrap();

        store
            .review_candidate(
                &candidate_id,
                ReviewAction::Approve {
                    title: "codex-smoke-release search".into(),
                    usage_hint: "Use when validating MCP search query handling.".into(),
                },
            )
            .unwrap();

        let matches = store.search_history(repo_root, "codex-smoke-release", 5).unwrap();

        assert!(matches
            .iter()
            .any(|item| item.title == "codex-smoke-release search"));
    }

    #[test]
    fn conversation_snapshot_indexes_late_message_chunks() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";
        let now = Utc::now();
        let mut messages = Vec::new();
        for index in 0..16 {
            messages.push(Message {
                id: Uuid::from_u128(0x7000_0000_0000_0000_0000_0000_0000_0000 + index as u128),
                timestamp: now,
                role: Role::User,
                content: format!("Ordinary setup message {index}"),
                tool_calls: vec![],
                metadata: HashMap::new(),
            });
        }
        messages.push(Message {
            id: Uuid::from_u128(0x8000_0000_0000_0000_0000_0000_0000_0001),
            timestamp: now,
            role: Role::Assistant,
            content: "Late recall marker: configure TAURI_PRIVATE_KEY before release packaging."
                .to_string(),
            tool_calls: vec![],
            metadata: HashMap::new(),
        });

        let conversation = Conversation {
            id: "conv-late-chunk-recall".to_string(),
            source_agent: AgentKind::Codex,
            project_dir: repo_root.to_string(),
            created_at: now,
            updated_at: now,
            summary: Some("Chunk recall regression test".to_string()),
            messages,
            file_changes: vec![],
        };

        store
            .upsert_conversation_snapshot("codex", &conversation, None)
            .unwrap();

        let matches = store
            .search_history(repo_root, "TAURI_PRIVATE_KEY release packaging", 5)
            .unwrap();

        assert!(matches.iter().any(|item| {
            item.r#type == "chunk" && item.summary.contains("TAURI_PRIVATE_KEY")
        }));
    }

    #[test]
    fn approving_memory_indexes_a_local_embedding_vector() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";
        let candidate_id = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "strategy".to_string(),
                summary: "Remote snapshot flow".to_string(),
                value: "WebDAV sync uploads manifest snapshots to the configured remote path.".to_string(),
                why_it_matters: "Agents need searchable cloud persistence context.".to_string(),
                evidence_refs: vec![],
                confidence: 0.89,
                proposed_by: "codex".to_string(),
            })
            .unwrap();

        store
            .review_candidate(
                &candidate_id,
                ReviewAction::Approve {
                    title: "Remote snapshot flow".into(),
                    usage_hint: "Use when investigating persistence or backup behavior.".into(),
                },
            )
            .unwrap();

        let conn = store.conn().unwrap();
        let count = conn
            .query_row(
                "SELECT COUNT(*)
                 FROM document_embeddings de
                 JOIN search_documents sd ON sd.doc_id = de.doc_id
                 WHERE sd.doc_type = 'memory'
                   AND sd.title = 'Remote snapshot flow'
                   AND de.embedding_model = 'chatmem-local-hash-v1'
                   AND de.dimensions > 0",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();

        assert_eq!(count, 1);
    }

    #[test]
    fn document_embeddings_can_keep_remote_model_and_local_fallback_for_same_doc() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";
        let candidate_id = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "strategy".to_string(),
                summary: "Remote snapshot flow".to_string(),
                value: "WebDAV sync uploads manifest snapshots to the configured remote path.".to_string(),
                why_it_matters: "Agents need searchable cloud persistence context.".to_string(),
                evidence_refs: vec![],
                confidence: 0.89,
                proposed_by: "codex".to_string(),
            })
            .unwrap();

        store
            .review_candidate(
                &candidate_id,
                ReviewAction::Approve {
                    title: "Remote snapshot flow".into(),
                    usage_hint: "Use when investigating persistence or backup behavior.".into(),
                },
            )
            .unwrap();

        let conn = store.conn().unwrap();
        let doc_id = conn
            .query_row(
                "SELECT doc_id
                 FROM search_documents
                 WHERE doc_type = 'memory'
                   AND title = 'Remote snapshot flow'",
                [],
                |row| row.get::<_, String>(0),
            )
            .unwrap();

        conn.execute(
            "INSERT INTO document_embeddings (
                doc_id, repo_id, embedding_model, dimensions, vector_json, updated_at
             )
             SELECT ?1, repo_id, ?2, ?3, ?4, ?5
             FROM search_documents
             WHERE doc_id = ?1",
            params![
                doc_id,
                "openai-compatible:text-embedding-3-small:1536",
                1536_i64,
                "[0.01,0.02,0.03]",
                "2026-04-22T00:00:00Z",
            ],
        )
        .unwrap();

        let models = conn
            .prepare(
                "SELECT embedding_model
                 FROM document_embeddings
                 WHERE doc_id = ?1
                 ORDER BY embedding_model ASC",
            )
            .unwrap()
            .query_map([doc_id], |row| row.get::<_, String>(0))
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

        let matches = store.search_history(repo_root, "cloud drive backup", 5).unwrap();
        assert!(matches.iter().any(|item| item.title == "Remote snapshot flow"));
    }

    #[test]
    fn search_history_can_use_openai_compatible_embedding_provider() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";
        let candidate_id = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "strategy".to_string(),
                summary: "Remote snapshot flow".to_string(),
                value: "WebDAV sync uploads manifest snapshots to the configured remote path.".to_string(),
                why_it_matters: "Agents need searchable cloud persistence context.".to_string(),
                evidence_refs: vec![],
                confidence: 0.89,
                proposed_by: "codex".to_string(),
            })
            .unwrap();

        store
            .review_candidate(
                &candidate_id,
                ReviewAction::Approve {
                    title: "Remote snapshot flow".into(),
                    usage_hint: "Use when investigating persistence or backup behavior.".into(),
                },
            )
            .unwrap();

        let server = spawn_embedding_server(2);
        let config = super::embedding::EmbeddingConfig::OpenAiCompatible {
            base_url: server.base_url,
            api_key: "test-key".to_string(),
            model: "text-embedding-3-small".to_string(),
            dimensions: 3,
        };

        let matches = store
            .search_history_with_embedding_config(repo_root, "cloud drive backup", 5, &config)
            .unwrap();

        let matched = matches
            .iter()
            .find(|item| item.title == "Remote snapshot flow")
            .expect("real provider vector search should find the WebDAV memory");
        assert!(matched.why_matched.contains("openai-compatible"));

        let conn = store.conn().unwrap();
        let count = conn
            .query_row(
                "SELECT COUNT(*)
                 FROM document_embeddings
                 WHERE embedding_model = ?1
                   AND dimensions = 3",
                [config.model_id()],
                |row| row.get::<_, i64>(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }

    #[test]
    fn rebuild_repo_embeddings_indexes_configured_provider_and_reports_counts() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";
        let candidate_id = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "strategy".to_string(),
                summary: "Remote snapshot flow".to_string(),
                value: "WebDAV sync uploads manifest snapshots to the configured remote path.".to_string(),
                why_it_matters: "Agents need searchable cloud persistence context.".to_string(),
                evidence_refs: vec![],
                confidence: 0.89,
                proposed_by: "codex".to_string(),
            })
            .unwrap();

        store
            .review_candidate(
                &candidate_id,
                ReviewAction::Approve {
                    title: "Remote snapshot flow".into(),
                    usage_hint: "Use when investigating persistence or backup behavior.".into(),
                },
            )
            .unwrap();

        let server = spawn_embedding_server(1);
        let config = super::embedding::EmbeddingConfig::OpenAiCompatible {
            base_url: server.base_url,
            api_key: "test-key".to_string(),
            model: "text-embedding-3-small".to_string(),
            dimensions: 3,
        };

        let report = store
            .rebuild_repo_embeddings_with_config(repo_root, &config)
            .unwrap();

        assert_eq!(report.provider, "openai-compatible");
        assert_eq!(report.embedding_model, "openai-compatible:text-embedding-3-small:3");
        assert_eq!(report.indexed_documents, 1);
        assert_eq!(report.fallback_indexed_documents, 1);
    }

    struct TestEmbeddingServer {
        base_url: String,
    }

    fn spawn_embedding_server(expected_requests: usize) -> TestEmbeddingServer {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        thread::spawn(move || {
            for _ in 0..expected_requests {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buffer = [0_u8; 8192];
                let _ = stream.read(&mut buffer).unwrap();
                let response_body = r#"{"data":[{"embedding":[1.0,0.0,0.0]}]}"#;
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                    response_body.len(),
                    response_body
                );
                stream.write_all(response.as_bytes()).unwrap();
            }
        });

        TestEmbeddingServer {
            base_url: format!("http://{addr}/v1"),
        }
    }

    #[test]
    fn search_history_uses_vector_similarity_when_keywords_miss() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";
        let candidate_id = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "strategy".to_string(),
                summary: "Remote snapshot flow".to_string(),
                value: "WebDAV sync uploads manifest snapshots to the configured remote path.".to_string(),
                why_it_matters: "Agents need searchable persistence context.".to_string(),
                evidence_refs: vec![],
                confidence: 0.89,
                proposed_by: "codex".to_string(),
            })
            .unwrap();

        store
            .review_candidate(
                &candidate_id,
                ReviewAction::Approve {
                    title: "Remote snapshot flow".into(),
                    usage_hint: "Use when investigating persistence behavior.".into(),
                },
            )
            .unwrap();

        let matches = store.search_history(repo_root, "cloud drive backup", 5).unwrap();

        let matched = matches
            .iter()
            .find(|item| item.title == "Remote snapshot flow")
            .expect("semantic vector search should find the WebDAV memory");

        assert_eq!(matched.r#type, "memory");
        assert!(
            matched.why_matched.contains("vector") || matched.why_matched.contains("hybrid"),
            "unexpected match reason: {}",
            matched.why_matched
        );
    }

    #[test]
    fn approving_memory_updates_the_entity_graph() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";
        let candidate_id = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "strategy".to_string(),
                summary: "Tauri updater sync".to_string(),
                value: "WebDAV sync and the Tauri updater both rely on TAURI_PRIVATE_KEY during release packaging.".to_string(),
                why_it_matters: "Future release agents need to see connected project entities.".to_string(),
                evidence_refs: vec![],
                confidence: 0.87,
                proposed_by: "codex".to_string(),
            })
            .unwrap();

        store
            .review_candidate(
                &candidate_id,
                ReviewAction::Approve {
                    title: "Tauri updater sync".into(),
                    usage_hint: "Use for release and sync investigations.".into(),
                },
            )
            .unwrap();

        let graph = store.list_entity_graph(repo_root, 10).unwrap();
        let names = graph
            .entities
            .iter()
            .map(|entity| entity.name.as_str())
            .collect::<Vec<_>>();

        assert!(names.contains(&"WebDAV"));
        assert!(names.contains(&"Tauri"));
        assert!(names.contains(&"TAURI_PRIVATE_KEY"));
        assert!(graph.links.iter().any(|link| link.owner_type == "memory"));
    }

    #[test]
    fn conversation_snapshot_auto_extracts_explicit_memory_candidates() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";
        let now = Utc::now();
        let conversation = Conversation {
            id: "conv-auto-memory".to_string(),
            source_agent: AgentKind::Codex,
            project_dir: repo_root.to_string(),
            created_at: now,
            updated_at: now,
            summary: Some("Explicit memory extraction".to_string()),
            messages: vec![Message {
                id: Uuid::new_v4(),
                timestamp: now,
                role: Role::User,
                content: "Remember: Always run npm run test:run before release.".to_string(),
                tool_calls: vec![],
                metadata: HashMap::new(),
            }],
            file_changes: vec![],
        };

        store
            .upsert_conversation_snapshot("codex", &conversation, None)
            .unwrap();

        let candidates = store
            .list_candidates_with_status(repo_root, Some("pending_review"))
            .unwrap();
        let extracted = candidates
            .iter()
            .find(|candidate| candidate.proposed_by == "auto_extractor")
            .expect("expected explicit memory wording to create a pending candidate");

        assert!(extracted.value.contains("npm run test:run"));
        assert!(extracted.why_it_matters.contains("自动提取"));
        assert_eq!(extracted.status, "pending_review");
        assert!(!extracted.evidence_refs.is_empty());
    }

    #[test]
    fn conflicting_candidate_is_linked_to_approved_memory() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";
        let approved_candidate_id = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "command".to_string(),
                summary: "Use npm test before release".to_string(),
                value: "Use npm run test:run before release.".to_string(),
                why_it_matters: "Primary release verification command.".to_string(),
                evidence_refs: vec![],
                confidence: 0.93,
                proposed_by: "codex".to_string(),
            })
            .unwrap();

        store
            .review_candidate(
                &approved_candidate_id,
                ReviewAction::Approve {
                    title: "Primary release test command".into(),
                    usage_hint: "Run before release packaging.".into(),
                },
            )
            .unwrap();

        let conflicting_candidate_id = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "command".to_string(),
                summary: "Do not use npm test before release".to_string(),
                value: "Do not use npm run test:run before release.".to_string(),
                why_it_matters: "This reverses the earlier release command.".to_string(),
                evidence_refs: vec![],
                confidence: 0.71,
                proposed_by: "claude".to_string(),
            })
            .unwrap();

        let conflicts = store.list_memory_conflicts(repo_root, Some("open")).unwrap();
        let conflict = conflicts
            .iter()
            .find(|item| item.candidate_id == conflicting_candidate_id)
            .expect("expected negated overlapping candidate to be flagged");

        assert_eq!(conflict.memory_title, "Primary release test command");
        assert!(conflict.reason.contains("冲突"));

        let candidates = store
            .list_candidates_with_status(repo_root, Some("pending_review"))
            .unwrap();
        let candidate = candidates
            .iter()
            .find(|item| item.candidate_id == conflicting_candidate_id)
            .unwrap();
        assert!(candidate.conflict_suggestion.is_some());
    }

    #[test]
    fn reverify_memory_marks_it_fresh_and_tracks_the_reviewer() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";
        let candidate_id = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "command".to_string(),
                summary: "Run tests".to_string(),
                value: "npm run test:run".to_string(),
                why_it_matters: "Needed before merge".to_string(),
                evidence_refs: vec![],
                confidence: 0.92,
                proposed_by: "codex".to_string(),
            })
            .unwrap();

        store
            .review_candidate(
                &candidate_id,
                ReviewAction::Approve {
                    title: "Primary test command".into(),
                    usage_hint: "Inject on startup when the task hint mentions tests".into(),
                },
            )
            .unwrap();

        let memory_id = store.list_repo_memories(repo_root).unwrap()[0].memory_id.clone();

        store.reverify_memory(&memory_id, "claude").unwrap();

        let memories = store.list_repo_memories(repo_root).unwrap();
        assert_eq!(memories[0].freshness_status, "fresh");
        assert_eq!(memories[0].freshness_score, 1.0);
        assert_eq!(memories[0].verified_by.as_deref(), Some("claude"));
        assert!(memories[0].verified_at.is_some());
        assert_eq!(memories[0].last_verified_at, memories[0].verified_at);
    }

    #[test]
    fn list_repo_memories_decays_freshness_from_verification_age() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";

        let needs_review_candidate = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "command".to_string(),
                summary: "Recent-ish verification".to_string(),
                value: "npm run lint".to_string(),
                why_it_matters: "Still relevant but should be checked soon".to_string(),
                evidence_refs: vec![],
                confidence: 0.91,
                proposed_by: "codex".to_string(),
            })
            .unwrap();
        store
            .review_candidate(
                &needs_review_candidate,
                ReviewAction::Approve {
                    title: "Lint command".into(),
                    usage_hint: "Use before submitting changes".into(),
                },
            )
            .unwrap();

        let stale_candidate = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "command".to_string(),
                summary: "Old verification".to_string(),
                value: "cargo test".to_string(),
                why_it_matters: "Needs a much fresher validation".to_string(),
                evidence_refs: vec![],
                confidence: 0.88,
                proposed_by: "claude".to_string(),
            })
            .unwrap();
        store
            .review_candidate(
                &stale_candidate,
                ReviewAction::Approve {
                    title: "Rust test command".into(),
                    usage_hint: "Use before merging Rust changes".into(),
                },
            )
            .unwrap();

        let memories = store.list_repo_memories(repo_root).unwrap();
        let lint_memory_id = memories
            .iter()
            .find(|memory| memory.title == "Lint command")
            .unwrap()
            .memory_id
            .clone();
        let rust_memory_id = memories
            .iter()
            .find(|memory| memory.title == "Rust test command")
            .unwrap()
            .memory_id
            .clone();

        let needs_review_at = (chrono::Utc::now() - chrono::Duration::days(10)).to_rfc3339();
        let stale_at = (chrono::Utc::now() - chrono::Duration::days(45)).to_rfc3339();
        let conn = store.conn().unwrap();
        conn.execute(
            "UPDATE approved_memories
             SET last_verified_at = ?2,
                 verified_at = ?2,
                 freshness_status = 'fresh',
                 freshness_score = 1.0
             WHERE memory_id = ?1",
            params![lint_memory_id, needs_review_at],
        )
        .unwrap();
        conn.execute(
            "UPDATE approved_memories
             SET last_verified_at = ?2,
                 verified_at = ?2,
                 freshness_status = 'fresh',
                 freshness_score = 1.0
             WHERE memory_id = ?1",
            params![rust_memory_id, stale_at],
        )
        .unwrap();

        let memories = store.list_repo_memories(repo_root).unwrap();
        let lint_memory = memories
            .iter()
            .find(|memory| memory.title == "Lint command")
            .unwrap();
        let rust_memory = memories
            .iter()
            .find(|memory| memory.title == "Rust test command")
            .unwrap();

        assert_eq!(lint_memory.freshness_status, "needs_review");
        assert!(lint_memory.freshness_score < 1.0);
        assert_eq!(rust_memory.freshness_status, "stale");
        assert!(rust_memory.freshness_score < lint_memory.freshness_score);
    }

    #[test]
    fn handoff_builder_excludes_non_fresh_approved_memories() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";

        let memories = [
            ("Fresh command", Some(0_i64)),
            ("Needs review command", Some(10_i64)),
            ("Stale command", Some(45_i64)),
            ("Unknown command", None),
        ];

        let conn = store.conn().unwrap();
        for (title, days_ago) in memories {
            let candidate_id = store
                .create_candidate(&CreateMemoryCandidateInput {
                    repo_root: repo_root.to_string(),
                    kind: "command".to_string(),
                    summary: title.to_string(),
                    value: format!("echo {title}"),
                    why_it_matters: "Should only ship when fresh".to_string(),
                    evidence_refs: vec![],
                    confidence: 0.86,
                    proposed_by: "codex".to_string(),
                })
                .unwrap();
            store
                .review_candidate(
                    &candidate_id,
                    ReviewAction::Approve {
                        title: title.to_string(),
                        usage_hint: "Keep the gate tight".to_string(),
                    },
                )
                .unwrap();

            let memory_id = store
                .list_repo_memories(repo_root)
                .unwrap()
                .into_iter()
                .find(|memory| memory.title == title)
                .map(|memory| memory.memory_id)
                .unwrap();

            match days_ago {
                Some(days_ago) => {
                    let verification_at =
                        (chrono::Utc::now() - chrono::Duration::days(days_ago)).to_rfc3339();
                    conn.execute(
                        "UPDATE approved_memories
                         SET last_verified_at = ?2,
                             verified_at = ?2,
                             freshness_status = 'fresh',
                             freshness_score = 1.0
                         WHERE memory_id = ?1",
                        params![memory_id, verification_at],
                    )
                    .unwrap();
                }
                None => {
                    conn.execute(
                        "UPDATE approved_memories
                         SET last_verified_at = NULL,
                             verified_at = NULL,
                             freshness_status = 'unknown',
                             freshness_score = 0.0
                         WHERE memory_id = ?1",
                        params![memory_id],
                    )
                    .unwrap();
                }
            }
        }

        let packet = store
            .build_and_store_handoff(repo_root, "codex", "claude", Some("Wrap schema changes"))
            .unwrap();

        let related_titles = packet
            .related_memories
            .iter()
            .map(|memory| memory.title.as_str())
            .collect::<Vec<_>>();

        assert_eq!(related_titles, vec!["Fresh command"]);
        assert!(packet
            .related_memories
            .iter()
            .all(|memory| memory.freshness_status == "fresh"));
    }

    #[test]
    fn suggest_memory_merges_flags_candidates_that_overlap_existing_memory() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";
        let approved_candidate_id = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "command".to_string(),
                summary: "Run tests before merge".to_string(),
                value: "npm run test:run".to_string(),
                why_it_matters: "Primary verification command".to_string(),
                evidence_refs: vec![],
                confidence: 0.95,
                proposed_by: "codex".to_string(),
            })
            .unwrap();

        store
            .review_candidate(
                &approved_candidate_id,
                ReviewAction::Approve {
                    title: "Primary verification".into(),
                    usage_hint: "Use before merge".into(),
                },
            )
            .unwrap();

        let candidate_id = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "command".to_string(),
                summary: "Run tests before shipping".to_string(),
                value: "npm run test:run".to_string(),
                why_it_matters: "Same command with updated wording".to_string(),
                evidence_refs: vec![],
                confidence: 0.74,
                proposed_by: "claude".to_string(),
            })
            .unwrap();

        let suggestions = store.suggest_memory_merges(repo_root).unwrap();
        let suggestion = suggestions
            .into_iter()
            .find(|item| item.candidate_id == candidate_id)
            .expect("expected merge suggestion for overlapping command memory");

        assert_eq!(suggestion.memory_title, "Primary verification");
        assert!(suggestion.reason.contains("合并"));
    }

    #[test]
    fn merge_suggestion_includes_a_reviewable_rewrite_proposal() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";
        let approved_candidate_id = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "command".to_string(),
                summary: "Run tests before merge".to_string(),
                value: "npm run test:run".to_string(),
                why_it_matters: "Primary verification command".to_string(),
                evidence_refs: vec![],
                confidence: 0.95,
                proposed_by: "codex".to_string(),
            })
            .unwrap();

        store
            .review_candidate(
                &approved_candidate_id,
                ReviewAction::Approve {
                    title: "Primary verification".into(),
                    usage_hint: "Use before merge".into(),
                },
            )
            .unwrap();

        let candidate_id = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "command".to_string(),
                summary: "Run tests before release".to_string(),
                value: "npm run test:run -- --runInBand".to_string(),
                why_it_matters: "Use the serial variant before release packaging".to_string(),
                evidence_refs: vec![],
                confidence: 0.82,
                proposed_by: "claude".to_string(),
            })
            .unwrap();

        let candidate = store
            .list_candidates_with_status(repo_root, Some("pending_review"))
            .unwrap()
            .into_iter()
            .find(|item| item.candidate_id == candidate_id)
            .unwrap();
        let proposal = candidate
            .merge_suggestion
            .expect("expected merge suggestion to carry a rewrite proposal");

        assert_eq!(proposal.memory_title, "Primary verification");
        assert_eq!(
            proposal.proposed_title.as_deref(),
            Some("Primary verification")
        );
        assert_eq!(
            proposal.proposed_value.as_deref(),
            Some("npm run test:run\n\n更新：npm run test:run -- --runInBand")
        );
        assert_eq!(
            proposal.proposed_usage_hint.as_deref(),
            Some("Use before merge\n\n更新：Use the serial variant before release packaging")
        );
        assert!(proposal
            .risk_note
            .as_deref()
            .unwrap_or_default()
            .contains("批准"));

        let memories = store.list_repo_memories(repo_root).unwrap();
        assert_eq!(memories[0].value, "npm run test:run");
    }

    #[test]
    fn approving_a_merge_updates_existing_memory_without_creating_a_duplicate() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";
        let approved_candidate_id = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "command".to_string(),
                summary: "Run tests before merge".to_string(),
                value: "npm run test:run".to_string(),
                why_it_matters: "Primary verification command".to_string(),
                evidence_refs: vec![],
                confidence: 0.95,
                proposed_by: "codex".to_string(),
            })
            .unwrap();

        store
            .review_candidate(
                &approved_candidate_id,
                ReviewAction::Approve {
                    title: "Primary verification".into(),
                    usage_hint: "Use before merge".into(),
                },
            )
            .unwrap();
        let memory_id = store.list_repo_memories(repo_root).unwrap()[0]
            .memory_id
            .clone();

        let candidate_id = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "command".to_string(),
                summary: "Run tests before release".to_string(),
                value: "npm run test:run -- --runInBand".to_string(),
                why_it_matters: "Use the serial variant before release packaging".to_string(),
                evidence_refs: vec![],
                confidence: 0.82,
                proposed_by: "claude".to_string(),
            })
            .unwrap();

        store
            .review_candidate(
                &candidate_id,
                ReviewAction::ApproveMerge {
                    memory_id: memory_id.clone(),
                    title: "Primary verification".into(),
                    value: "npm run test:run\n\n更新：npm run test:run -- --runInBand".into(),
                    usage_hint:
                        "Use before merge\n\n更新：Use the serial variant before release packaging"
                            .into(),
                },
            )
            .unwrap();

        let memories = store.list_repo_memories(repo_root).unwrap();
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].memory_id, memory_id);
        assert!(memories[0].value.contains("--runInBand"));
        assert!(memories[0].usage_hint.contains("release packaging"));

        let candidates = store.list_candidates(repo_root).unwrap();
        let candidate = candidates
            .iter()
            .find(|item| item.candidate_id == candidate_id)
            .unwrap();
        assert_eq!(candidate.status, "approved");

        let matches = store.search_history(repo_root, "serial release packaging", 5).unwrap();
        assert!(matches.iter().any(|item| item.title == "Primary verification"));
    }

    #[test]
    fn agent_authored_merge_proposal_overrides_deterministic_merge_suggestion() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";
        let approved_candidate_id = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "command".to_string(),
                summary: "Run tests before merge".to_string(),
                value: "npm run test:run".to_string(),
                why_it_matters: "Primary verification command".to_string(),
                evidence_refs: vec![],
                confidence: 0.95,
                proposed_by: "codex".to_string(),
            })
            .unwrap();

        store
            .review_candidate(
                &approved_candidate_id,
                ReviewAction::Approve {
                    title: "Primary verification".into(),
                    usage_hint: "Use before merge".into(),
                },
            )
            .unwrap();
        let memory_id = store.list_repo_memories(repo_root).unwrap()[0]
            .memory_id
            .clone();

        let candidate_id = store
            .create_candidate(&CreateMemoryCandidateInput {
                repo_root: repo_root.to_string(),
                kind: "command".to_string(),
                summary: "Run tests before release".to_string(),
                value: "npm run test:run -- --runInBand".to_string(),
                why_it_matters: "Use the serial variant before release packaging".to_string(),
                evidence_refs: vec![],
                confidence: 0.82,
                proposed_by: "claude".to_string(),
            })
            .unwrap();

        let proposal_id = store
            .propose_memory_merge(&CreateMemoryMergeProposalInput {
                repo_root: repo_root.to_string(),
                candidate_id: candidate_id.clone(),
                target_memory_id: memory_id.clone(),
                proposed_title: "Primary verification".to_string(),
                proposed_value: "npm run test:run\n\nBefore packaging, use npm run test:run -- --runInBand.".to_string(),
                proposed_usage_hint: "Use before merge; prefer the serial variant before release packaging.".to_string(),
                risk_note: Some("Agent-authored rewrite; review wording before approval.".to_string()),
                proposed_by: "codex".to_string(),
                evidence_refs: vec![],
            })
            .unwrap();

        let candidate = store
            .list_candidates_with_status(repo_root, Some("pending_review"))
            .unwrap()
            .into_iter()
            .find(|item| item.candidate_id == candidate_id)
            .unwrap();
        let proposal = candidate.merge_suggestion.unwrap();

        assert_eq!(proposal.proposal_id.as_deref(), Some(proposal_id.as_str()));
        assert_eq!(proposal.memory_id, memory_id);
        assert_eq!(proposal.proposed_by.as_deref(), Some("codex"));
        assert_eq!(
            proposal.proposed_value.as_deref(),
            Some("npm run test:run\n\nBefore packaging, use npm run test:run -- --runInBand.")
        );
        assert_eq!(
            proposal.proposed_usage_hint.as_deref(),
            Some("Use before merge; prefer the serial variant before release packaging.")
        );
    }

    #[test]
    fn marking_a_handoff_consumed_updates_lifecycle_metadata() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";
        let packet = store
            .build_and_store_handoff(repo_root, "codex", "claude", Some("Wrap schema changes"))
            .unwrap();

        store
            .mark_handoff_consumed(&packet.handoff_id, "claude")
            .unwrap();

        let latest = store.latest_handoff(repo_root).unwrap().unwrap();
        assert_eq!(latest.status, "consumed");
        assert_eq!(latest.consumed_by.as_deref(), Some("claude"));
        assert!(latest.consumed_at.is_some());
    }

    #[test]
    fn mark_handoff_consumed_rejects_wrong_agent() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";
        let packet = store
            .build_and_store_handoff(repo_root, "codex", "claude", Some("Wrap schema changes"))
            .unwrap();

        let error = store
            .mark_handoff_consumed(&packet.handoff_id, "gemini")
            .unwrap_err()
            .to_string();

        assert!(error.contains("cannot be consumed by gemini"));

        let latest = store.latest_handoff(repo_root).unwrap().unwrap();
        assert_eq!(latest.status, packet.status);
        assert!(latest.consumed_at.is_none());
        assert!(latest.consumed_by.is_none());
    }

    #[test]
    fn mark_handoff_consumed_does_not_overwrite_existing_audit_metadata() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";
        let packet = store
            .build_and_store_handoff(repo_root, "codex", "claude", Some("Wrap schema changes"))
            .unwrap();

        store
            .mark_handoff_consumed(&packet.handoff_id, "claude")
            .unwrap();

        let consumed = store.latest_handoff(repo_root).unwrap().unwrap();
        let original_consumed_at = consumed.consumed_at.clone();
        let original_consumed_by = consumed.consumed_by.clone();

        thread::sleep(Duration::from_millis(5));

        let error = store
            .mark_handoff_consumed(&packet.handoff_id, "claude")
            .unwrap_err()
            .to_string();

        assert!(error.contains("already consumed"));

        let latest = store.latest_handoff(repo_root).unwrap().unwrap();
        assert_eq!(latest.status, "consumed");
        assert_eq!(latest.consumed_at, original_consumed_at);
        assert_eq!(latest.consumed_by, original_consumed_by);
    }

    #[test]
    fn building_a_handoff_with_target_profile_persists_it() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";

        let packet = store
            .build_and_store_handoff_for_target_profile(
                repo_root,
                "codex",
                "claude",
                Some("Wrap schema changes"),
                Some("claude_contextual"),
            )
            .unwrap();

        assert_eq!(packet.target_profile.as_deref(), Some("claude_contextual"));

        let latest = store.latest_handoff(repo_root).unwrap().unwrap();
        assert_eq!(latest.target_profile.as_deref(), Some("claude_contextual"));
    }

    #[test]
    fn create_checkpoint_persists_an_active_resume_snapshot() {
        let store = new_store();
        let checkpoint = store
            .create_checkpoint(&CreateCheckpointInput {
                repo_root: "d:/vsp/agentswap-gui".to_string(),
                conversation_id: "claude:conv-001".to_string(),
                source_agent: "claude".to_string(),
                summary: "Freeze the current debugging state".to_string(),
                resume_command: Some("claude --resume conv-001".to_string()),
                metadata_json: Some("{\"storage_path\":\"C:/Users/demo/.claude/projects/conv-001.jsonl\"}".to_string()),
            })
            .unwrap();

        assert_eq!(checkpoint.status, "active");

        let checkpoints = store.list_checkpoints("d:/vsp/agentswap-gui").unwrap();
        assert_eq!(checkpoints.len(), 1);
        assert_eq!(checkpoints[0].resume_command.as_deref(), Some("claude --resume conv-001"));
        assert_eq!(checkpoints[0].status, "active");
    }

    #[test]
    fn promoting_a_checkpoint_to_handoff_links_both_records() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";

        let checkpoint = store
            .create_checkpoint(&CreateCheckpointInput {
                repo_root: repo_root.to_string(),
                conversation_id: "claude:conv-001".to_string(),
                source_agent: "claude".to_string(),
                summary: "Freeze the current debugging state".to_string(),
                resume_command: Some("claude --resume conv-001".to_string()),
                metadata_json: None,
            })
            .unwrap();

        let packet = store
            .build_and_store_handoff_from_checkpoint(
                &checkpoint.checkpoint_id,
                "claude",
                "codex",
                Some("Continue from the frozen checkpoint"),
                Some("codex_execution"),
            )
            .unwrap();

        assert_eq!(packet.checkpoint_id.as_deref(), Some(checkpoint.checkpoint_id.as_str()));

        let checkpoints = store.list_checkpoints(repo_root).unwrap();
        assert_eq!(checkpoints[0].status, "promoted");
        assert_eq!(checkpoints[0].handoff_id.as_deref(), Some(packet.handoff_id.as_str()));
    }

    #[test]
    fn promoting_a_checkpoint_uses_checkpoint_provenance_and_goal() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";

        let checkpoint = store
            .create_checkpoint(&CreateCheckpointInput {
                repo_root: repo_root.to_string(),
                conversation_id: "codex:conv-777".to_string(),
                source_agent: "codex".to_string(),
                summary: "Checkpoint-owned goal".to_string(),
                resume_command: Some("codex resume conv-777".to_string()),
                metadata_json: None,
            })
            .unwrap();

        let packet = store
            .build_and_store_handoff_from_checkpoint(
                &checkpoint.checkpoint_id,
                "claude",
                "gemini",
                Some("Wrong UI goal"),
                Some("gemini_research"),
            )
            .unwrap();

        assert_eq!(packet.from_agent, "codex");
        assert_eq!(packet.to_agent, "gemini");
        assert_eq!(packet.current_goal, "Checkpoint-owned goal");
        assert!(packet
            .done_items
            .iter()
            .any(|item| item.contains("已从 codex checkpoint 固化上下文")));
        assert!(packet
            .useful_commands
            .iter()
            .any(|command| command == "codex resume conv-777"));
    }

    #[test]
    fn promoting_the_same_checkpoint_twice_is_rejected_without_creating_a_duplicate_handoff() {
        let store = new_store();
        let repo_root = "d:/vsp/agentswap-gui";

        let checkpoint = store
            .create_checkpoint(&CreateCheckpointInput {
                repo_root: repo_root.to_string(),
                conversation_id: "claude:conv-001".to_string(),
                source_agent: "claude".to_string(),
                summary: "Freeze the current debugging state".to_string(),
                resume_command: Some("claude --resume conv-001".to_string()),
                metadata_json: None,
            })
            .unwrap();

        let repo_id = store.ensure_repo(repo_root).unwrap();
        let existing_handoff_id = "handoff-existing".to_string();
        let conn = store.conn().unwrap();
        conn.execute(
            "INSERT INTO handoff_packets (
                handoff_id, repo_id, from_agent, to_agent, status, target_profile, checkpoint_id,
                compression_strategy, current_goal,
                done_json, next_json, key_files_json, commands_json,
                related_memories_json, related_episodes_json, consumed_at, consumed_by, created_at
             ) VALUES (?1, ?2, ?3, ?4, 'draft', NULL, ?5, NULL, ?6, '[]', '[]', '[]', '[]', '[]', '[]', NULL, NULL, ?7)",
            params![
                existing_handoff_id,
                repo_id,
                "claude",
                "codex",
                checkpoint.checkpoint_id,
                "Freeze the current debugging state",
                "2026-04-20T10:40:00Z",
            ],
        )
        .unwrap();

        let error = store
            .build_and_store_handoff_from_checkpoint(
                &checkpoint.checkpoint_id,
                "gemini",
                "claude",
                Some("Wrong second goal"),
                Some("claude_contextual"),
            )
            .unwrap_err()
            .to_string();

        assert!(
            error.contains("already") || error.contains("duplicate") || error.contains("checkpoint"),
            "unexpected error: {error}"
        );

        let handoffs = store.list_handoffs(repo_root).unwrap();
        assert_eq!(handoffs.len(), 1);
        assert_eq!(handoffs[0].handoff_id, existing_handoff_id);

        let checkpoints = store.list_checkpoints(repo_root).unwrap();
        assert_eq!(checkpoints[0].status, "active");
        assert_eq!(checkpoints[0].handoff_id.as_deref(), Some(existing_handoff_id.as_str()));
    }
}
