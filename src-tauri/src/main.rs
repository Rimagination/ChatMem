#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::{fs, path::PathBuf, time::Duration};

use serde::{Deserialize, Serialize};
use tauri::command;
use chatmem::chatmem_memory::{
    a2a::AgentCard,
    checkpoints::{CheckpointRecord, CreateCheckpointInput},
    models::{
        ApprovedMemoryResponse, EmbeddingRebuildReport, EntityGraphPayload, EpisodeResponse,
        HandoffPacketResponse, MemoryCandidateResponse, MemoryConflictResponse,
        ProjectContextPayload, RepoMemoryHealthResponse, RepoScanReport, WikiPageResponse,
    },
    runs::{list_artifacts as load_artifacts, list_runs as load_runs, ArtifactRecord, RunRecord},
    store::{MemoryStore, ReviewAction},
    sync::{
        build_resume_command, resolve_storage_path,
        scan_repo_conversations as sync_scan_repo_conversations, sync_conversation_into_store,
    },
};

// Import AgentSwap adapters
use agentswap_claude::ClaudeAdapter;
use agentswap_codex::CodexAdapter;
use agentswap_gemini::GeminiAdapter;
use agentswap_core::adapter::AgentAdapter;
use agentswap_core::types::{Conversation, ConversationSummary, AgentKind};

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
    auto_check_updates: bool,
    sync: SyncSettingsPayload,
}

fn get_adapter(agent: &str) -> Result<Box<dyn AgentAdapter>, String> {
    match agent {
        "claude" => Ok(Box::new(ClaudeAdapter::new())),
        "codex" => Ok(Box::new(CodexAdapter::new())),
        "gemini" => Ok(Box::new(GeminiAdapter::new())),
        _ => Err(format!("Unknown agent: {}", agent)),
    }
}

fn agent_key(agent: &AgentKind) -> &'static str {
    match agent {
        AgentKind::Claude => "claude",
        AgentKind::Codex => "codex",
        AgentKind::Gemini => "gemini",
    }
}

fn contains_query(haystack: &str, query: &str) -> bool {
    haystack.to_lowercase().contains(query)
}

