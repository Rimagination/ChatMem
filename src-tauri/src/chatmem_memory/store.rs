use anyhow::{Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use agentswap_core::types::{ChangeType, Conversation, Role, ToolStatus};

use super::{
    checkpoints::{CheckpointRecord, CreateCheckpointInput},
    db,
    handoff,
    models::{
        ApprovedMemoryResponse, CreateMemoryCandidateInput, EpisodeResponse, EvidenceRef,
        HandoffPacketResponse, MemoryCandidateResponse, MemoryMergeSuggestion, SearchHistoryMatch,
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

    fn conn(&self) -> Result<Connection> {
        db::open_connection(&self.db_path)
    }

    pub fn ensure_repo(&self, repo_root: &str) -> Result<String> {
        let repo_root = repo_identity::normalize_repo_root(repo_root);
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
        tx.commit()?;

        Ok(candidate_id)
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
                let evidence = load_evidence_refs_from_conn(&tx, "candidate", candidate_id)?;
                replace_evidence_refs_tx(&tx, "memory", &memory_id, &evidence)?;
                upsert_search_document_tx(
                    &tx,
                    &format!("memory:{memory_id}"),
                    &candidate.0,
                    "memory",
                    &memory_id,
                    &title,
                    &candidate.3,
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
                let evidence = load_evidence_refs_from_conn(&tx, "candidate", candidate_id)?;
                replace_evidence_refs_tx(&tx, "memory", &memory_id, &evidence)?;
                upsert_search_document_tx(
                    &tx,
                    &format!("memory:{memory_id}"),
                    &candidate.0,
                    "memory",
                    &memory_id,
                    &title,
                    &value,
                )?;
            }
            ReviewAction::Reject => {
                tx.execute(
                    "UPDATE memory_candidates SET status = 'rejected', reviewed_at = ?2 WHERE candidate_id = ?1",
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

    pub fn list_repo_memories(&self, repo_root: &str) -> Result<Vec<ApprovedMemoryResponse>> {
        let repo_id = self.ensure_repo(repo_root)?;
        let conn = self.conn()?;
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
                })
            })?
            .collect::<Result<Vec<_>, _>>()?
        };

        let merge_suggestions = self
            .suggest_memory_merges_by_repo_id(repo_id)?
            .into_iter()
            .map(|suggestion| (suggestion.candidate_id.clone(), suggestion))
            .collect::<HashMap<_, _>>();
        let mut candidates = rows;
        for candidate in &mut candidates {
            candidate.evidence_refs = self.evidence_refs("candidate", &candidate.candidate_id)?;
            candidate.merge_suggestion = merge_suggestions.get(&candidate.candidate_id).cloned();
        }
        Ok(candidates)
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
                    let score = merge_similarity(&summary, &value, &why_it_matters, title, memory_value, usage_hint);
                    if score >= 0.72 {
                        Some((score, memory_id, title, memory_value))
                    } else {
                        None
                    }
                })
                .max_by(|left, right| left.0.partial_cmp(&right.0).unwrap_or(std::cmp::Ordering::Equal));

            if let Some((score, memory_id, title, memory_value)) = best_match {
                let reason = if normalize_text(&value) == normalize_text(memory_value) {
                    format!(
                        "This candidate matches the approved memory value and should be merge-reviewed instead of stored twice (score {:.2}).",
                        score
                    )
                } else {
                    format!(
                        "This candidate overlaps an approved memory and likely needs a merge-aware review (score {:.2}).",
                        score
                    )
                };

                suggestions.push(MemoryMergeSuggestion {
                    candidate_id,
                    memory_id: memory_id.clone(),
                    memory_title: title.clone(),
                    reason,
                });
            }
        }

        Ok(suggestions)
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
            .filter(|memory| memory.status == "active")
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
            .filter(|memory| memory.status == "active")
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
        let done_items = vec![format!("Checkpoint frozen from {source_agent}: {summary}")];
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
        let repo_id = self.ensure_repo(repo_root)?;
        let conn = self.conn()?;
        let mut matches = Vec::new();

        let fts_result = conn.prepare(
            "SELECT sd.doc_type, sd.doc_ref_id, sd.title, sd.body
             FROM search_documents_fts
             JOIN search_documents sd ON sd.doc_id = search_documents_fts.doc_id
             WHERE sd.repo_id = ?1 AND search_documents_fts MATCH ?2
             ORDER BY bm25(search_documents_fts)
             LIMIT ?3",
        );

        if let Ok(mut stmt) = fts_result {
            let rows = stmt.query_map(params![repo_id.clone(), query, limit as i64], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })?;

            for row in rows {
                let (doc_type, doc_ref_id, title, body) = row?;
                matches.push(SearchHistoryMatch {
                    r#type: doc_type.clone(),
                    title,
                    summary: truncate_text(&body, 280),
                    why_matched: "Repository history search match".to_string(),
                    score: 1.0,
                    evidence_refs: self.evidence_refs(evidence_owner_for_doc_type(&doc_type), &doc_ref_id)?,
                });
            }
        }

        if matches.is_empty() {
            let like = format!("%{}%", query.to_lowercase());
            let mut stmt = conn.prepare(
                "SELECT doc_type, doc_ref_id, title, body
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
                ))
            })?;

            for row in rows {
                let (doc_type, doc_ref_id, title, body) = row?;
                matches.push(SearchHistoryMatch {
                    r#type: doc_type.clone(),
                    title,
                    summary: truncate_text(&body, 280),
                    why_matched: "Repository history search fallback match".to_string(),
                    score: 0.5,
                    evidence_refs: self.evidence_refs(evidence_owner_for_doc_type(&doc_type), &doc_ref_id)?,
                });
            }
        }

        Ok(matches)
    }

    pub fn evidence_refs(&self, owner_type: &str, owner_id: &str) -> Result<Vec<EvidenceRef>> {
        let conn = self.conn()?;
        load_evidence_refs_from_conn(&conn, owner_type, owner_id)
    }
}

fn evidence_owner_for_doc_type(doc_type: &str) -> &'static str {
    match doc_type {
        "memory" => "memory",
        "episode" => "episode",
        _ => "conversation",
    }
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
    Ok(())
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
        models::CreateMemoryCandidateInput,
    };
    use rusqlite::params;
    use std::{thread, time::Duration};

    fn new_store() -> MemoryStore {
        let path = std::env::temp_dir().join(format!("chatmem-store-test-{}.sqlite", uuid::Uuid::new_v4()));
        MemoryStore::new(path).unwrap()
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
        assert!(suggestion.reason.contains("merge"));
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
            .any(|item| item.contains("Checkpoint frozen from codex")));
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
        assert!(checkpoints[0].handoff_id.is_none());
    }
}
