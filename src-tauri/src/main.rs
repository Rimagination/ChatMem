#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod agent_integration;

use std::{
    fs,
    path::{Path, PathBuf},
    time::Duration,
};

use chatmem::chatmem_memory::{
    a2a::AgentCard,
    checkpoints::{CheckpointRecord, CreateCheckpointInput},
    mcp::ChatMemMcpService,
    models::{
        ApprovedMemoryResponse, EmbeddingRebuildReport, EntityGraphPayload, EpisodeResponse,
        HandoffPacketResponse, LocalHistoryImportReport, MemoryCandidateResponse,
        MemoryConflictResponse, ProjectContextPayload, RepoAliasResponse, RepoMemoryHealthResponse,
        RepoScanReport, WikiPageResponse,
    },
    runs::{list_artifacts as load_artifacts, list_runs as load_runs, ArtifactRecord, RunRecord},
    store::{MemoryStore, ReviewAction},
    sync::{
        build_resume_command, import_all_local_history as sync_import_all_local_history,
        resolve_storage_path, scan_repo_conversations as sync_scan_repo_conversations,
        sync_conversation_into_store,
    },
};
use rmcp::{transport::stdio, ServiceExt};
use serde::{Deserialize, Serialize};
use tauri::command;

// Import AgentSwap adapters
use agentswap_claude::ClaudeAdapter;
use agentswap_codex::CodexAdapter;
use agentswap_core::adapter::AgentAdapter;
use agentswap_core::types::{AgentKind, Conversation, ConversationSummary};
use agentswap_gemini::GeminiAdapter;
use agentswap_opencode::OpenCodeAdapter;