fn is_file_like_path_leaf(leaf: &str) -> bool {
    let extension = leaf.rsplit_once('.').map(|(_, extension)| {
        extension.to_ascii_lowercase()
    });

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
    if bytes.len() >= 3
        && bytes[0].is_ascii_alphabetic()
        && bytes[1] == b'/'
        && bytes[2] != b'/'
    {
        normalized = format!("{}:/{}", normalized.chars().next().unwrap(), &normalized[2..]);
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

    if conversation.file_changes.iter().any(|change| contains_query(&change.path, query)) {
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

fn webdav_credential_entry(username: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new("com.chatmem.app.webdav", username)
        .map_err(|error| format!("Cannot open OS credential store: {error}"))
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
        return Err(format!("Server returned HTTP {} for {url}", response.status()));
    }

    let response = client
        .request(reqwest::Method::from_bytes(b"MKCOL").map_err(|error| error.to_string())?, url.clone())
        .basic_auth(username, Some(password))
        .send()
        .await
        .map_err(|error| format!("Cannot create WebDAV folder {url}: {error}"))?;

    if response.status().is_success() || response.status() == reqwest::StatusCode::METHOD_NOT_ALLOWED {
        Ok(())
    } else {
        Err(format!("Server returned HTTP {} while creating {url}", response.status()))
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
        Err(format!("Server returned HTTP {} while uploading {url}", response.status()))
    }
}

#[command]
async fn get_agent_card() -> Result<AgentCard, String> {
    Ok(AgentCard::chatmem_default())
}

#[command]
async fn load_app_settings() -> Result<Option<AppSettingsPayload>, String> {
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

#[command]
async fn save_app_settings(settings: AppSettingsPayload) -> Result<(), String> {
    let path = app_settings_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Cannot create settings directory {}: {error}", parent.display()))?;
    }

    let body = serde_json::to_vec_pretty(&settings)
        .map_err(|error| format!("Cannot serialize settings: {error}"))?;
    fs::write(&path, body)
        .map_err(|error| format!("Cannot write settings file {}: {error}", path.display()))
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
        Err(error) => Err(format!("Cannot read WebDAV password from OS credential store: {error}")),
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

    let mut uploads = Vec::new();

    for agent in ["claude", "codex", "gemini"] {
        let adapter = get_adapter(agent)?;
        if !adapter.is_available() {
            continue;
        }

        let summaries = adapter.list_conversations().map_err(|error| error.to_string())?;
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

    let mut manifest_entries = Vec::new();
    let mut uploaded_count = 0usize;

    for agent in ["claude", "codex", "gemini"] {
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
            manifest_entries.push(WebDavManifestEntry {
                source_agent: upload.agent.clone(),
                id: upload.id.clone(),
                project_dir: upload.project_dir.clone(),
                updated_at: upload.updated_at.clone(),
                remote_file: upload.remote_file.clone(),
            });
        }
    }

    let manifest = WebDavManifest {
        schema_version: 1,
        app_version: env!("CARGO_PKG_VERSION").to_string(),
        synced_at: chrono::Utc::now().to_rfc3339(),
        conversations: manifest_entries,
    };
    let manifest_url = build_webdav_child_url(&remote_url, &["manifest.json".to_string()], false)?;
    let manifest_body = serde_json::to_vec_pretty(&manifest).map_err(|error| error.to_string())?;
    put_webdav_json(&client, &manifest_url, &username, &password, manifest_body).await?;
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

    let conversations = adapter
        .list_conversations()
        .map_err(|e| e.to_string())?;

    Ok(conversations.into_iter().map(convert_summary).collect())
}

#[command]
async fn search_conversations(agent: String, query: String) -> Result<Vec<ConversationSummaryResponse>, String> {
    let trimmed_query = query.trim();
    if trimmed_query.is_empty() {
        return list_conversations(agent).await;
    }

    let adapter = get_adapter(&agent)?;
    
    if !adapter.is_available() {
        return Ok(vec![]);
    }

    let normalized_query = trimmed_query.to_lowercase();
    let summaries = adapter
        .list_conversations()
        .map_err(|e| e.to_string())?;

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
    Ok(convert_conversation(conversation, storage_path, resume_command))
}

#[command]
async fn migrate_conversation(
    source: String,
    target: String,
    id: String,
    mode: String,  // "copy" or "cut"
) -> Result<String, String> {
    let source_adapter = get_adapter(&source)?;
    let target_adapter = get_adapter(&target)?;

    // Read from source
    let conversation = source_adapter.read_conversation(&id).map_err(|e| e.to_string())?;

    // Write to target
    let new_id = target_adapter
        .write_conversation(&conversation)
        .map_err(|e| e.to_string())?;

    // If cut mode, delete from source
    if mode == "cut" {
        source_adapter.delete_conversation(&id).map_err(|e| e.to_string())?;
    }

    Ok(new_id)
}

#[command]
async fn delete_conversation(agent: String, id: String) -> Result<(), String> {
    let adapter = get_adapter(&agent)?;
    adapter.delete_conversation(&id).map_err(|e| e.to_string())?;
    Ok(())
}

#[command]
async fn check_agent_available(agent: String) -> Result<bool, String> {
    let adapter = get_adapter(&agent)?;
    Ok(adapter.is_available())
}

#[command]
async fn list_repo_memories(repo_root: String) -> Result<Vec<ApprovedMemoryResponse>, String> {
    let store = open_memory_store()?;
    store.list_repo_memories(&repo_root).map_err(|e| e.to_string())
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
async fn list_entity_graph(repo_root: String, limit: Option<usize>) -> Result<EntityGraphPayload, String> {
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
            usage_hint: edited_usage_hint.unwrap_or_else(|| "Used for startup injection".to_string()),
        },
        "approve_with_edit" => ReviewAction::ApproveWithEdit {
            title: edited_title.unwrap_or_else(|| "Approved memory".to_string()),
            value: edited_value.unwrap_or_default(),
            usage_hint: edited_usage_hint.unwrap_or_else(|| "Used for startup injection".to_string()),
        },
        "approve_merge" => ReviewAction::ApproveMerge {
            memory_id: merge_memory_id
                .ok_or_else(|| "approve_merge requires merge_memory_id".to_string())?,
            title: edited_title.unwrap_or_else(|| "Approved memory".to_string()),
            value: edited_value.unwrap_or_default(),
            usage_hint: edited_usage_hint.unwrap_or_else(|| "Used for startup injection".to_string()),
        },
        "reject" => ReviewAction::Reject,
        _ => ReviewAction::Snooze,
    };

    store.review_candidate(&candidate_id, review).map_err(|e| e.to_string())
}

#[command]
async fn reverify_memory(memory_id: String, verified_by: String) -> Result<(), String> {
    let store = open_memory_store()?;
    store
        .reverify_memory(&memory_id, &verified_by)
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
    store.rebuild_repo_wiki(&repo_root).map_err(|e| e.to_string())
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
    store.list_checkpoints(&repo_root).map_err(|e| e.to_string())
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

fn main() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![
            list_conversations,
            search_conversations,
            read_conversation,
            migrate_conversation,
            delete_conversation,
            check_agent_available,
            get_agent_card,
            load_app_settings,
            save_app_settings,
            load_webdav_password,
            save_webdav_password,
            verify_webdav_server,
            sync_webdav_now,
            list_repo_memories,
            get_repo_memory_health,
            get_project_context,
            scan_repo_conversations,
            list_memory_candidates,
            list_memory_conflicts,
            list_entity_graph,
            review_memory_candidate,
            reverify_memory,
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
        build_resume_command, build_webdav_probe_url, build_webdav_remote_collection_url,
        conversation_matches_query, normalize_project_dir, AgentKind, Conversation,
    };
    use agentswap_core::types::{Message, Role, ToolCall, ToolStatus};
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
    fn normalizes_windows_extended_project_paths() {
        assert_eq!(
            normalize_project_dir(r"\\?\D:\VSP"),
            "D:/VSP".to_string()
        );
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
}