const DEFAULT_TRASH_RETENTION_DAYS: i64 = 14;

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConversationSummaryResponse {
    id: String,
    source_agent: String,
    project_dir: String,
    created_at: String,
    updated_at: String,
    summary: Option<String>,
    message_count: usize,
    file_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConversationResponse {
    id: String,
    source_agent: String,
    project_dir: String,
    created_at: String,
    updated_at: String,
    summary: Option<String>,
    storage_path: Option<String>,
    resume_command: Option<String>,
    messages: Vec<MessageResponse>,
    file_changes: Vec<FileChangeResponse>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationVerificationResponse {
    read_back: bool,
    listed: bool,
    source_message_count: usize,
    target_message_count: usize,
    source_file_count: usize,
    target_file_count: usize,
    first_user_preserved: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MigrationResponse {
    new_id: String,
    source: String,
    target: String,
    mode: String,
    verified: bool,
    verification: MigrationVerificationResponse,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MessageResponse {
    id: String,
    timestamp: String,
    role: String,
    content: String,
    tool_calls: Vec<ToolCallResponse>,
    metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ToolCallResponse {
    name: String,
    input: serde_json::Value,
    output: Option<String>,
    status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct FileChangeResponse {
    path: String,
    change_type: String,
    timestamp: String,
    message_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebDavSyncResponse {
    uploaded_count: usize,
    remote_url: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebDavManifest {
    schema_version: u8,
    app_version: String,
    synced_at: String,
    conversations: Vec<WebDavManifestEntry>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebDavManifestEntry {
    source_agent: String,
    id: String,
    project_dir: String,
    updated_at: String,
    remote_file: String,
}

struct WebDavConversationUpload {
    agent: String,
    id: String,
    project_dir: String,
    updated_at: String,
    file_name: String,
    remote_file: String,
    body: Vec<u8>,
}

#[derive(Debug, Clone)]
struct WebDavDeleteSettings {
    webdav_scheme: String,
    webdav_host: String,
    webdav_path: String,
    remote_path: String,
    username: String,
    password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SyncSettingsPayload {
    provider: String,
    webdav_scheme: String,
    webdav_host: String,
    webdav_path: String,
    username: String,
    remote_path: String,
    download_mode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AppSettingsPayload {
    locale: String,
    #[serde(default = "default_font_family")]
    font_family: String,
    auto_check_updates: bool,
    #[serde(default = "default_trash_retention_days")]
    trash_retention_days: i64,
    sync: SyncSettingsPayload,
}

fn default_font_family() -> String {
    "system".to_string()
}

fn default_trash_retention_days() -> i64 {
    DEFAULT_TRASH_RETENTION_DAYS
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TrashConversationRecord {
    schema_version: u8,
    trash_id: String,
    original_id: String,
    source_agent: String,
    project_dir: String,
    summary: Option<String>,
    trashed_at: String,
    expires_at: String,
    storage_path: Option<String>,
    resume_command: Option<String>,
    remote_backup_deleted: bool,
    remote_backup_path: Option<String>,
    warnings: Vec<String>,
    conversation: Conversation,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TrashConversationResponse {
    trash_id: String,
    original_id: String,
    source_agent: String,
    project_dir: String,
    summary: Option<String>,
    trashed_at: String,
    expires_at: String,
    storage_path: Option<String>,
    resume_command: Option<String>,
    remote_backup_deleted: bool,
    remote_backup_path: Option<String>,
    warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RestoreTrashResponse {
    trash_id: String,
    original_id: String,
    restored_id: String,
    source_agent: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpgradeReadinessCheck {
    key: String,
    label: String,
    status: String,
    detail: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct UpgradeReadinessReport {
    status: String,
    summary: String,
    checks: Vec<UpgradeReadinessCheck>,
    warnings: Vec<String>,
}

#[derive(Debug, Clone)]
enum CredentialCheck {
    NotNeeded,
    Present,
    Missing,
    Error(String),
}

fn get_adapter(agent: &str) -> Result<Box<dyn AgentAdapter>, String> {
    match agent {
        "claude" => Ok(Box::new(ClaudeAdapter::new())),
        "codex" => Ok(Box::new(CodexAdapter::new())),
        "gemini" => Ok(Box::new(GeminiAdapter::new())),
        "opencode" => Ok(Box::new(OpenCodeAdapter::new())),
        _ => Err(format!("Unknown agent: {}", agent)),
    }
}

fn agent_key(agent: &AgentKind) -> &'static str {
    match agent {
        AgentKind::Claude => "claude",
        AgentKind::Codex => "codex",
        AgentKind::Gemini => "gemini",
        AgentKind::OpenCode => "opencode",
    }
}

fn contains_query(haystack: &str, query: &str) -> bool {
    let haystack = haystack.to_lowercase();
    if haystack.contains(query) {
        return true;
    }
    cjk_query_terms(query)
        .iter()
        .any(|term| haystack.contains(term))
}

fn cjk_query_terms(query: &str) -> Vec<String> {
    if !query.chars().any(is_cjk) {
        return vec![];
    }

    let mut terms = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let normalized = query
        .to_lowercase()
        .chars()
        .map(|ch| if ch.is_alphanumeric() { ch } else { ' ' })
        .collect::<String>();

    for segment in normalized.split_whitespace() {
        let cleaned = clean_cjk_query_segment(segment);
        for term in cleaned.split_whitespace() {
            push_cjk_query_term(&mut terms, &mut seen, term);
            let chars = term.chars().collect::<Vec<_>>();
            for window_len in [4usize, 3usize] {
                if chars.len() < window_len {
                    continue;
                }
                for window in chars.windows(window_len) {
                    let ngram = window.iter().collect::<String>();
                    push_cjk_query_term(&mut terms, &mut seen, &ngram);
                    if terms.len() >= 12 {
                        return terms;
                    }
                }
            }
        }
    }

    terms
}

fn clean_cjk_query_segment(segment: &str) -> String {
    let mut cleaned = segment.to_string();
    for phrase in CJK_QUERY_NOISE_PHRASES {
        cleaned = cleaned.replace(phrase, " ");
    }
    cleaned.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn push_cjk_query_term(
    terms: &mut Vec<String>,
    seen: &mut std::collections::HashSet<String>,
    term: &str,
) {
    let term = term.trim().to_string();
    if terms.len() >= 12
        || term.chars().count() < 2
        || CJK_QUERY_NOISE_PHRASES.contains(&term.as_str())
    {
        return;
    }
    if seen.insert(term.clone()) {
        terms.push(term);
    }
}

fn is_cjk(ch: char) -> bool {
    matches!(
        ch,
        '\u{3400}'..='\u{4DBF}'
            | '\u{4E00}'..='\u{9FFF}'
            | '\u{F900}'..='\u{FAFF}'
            | '\u{20000}'..='\u{2A6DF}'
            | '\u{2A700}'..='\u{2B73F}'
            | '\u{2B740}'..='\u{2B81F}'
            | '\u{2B820}'..='\u{2CEAF}'
    )
}

const CJK_QUERY_NOISE_PHRASES: &[&str] = &[
    "你还记得",
    "还记得",
    "记得",
    "我们之前",
    "之前",
    "以前",
    "上次",
    "讨论过",
    "聊过",
    "说过",
    "提到过",
    "有没有",
    "是不是",
    "是否",
    "这个",
    "那个",
    "关于",
    "有关",
    "项目",
    "话题",
    "问题",
    "事情",
    "吗",
    "呢",
    "吧",
    "的",
    "了",
];

fn is_file_like_path_leaf(leaf: &str) -> bool {
    let extension = leaf
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase());

    matches!(
        extension.as_deref(),
        Some(
            "c" | "cc"
                | "cpp"
                | "cs"
                | "css"
                | "csv"
                | "go"
                | "h"
                | "hpp"
                | "html"
                | "java"
                | "js"
                | "json"
                | "jsonl"
                | "jsx"
                | "lock"
                | "md"
                | "mdx"
                | "py"
                | "rs"
                | "scss"
                | "toml"
                | "ts"
                | "tsx"
                | "txt"
                | "yaml"
                | "yml"
        )
    )
}

fn strip_file_like_leaf(path: &str) -> String {
    let Some(leaf) = path.rsplit('/').next() else {
        return path.to_string();
    };

    if !is_file_like_path_leaf(leaf) {
        return path.to_string();
    }

    path.strip_suffix(leaf)
        .map(|parent| parent.trim_end_matches('/').to_string())
        .filter(|parent| !parent.is_empty())
        .unwrap_or_else(|| path.to_string())
}

fn normalize_project_dir(project_dir: &str) -> String {
    let mut normalized = project_dir.trim().to_string();

    if let Some(stripped) = normalized.strip_prefix(r"\\?\UNC\") {
        normalized = format!("//{stripped}");
    } else if let Some(stripped) = normalized.strip_prefix(r"\\?\") {
        normalized = stripped.to_string();
    } else if let Some(stripped) = normalized.strip_prefix("//?/") {
        normalized = stripped.to_string();
    }

    normalized = normalized.replace('\\', "/");
    while normalized.contains("//") {
        normalized = normalized.replace("//", "/");
    }

    let bytes = normalized.as_bytes();
    if bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b'/' && bytes[2] != b'/' {
        normalized = format!(
            "{}:/{}",
            normalized.chars().next().unwrap(),
            &normalized[2..]
        );
    }

    normalized = normalized.trim_end_matches('/').to_string();
    strip_file_like_leaf(&normalized)
}

fn summary_matches_query(summary: &ConversationSummary, query: &str) -> bool {
    contains_query(&summary.id, query)
        || contains_query(&summary.project_dir, query)
        || summary
            .summary
            .as_deref()
            .map(|text| contains_query(text, query))
            .unwrap_or(false)
}

fn conversation_matches_query(conversation: &Conversation, query: &str) -> bool {
    if contains_query(&conversation.id, query)
        || contains_query(&conversation.project_dir, query)
        || conversation
            .summary
            .as_deref()
            .map(|text| contains_query(text, query))
            .unwrap_or(false)
    {
        return true;
    }

    if conversation
        .messages
        .iter()
        .any(|message| contains_query(&message.content, query))
    {
        return true;
    }

    if conversation
        .file_changes
        .iter()
        .any(|change| contains_query(&change.path, query))
    {
        return true;
    }

    conversation.messages.iter().any(|message| {
        message.tool_calls.iter().any(|tool_call| {
            contains_query(&tool_call.name, query)
                || contains_query(&tool_call.input.to_string(), query)
                || tool_call
                    .output
                    .as_deref()
                    .map(|output| contains_query(output, query))
                    .unwrap_or(false)
        })
    })
}

fn meaningful_message_count(conversation: &Conversation) -> usize {
    conversation
        .messages
        .iter()
        .filter(|message| !message.content.trim().is_empty() || !message.tool_calls.is_empty())
        .count()
}

fn first_user_message(conversation: &Conversation) -> Option<&str> {
    conversation
        .messages
        .iter()
        .find(|message| {
            message.role == agentswap_core::types::Role::User && !message.content.trim().is_empty()
        })
        .map(|message| message.content.trim())
}

fn normalize_compare_text(value: &str) -> String {
    value
        .to_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn first_user_preserved(source: &Conversation, target: &Conversation) -> bool {
    let Some(source_first_user) = first_user_message(source) else {
        return true;
    };
    let Some(target_first_user) = first_user_message(target) else {
        return false;
    };
    let source = normalize_compare_text(source_first_user);
    let target = normalize_compare_text(target_first_user);
    source == target || source.contains(&target) || target.contains(&source)
}

fn build_migration_verification(
    source: &Conversation,
    target: &Conversation,
    listed: bool,
) -> (MigrationVerificationResponse, Vec<String>) {
    let source_message_count = meaningful_message_count(source);
    let target_message_count = meaningful_message_count(target);
    let source_file_count = source.file_changes.len();
    let target_file_count = target.file_changes.len();
    let first_user_preserved = first_user_preserved(source, target);
    let mut warnings = Vec::new();

    if target_message_count == 0 && source_message_count > 0 {
        warnings.push("目标对话读回成功，但没有可用消息；目标客户端可能显示为空。".to_string());
    } else if target_message_count < source_message_count {
        warnings.push(format!(
            "目标消息数少于源对话：源 {} 条，目标 {} 条。请抽查目标客户端显示。",
            source_message_count, target_message_count
        ));
    }

    if target_file_count < source_file_count {
        warnings.push(format!(
            "目标文件变更数少于源对话：源 {} 个，目标 {} 个。",
            source_file_count, target_file_count
        ));
    }

    if !first_user_preserved {
        warnings.push("目标对话首条用户消息与源对话不一致。".to_string());
    }

    (
        MigrationVerificationResponse {
            read_back: true,
            listed,
            source_message_count,
            target_message_count,
            source_file_count,
            target_file_count,
            first_user_preserved,
        },
        warnings,
    )
}

fn convert_summary(summary: ConversationSummary) -> ConversationSummaryResponse {
    ConversationSummaryResponse {
        id: summary.id,
        source_agent: agent_key(&summary.source_agent).to_string(),
        project_dir: normalize_project_dir(&summary.project_dir),
        created_at: summary.created_at.to_rfc3339(),
        updated_at: summary.updated_at.to_rfc3339(),
        summary: summary.summary,
        message_count: summary.message_count,
        file_count: summary.file_count,
    }
}

fn convert_conversation(
    conv: Conversation,
    storage_path: Option<String>,
    resume_command: Option<String>,
) -> ConversationResponse {
    ConversationResponse {
        id: conv.id,
        source_agent: agent_key(&conv.source_agent).to_string(),
        project_dir: normalize_project_dir(&conv.project_dir),
        created_at: conv.created_at.to_rfc3339(),
        updated_at: conv.updated_at.to_rfc3339(),
        summary: conv.summary,
        storage_path,
        resume_command,
        messages: conv
            .messages
            .into_iter()
            .map(|m| MessageResponse {
                id: m.id.to_string(),
                timestamp: m.timestamp.to_rfc3339(),
                role: match m.role {
                    agentswap_core::types::Role::User => "user".to_string(),
                    agentswap_core::types::Role::Assistant => "assistant".to_string(),
                    agentswap_core::types::Role::System => "system".to_string(),
                },
                content: m.content,
                tool_calls: m
                    .tool_calls
                    .into_iter()
                    .map(|tc| ToolCallResponse {
                        name: tc.name,
                        input: tc.input,
                        output: tc.output,
                        status: match tc.status {
                            agentswap_core::types::ToolStatus::Success => "success".to_string(),
                            agentswap_core::types::ToolStatus::Error => "error".to_string(),
                        },
                    })
                    .collect(),
                metadata: serde_json::to_value(m.metadata).unwrap_or(serde_json::Value::Null),
            })
            .collect(),
        file_changes: conv
            .file_changes
            .into_iter()
            .map(|fc| FileChangeResponse {
                path: fc.path,
                change_type: match fc.change_type {
                    agentswap_core::types::ChangeType::Created => "created".to_string(),
                    agentswap_core::types::ChangeType::Modified => "modified".to_string(),
                    agentswap_core::types::ChangeType::Deleted => "deleted".to_string(),
                },
                timestamp: fc.timestamp.to_rfc3339(),
                message_id: fc.message_id.to_string(),
            })
            .collect(),
    }
}

fn open_memory_store() -> Result<MemoryStore, String> {
    MemoryStore::open_app().map_err(|e| e.to_string())
}

fn app_settings_path() -> Result<PathBuf, String> {
    let base = dirs::config_dir()
        .or_else(dirs::data_local_dir)
        .or_else(dirs::home_dir)
        .ok_or_else(|| "Unable to resolve a settings directory for ChatMem".to_string())?;
    Ok(base.join("ChatMem").join("settings.json"))
}

fn app_data_dir() -> Result<PathBuf, String> {
    let base = dirs::data_local_dir()
        .or_else(dirs::config_dir)
        .or_else(dirs::home_dir)
        .ok_or_else(|| "Unable to resolve a local data directory for ChatMem".to_string())?;
    Ok(base.join("ChatMem"))
}

fn trash_root_dir() -> Result<PathBuf, String> {
    Ok(app_data_dir()?.join("trash"))
}

fn normalize_trash_retention_days(retention_days: Option<i64>) -> i64 {
    retention_days
        .unwrap_or(DEFAULT_TRASH_RETENTION_DAYS)
        .clamp(1, 365)
}

fn make_trash_id(agent: &str, id: &str, trashed_at: chrono::DateTime<chrono::Utc>) -> String {
    format!(
        "{}-{}-{}",
        safe_remote_file_name(agent),
        safe_remote_file_name(id),
        trashed_at.timestamp_millis()
    )
}

fn trash_record_path(agent: &str, trash_id: &str) -> Result<PathBuf, String> {
    Ok(trash_root_dir()?
        .join(safe_remote_file_name(agent))
        .join(format!("{}.json", safe_remote_file_name(trash_id))))
}

fn write_trash_record(record: &TrashConversationRecord) -> Result<PathBuf, String> {
    let path = trash_record_path(&record.source_agent, &record.trash_id)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Cannot create ChatMem Trash directory {}: {error}",
                parent.display()
            )
        })?;
    }
    let body = serde_json::to_vec_pretty(record)
        .map_err(|error| format!("Cannot serialize Trash record: {error}"))?;
    fs::write(&path, body)
        .map_err(|error| format!("Cannot write Trash record {}: {error}", path.display()))?;
    Ok(path)
}

fn read_trash_record(path: &Path) -> Result<TrashConversationRecord, String> {
    let raw = fs::read_to_string(path)
        .map_err(|error| format!("Cannot read Trash record {}: {error}", path.display()))?;
    serde_json::from_str::<TrashConversationRecord>(&raw)
        .map_err(|error| format!("Cannot parse Trash record {}: {error}", path.display()))
}

fn trash_record_to_response(record: &TrashConversationRecord) -> TrashConversationResponse {
    TrashConversationResponse {
        trash_id: record.trash_id.clone(),
        original_id: record.original_id.clone(),
        source_agent: record.source_agent.clone(),
        project_dir: record.project_dir.clone(),
        summary: record.summary.clone(),
        trashed_at: record.trashed_at.clone(),
        expires_at: record.expires_at.clone(),
        storage_path: record.storage_path.clone(),
        resume_command: record.resume_command.clone(),
        remote_backup_deleted: record.remote_backup_deleted,
        remote_backup_path: record.remote_backup_path.clone(),
        warnings: record.warnings.clone(),
    }
}

fn list_trash_record_paths() -> Result<Vec<PathBuf>, String> {
    let root = trash_root_dir()?;
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    for agent_entry in fs::read_dir(&root)
        .map_err(|error| format!("Cannot read Trash directory {}: {error}", root.display()))?
    {
        let agent_entry = agent_entry.map_err(|error| error.to_string())?;
        let agent_path = agent_entry.path();
        if !agent_path.is_dir() {
            continue;
        }

        for record_entry in fs::read_dir(&agent_path).map_err(|error| {
            format!(
                "Cannot read Trash agent directory {}: {error}",
                agent_path.display()
            )
        })? {
            let record_entry = record_entry.map_err(|error| error.to_string())?;
            let path = record_entry.path();
            if path.extension().and_then(|extension| extension.to_str()) == Some("json") {
                paths.push(path);
            }
        }
    }
    Ok(paths)
}

fn find_trash_record_path(trash_id: &str) -> Result<PathBuf, String> {
    let safe_id = safe_remote_file_name(trash_id);
    for path in list_trash_record_paths()? {
        if path.file_stem().and_then(|stem| stem.to_str()) == Some(safe_id.as_str()) {
            return Ok(path);
        }
    }
    Err(format!("Trash record not found: {trash_id}"))
}

fn remove_expired_trash_records() -> Result<Vec<String>, String> {
    let now = chrono::Utc::now();
    let mut removed = Vec::new();

    for path in list_trash_record_paths()? {
        let record = match read_trash_record(&path) {
            Ok(record) => record,
            Err(_) => continue,
        };
        let expires_at = match chrono::DateTime::parse_from_rfc3339(&record.expires_at) {
            Ok(value) => value.with_timezone(&chrono::Utc),
            Err(_) => continue,
        };
        if expires_at <= now {
            fs::remove_file(&path).map_err(|error| {
                format!("Cannot purge Trash record {}: {error}", path.display())
            })?;
            removed.push(record.trash_id);
        }
    }

    Ok(removed)
}

fn read_app_settings_from_disk() -> Result<Option<AppSettingsPayload>, String> {
    let path = app_settings_path()?;
    if !path.exists() {
        return Ok(None);
    }

    let raw = fs::read_to_string(&path)
        .map_err(|error| format!("Cannot read settings file {}: {error}", path.display()))?;
    serde_json::from_str::<AppSettingsPayload>(&raw)
        .map(Some)
        .map_err(|error| format!("Cannot parse settings file {}: {error}", path.display()))
}

fn webdav_credential_entry(username: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new("com.chatmem.app.webdav", username)
        .map_err(|error| format!("Cannot open OS credential store: {error}"))
}

fn probe_webdav_credential(settings: Option<&AppSettingsPayload>) -> CredentialCheck {
    let Some(settings) = settings else {
        return CredentialCheck::NotNeeded;
    };

    if settings.sync.provider != "webdav" || settings.sync.username.trim().is_empty() {
        return CredentialCheck::NotNeeded;
    }

    let entry = match webdav_credential_entry(settings.sync.username.trim()) {
        Ok(entry) => entry,
        Err(error) => return CredentialCheck::Error(error),
    };

    match entry.get_password() {
        Ok(password) if !password.is_empty() => CredentialCheck::Present,
        Ok(_) | Err(keyring::Error::NoEntry) => CredentialCheck::Missing,
        Err(error) => CredentialCheck::Error(error.to_string()),
    }
}

fn build_upgrade_readiness_report(
    settings: Option<AppSettingsPayload>,
    credential_check: CredentialCheck,
    memory_store_result: Result<(), String>,
) -> UpgradeReadinessReport {
    let mut checks = Vec::new();

    let mut push_check = |key: &str, label: &str, status: &str, detail: String| {
        checks.push(UpgradeReadinessCheck {
            key: key.to_string(),
            label: label.to_string(),
            status: status.to_string(),
            detail,
        });
    };

    let webdav_enabled = settings
        .as_ref()
        .map(|settings| settings.sync.provider == "webdav")
        .unwrap_or(false);

    match settings.as_ref() {
        Some(_) => push_check(
            "settings",
            "Native settings file",
            "ok",
            "Settings file is available and can be parsed.".to_string(),
        ),
        None => push_check(
            "settings",
            "Native settings file",
            "warning",
            "No native settings file was found. ChatMem may still use browser fallback settings until you save settings once."
                .to_string(),
        ),
    }

    if let Some(settings) = settings.as_ref() {
        if webdav_enabled {
            let has_profile = !settings.sync.webdav_host.trim().is_empty()
                && !settings.sync.username.trim().is_empty()
                && !settings.sync.remote_path.trim().is_empty();
            push_check(
                "webdav_profile",
                "WebDAV profile",
                if has_profile { "ok" } else { "warning" },
                if has_profile {
                    format!(
                        "{}://{}/{}/{}",
                        settings.sync.webdav_scheme,
                        settings.sync.webdav_host.trim().trim_matches('/'),
                        settings.sync.webdav_path.trim().trim_matches('/'),
                        settings.sync.remote_path.trim().trim_matches('/')
                    )
                } else {
                    "WebDAV is enabled, but host, username, or remote folder is incomplete."
                        .to_string()
                },
            );
        } else {
            push_check(
                "webdav_profile",
                "WebDAV profile",
                "ok",
                "WebDAV sync is not enabled.".to_string(),
            );
        }
    } else {
        push_check(
            "webdav_profile",
            "WebDAV profile",
            "warning",
            "Cannot verify WebDAV profile until settings are saved to the native settings file."
                .to_string(),
        );
    }

    match credential_check {
        CredentialCheck::NotNeeded => push_check(
            "webdav_password",
            "WebDAV password",
            "ok",
            "No WebDAV password is required for the current sync mode.".to_string(),
        ),
        CredentialCheck::Present => push_check(
            "webdav_password",
            "WebDAV password",
            "ok",
            "Password exists in the OS credential store.".to_string(),
        ),
        CredentialCheck::Missing => push_check(
            "webdav_password",
            "WebDAV password",
            "warning",
            "Password is not in the OS credential store; enter it once after upgrade and verify the server."
                .to_string(),
        ),
        CredentialCheck::Error(error) => push_check(
            "webdav_password",
            "WebDAV password",
            "warning",
            format!("Could not read the OS credential store: {error}"),
        ),
    }

    match memory_store_result {
        Ok(()) => push_check(
            "memory_store",
            "Memory database",
            "ok",
            "Memory database can be opened.".to_string(),
        ),
        Err(error) => push_check(
            "memory_store",
            "Memory database",
            "error",
            format!("Memory database cannot be opened: {error}"),
        ),
    }

    let error_count = checks
        .iter()
        .filter(|check| check.status == "error")
        .count();
    let warning_count = checks
        .iter()
        .filter(|check| check.status == "warning")
        .count();
    let status = if error_count > 0 {
        "error"
    } else if warning_count > 0 {
        "warning"
    } else {
        "ok"
    };
    let summary = if error_count > 0 {
        format!("Upgrade check found {error_count} blocking item(s).")
    } else if warning_count > 0 {
        format!("Upgrade check found {warning_count} item(s) that need attention.")
    } else {
        "Upgrade check passed.".to_string()
    };
    let warnings = checks
        .iter()
        .filter(|check| check.status != "ok")
        .map(|check| check.detail.clone())
        .collect();

    UpgradeReadinessReport {
        status: status.to_string(),
        summary,
        checks,
        warnings,
    }
}

fn build_webdav_probe_url(
    scheme: &str,
    host: &str,
    webdav_path: &str,
) -> Result<reqwest::Url, String> {
    let scheme = match scheme {
        "http" => "http",
        _ => "https",
    };
    let host = host
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_matches('/');

    if host.is_empty() {
        return Err("Missing WebDAV host".to_string());
    }

    let path = webdav_path.trim().trim_matches('/');
    let url = if path.is_empty() {
        format!("{scheme}://{host}/")
    } else {
        format!("{scheme}://{host}/{path}/")
    };

    reqwest::Url::parse(&url).map_err(|error| format!("Invalid WebDAV URL: {error}"))
}

fn push_url_segments(
    url: &mut reqwest::Url,
    segments: impl IntoIterator<Item = String>,
    collection: bool,
) -> Result<(), String> {
    let mut path_segments = url
        .path_segments_mut()
        .map_err(|_| "Invalid WebDAV URL cannot be used as a base".to_string())?;
    path_segments.pop_if_empty();
    for segment in segments {
        let trimmed = segment.trim().trim_matches('/');
        if !trimmed.is_empty() {
            path_segments.push(trimmed);
        }
    }
    if collection {
        path_segments.push("");
    }
    Ok(())
}

fn build_webdav_remote_collection_url(
    scheme: &str,
    host: &str,
    webdav_path: &str,
    remote_path: &str,
) -> Result<reqwest::Url, String> {
    let remote_path = remote_path.trim().trim_matches('/');
    if remote_path.is_empty() {
        return Err("Missing remote folder".to_string());
    }

    let mut url = build_webdav_probe_url(scheme, host, webdav_path)?;
    push_url_segments(
        &mut url,
        remote_path
            .split('/')
            .map(|segment| segment.to_string())
            .collect::<Vec<_>>(),
        true,
    )?;
    Ok(url)
}

fn build_webdav_child_url(
    collection_url: &reqwest::Url,
    segments: &[String],
    collection: bool,
) -> Result<reqwest::Url, String> {
    let mut url = collection_url.clone();
    push_url_segments(&mut url, segments.iter().cloned(), collection)?;
    Ok(url)
}

fn safe_remote_file_name(value: &str) -> String {
    let sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>()
        .trim_matches('_')
        .to_string();

    if sanitized.is_empty() {
        "conversation".to_string()
    } else {
        sanitized
    }
}

async fn ensure_webdav_collection(
    client: &reqwest::Client,
    url: &reqwest::Url,
    username: &str,
    password: &str,
) -> Result<(), String> {
    let propfind = reqwest::Method::from_bytes(b"PROPFIND").map_err(|error| error.to_string())?;
    let response = client
        .request(propfind, url.clone())
        .basic_auth(username, Some(password))
        .header("Depth", "0")
        .send()
        .await
        .map_err(|error| format!("Cannot reach WebDAV folder {url}: {error}"))?;

    if response.status().is_success() {
        return Ok(());
    }

    if response.status() != reqwest::StatusCode::NOT_FOUND {
        return Err(format!(
            "Server returned HTTP {} for {url}",
            response.status()
        ));
    }

    let response = client
        .request(
            reqwest::Method::from_bytes(b"MKCOL").map_err(|error| error.to_string())?,
            url.clone(),
        )
        .basic_auth(username, Some(password))
        .send()
        .await
        .map_err(|error| format!("Cannot create WebDAV folder {url}: {error}"))?;

    if response.status().is_success()
        || response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED
    {
        Ok(())
    } else {
        Err(format!(
            "Server returned HTTP {} while creating {url}",
            response.status()
        ))
    }
}

async fn put_webdav_json(
    client: &reqwest::Client,
    url: &reqwest::Url,
    username: &str,
    password: &str,
    body: Vec<u8>,
) -> Result<(), String> {
    let response = client
        .put(url.clone())
        .basic_auth(username, Some(password))
        .header("Content-Type", "application/json; charset=utf-8")
        .body(body)
        .send()
        .await
        .map_err(|error| format!("Cannot upload {url}: {error}"))?;

    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!(
            "Server returned HTTP {} while uploading {url}",
            response.status()
        ))
    }
}

fn collect_webdav_conversation_uploads() -> Result<Vec<WebDavConversationUpload>, String> {
    let mut uploads = Vec::new();

    for agent in ["claude", "codex", "gemini", "opencode"] {
        let adapter = get_adapter(agent)?;
        if !adapter.is_available() {
            continue;
        }

        let summaries = adapter
            .list_conversations()
            .map_err(|error| error.to_string())?;
        for summary in summaries {
            let mut conversation = adapter
                .read_conversation(&summary.id)
                .map_err(|error| error.to_string())?;
            conversation.project_dir = normalize_project_dir(&conversation.project_dir);
            let id = conversation.id.clone();
            let project_dir = conversation.project_dir.clone();
            let updated_at = conversation.updated_at.to_rfc3339();
            let file_name = format!("{}.json", safe_remote_file_name(&id));
            let remote_file = format!("conversations/{agent}/{file_name}");
            let storage_path = resolve_storage_path(agent, &id);
            let resume_command = build_resume_command(agent, &id);
            let payload = convert_conversation(conversation, storage_path, resume_command);
            let body = serde_json::to_vec_pretty(&payload).map_err(|error| error.to_string())?;

            uploads.push(WebDavConversationUpload {
                agent: agent.to_string(),
                id,
                project_dir,
                updated_at,
                file_name,
                remote_file,
                body,
            });
        }
    }

    uploads.sort_by(|left, right| {
        left.agent
            .cmp(&right.agent)
            .then_with(|| left.project_dir.cmp(&right.project_dir))
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(uploads)
}

fn build_webdav_manifest(uploads: &[WebDavConversationUpload]) -> WebDavManifest {
    WebDavManifest {
        schema_version: 1,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        synced_at: chrono::Utc::now().to_rfc3339(),
        conversations: uploads
            .iter()
            .map(|upload| WebDavManifestEntry {
                source_agent: upload.agent.clone(),
                id: upload.id.clone(),
                project_dir: upload.project_dir.clone(),
                updated_at: upload.updated_at.clone(),
                remote_file: upload.remote_file.clone(),
            })
            .collect(),
    }
}

async fn upload_webdav_manifest(
    client: &reqwest::Client,
    remote_url: &reqwest::Url,
    username: &str,
    password: &str,
    uploads: &[WebDavConversationUpload],
) -> Result<(), String> {
    let manifest = build_webdav_manifest(uploads);
    let manifest_url = build_webdav_child_url(remote_url, &["manifest.json".to_string()], false)?;
    let manifest_body = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    put_webdav_json(client, &manifest_url, username, password, manifest_body).await
}

fn validate_webdav_delete_settings(
    webdav_scheme: Option<String>,
    webdav_host: Option<String>,
    webdav_path: Option<String>,
    remote_path: Option<String>,
    username: Option<String>,
    password: Option<String>,
) -> Result<WebDavDeleteSettings, String> {
    let settings = WebDavDeleteSettings {
        webdav_scheme: webdav_scheme.unwrap_or_else(|| "https".to_string()),
        webdav_host: webdav_host.unwrap_or_default(),
        webdav_path: webdav_path.unwrap_or_default(),
        remote_path: remote_path.unwrap_or_else(|| "chatmem".to_string()),
        username: username.unwrap_or_default().trim().to_string(),
        password: password.unwrap_or_default(),
    };

    if settings.webdav_host.trim().is_empty()
        || settings.remote_path.trim().is_empty()
        || settings.username.trim().is_empty()
        || settings.password.trim().is_empty()
    {
        return Err("Missing WebDAV settings or password".to_string());
    }

    Ok(settings)
}

async fn delete_webdav_conversation_backup(
    client: &reqwest::Client,
    settings: &WebDavDeleteSettings,
    agent: &str,
    id: &str,
) -> Result<(reqwest::Url, String), String> {
    let remote_url = build_webdav_remote_collection_url(
        &settings.webdav_scheme,
        &settings.webdav_host,
        &settings.webdav_path,
        &settings.remote_path,
    )?;
    let conversations_url =
        build_webdav_child_url(&remote_url, &["conversations".to_string()], true)?;
    let agent_url = build_webdav_child_url(&conversations_url, &[agent.to_string()], true)?;
    let file_name = format!("{}.json", safe_remote_file_name(id));
    let remote_file = format!("conversations/{agent}/{file_name}");
    let file_url = build_webdav_child_url(&agent_url, &[file_name], false)?;

    let response = client
        .delete(file_url.clone())
        .basic_auth(settings.username.as_str(), Some(settings.password.as_str()))
        .send()
        .await
        .map_err(|error| format!("Cannot delete WebDAV backup {file_url}: {error}"))?;
    let status = response.status();

    if status.is_success() || status == reqwest::StatusCode::NOT_FOUND {
        Ok((remote_url, remote_file))
    } else {
        Err(format!(
            "Server returned HTTP {status} while deleting WebDAV backup {file_url}"
        ))
    }
}

async fn upload_current_webdav_manifest(
    client: &reqwest::Client,
    remote_url: &reqwest::Url,
    settings: &WebDavDeleteSettings,
) -> Result<(), String> {
    let uploads = collect_webdav_conversation_uploads()?;
    upload_webdav_manifest(
        client,
        remote_url,
        settings.username.as_str(),
        settings.password.as_str(),
        &uploads,
    )
    .await
}

#[command]
async fn get_agent_card() -> Result<AgentCard, String> {
    Ok(AgentCard::chatmem_default())
}

#[command]
async fn load_app_settings() -> Result<Option<AppSettingsPayload>, String> {
    read_app_settings_from_disk()
}

#[command]
async fn save_app_settings(settings: AppSettingsPayload) -> Result<(), String> {
    let path = app_settings_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| {
            format!(
                "Cannot create settings directory {}: {error}",
                parent.display()
            )
        })?;
    }

    let body = serde_json::to_vec_pretty(&settings)
        .map_err(|error| format!("Cannot serialize settings: {error}"))?;
    fs::write(&path, body)
        .map_err(|error| format!("Cannot write settings file {}: {error}", path.display()))
}

#[command]
async fn run_upgrade_readiness_check() -> Result<UpgradeReadinessReport, String> {
    let settings = read_app_settings_from_disk()?;
    let credential_check = probe_webdav_credential(settings.as_ref());
    let memory_store_result = MemoryStore::open_app()
        .map(|_| ())
        .map_err(|error| error.to_string());

    Ok(build_upgrade_readiness_report(
        settings,
        credential_check,
        memory_store_result,
    ))
}

#[command]
async fn load_webdav_password(username: String) -> Result<Option<String>, String> {
    let username = username.trim();
    if username.is_empty() {
        return Ok(None);
    }

    let entry = webdav_credential_entry(username)?;
    match entry.get_password() {
        Ok(password) => Ok(Some(password)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(format!(
            "Cannot read WebDAV password from OS credential store: {error}"
        )),
    }
}

#[command]
async fn save_webdav_password(username: String, password: String) -> Result<(), String> {
    let username = username.trim();
    if username.is_empty() || password.is_empty() {
        return Ok(());
    }

    let entry = webdav_credential_entry(username)?;
    entry
        .set_password(&password)
        .map_err(|error| format!("Cannot save WebDAV password to OS credential store: {error}"))
}

#[command]
async fn verify_webdav_server(
    webdav_scheme: String,
    webdav_host: String,
    webdav_path: String,
    remote_path: String,
    username: String,
    password: String,
) -> Result<(), String> {
    if username.trim().is_empty() || password.trim().is_empty() {
        return Err("Missing WebDAV username or password".to_string());
    }

    if remote_path.trim().is_empty() {
        return Err("Missing remote folder".to_string());
    }

    let url = build_webdav_probe_url(&webdav_scheme, &webdav_host, &webdav_path)?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(12))
        .build()
        .map_err(|error| error.to_string())?;
    let method = reqwest::Method::from_bytes(b"PROPFIND").map_err(|error| error.to_string())?;
    let response = client
        .request(method, url.clone())
        .basic_auth(username.trim(), Some(password))
        .header("Depth", "0")
        .send()
        .await
        .map_err(|error| format!("Cannot reach WebDAV server: {error}"))?;
    let status = response.status();

    if status.is_success() {
        Ok(())
    } else {
        Err(format!("Server returned HTTP {status} for {url}"))
    }
}

#[command]
async fn sync_webdav_now(
    webdav_scheme: String,
    webdav_host: String,
    webdav_path: String,
    remote_path: String,
    username: String,
    password: String,
) -> Result<WebDavSyncResponse, String> {
    let username = username.trim().to_string();
    if username.is_empty() || password.trim().is_empty() {
        return Err("Missing WebDAV username or password".to_string());
    }

    let uploads = collect_webdav_conversation_uploads()?;

    let remote_url = build_webdav_remote_collection_url(
        &webdav_scheme,
        &webdav_host,
        &webdav_path,
        &remote_path,
    )?;
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .map_err(|error| error.to_string())?;

    ensure_webdav_collection(&client, &remote_url, &username, &password).await?;

    let conversations_url =
        build_webdav_child_url(&remote_url, &["conversations".to_string()], true)?;
    ensure_webdav_collection(&client, &conversations_url, &username, &password).await?;

    let mut uploaded_count = 0usize;

    for agent in ["claude", "codex", "gemini", "opencode"] {
        let agent_url = build_webdav_child_url(&conversations_url, &[agent.to_string()], true)?;
        ensure_webdav_collection(&client, &agent_url, &username, &password).await?;

        for upload in uploads.iter().filter(|upload| upload.agent == agent) {
            let file_url = build_webdav_child_url(&agent_url, &[upload.file_name.clone()], false)?;
            put_webdav_json(
                &client,
                &file_url,
                &username,
                &password,
                upload.body.clone(),
            )
            .await?;
            uploaded_count += 1;
        }
    }

    upload_webdav_manifest(&client, &remote_url, &username, &password, &uploads).await?;
    uploaded_count += 1;

    Ok(WebDavSyncResponse {
        uploaded_count,
        remote_url: remote_url.to_string(),
    })
}

#[command]
async fn list_conversations(agent: String) -> Result<Vec<ConversationSummaryResponse>, String> {
    let adapter = get_adapter(&agent)?;

    if !adapter.is_available() {
        return Ok(vec![]);
    }

    let conversations = adapter.list_conversations().map_err(|e| e.to_string())?;

    Ok(conversations.into_iter().map(convert_summary).collect())
}

#[command]
async fn search_conversations(
    agent: String,
    query: String,
) -> Result<Vec<ConversationSummaryResponse>, String> {
    let trimmed_query = query.trim();
    if trimmed_query.is_empty() {
        return list_conversations(agent).await;
    }

    let adapter = get_adapter(&agent)?;

    if !adapter.is_available() {
        return Ok(vec![]);
    }

    let normalized_query = trimmed_query.to_lowercase();
    let summaries = adapter.list_conversations().map_err(|e| e.to_string())?;

    let mut matches = Vec::new();

    for summary in summaries {
        if summary_matches_query(&summary, &normalized_query) {
            matches.push(convert_summary(summary));
            continue;
        }

        let conversation = adapter
            .read_conversation(&summary.id)
            .map_err(|e| e.to_string())?;

        if conversation_matches_query(&conversation, &normalized_query) {
            matches.push(convert_summary(summary));
        }
    }

    Ok(matches)
}

#[command]
async fn read_conversation(agent: String, id: String) -> Result<ConversationResponse, String> {
    let adapter = get_adapter(&agent)?;
    let mut conversation = adapter.read_conversation(&id).map_err(|e| e.to_string())?;
    conversation.project_dir = normalize_project_dir(&conversation.project_dir);
    let storage_path = resolve_storage_path(&agent, &id);
    let resume_command = build_resume_command(&agent, &id);
    if let Ok(store) = MemoryStore::open_app() {
        let _ = sync_conversation_into_store(&store, &agent, &conversation);
    }
    Ok(convert_conversation(
        conversation,
        storage_path,
        resume_command,
    ))
}

#[command]
async fn migrate_conversation(
    source: String,
    target: String,
    id: String,
    mode: String, // "copy" or "cut"
) -> Result<MigrationResponse, String> {
    if mode != "copy" && mode != "cut" {
        return Err(format!("Unsupported migration mode: {mode}"));
    }

    let source_adapter = get_adapter(&source)?;
    let target_adapter = get_adapter(&target)?;

    // Read from source
    let conversation = source_adapter
        .read_conversation(&id)
        .map_err(|e| e.to_string())?;

    // Write to target
    let new_id = target_adapter
        .write_conversation(&conversation)
        .map_err(|e| e.to_string())?;

    // Verify target readability and list visibility before reporting success or deleting the source.
    let target_conversation = target_adapter.read_conversation(&new_id).map_err(|e| {
        format!("Target write verification failed for {target} conversation {new_id}: {e}")
    })?;
    let target_summaries = target_adapter
        .list_conversations()
        .map_err(|e| format!("Target list verification failed for {target}: {e}"))?;
    let listed = target_summaries.iter().any(|summary| summary.id == new_id);
    if !listed {
        return Err(format!(
            "Target visibility verification failed: {target} conversation {new_id} was readable but did not appear in the target conversation list"
        ));
    }
    let (verification, warnings) =
        build_migration_verification(&conversation, &target_conversation, listed);

    // If cut mode, delete from source after target verification succeeds.
    if mode == "cut" {
        source_adapter
            .delete_conversation(&id)
            .map_err(|e| e.to_string())?;
    }

    Ok(MigrationResponse {
        new_id,
        source,
        target,
        mode,
        verified: true,
        verification,
        warnings,
    })
}

#[command]
async fn delete_conversation(agent: String, id: String) -> Result<(), String> {
    trash_conversation(agent, id, None, None, None, None, None, None, None, None)
        .await
        .map(|_| ())
}

#[command]
async fn trash_conversation(
    agent: String,
    id: String,
    retention_days: Option<i64>,
    delete_remote_backup: Option<bool>,
    webdav_scheme: Option<String>,
    webdav_host: Option<String>,
    webdav_path: Option<String>,
    remote_path: Option<String>,
    username: Option<String>,
    password: Option<String>,
) -> Result<TrashConversationResponse, String> {
    let conversation = {
        let adapter = get_adapter(&agent)?;
        adapter
            .read_conversation(&id)
            .map_err(|error| error.to_string())?
    };
    let retention_days = normalize_trash_retention_days(retention_days);
    let trashed_at = chrono::Utc::now();
    let expires_at = trashed_at + chrono::Duration::days(retention_days);
    let trash_id = make_trash_id(&agent, &id, trashed_at);
    let storage_path = resolve_storage_path(&agent, &id);
    let resume_command = build_resume_command(&agent, &id);
    let mut record = TrashConversationRecord {
        schema_version: 1,
        trash_id,
        original_id: id.clone(),
        source_agent: agent.clone(),
        project_dir: normalize_project_dir(&conversation.project_dir),
        summary: conversation.summary.clone(),
        trashed_at: trashed_at.to_rfc3339(),
        expires_at: expires_at.to_rfc3339(),
        storage_path,
        resume_command,
        remote_backup_deleted: false,
        remote_backup_path: None,
        warnings: Vec::new(),
        conversation,
    };
    let record_path = write_trash_record(&record)?;

    let should_delete_remote = delete_remote_backup.unwrap_or(false);
    let mut remote_manifest_update: Option<(reqwest::Client, reqwest::Url, WebDavDeleteSettings)> =
        None;

    if should_delete_remote {
        let settings = validate_webdav_delete_settings(
            webdav_scheme,
            webdav_host,
            webdav_path,
            remote_path,
            username,
            password,
        )
        .inspect_err(|_| {
            let _ = fs::remove_file(&record_path);
        })?;
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| error.to_string())
            .inspect_err(|_| {
                let _ = fs::remove_file(&record_path);
            })?;
        let (remote_url, remote_file) =
            delete_webdav_conversation_backup(&client, &settings, &agent, &id)
                .await
                .inspect_err(|_| {
                    let _ = fs::remove_file(&record_path);
                })?;
        record.remote_backup_deleted = true;
        record.remote_backup_path = Some(remote_file);
        remote_manifest_update = Some((client, remote_url, settings));
    }

    {
        let adapter = get_adapter(&agent)?;
        adapter.delete_conversation(&id).map_err(|error| {
            let _ = fs::remove_file(&record_path);
            error.to_string()
        })?;
    }

    if let Some((client, remote_url, settings)) = remote_manifest_update {
        if let Err(error) = upload_current_webdav_manifest(&client, &remote_url, &settings).await {
            record.warnings.push(format!(
                "WebDAV backup was deleted, but manifest refresh failed: {error}"
            ));
        }
    }

    write_trash_record(&record)?;
    Ok(trash_record_to_response(&record))
}

#[command]
async fn list_trashed_conversations() -> Result<Vec<TrashConversationResponse>, String> {
    let _ = remove_expired_trash_records()?;
    let mut records = Vec::new();
    for path in list_trash_record_paths()? {
        match read_trash_record(&path) {
            Ok(record) => records.push(record),
            Err(error) => eprintln!("{error}"),
        }
    }
    records.sort_by(|left, right| right.trashed_at.cmp(&left.trashed_at));
    Ok(records.iter().map(trash_record_to_response).collect())
}

#[command]
async fn restore_trashed_conversation(trash_id: String) -> Result<RestoreTrashResponse, String> {
    let path = find_trash_record_path(&trash_id)?;
    let record = read_trash_record(&path)?;
    let restored_id = if record.source_agent == "opencode" {
        if let Some(storage_path) = record.storage_path.as_deref() {
            let db_path = Path::new(storage_path);
            if db_path.exists() {
                OpenCodeAdapter::restore_conversation_in_db(db_path, &record.original_id)
                    .map_err(|error| error.to_string())?;
            } else {
                OpenCodeAdapter::new()
                    .restore_conversation(&record.original_id)
                    .map_err(|error| error.to_string())?;
            }
        } else {
            OpenCodeAdapter::new()
                .restore_conversation(&record.original_id)
                .map_err(|error| error.to_string())?;
        }
        record.original_id.clone()
    } else {
        let adapter = get_adapter(&record.source_agent)?;
        adapter
            .write_conversation(&record.conversation)
            .map_err(|error| error.to_string())?
    };
    fs::remove_file(&path).map_err(|error| {
        format!(
            "Cannot remove restored Trash record {}: {error}",
            path.display()
        )
    })?;

    Ok(RestoreTrashResponse {
        trash_id: record.trash_id,
        original_id: record.original_id,
        restored_id,
        source_agent: record.source_agent,
    })
}

#[command]
async fn check_agent_available(agent: String) -> Result<bool, String> {
    let adapter = get_adapter(&agent)?;
    Ok(adapter.is_available())
}

#[command]
async fn list_repo_memories(repo_root: String) -> Result<Vec<ApprovedMemoryResponse>, String> {
    let store = open_memory_store()?;
    store
        .list_repo_memories(&repo_root)
        .map_err(|e| e.to_string())
}

#[command]
async fn get_repo_memory_health(repo_root: String) -> Result<RepoMemoryHealthResponse, String> {
    let store = open_memory_store()?;
    store
        .repo_memory_health(&repo_root)
        .map_err(|e| e.to_string())
}

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

#[command]
async fn scan_repo_conversations(repo_root: String) -> Result<RepoScanReport, String> {
    let store = open_memory_store()?;
    sync_scan_repo_conversations(&store, &repo_root).map_err(|e| e.to_string())
}

#[command]
async fn import_all_local_history() -> Result<LocalHistoryImportReport, String> {
    let store = open_memory_store()?;
    sync_import_all_local_history(&store).map_err(|e| e.to_string())
}

#[command]
async fn merge_repo_alias(
    repo_root: String,
    alias_root: String,
) -> Result<RepoAliasResponse, String> {
    let store = open_memory_store()?;
    store
        .merge_repo_alias(&repo_root, &alias_root)
        .map_err(|e| e.to_string())
}

#[command]
async fn list_memory_candidates(
    repo_root: String,
    status: Option<String>,
) -> Result<Vec<MemoryCandidateResponse>, String> {
    let store = open_memory_store()?;
    store
        .list_candidates_with_status(&repo_root, status.as_deref())
        .map_err(|e| e.to_string())
}

#[command]
async fn list_memory_conflicts(
    repo_root: String,
    status: Option<String>,
) -> Result<Vec<MemoryConflictResponse>, String> {
    let store = open_memory_store()?;
    store
        .list_memory_conflicts(&repo_root, status.as_deref())
        .map_err(|e| e.to_string())
}

#[command]
async fn list_entity_graph(
    repo_root: String,
    limit: Option<usize>,
) -> Result<EntityGraphPayload, String> {
    let store = open_memory_store()?;
    store
        .list_entity_graph(&repo_root, limit.unwrap_or(25))
        .map_err(|e| e.to_string())
}

#[command]
async fn review_memory_candidate(
    candidate_id: String,
    action: String,
    edited_title: Option<String>,
    edited_value: Option<String>,
    edited_usage_hint: Option<String>,
    merge_memory_id: Option<String>,
) -> Result<(), String> {
    let store = open_memory_store()?;
    let review = match action.as_str() {
        "approve" => ReviewAction::Approve {
            title: edited_title.unwrap_or_else(|| "Approved memory".to_string()),
            usage_hint: edited_usage_hint
                .unwrap_or_else(|| "Used for startup injection".to_string()),
        },
        "approve_with_edit" => ReviewAction::ApproveWithEdit {
            title: edited_title.unwrap_or_else(|| "Approved memory".to_string()),
            value: edited_value.unwrap_or_default(),
            usage_hint: edited_usage_hint
                .unwrap_or_else(|| "Used for startup injection".to_string()),
        },
        "approve_merge" => ReviewAction::ApproveMerge {
            memory_id: merge_memory_id
                .ok_or_else(|| "approve_merge requires merge_memory_id".to_string())?,
            title: edited_title.unwrap_or_else(|| "Approved memory".to_string()),
            value: edited_value.unwrap_or_default(),
            usage_hint: edited_usage_hint
                .unwrap_or_else(|| "Used for startup injection".to_string()),
        },
        "reject" => ReviewAction::Reject,
        _ => ReviewAction::Snooze,
    };

    store
        .review_candidate(&candidate_id, review)
        .map_err(|e| e.to_string())
}

#[command]
async fn reverify_memory(memory_id: String, verified_by: String) -> Result<(), String> {
    let store = open_memory_store()?;
    store
        .reverify_memory(&memory_id, &verified_by)
        .map_err(|e| e.to_string())
}

#[command]
async fn retire_memory(memory_id: String, retired_by: String) -> Result<(), String> {
    let store = open_memory_store()?;
    store
        .retire_memory(&memory_id, &retired_by)
        .map_err(|e| e.to_string())
}

#[command]
async fn list_episodes(repo_root: String) -> Result<Vec<EpisodeResponse>, String> {
    let store = open_memory_store()?;
    store.list_episodes(&repo_root).map_err(|e| e.to_string())
}

#[command]
async fn list_wiki_pages(repo_root: String) -> Result<Vec<WikiPageResponse>, String> {
    let store = open_memory_store()?;
    store.list_wiki_pages(&repo_root).map_err(|e| e.to_string())
}

#[command]
async fn rebuild_repo_wiki(repo_root: String) -> Result<Vec<WikiPageResponse>, String> {
    let store = open_memory_store()?;
    store
        .rebuild_repo_wiki(&repo_root)
        .map_err(|e| e.to_string())
}

#[command]
async fn rebuild_repo_embeddings(repo_root: String) -> Result<EmbeddingRebuildReport, String> {
    let store = open_memory_store()?;
    store
        .rebuild_repo_embeddings(&repo_root)
        .map_err(|e| e.to_string())
}

#[command]
async fn list_handoffs(repo_root: String) -> Result<Vec<HandoffPacketResponse>, String> {
    let store = open_memory_store()?;
    store.list_handoffs(&repo_root).map_err(|e| e.to_string())
}

#[command]
async fn list_checkpoints(repo_root: String) -> Result<Vec<CheckpointRecord>, String> {
    let store = open_memory_store()?;
    store
        .list_checkpoints(&repo_root)
        .map_err(|e| e.to_string())
}

#[command]
async fn list_runs(repo_root: String) -> Result<Vec<RunRecord>, String> {
    load_runs(&repo_root).map_err(|e| e.to_string())
}

#[command]
async fn list_artifacts(repo_root: String) -> Result<Vec<ArtifactRecord>, String> {
    load_artifacts(&repo_root).map_err(|e| e.to_string())
}

#[command]
async fn create_handoff_packet(
    repo_root: String,
    from_agent: String,
    to_agent: String,
    goal_hint: Option<String>,
    target_profile: Option<String>,
    checkpoint_id: Option<String>,
) -> Result<HandoffPacketResponse, String> {
    let store = open_memory_store()?;
    if let Some(checkpoint_id) = checkpoint_id {
        store
            .build_and_store_handoff_from_checkpoint(
                &checkpoint_id,
                &from_agent,
                &to_agent,
                goal_hint.as_deref(),
                target_profile.as_deref(),
            )
            .map_err(|e| e.to_string())
    } else {
        store
            .build_and_store_handoff_for_target_profile(
                &repo_root,
                &from_agent,
                &to_agent,
                goal_hint.as_deref(),
                target_profile.as_deref(),
            )
            .map_err(|e| e.to_string())
    }
}

#[command]
async fn mark_handoff_consumed(handoff_id: String, consumed_by: String) -> Result<(), String> {
    let store = open_memory_store()?;
    store
        .mark_handoff_consumed(&handoff_id, &consumed_by)
        .map_err(|e| e.to_string())
}

#[command]
async fn create_checkpoint(
    repo_root: String,
    conversation_id: String,
    source_agent: String,
    summary: String,
    resume_command: Option<String>,
    metadata_json: Option<String>,
) -> Result<CheckpointRecord, String> {
    let store = open_memory_store()?;
    store
        .create_checkpoint(&CreateCheckpointInput {
            repo_root,
            conversation_id,
            source_agent,
            summary,
            resume_command,
            metadata_json,
        })
        .map_err(|e| e.to_string())
}

fn run_mcp_stdio() -> anyhow::Result<()> {
    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(async {
        let store = MemoryStore::open_app()?;
        let service = ChatMemMcpService::new(store);
        let server = service.serve(stdio()).await?;
        server.waiting().await?;
        anyhow::Ok(())
    })
}

fn main() {
    if std::env::args().any(|arg| arg == "--mcp") {
        if let Err(error) = run_mcp_stdio() {
            eprintln!("ChatMem MCP failed: {error}");
            std::process::exit(1);
        }
        return;
    }

    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            agent_integration::detect_agent_integrations,
            agent_integration::install_agent_integration,
            agent_integration::uninstall_agent_integration,
            list_conversations,
            search_conversations,
            read_conversation,
            migrate_conversation,
            delete_conversation,
            trash_conversation,
            list_trashed_conversations,
            restore_trashed_conversation,
            check_agent_available,
            get_agent_card,
            load_app_settings,
            save_app_settings,
            run_upgrade_readiness_check,
            load_webdav_password,
            save_webdav_password,
            verify_webdav_server,
            sync_webdav_now,
            list_repo_memories,
            get_repo_memory_health,
            get_project_context,
            scan_repo_conversations,
            import_all_local_history,
            merge_repo_alias,
            list_memory_candidates,
            list_memory_conflicts,
            list_entity_graph,
            review_memory_candidate,
            reverify_memory,
            retire_memory,
            list_episodes,
            list_wiki_pages,
            rebuild_repo_wiki,
            rebuild_repo_embeddings,
            list_handoffs,
            list_checkpoints,
            list_runs,
            list_artifacts,
            create_checkpoint,
            create_handoff_packet,
            mark_handoff_consumed,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod tests {
    use super::{
        build_migration_verification, build_resume_command, build_webdav_probe_url,
        build_webdav_remote_collection_url, conversation_matches_query, normalize_project_dir,
        AgentKind, Conversation,
    };
    use agentswap_core::types::{ChangeType, FileChange, Message, Role, ToolCall, ToolStatus};
    use chrono::Utc;
    use serde_json::json;
    use std::collections::HashMap;
    use uuid::Uuid;

    #[test]
    fn builds_resume_command_for_codex() {
        assert_eq!(
            build_resume_command("codex", "conv-001"),
            Some("codex resume conv-001".to_string())
        );
    }

    #[test]
    fn returns_none_for_unknown_agent_resume_command() {
        assert_eq!(build_resume_command("unknown", "conv-001"), None);
    }

    #[test]
    fn builds_webdav_probe_url_from_host_and_path() {
        let url = build_webdav_probe_url("https", "example.com", "dav/chatmem").unwrap();

        assert_eq!(url.as_str(), "https://example.com/dav/chatmem/");
    }

    #[test]
    fn builds_webdav_remote_collection_url_from_base_and_remote_folder() {
        let url =
            build_webdav_remote_collection_url("https", "example.com", "dav", "chatmem").unwrap();

        assert_eq!(url.as_str(), "https://example.com/dav/chatmem/");
    }

    #[test]
    fn strips_scheme_and_slashes_from_webdav_host() {
        let url = build_webdav_probe_url("http", "https://example.com/", "/dav/").unwrap();

        assert_eq!(url.as_str(), "http://example.com/dav/");
    }

    #[test]
    fn rejects_missing_webdav_host() {
        assert!(build_webdav_probe_url("https", "   ", "dav").is_err());
    }

    #[test]
    fn migration_verification_accepts_preserved_messages_and_files() {
        let now = Utc::now();
        let user_message_id = Uuid::new_v4();
        let assistant_message_id = Uuid::new_v4();
        let source = Conversation {
            id: "source-001".to_string(),
            source_agent: AgentKind::Claude,
            project_dir: "D:/VSP".to_string(),
            created_at: now,
            updated_at: now,
            summary: Some("迁移测试".to_string()),
            messages: vec![
                Message {
                    id: user_message_id,
                    timestamp: now,
                    role: Role::User,
                    content: "请继续优化迁移流程。".to_string(),
                    tool_calls: vec![],
                    metadata: HashMap::new(),
                },
                Message {
                    id: assistant_message_id,
                    timestamp: now,
                    role: Role::Assistant,
                    content: "我会先验证目标对话可读。".to_string(),
                    tool_calls: vec![],
                    metadata: HashMap::new(),
                },
            ],
            file_changes: vec![FileChange {
                path: "src/App.tsx".to_string(),
                change_type: ChangeType::Modified,
                timestamp: now,
                message_id: assistant_message_id,
            }],
        };
        let target = Conversation {
            id: "target-001".to_string(),
            source_agent: AgentKind::Codex,
            project_dir: source.project_dir.clone(),
            created_at: now,
            updated_at: now,
            summary: source.summary.clone(),
            messages: source.messages.clone(),
            file_changes: source.file_changes.clone(),
        };

        let (verification, warnings) = build_migration_verification(&source, &target, true);

        assert!(verification.read_back);
        assert!(verification.listed);
        assert_eq!(verification.source_message_count, 2);
        assert_eq!(verification.target_message_count, 2);
        assert_eq!(verification.source_file_count, 1);
        assert_eq!(verification.target_file_count, 1);
        assert!(verification.first_user_preserved);
        assert!(warnings.is_empty());
    }

    #[test]
    fn migration_verification_warns_when_target_loses_messages() {
        let now = Utc::now();
        let source = Conversation {
            id: "source-empty-target".to_string(),
            source_agent: AgentKind::Claude,
            project_dir: "D:/VSP".to_string(),
            created_at: now,
            updated_at: now,
            summary: Some("白屏风险".to_string()),
            messages: vec![Message {
                id: Uuid::new_v4(),
                timestamp: now,
                role: Role::User,
                content: "这段对话迁移过去不能变成白屏。".to_string(),
                tool_calls: vec![],
                metadata: HashMap::new(),
            }],
            file_changes: vec![],
        };
        let target = Conversation {
            id: "target-empty".to_string(),
            source_agent: AgentKind::Codex,
            project_dir: source.project_dir.clone(),
            created_at: now,
            updated_at: now,
            summary: source.summary.clone(),
            messages: vec![],
            file_changes: vec![],
        };

        let (verification, warnings) = build_migration_verification(&source, &target, true);

        assert_eq!(verification.source_message_count, 1);
        assert_eq!(verification.target_message_count, 0);
        assert!(!verification.first_user_preserved);
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("没有可用消息")));
        assert!(warnings
            .iter()
            .any(|warning| warning.contains("首条用户消息")));
    }

    #[test]
    fn upgrade_check_report_flags_missing_webdav_password_without_failing_database() {
        let report = super::build_upgrade_readiness_report(
            Some(super::AppSettingsPayload {
                locale: "zh-CN".to_string(),
                font_family: "system".to_string(),
                auto_check_updates: true,
                trash_retention_days: super::DEFAULT_TRASH_RETENTION_DAYS,
                sync: super::SyncSettingsPayload {
                    provider: "webdav".to_string(),
                    webdav_scheme: "https".to_string(),
                    webdav_host: "dav.example.com".to_string(),
                    webdav_path: "remote.php/dav/files/liang".to_string(),
                    username: "liang@example.com".to_string(),
                    remote_path: "chatmem".to_string(),
                    download_mode: "as-needed".to_string(),
                },
            }),
            super::CredentialCheck::Missing,
            Ok(()),
        );

        assert_eq!(report.status, "warning");
        assert!(report
            .checks
            .iter()
            .any(|check| check.key == "webdav_password" && check.status == "warning"));
        assert!(report
            .checks
            .iter()
            .any(|check| check.key == "memory_store" && check.status == "ok"));
    }

    #[test]
    fn upgrade_check_report_errors_when_memory_store_cannot_open() {
        let report = super::build_upgrade_readiness_report(
            None,
            super::CredentialCheck::NotNeeded,
            Err("database is locked".to_string()),
        );

        assert_eq!(report.status, "error");
        assert!(report
            .checks
            .iter()
            .any(|check| check.key == "memory_store" && check.status == "error"));
    }

    #[test]
    fn normalizes_windows_extended_project_paths() {
        assert_eq!(normalize_project_dir(r"\\?\D:\VSP"), "D:/VSP".to_string());
    }

    #[test]
    fn normalizes_file_cwd_to_parent_project_path() {
        assert_eq!(
            normalize_project_dir(r"\\?\D:\VSP\bm.md"),
            "D:/VSP".to_string()
        );
    }

    #[test]
    fn full_text_search_matches_message_content() {
        let now = Utc::now();
        let message_id = Uuid::new_v4();

        let conversation = Conversation {
            id: "conv-002".to_string(),
            source_agent: AgentKind::Claude,
            project_dir: "D:/VSP/service".to_string(),
            created_at: now,
            updated_at: now,
            summary: Some("Memory investigation".to_string()),
            messages: vec![Message {
                id: message_id,
                timestamp: now,
                role: Role::Assistant,
                content: "问题根因是内存泄漏出现在缓存清理逻辑。".to_string(),
                tool_calls: vec![ToolCall {
                    name: "read_logs".to_string(),
                    input: json!({"path": "logs/app.log"}),
                    output: Some("found repeated allocation spikes".to_string()),
                    status: ToolStatus::Success,
                }],
                metadata: HashMap::new(),
            }],
            file_changes: vec![],
        };

        assert!(conversation_matches_query(&conversation, "内存泄漏"));
    }

    #[test]
    fn full_text_search_extracts_chinese_keywords_from_recall_sentence() {
        let now = Utc::now();
        let conversation = Conversation {
            id: "conv-ultraman-pig-hero".to_string(),
            source_agent: AgentKind::Codex,
            project_dir: "D:/VSP/games".to_string(),
            created_at: now,
            updated_at: now,
            summary: Some("小游戏项目讨论".to_string()),
            messages: vec![Message {
                id: Uuid::new_v4(),
                timestamp: now,
                role: Role::User,
                content: "我们之前聊过奥特曼打猪猪侠这个小游戏项目。".to_string(),
                tool_calls: vec![],
                metadata: HashMap::new(),
            }],
            file_changes: vec![],
        };

        assert!(conversation_matches_query(
            &conversation,
            "你还记得奥特曼打猪猪侠的项目吗？"
        ));
    }
}
