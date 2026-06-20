use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use agentswap_claude::parser::{
    decode_project_path, parse_event as parse_claude_event, ClaudeContent, ClaudeContentBlock,
    ToolResultContent,
};
use agentswap_codex::CodexAdapter;
use agentswap_core::adapter::AgentAdapter;
use agentswap_core::files::move_path_to_trash;
use agentswap_core::types::{
    AgentKind, ChangeType, Conversation, ConversationSummary, FileChange, Message, Role, ToolCall,
    ToolStatus,
};
use anyhow::{Context, Result};
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{Connection, OpenFlags};
use serde_json::{json, Value};
use uuid::Uuid;
use walkdir::WalkDir;

const ZCODE_ROOT_PARTS: &[&str] = &[".zcode", "v2", "acp-config"];
const CLAUDE_ENGINE: &str = "claude";
const CODEX_ENGINE: &str = "codex";
const GEMINI_ENGINE: &str = "gemini";
const OPENCODE_ENGINE: &str = "opencode";
const GLM_ENGINE: &str = "glm";
const SUBAGENT_MARKER: &str = "subagent";
const TASK_MARKER: &str = "task";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZCodeProfileStatus {
    pub engine: String,
    pub profile_id: String,
    pub profile_dir: PathBuf,
    pub capability: String,
    pub conversation_count: usize,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ZCodeAdapter {
    root_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ZCodeClaudeAdapter {
    root_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ZCodeCodexAdapter {
    root_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ZCodeGeminiAdapter {
    root_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct ZCodeOpenCodeAdapter {
    root_dir: PathBuf,
}

#[derive(Debug, Clone)]
struct ClaudeSessionPath {
    id: String,
    profile_id: String,
    path: PathBuf,
    project_dir: String,
    parent_session_id: Option<String>,
    subagent_id: Option<String>,
}

#[derive(Debug, Clone)]
struct ZCodeTaskPath {
    task_id: String,
    profile_id: String,
    path: PathBuf,
}

#[derive(Debug, Clone)]
struct ParsedZCodeTask {
    task_id: String,
    profile_id: String,
    provider: String,
    acp_session_id: Option<String>,
    project_dir: String,
    title: Option<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    message_count: usize,
    file_count: usize,
    value: Value,
    path: PathBuf,
}

#[derive(Debug, Clone)]
struct PendingClaudeTool {
    message_index: usize,
    tool_index: usize,
    name: String,
    input: Value,
}

impl Default for ZCodeClaudeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ZCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ZCodeAdapter {
    pub fn new() -> Self {
        Self {
            root_dir: default_zcode_root(),
        }
    }

    pub fn with_root_dir(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }

    pub fn storage_path_for_id(&self, id: &str) -> Option<String> {
        let (engine, raw_id) = split_zcode_cli_id(id)?;
        if let Some((profile_id, task_id)) = split_task_id(raw_id) {
            return self
                .find_task_path(profile_id, task_id)
                .map(|path| normalize_storage_path(path.to_string_lossy().as_ref()));
        }
        match engine {
            CLAUDE_ENGINE => self.claude_adapter().storage_path_for_id(raw_id),
            CODEX_ENGINE => self.codex_adapter().storage_path_for_id(raw_id),
            _ => None,
        }
    }

    fn claude_adapter(&self) -> ZCodeClaudeAdapter {
        ZCodeClaudeAdapter::with_root_dir(self.root_dir.clone())
    }

    fn codex_adapter(&self) -> ZCodeCodexAdapter {
        ZCodeCodexAdapter::with_root_dir(self.root_dir.clone())
    }

    fn gemini_adapter(&self) -> ZCodeGeminiAdapter {
        ZCodeGeminiAdapter::with_root_dir(self.root_dir.clone())
    }

    fn opencode_adapter(&self) -> ZCodeOpenCodeAdapter {
        ZCodeOpenCodeAdapter::with_root_dir(self.root_dir.clone())
    }

    fn engine_dir(&self) -> PathBuf {
        self.root_dir.clone()
    }

    fn task_sessions_dir(&self) -> PathBuf {
        zcode_v2_dir(&self.root_dir).join("sessions")
    }

    fn list_task_paths(&self) -> Vec<ZCodeTaskPath> {
        list_zcode_task_paths(&self.task_sessions_dir())
    }

    fn find_task_path(&self, profile_id: &str, task_id: &str) -> Option<PathBuf> {
        self.task_sessions_dir()
            .join(profile_id)
            .join(format!("{task_id}.json"))
            .is_file()
            .then(|| {
                self.task_sessions_dir()
                    .join(profile_id)
                    .join(format!("{task_id}.json"))
            })
    }

    fn read_task_conversation(
        &self,
        provider: &str,
        profile_id: &str,
        task_id: &str,
        full_id: &str,
    ) -> Result<Conversation> {
        let path = self
            .find_task_path(profile_id, task_id)
            .ok_or_else(|| anyhow::anyhow!("ZCode task session not found: {profile_id}:{task_id}"))?;
        let mut conversation = parse_zcode_task_conversation(&path, profile_id)?;
        conversation.id = full_id.to_string();
        for message in &mut conversation.messages {
            message
                .metadata
                .insert("zcode_cli".to_string(), json!(provider));
        }
        Ok(conversation)
    }
}

impl Default for ZCodeCodexAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ZCodeGeminiAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl Default for ZCodeOpenCodeAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl ZCodeClaudeAdapter {
    pub fn new() -> Self {
        Self {
            root_dir: default_zcode_root(),
        }
    }

    pub fn with_root_dir(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }

    pub fn storage_path_for_id(&self, id: &str) -> Option<String> {
        self.find_session(id)
            .ok()
            .map(|session| normalize_storage_path(session.path.to_string_lossy().as_ref()))
    }

    fn engine_dir(&self) -> PathBuf {
        self.root_dir.join(CLAUDE_ENGINE)
    }

    fn profile_dirs(&self) -> Vec<(String, PathBuf)> {
        profile_dirs(&self.engine_dir())
    }

    fn default_profile_projects_dir(&self) -> Result<(String, PathBuf)> {
        let (profile_id, profile_dir) = self
            .profile_dirs()
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No ZCode Claude profile found"))?;
        Ok((profile_id, profile_dir.join("projects")))
    }

    fn list_session_paths(&self) -> Vec<ClaudeSessionPath> {
        let mut sessions = Vec::new();
        for (profile_id, profile_dir) in self.profile_dirs() {
            let projects_dir = profile_dir.join("projects");
            if !projects_dir.is_dir() {
                continue;
            }
            for entry in WalkDir::new(&projects_dir)
                .min_depth(2)
                .max_depth(4)
                .into_iter()
                .filter_map(|entry| entry.ok())
            {
                let path = entry.path();
                if !entry.file_type().is_file()
                    || path.extension().and_then(|ext| ext.to_str()) != Some("jsonl")
                {
                    continue;
                }
                if let Some(session) = classify_claude_session(&projects_dir, &profile_id, path) {
                    sessions.push(session);
                }
            }
        }
        sessions.sort_by(|left, right| left.id.cmp(&right.id));
        sessions
    }

    fn find_session(&self, id: &str) -> Result<ClaudeSessionPath> {
        self.list_session_paths()
            .into_iter()
            .find(|session| session.id == id)
            .ok_or_else(|| anyhow::anyhow!("ZCode Claude session not found: {id}"))
    }
}

impl ZCodeCodexAdapter {
    pub fn new() -> Self {
        Self {
            root_dir: default_zcode_root(),
        }
    }

    pub fn with_root_dir(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }

    pub fn storage_path_for_id(&self, id: &str) -> Option<String> {
        let (profile_id, raw_id) = split_profile_id(id)?;
        let profile_dir = self.profile_dir(profile_id);
        let db_path = profile_dir.join("state_5.sqlite");
        let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).ok()?;
        let mut stmt = conn
            .prepare("SELECT rollout_path FROM threads WHERE id = ?1")
            .ok()?;
        let path = stmt
            .query_row([raw_id], |row| row.get::<_, String>(0))
            .ok()?;
        Some(normalize_storage_path(&path))
    }

    fn engine_dir(&self) -> PathBuf {
        self.root_dir.join(CODEX_ENGINE)
    }

    fn profile_dirs(&self) -> Vec<(String, PathBuf)> {
        profile_dirs(&self.engine_dir())
    }

    fn profile_dir(&self, profile_id: &str) -> PathBuf {
        self.engine_dir().join(profile_id)
    }

    fn default_profile_dir(&self) -> Result<(String, PathBuf)> {
        self.profile_dirs()
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No ZCode Codex profile found"))
    }
}

impl ZCodeGeminiAdapter {
    pub fn new() -> Self {
        Self {
            root_dir: default_zcode_root(),
        }
    }

    pub fn with_root_dir(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }

    fn engine_dir(&self) -> PathBuf {
        self.root_dir.join(GEMINI_ENGINE)
    }
}

impl ZCodeOpenCodeAdapter {
    pub fn new() -> Self {
        Self {
            root_dir: default_zcode_root(),
        }
    }

    pub fn with_root_dir(root_dir: PathBuf) -> Self {
        Self { root_dir }
    }

    fn engine_dir(&self) -> PathBuf {
        self.root_dir.join(OPENCODE_ENGINE)
    }
}

pub fn discover_profiles(root_dir: &Path) -> Vec<ZCodeProfileStatus> {
    let mut statuses = Vec::new();
    for engine in [
        CLAUDE_ENGINE,
        CODEX_ENGINE,
        GEMINI_ENGINE,
        OPENCODE_ENGINE,
        GLM_ENGINE,
    ] {
        let engine_dir = root_dir.join(engine);
        for (profile_id, profile_dir) in profile_dirs(&engine_dir) {
            statuses.push(discover_profile(engine, &profile_id, &profile_dir));
        }
    }
    statuses.sort_by(|left, right| {
        left.engine
            .cmp(&right.engine)
            .then_with(|| left.profile_id.cmp(&right.profile_id))
    });
    statuses
}

fn discover_profile(engine: &str, profile_id: &str, profile_dir: &Path) -> ZCodeProfileStatus {
    let mut warnings = Vec::new();
    let (capability, conversation_count) = match engine {
        CLAUDE_ENGINE => {
            let projects_dir = profile_dir.join("projects");
            let count = count_claude_sessions(&projects_dir);
            ("conversation_reader".to_string(), count)
        }
        CODEX_ENGINE => {
            let db_path = profile_dir.join("state_5.sqlite");
            let jsonl_count = count_extension(profile_dir, "jsonl", true);
            if db_path.exists() {
                let thread_count = count_codex_threads(&db_path).unwrap_or_else(|error| {
                    warnings.push(format!("Cannot inspect Codex DB: {error}"));
                    jsonl_count
                });
                ("conversation_reader".to_string(), thread_count)
            } else {
                ("config_only".to_string(), jsonl_count)
            }
        }
        GEMINI_ENGINE => {
            let chat_count = count_named_parent_files(profile_dir, "chats", "json");
            let capability = if chat_count > 0 {
                "conversation_reader"
            } else {
                "config_only"
            };
            (capability.to_string(), chat_count)
        }
        OPENCODE_ENGINE => {
            let db_count = count_opencode_dbs(profile_dir);
            let capability = if db_count > 0 {
                "conversation_reader"
            } else {
                "config_only"
            };
            (capability.to_string(), db_count)
        }
        _ => ("config_only".to_string(), 0),
    };

    ZCodeProfileStatus {
        engine: engine.to_string(),
        profile_id: profile_id.to_string(),
        profile_dir: profile_dir.to_path_buf(),
        capability,
        conversation_count,
        warnings,
    }
}

impl AgentAdapter for ZCodeAdapter {
    fn is_available(&self) -> bool {
        self.root_dir.is_dir()
            && (self.claude_adapter().is_available()
                || self.codex_adapter().is_available()
                || self.gemini_adapter().is_available()
                || self.opencode_adapter().is_available()
                || self.task_sessions_dir().is_dir())
    }

    fn list_conversations(&self) -> Result<Vec<ConversationSummary>> {
        let mut summaries = Vec::new();
        let mut task_backed_cli_ids = HashSet::new();
        for task_path in self.list_task_paths() {
            match parse_zcode_task(&task_path) {
                Ok(task) => {
                    if let Some(acp_session_id) = &task.acp_session_id {
                        task_backed_cli_ids.insert(encode_zcode_cli_id(
                            &task.provider,
                            &encode_profile_id(&task.profile_id, acp_session_id),
                        ));
                    }
                    summaries.push(zcode_task_summary(&task));
                }
                Err(error) => eprintln!(
                    "Warning: failed to parse ZCode task session {}: {error}",
                    task_path.path.display()
                ),
            }
        }
        for mut summary in self.claude_adapter().list_conversations()? {
            if !is_listable_zcode_claude_summary(&summary) {
                continue;
            }
            summary.id = encode_zcode_cli_id(CLAUDE_ENGINE, &summary.id);
            if task_backed_cli_ids.contains(&summary.id) {
                continue;
            }
            summary.source_agent = AgentKind::ZCode;
            summaries.push(summary);
        }
        for mut summary in self.codex_adapter().list_conversations()? {
            summary.id = encode_zcode_cli_id(CODEX_ENGINE, &summary.id);
            if task_backed_cli_ids.contains(&summary.id) {
                continue;
            }
            summary.source_agent = AgentKind::ZCode;
            summaries.push(summary);
        }
        summaries.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(summaries)
    }

    fn read_conversation(&self, id: &str) -> Result<Conversation> {
        let (engine, raw_id) = split_zcode_cli_id(id)
            .ok_or_else(|| anyhow::anyhow!("Invalid ZCode conversation id: {id}"))?;
        if let Some((profile_id, task_id)) = split_task_id(raw_id) {
            return self.read_task_conversation(engine, profile_id, task_id, id);
        }
        let mut conv = match engine {
            CLAUDE_ENGINE => self.claude_adapter().read_conversation(raw_id)?,
            CODEX_ENGINE => self.codex_adapter().read_conversation(raw_id)?,
            GEMINI_ENGINE => self.gemini_adapter().read_conversation(raw_id)?,
            OPENCODE_ENGINE => self.opencode_adapter().read_conversation(raw_id)?,
            _ => anyhow::bail!("Unsupported ZCode CLI in id: {engine}"),
        };
        conv.id = id.to_string();
        Ok(conv)
    }

    fn write_conversation(&self, conv: &Conversation) -> Result<String> {
        match conv.source_agent {
            AgentKind::Codex | AgentKind::ZCodeCodex => self
                .codex_adapter()
                .write_conversation(conv)
                .map(|id| encode_zcode_cli_id(CODEX_ENGINE, &id)),
            AgentKind::Gemini | AgentKind::ZCodeGemini => {
                anyhow::bail!(
                    "ZCode Gemini write is disabled until a native conversation store is detected"
                )
            }
            AgentKind::OpenCode | AgentKind::ZCodeOpenCode => {
                anyhow::bail!("ZCode OpenCode write is disabled until a native conversation store is detected")
            }
            AgentKind::Hermes => {
                anyhow::bail!("Hermes write is not supported (read-only adapter)")
            }
            AgentKind::Claude | AgentKind::ZCode | AgentKind::ZCodeClaude => self
                .claude_adapter()
                .write_conversation(conv)
                .map(|id| encode_zcode_cli_id(CLAUDE_ENGINE, &id)),
        }
    }

    fn delete_conversation(&self, id: &str) -> Result<()> {
        let (engine, raw_id) = split_zcode_cli_id(id)
            .ok_or_else(|| anyhow::anyhow!("Invalid ZCode conversation id: {id}"))?;
        if let Some((profile_id, task_id)) = split_task_id(raw_id) {
            let path = self.find_task_path(profile_id, task_id).ok_or_else(|| {
                anyhow::anyhow!("ZCode task session not found: {profile_id}:{task_id}")
            })?;
            return move_path_to_trash(&path);
        }
        match engine {
            CLAUDE_ENGINE => self.claude_adapter().delete_conversation(raw_id),
            CODEX_ENGINE => self.codex_adapter().delete_conversation(raw_id),
            GEMINI_ENGINE => self.gemini_adapter().delete_conversation(raw_id),
            OPENCODE_ENGINE => self.opencode_adapter().delete_conversation(raw_id),
            _ => anyhow::bail!("Unsupported ZCode CLI in id: {engine}"),
        }
    }

    fn render_prompt(&self, conv: &Conversation) -> Result<String> {
        Ok(render_prompt(conv))
    }

    fn agent_kind(&self) -> AgentKind {
        AgentKind::ZCode
    }

    fn display_name(&self) -> &str {
        "ZCode"
    }

    fn data_dir(&self) -> PathBuf {
        self.engine_dir()
    }
}

impl AgentAdapter for ZCodeClaudeAdapter {
    fn is_available(&self) -> bool {
        self.engine_dir().is_dir()
    }

    fn list_conversations(&self) -> Result<Vec<ConversationSummary>> {
        let mut summaries = Vec::new();
        for session in self.list_session_paths() {
            match parse_claude_summary(&session) {
                Ok(summary) => summaries.push(summary),
                Err(error) => eprintln!(
                    "Warning: failed to parse ZCode Claude session {}: {error}",
                    session.path.display()
                ),
            }
        }
        summaries.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(summaries)
    }

    fn read_conversation(&self, id: &str) -> Result<Conversation> {
        let session = self.find_session(id)?;
        parse_claude_conversation(&session)
    }

    fn write_conversation(&self, conv: &Conversation) -> Result<String> {
        let (profile_id, projects_dir) = self.default_profile_projects_dir()?;
        fs::create_dir_all(&projects_dir)?;
        let native = agentswap_claude::ClaudeAdapter::with_projects_dir(projects_dir);
        let raw_id = native.write_conversation(conv)?;
        Ok(encode_profile_id(&profile_id, &raw_id))
    }

    fn delete_conversation(&self, id: &str) -> Result<()> {
        let session = self.find_session(id)?;
        move_path_to_trash(&session.path)?;
        Ok(())
    }

    fn render_prompt(&self, conv: &Conversation) -> Result<String> {
        Ok(render_prompt(conv))
    }

    fn agent_kind(&self) -> AgentKind {
        AgentKind::ZCodeClaude
    }

    fn display_name(&self) -> &str {
        "ZCode Claude"
    }

    fn data_dir(&self) -> PathBuf {
        self.engine_dir()
    }
}

impl AgentAdapter for ZCodeCodexAdapter {
    fn is_available(&self) -> bool {
        self.engine_dir().is_dir()
    }

    fn list_conversations(&self) -> Result<Vec<ConversationSummary>> {
        let mut summaries = Vec::new();
        for (profile_id, profile_dir) in self.profile_dirs() {
            let native = CodexAdapter::with_codex_dir(profile_dir);
            if !native.is_available() {
                continue;
            }
            for mut summary in native.list_conversations()? {
                summary.id = encode_profile_id(&profile_id, &summary.id);
                summary.source_agent = AgentKind::ZCodeCodex;
                summary.project_dir = normalize_project_dir(&summary.project_dir);
                summaries.push(summary);
            }
        }
        summaries.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(summaries)
    }

    fn read_conversation(&self, id: &str) -> Result<Conversation> {
        let (profile_id, raw_id) =
            split_profile_id(id).ok_or_else(|| anyhow::anyhow!("Invalid ZCode Codex id: {id}"))?;
        let native = CodexAdapter::with_codex_dir(self.profile_dir(profile_id));
        let mut conv = native.read_conversation(raw_id)?;
        conv.id = id.to_string();
        conv.source_agent = AgentKind::ZCodeCodex;
        conv.project_dir = normalize_project_dir(&conv.project_dir);
        add_zcode_metadata(
            &mut conv,
            CODEX_ENGINE,
            profile_id,
            self.storage_path_for_id(id),
            None,
            None,
        );
        Ok(conv)
    }

    fn write_conversation(&self, conv: &Conversation) -> Result<String> {
        let (profile_id, profile_dir) = self.default_profile_dir()?;
        fs::create_dir_all(&profile_dir)?;
        let native = CodexAdapter::with_codex_dir(profile_dir);
        let raw_id = native.write_conversation(conv)?;
        Ok(encode_profile_id(&profile_id, &raw_id))
    }

    fn delete_conversation(&self, id: &str) -> Result<()> {
        let (profile_id, raw_id) =
            split_profile_id(id).ok_or_else(|| anyhow::anyhow!("Invalid ZCode Codex id: {id}"))?;
        let native = CodexAdapter::with_codex_dir(self.profile_dir(profile_id));
        native.delete_conversation(raw_id)
    }

    fn render_prompt(&self, conv: &Conversation) -> Result<String> {
        Ok(render_prompt(conv))
    }

    fn agent_kind(&self) -> AgentKind {
        AgentKind::ZCodeCodex
    }

    fn display_name(&self) -> &str {
        "ZCode Codex"
    }

    fn data_dir(&self) -> PathBuf {
        self.engine_dir()
    }
}

impl AgentAdapter for ZCodeGeminiAdapter {
    fn is_available(&self) -> bool {
        self.engine_dir().is_dir()
    }

    fn list_conversations(&self) -> Result<Vec<ConversationSummary>> {
        Ok(Vec::new())
    }

    fn read_conversation(&self, id: &str) -> Result<Conversation> {
        anyhow::bail!("ZCode Gemini profile is config-only; no conversation store found for {id}")
    }

    fn write_conversation(&self, _conv: &Conversation) -> Result<String> {
        anyhow::bail!(
            "ZCode Gemini write is disabled until a native conversation store is detected"
        )
    }

    fn delete_conversation(&self, id: &str) -> Result<()> {
        anyhow::bail!("ZCode Gemini profile is config-only; cannot delete {id}")
    }

    fn render_prompt(&self, conv: &Conversation) -> Result<String> {
        Ok(render_prompt(conv))
    }

    fn agent_kind(&self) -> AgentKind {
        AgentKind::ZCodeGemini
    }

    fn display_name(&self) -> &str {
        "ZCode Gemini"
    }

    fn data_dir(&self) -> PathBuf {
        self.engine_dir()
    }
}

impl AgentAdapter for ZCodeOpenCodeAdapter {
    fn is_available(&self) -> bool {
        self.engine_dir().is_dir()
    }

    fn list_conversations(&self) -> Result<Vec<ConversationSummary>> {
        Ok(Vec::new())
    }

    fn read_conversation(&self, id: &str) -> Result<Conversation> {
        anyhow::bail!("ZCode OpenCode profile is config-only; no conversation store found for {id}")
    }

    fn write_conversation(&self, _conv: &Conversation) -> Result<String> {
        anyhow::bail!(
            "ZCode OpenCode write is disabled until a native conversation store is detected"
        )
    }

    fn delete_conversation(&self, id: &str) -> Result<()> {
        anyhow::bail!("ZCode OpenCode profile is config-only; cannot delete {id}")
    }

    fn render_prompt(&self, conv: &Conversation) -> Result<String> {
        Ok(render_prompt(conv))
    }

    fn agent_kind(&self) -> AgentKind {
        AgentKind::ZCodeOpenCode
    }

    fn display_name(&self) -> &str {
        "ZCode OpenCode"
    }

    fn data_dir(&self) -> PathBuf {
        self.engine_dir()
    }
}

fn default_zcode_root() -> PathBuf {
    let mut root = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));
    for part in ZCODE_ROOT_PARTS {
        root = root.join(part);
    }
    root
}

fn zcode_v2_dir(root_dir: &Path) -> PathBuf {
    if root_dir.file_name().and_then(|name| name.to_str()) == Some("acp-config") {
        return root_dir
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| root_dir.to_path_buf());
    }

    root_dir.to_path_buf()
}

fn profile_dirs(engine_dir: &Path) -> Vec<(String, PathBuf)> {
    let mut profiles = match fs::read_dir(engine_dir) {
        Ok(entries) => entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().map(|ty| ty.is_dir()).unwrap_or(false))
            .filter_map(|entry| {
                let profile_id = entry.file_name().to_string_lossy().to_string();
                if profile_id.is_empty() {
                    None
                } else {
                    Some((profile_id, entry.path()))
                }
            })
            .collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };
    profiles.sort_by(|left, right| left.0.cmp(&right.0));
    profiles
}

fn list_zcode_task_paths(sessions_dir: &Path) -> Vec<ZCodeTaskPath> {
    let mut tasks = match fs::read_dir(sessions_dir) {
        Ok(profile_entries) => profile_entries
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_type().map(|ty| ty.is_dir()).unwrap_or(false))
            .flat_map(|profile_entry| {
                let profile_id = profile_entry.file_name().to_string_lossy().to_string();
                fs::read_dir(profile_entry.path())
                    .into_iter()
                    .flat_map(|entries| entries.filter_map(|entry| entry.ok()))
                    .filter(|entry| entry.file_type().map(|ty| ty.is_file()).unwrap_or(false))
                    .filter_map(move |entry| {
                        let path = entry.path();
                        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                            return None;
                        }
                        let task_id = path
                            .file_stem()
                            .and_then(|stem| stem.to_str())
                            .filter(|stem| !stem.is_empty())?
                            .to_string();
                        Some(ZCodeTaskPath {
                            task_id,
                            profile_id: profile_id.clone(),
                            path,
                        })
                    })
            })
            .collect::<Vec<_>>(),
        Err(_) => Vec::new(),
    };
    tasks.sort_by(|left, right| left.path.cmp(&right.path));
    tasks
}

fn parse_zcode_task(task_path: &ZCodeTaskPath) -> Result<ParsedZCodeTask> {
    let data = fs::read(&task_path.path)
        .with_context(|| format!("Failed to read ZCode task: {}", task_path.path.display()))?;
    let value: Value = serde_json::from_slice(&data)
        .with_context(|| format!("Failed to parse ZCode task: {}", task_path.path.display()))?;
    let meta = value.get("meta").unwrap_or(&Value::Null);
    let task_id = value_string(meta, "taskId").unwrap_or_else(|| task_path.task_id.clone());
    let provider = normalize_zcode_task_provider(value_string(meta, "provider").as_deref());
    let title = value_string(meta, "title").filter(|title| is_meaningful_task_text(title));
    let project_dir = value_string(meta, "workspacePath")
        .or_else(|| value_string(meta, "cwd"))
        .map(|path| normalize_project_dir(&path))
        .unwrap_or_default();
    let now = Utc::now();
    let created_at = value_i64(meta, "createdAt")
        .map(ms_to_datetime)
        .unwrap_or(now);
    let updated_at = value_i64(meta, "updatedAt")
        .map(ms_to_datetime)
        .unwrap_or(created_at);
    let acp_session_id = value_string(meta, "acpSessionId");
    let message_count = value
        .get("messages")
        .and_then(|messages| messages.as_array())
        .map(|messages| {
            messages
                .iter()
                .filter(|message| task_message_is_visible(message))
                .count()
        })
        .unwrap_or(0);
    let file_count = meta
        .get("changeSummary")
        .and_then(|summary| value_i64(summary, "fileCount"))
        .map(|count| count.max(0) as usize)
        .or_else(|| {
            value
                .get("fileChanges")
                .and_then(|changes| changes.as_array())
                .map(|changes| changes.len())
        })
        .unwrap_or(0);

    Ok(ParsedZCodeTask {
        task_id,
        profile_id: task_path.profile_id.clone(),
        provider,
        acp_session_id,
        project_dir,
        title,
        created_at,
        updated_at,
        message_count,
        file_count,
        value,
        path: task_path.path.clone(),
    })
}

fn zcode_task_summary(task: &ParsedZCodeTask) -> ConversationSummary {
    ConversationSummary {
        id: encode_zcode_task_id(&task.provider, &task.profile_id, &task.task_id),
        source_agent: AgentKind::ZCode,
        project_dir: task.project_dir.clone(),
        created_at: task.created_at,
        updated_at: task.updated_at,
        summary: task
            .title
            .clone()
            .or_else(|| first_task_user_message(&task.value).map(|text| truncate_str(&text, 100))),
        message_count: task.message_count,
        file_count: task.file_count,
    }
}

fn parse_zcode_task_conversation(path: &Path, profile_id: &str) -> Result<Conversation> {
    let task_path = ZCodeTaskPath {
        task_id: path
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("task")
            .to_string(),
        profile_id: profile_id.to_string(),
        path: path.to_path_buf(),
    };
    let task = parse_zcode_task(&task_path)?;
    let mut messages = Vec::new();

    if let Some(task_messages) = task.value.get("messages").and_then(|value| value.as_array()) {
        for (index, message_value) in task_messages.iter().enumerate() {
            let role = task_role(message_value);
            let content = task_message_content(message_value);
            let tool_calls = task_tool_calls(message_value);
            if content.trim().is_empty() && tool_calls.is_empty() {
                continue;
            }
            if role == Role::User && !is_meaningful_task_text(&content) {
                continue;
            }

            let timestamp = value_i64(message_value, "timestamp")
                .map(ms_to_datetime)
                .unwrap_or(task.updated_at);
            let mut metadata = HashMap::new();
            metadata.insert("zcode_engine".to_string(), json!(task.provider));
            metadata.insert("zcode_cli".to_string(), json!(task.provider));
            metadata.insert("zcode_profile".to_string(), json!(task.profile_id));
            metadata.insert("zcode_task_id".to_string(), json!(task.task_id));
            metadata.insert(
                "zcode_storage_path".to_string(),
                json!(normalize_storage_path(task.path.to_string_lossy().as_ref())),
            );
            if let Some(acp_session_id) = &task.acp_session_id {
                metadata.insert("zcode_acp_session_id".to_string(), json!(acp_session_id));
            }
            if let Some(model) = value_string(message_value, "model")
                .or_else(|| value_string(task.value.get("meta").unwrap_or(&Value::Null), "model"))
            {
                metadata.insert("model".to_string(), json!(model));
            }
            if let Some(turn_index) = value_i64(message_value, "turnIndex") {
                metadata.insert("turn_index".to_string(), json!(turn_index));
            }

            messages.push(Message {
                id: stable_uuid(&format!(
                    "zcode-task:{}:{}:{}",
                    task.profile_id, task.task_id, index
                )),
                timestamp,
                role,
                content,
                tool_calls,
                metadata,
            });
        }
    }

    let file_changes = task_file_changes(&task, messages.last().map(|message| message.id));
    Ok(Conversation {
        id: encode_zcode_task_id(&task.provider, &task.profile_id, &task.task_id),
        source_agent: AgentKind::ZCode,
        project_dir: task.project_dir,
        created_at: task.created_at,
        updated_at: task.updated_at,
        summary: task
            .title
            .or_else(|| first_task_user_message(&task.value).map(|text| truncate_str(&text, 100))),
        messages,
        file_changes,
    })
}

fn classify_claude_session(
    projects_dir: &Path,
    profile_id: &str,
    path: &Path,
) -> Option<ClaudeSessionPath> {
    let relative = path.strip_prefix(projects_dir).ok()?;
    let parts = relative
        .components()
        .map(|part| part.as_os_str().to_string_lossy().to_string())
        .collect::<Vec<_>>();

    match parts.as_slice() {
        [project_encoded, file_name] if file_name.ends_with(".jsonl") => {
            let raw_session_id = trim_extension(file_name, ".jsonl");
            Some(ClaudeSessionPath {
                id: encode_profile_id(profile_id, &raw_session_id),
                profile_id: profile_id.to_string(),
                path: path.to_path_buf(),
                project_dir: decode_project_path(project_encoded),
                parent_session_id: None,
                subagent_id: None,
            })
        }
        [project_encoded, parent_session_id, subagents, file_name]
            if subagents == "subagents" && file_name.ends_with(".jsonl") =>
        {
            let subagent_id = trim_extension(file_name, ".jsonl");
            let raw_session_id = format!("{parent_session_id}:{SUBAGENT_MARKER}:{subagent_id}");
            Some(ClaudeSessionPath {
                id: encode_profile_id(profile_id, &raw_session_id),
                profile_id: profile_id.to_string(),
                path: path.to_path_buf(),
                project_dir: decode_project_path(project_encoded),
                parent_session_id: Some(parent_session_id.to_string()),
                subagent_id: Some(subagent_id),
            })
        }
        _ => None,
    }
}

fn parse_claude_summary(session: &ClaudeSessionPath) -> Result<ConversationSummary> {
    let file = fs::File::open(&session.path)
        .with_context(|| format!("Failed to open session file: {}", session.path.display()))?;
    let reader = BufReader::new(file);
    let mut message_count = 0usize;
    let mut file_paths = HashSet::new();
    let mut first_user_message: Option<String> = None;
    let mut summary: Option<String> = None;
    let mut first_timestamp: Option<DateTime<Utc>> = None;
    let mut last_timestamp: Option<DateTime<Utc>> = None;
    let mut first_cwd: Option<String> = None;

    for line in reader.lines() {
        let event = match parse_claude_event(&line?) {
            Some(event) => event,
            None => continue,
        };
        if first_cwd.is_none() {
            first_cwd = event
                .cwd
                .as_deref()
                .filter(|cwd| !cwd.trim().is_empty())
                .map(normalize_project_dir);
        }
        if let Some(ts) = event
            .timestamp
            .as_ref()
            .and_then(|value| value.parse::<DateTime<Utc>>().ok())
        {
            if first_timestamp.map(|current| ts < current).unwrap_or(true) {
                first_timestamp = Some(ts);
            }
            if last_timestamp.map(|current| ts > current).unwrap_or(true) {
                last_timestamp = Some(ts);
            }
        }

        match event.event_type.as_str() {
            "user" if include_claude_event(session, event.is_sidechain) => {
                if let Some(message) = &event.message {
                    if let ClaudeContent::Text(text) = &message.content {
                        if is_meaningful_claude_title_text(text) {
                            message_count += 1;
                            if first_user_message.is_none() {
                                first_user_message = Some(truncate_str(text, 100));
                            }
                        }
                    }
                }
            }
            "assistant" if include_claude_event(session, event.is_sidechain) => {
                if let Some(message) = &event.message {
                    if let ClaudeContent::Blocks(blocks) = &message.content {
                        if blocks.iter().any(|block| match block {
                            ClaudeContentBlock::Text { text } => {
                                is_visible_claude_assistant_text(text)
                            }
                            _ => false,
                        }) {
                            message_count += 1;
                        }
                        for block in blocks {
                            if let ClaudeContentBlock::ToolUse { name, input, .. } = block {
                                if let Some(path) = extract_claude_file_path(name, input) {
                                    file_paths.insert(path);
                                }
                            }
                        }
                    }
                }
            }
            "summary" => {
                summary = event.summary;
            }
            _ => {}
        }
    }

    let now = Utc::now();
    Ok(ConversationSummary {
        id: session.id.clone(),
        source_agent: AgentKind::ZCodeClaude,
        project_dir: first_cwd.unwrap_or_else(|| normalize_project_dir(&session.project_dir)),
        created_at: first_timestamp.unwrap_or(now),
        updated_at: last_timestamp.unwrap_or(now),
        summary: summary.or(first_user_message),
        message_count,
        file_count: file_paths.len(),
    })
}

fn parse_claude_conversation(session: &ClaudeSessionPath) -> Result<Conversation> {
    let file = fs::File::open(&session.path)
        .with_context(|| format!("Failed to open session file: {}", session.path.display()))?;
    let reader = BufReader::new(file);
    let mut messages = Vec::new();
    let mut file_changes = Vec::new();
    let mut pending_tools: HashMap<String, PendingClaudeTool> = HashMap::new();
    let mut summary: Option<String> = None;
    let mut first_timestamp: Option<DateTime<Utc>> = None;
    let mut last_timestamp: Option<DateTime<Utc>> = None;
    let mut first_cwd: Option<String> = None;

    for line in reader.lines() {
        let event = match parse_claude_event(&line?) {
            Some(event) => event,
            None => continue,
        };
        if first_cwd.is_none() {
            first_cwd = event
                .cwd
                .as_deref()
                .filter(|cwd| !cwd.trim().is_empty())
                .map(normalize_project_dir);
        }
        if !include_claude_event(session, event.is_sidechain) {
            continue;
        }

        let ts = event
            .timestamp
            .as_ref()
            .and_then(|value| value.parse::<DateTime<Utc>>().ok())
            .unwrap_or_else(Utc::now);
        if first_timestamp.map(|current| ts < current).unwrap_or(true) {
            first_timestamp = Some(ts);
        }
        if last_timestamp.map(|current| ts > current).unwrap_or(true) {
            last_timestamp = Some(ts);
        }

        match event.event_type.as_str() {
            "user" => {
                let Some(message) = &event.message else {
                    continue;
                };
                match &message.content {
                    ClaudeContent::Text(text) => {
                        if is_meaningful_claude_title_text(text) {
                            messages.push(Message {
                                id: event_uuid(event.uuid.as_deref()),
                                timestamp: ts,
                                role: Role::User,
                                content: text.clone(),
                                tool_calls: Vec::new(),
                                metadata: HashMap::new(),
                            });
                        }
                    }
                    ClaudeContent::Blocks(blocks) => {
                        for block in blocks {
                            let ClaudeContentBlock::ToolResult {
                                tool_use_id,
                                content,
                            } = block
                            else {
                                continue;
                            };
                            let output = content.as_ref().map(|content| match content {
                                ToolResultContent::Text(text) => text.clone(),
                                ToolResultContent::Other(value) => value.to_string(),
                            });
                            if let Some(pending) = pending_tools.remove(tool_use_id) {
                                if let Some(message) = messages.get_mut(pending.message_index) {
                                    if let Some(tool_call) =
                                        message.tool_calls.get_mut(pending.tool_index)
                                    {
                                        tool_call.output = output.clone();
                                        tool_call.status = ToolStatus::Success;
                                    }
                                }
                                if let Some(path) =
                                    extract_claude_file_path(&pending.name, &pending.input)
                                {
                                    let message_id = messages
                                        .get(pending.message_index)
                                        .map(|message| message.id)
                                        .unwrap_or_else(Uuid::new_v4);
                                    file_changes.push(FileChange {
                                        path,
                                        change_type: ChangeType::Modified,
                                        timestamp: ts,
                                        message_id,
                                    });
                                }
                            }
                        }
                    }
                }
            }
            "assistant" => {
                let Some(message) = &event.message else {
                    continue;
                };
                let ClaudeContent::Blocks(blocks) = &message.content else {
                    continue;
                };
                let message_index = messages.len();
                let mut assistant = Message {
                    id: event_uuid(event.uuid.as_deref()),
                    timestamp: ts,
                    role: Role::Assistant,
                    content: String::new(),
                    tool_calls: Vec::new(),
                    metadata: HashMap::new(),
                };
                let mut thinking = Vec::new();
                for block in blocks {
                    match block {
                        ClaudeContentBlock::Text { text } => {
                            if is_visible_claude_assistant_text(text) {
                                if !assistant.content.is_empty() {
                                    assistant.content.push('\n');
                                }
                                assistant.content.push_str(text);
                            }
                        }
                        ClaudeContentBlock::Thinking { thinking: text } => {
                            thinking.push(text.clone());
                        }
                        ClaudeContentBlock::ToolUse { id, name, input } => {
                            let tool_index = assistant.tool_calls.len();
                            assistant.tool_calls.push(ToolCall {
                                name: name.clone(),
                                input: input.clone(),
                                output: None,
                                status: ToolStatus::Success,
                            });
                            pending_tools.insert(
                                id.clone(),
                                PendingClaudeTool {
                                    message_index,
                                    tool_index,
                                    name: name.clone(),
                                    input: input.clone(),
                                },
                            );
                        }
                        ClaudeContentBlock::ToolResult { .. } => {}
                    }
                }
                if !thinking.is_empty() {
                    assistant
                        .metadata
                        .insert("thinking".to_string(), json!(thinking));
                }
                if !assistant.content.trim().is_empty() || !assistant.tool_calls.is_empty() {
                    messages.push(assistant);
                }
            }
            "summary" => {
                summary = event.summary;
            }
            _ => {}
        }
    }

    let now = Utc::now();
    let mut conv = Conversation {
        id: session.id.clone(),
        source_agent: AgentKind::ZCodeClaude,
        project_dir: first_cwd.unwrap_or_else(|| normalize_project_dir(&session.project_dir)),
        created_at: first_timestamp.unwrap_or(now),
        updated_at: last_timestamp.unwrap_or(now),
        summary,
        messages,
        file_changes,
    };
    add_zcode_metadata(
        &mut conv,
        CLAUDE_ENGINE,
        &session.profile_id,
        Some(normalize_storage_path(
            session.path.to_string_lossy().as_ref(),
        )),
        session.parent_session_id.as_deref(),
        session.subagent_id.as_deref(),
    );
    Ok(conv)
}

fn add_zcode_metadata(
    conv: &mut Conversation,
    engine: &str,
    profile_id: &str,
    storage_path: Option<String>,
    parent_session_id: Option<&str>,
    subagent_id: Option<&str>,
) {
    for message in &mut conv.messages {
        message
            .metadata
            .insert("zcode_engine".to_string(), json!(engine));
        message
            .metadata
            .insert("zcode_cli".to_string(), json!(engine));
        message
            .metadata
            .insert("zcode_profile".to_string(), json!(profile_id));
        if let Some(path) = &storage_path {
            message
                .metadata
                .insert("zcode_storage_path".to_string(), json!(path));
        }
        if let Some(parent) = parent_session_id {
            message
                .metadata
                .insert("zcode_parent_session_id".to_string(), json!(parent));
        }
        if let Some(subagent) = subagent_id {
            message
                .metadata
                .insert("zcode_subagent_id".to_string(), json!(subagent));
        }
    }
}

fn count_claude_sessions(projects_dir: &Path) -> usize {
    if !projects_dir.is_dir() {
        return 0;
    }
    WalkDir::new(projects_dir)
        .min_depth(2)
        .max_depth(4)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.file_type().is_file()
                && entry.path().extension().and_then(|ext| ext.to_str()) == Some("jsonl")
                && classify_claude_session(projects_dir, "", entry.path()).is_some()
        })
        .count()
}

fn count_codex_threads(db_path: &Path) -> Result<usize> {
    let conn = Connection::open_with_flags(db_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let count = conn.query_row("SELECT COUNT(*) FROM threads", [], |row| {
        row.get::<_, i64>(0)
    })?;
    Ok(count.max(0) as usize)
}

fn count_extension(root: &Path, extension: &str, skip_node_modules: bool) -> usize {
    if !root.is_dir() {
        return 0;
    }
    WalkDir::new(root)
        .into_iter()
        .filter_entry(|entry| {
            !skip_node_modules || entry.file_name().to_string_lossy() != "node_modules"
        })
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.file_type().is_file()
                && entry.path().extension().and_then(|ext| ext.to_str()) == Some(extension)
        })
        .count()
}

fn count_named_parent_files(root: &Path, parent_name: &str, extension: &str) -> usize {
    if !root.is_dir() {
        return 0;
    }
    WalkDir::new(root)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            entry.file_type().is_file()
                && entry.path().extension().and_then(|ext| ext.to_str()) == Some(extension)
                && entry
                    .path()
                    .parent()
                    .and_then(|parent| parent.file_name())
                    .and_then(|name| name.to_str())
                    == Some(parent_name)
        })
        .count()
}

fn count_opencode_dbs(root: &Path) -> usize {
    if !root.is_dir() {
        return 0;
    }
    WalkDir::new(root)
        .max_depth(2)
        .into_iter()
        .filter_map(|entry| entry.ok())
        .filter(|entry| {
            let file_name = entry.file_name().to_string_lossy().to_ascii_lowercase();
            entry.file_type().is_file()
                && (file_name == "opencode.db"
                    || (file_name.starts_with("opencode-") && file_name.ends_with(".db")))
        })
        .count()
}

fn trim_extension(value: &str, extension: &str) -> String {
    value.strip_suffix(extension).unwrap_or(value).to_string()
}

fn value_string(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(|value| value.as_str())
        .map(ToString::to_string)
}

fn value_i64(value: &Value, key: &str) -> Option<i64> {
    value.get(key).and_then(|value| value.as_i64())
}

fn ms_to_datetime(ms: i64) -> DateTime<Utc> {
    Utc.timestamp_millis_opt(ms)
        .single()
        .unwrap_or_else(Utc::now)
}

fn stable_uuid(source: &str) -> Uuid {
    Uuid::new_v5(&Uuid::NAMESPACE_URL, source.as_bytes())
}

fn encode_profile_id(profile_id: &str, raw_id: &str) -> String {
    format!("{profile_id}:{raw_id}")
}

fn encode_zcode_cli_id(engine: &str, raw_id: &str) -> String {
    format!("{engine}:{raw_id}")
}

fn encode_zcode_task_id(provider: &str, profile_id: &str, task_id: &str) -> String {
    encode_zcode_cli_id(provider, &format!("{TASK_MARKER}:{profile_id}:{task_id}"))
}

fn split_zcode_cli_id(id: &str) -> Option<(&str, &str)> {
    let (engine, raw) = id.split_once(':')?;
    if engine.is_empty() || raw.is_empty() {
        None
    } else {
        Some((engine, raw))
    }
}

fn split_task_id(id: &str) -> Option<(&str, &str)> {
    let raw = id.strip_prefix(&format!("{TASK_MARKER}:"))?;
    let (profile, task_id) = raw.split_once(':')?;
    if profile.is_empty() || task_id.is_empty() {
        None
    } else {
        Some((profile, task_id))
    }
}

fn split_profile_id(id: &str) -> Option<(&str, &str)> {
    let (profile, raw) = id.split_once(':')?;
    if profile.is_empty() || raw.is_empty() {
        None
    } else {
        Some((profile, raw))
    }
}

fn normalize_project_dir(path: &str) -> String {
    let mut normalized = path.trim().to_string();
    if let Some(stripped) = normalized.strip_prefix(r"\\?\UNC\") {
        normalized = format!(r"\\{stripped}");
    } else if let Some(stripped) = normalized.strip_prefix(r"\\?\") {
        normalized = stripped.to_string();
    } else if let Some(stripped) = normalized.strip_prefix("//?/") {
        normalized = stripped.to_string();
    }
    normalized
}

fn normalize_storage_path(path: &str) -> String {
    normalize_project_dir(path)
}

fn is_listable_zcode_claude_summary(summary: &ConversationSummary) -> bool {
    !summary.id.contains(":subagent:")
        && summary
            .summary
            .as_deref()
            .map(str::trim)
            .is_some_and(|value| !value.is_empty())
        && summary.message_count > 0
}

fn event_uuid(value: Option<&str>) -> Uuid {
    value
        .and_then(|value| Uuid::parse_str(value).ok())
        .unwrap_or_else(Uuid::new_v4)
}

fn include_claude_event(session: &ClaudeSessionPath, is_sidechain: bool) -> bool {
    !is_sidechain || session.subagent_id.is_some()
}

fn is_meaningful_claude_title_text(value: &str) -> bool {
    !is_claude_control_text(value)
}

fn is_visible_claude_assistant_text(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty() && trimmed != "No response requested."
}

fn normalize_zcode_task_provider(provider: Option<&str>) -> String {
    match provider.unwrap_or("unknown").trim().to_ascii_lowercase().as_str() {
        CLAUDE_ENGINE => CLAUDE_ENGINE.to_string(),
        CODEX_ENGINE => CODEX_ENGINE.to_string(),
        GEMINI_ENGINE => GEMINI_ENGINE.to_string(),
        OPENCODE_ENGINE => OPENCODE_ENGINE.to_string(),
        GLM_ENGINE => GLM_ENGINE.to_string(),
        _ => "unknown".to_string(),
    }
}

fn task_role(value: &Value) -> Role {
    match value.get("role").and_then(|role| role.as_str()) {
        Some("assistant") => Role::Assistant,
        Some("system") => Role::System,
        _ => Role::User,
    }
}

fn task_message_content(value: &Value) -> String {
    if let Some(content) = value.get("content").and_then(|content| content.as_str()) {
        return content.to_string();
    }

    value
        .get("parts")
        .and_then(|parts| parts.as_array())
        .map(|parts| {
            parts
                .iter()
                .filter_map(task_part_text)
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
}

fn task_part_text(value: &Value) -> Option<String> {
    if let Some(text) = value.get("content").and_then(|content| content.as_str()) {
        return Some(text.to_string());
    }
    value
        .get("content")
        .and_then(|content| content.get("text"))
        .and_then(|text| text.as_str())
        .map(ToString::to_string)
}

fn is_meaningful_task_text(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed != "No response requested."
        && !is_claude_control_text(trimmed)
}

fn task_message_is_visible(value: &Value) -> bool {
    let content = task_message_content(value);
    if is_meaningful_task_text(&content) {
        return true;
    }
    task_role(value) == Role::Assistant && !task_tool_calls(value).is_empty()
}

fn first_task_user_message(value: &Value) -> Option<String> {
    value
        .get("messages")
        .and_then(|messages| messages.as_array())?
        .iter()
        .filter(|message| task_role(message) == Role::User)
        .map(task_message_content)
        .find(|content| is_meaningful_task_text(content))
}

fn task_tool_calls(value: &Value) -> Vec<ToolCall> {
    value
        .get("tools")
        .and_then(|tools| tools.as_array())
        .map(|tools| {
            tools
                .iter()
                .enumerate()
                .map(|(index, tool)| task_tool_call(tool, index))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn task_tool_call(tool: &Value, index: usize) -> ToolCall {
    let name = value_string(tool, "title")
        .or_else(|| value_string(tool, "kind"))
        .or_else(|| {
            tool.get("raw")
                .and_then(|raw| raw.get("_meta"))
                .and_then(|meta| meta.get("claudeCode"))
                .and_then(|claude| value_string(claude, "toolName"))
        })
        .unwrap_or_else(|| format!("tool {}", index + 1));
    let input = tool
        .get("input")
        .cloned()
        .or_else(|| tool.get("raw").cloned())
        .unwrap_or_else(|| json!({}));
    let output = value_string(tool, "output").or_else(|| {
        tool.get("raw")
            .and_then(|raw| raw.get("rawOutput"))
            .and_then(|output| output.as_str())
            .map(ToString::to_string)
    });
    let status = match tool
        .get("status")
        .and_then(|status| status.as_str())
        .unwrap_or_default()
    {
        "completed" | "success" | "succeeded" => ToolStatus::Success,
        _ => ToolStatus::Error,
    };

    ToolCall {
        name,
        input,
        output,
        status,
    }
}

fn task_file_changes(task: &ParsedZCodeTask, fallback_message_id: Option<Uuid>) -> Vec<FileChange> {
    let message_id = fallback_message_id.unwrap_or_else(|| {
        stable_uuid(&format!(
            "zcode-task:{}:{}:file-change",
            task.profile_id, task.task_id
        ))
    });
    task.value
        .get("meta")
        .and_then(|meta| meta.get("changeSummary"))
        .and_then(|summary| summary.get("files"))
        .and_then(|files| files.as_array())
        .map(|files| {
            files
                .iter()
                .filter_map(|file| {
                    let path = value_string(file, "path")?;
                    let removed = value_i64(file, "removed").unwrap_or(0);
                    let added = value_i64(file, "added").unwrap_or(0);
                    let change_type = if added > 0 && removed == 0 {
                        ChangeType::Created
                    } else {
                        ChangeType::Modified
                    };
                    Some(FileChange {
                        path,
                        change_type,
                        timestamp: task.updated_at,
                        message_id,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

fn is_claude_control_text(value: &str) -> bool {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return true;
    }

    let lower = trimmed.to_ascii_lowercase();
    lower.starts_with("<local-command-caveat")
        || lower.starts_with("<local-command-stdout")
        || lower.starts_with("<local-command-stderr")
        || lower.starts_with("<local-command-error")
        || lower.starts_with("<command-name")
        || lower.starts_with("<command-message")
        || lower.starts_with("<command-args")
        || lower.starts_with("<system-reminder")
}

fn truncate_str(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

fn extract_claude_file_path(tool_name: &str, input: &Value) -> Option<String> {
    match tool_name {
        "Write" | "Edit" | "NotebookEdit" => input
            .get("file_path")
            .or_else(|| input.get("notebook_path"))
            .and_then(|value| value.as_str())
            .map(|value| value.to_string()),
        _ => None,
    }
}

fn render_prompt(conv: &Conversation) -> String {
    let mut output = String::new();
    output.push_str(&format!(
        "# Conversation: {}\n\n",
        conv.summary.as_deref().unwrap_or(&conv.id)
    ));
    output.push_str(&format!("**Source:** {}\n", conv.source_agent_name()));
    output.push_str(&format!("**Project:** {}\n\n", conv.project_dir));
    for message in &conv.messages {
        let role = match message.role {
            Role::User => "User",
            Role::Assistant => "Assistant",
            Role::System => "System",
        };
        if !message.content.trim().is_empty() {
            output.push_str(&format!("## {role}\n{}\n\n", message.content));
        }
        for tool_call in &message.tool_calls {
            output.push_str(&format!("**Tool: {}**\n", tool_call.name));
            if let Some(output_text) = &tool_call.output {
                output.push_str(&format!("Output: {output_text}\n"));
            }
            output.push('\n');
        }
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use tempfile::TempDir;

    fn write_jsonl(path: &Path, lines: &[&str]) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, lines.join("\n")).unwrap();
    }

    #[test]
    fn discovery_classifies_all_zcode_engines_without_reading_runtime_metadata() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_jsonl(
            &root
                .join("claude")
                .join("p1")
                .join("projects")
                .join("D--VSP")
                .join("session-1.jsonl"),
            &[r#"{"type":"user","message":{"role":"user","content":"hello"}}"#],
        );
        fs::create_dir_all(root.join("gemini").join("p1").join(".gemini")).unwrap();
        fs::write(
            root.join("gemini").join("p1").join("runtime-meta.json"),
            r#"{"api_key":"do-not-index"}"#,
        )
        .unwrap();
        fs::create_dir_all(root.join("opencode").join("p1").join("node_modules")).unwrap();
        fs::create_dir_all(root.join("glm").join("p1").join("skills")).unwrap();

        let statuses = discover_profiles(root);

        assert!(statuses.iter().any(|status| {
            status.engine == "claude"
                && status.profile_id == "p1"
                && status.capability == "conversation_reader"
                && status.conversation_count == 1
        }));
        assert!(statuses.iter().any(|status| {
            status.engine == "gemini"
                && status.profile_id == "p1"
                && status.capability == "config_only"
                && status.conversation_count == 0
        }));
        assert!(statuses.iter().any(|status| {
            status.engine == "opencode"
                && status.profile_id == "p1"
                && status.capability == "config_only"
        }));
        assert!(statuses
            .iter()
            .any(|status| status.engine == "glm" && status.capability == "config_only"));
    }

    #[test]
    fn zcode_claude_reads_parent_and_subagent_jsonl() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let session_path = root
            .join("claude")
            .join("p1")
            .join("projects")
            .join("D--VSP")
            .join("session-1.jsonl");
        write_jsonl(
            &session_path,
            &[
                r#"{"type":"user","uuid":"11111111-1111-1111-1111-111111111111","timestamp":"2026-05-01T00:00:00Z","cwd":"D:\\VSP","message":{"role":"user","content":"main request"}}"#,
                r#"{"type":"assistant","uuid":"22222222-2222-2222-2222-222222222222","timestamp":"2026-05-01T00:00:01Z","message":{"role":"assistant","id":"msg_1","content":[{"type":"text","text":"main answer"}]}}"#,
            ],
        );
        let subagent_path = root
            .join("claude")
            .join("p1")
            .join("projects")
            .join("D--VSP")
            .join("session-1")
            .join("subagents")
            .join("agent-1.jsonl");
        write_jsonl(
            &subagent_path,
            &[
                r#"{"type":"user","uuid":"33333333-3333-3333-3333-333333333333","timestamp":"2026-05-01T00:00:02Z","message":{"role":"user","content":"sub task"}}"#,
                r#"{"type":"assistant","uuid":"44444444-4444-4444-4444-444444444444","timestamp":"2026-05-01T00:00:03Z","message":{"role":"assistant","id":"msg_2","content":[{"type":"text","text":"sub answer"}]},"agentId":"agent-1","slug":"helper"}"#,
            ],
        );

        let adapter = ZCodeClaudeAdapter::with_root_dir(root.to_path_buf());
        let summaries = adapter.list_conversations().unwrap();

        assert_eq!(summaries.len(), 2);
        assert!(summaries.iter().any(|summary| summary.id == "p1:session-1"));
        assert!(summaries
            .iter()
            .any(|summary| summary.id == "p1:session-1:subagent:agent-1"));

        let subagent = adapter
            .read_conversation("p1:session-1:subagent:agent-1")
            .unwrap();
        assert_eq!(subagent.source_agent, AgentKind::ZCodeClaude);
        assert_eq!(subagent.project_dir, "D:/VSP");
        assert!(subagent
            .messages
            .iter()
            .any(|message| message.content == "sub answer"));
        assert_eq!(
            subagent.messages[0]
                .metadata
                .get("zcode_subagent_id")
                .and_then(|value| value.as_str()),
            Some("agent-1")
        );
    }

    #[test]
    fn zcode_claude_includes_sidechain_events_inside_subagent_files() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let subagent_path = root
            .join("claude")
            .join("p1")
            .join("projects")
            .join("D--vlog")
            .join("session-1")
            .join("subagents")
            .join("agent-1.jsonl");
        write_jsonl(
            &subagent_path,
            &[
                r#"{"type":"user","isSidechain":true,"uuid":"11111111-1111-1111-1111-111111111111","timestamp":"2026-05-01T00:00:00Z","cwd":"D:\\vlog","message":{"role":"user","content":"Look at the D:\\vlog directory and understand the existing audio processing scripts."}}"#,
                r#"{"type":"assistant","isSidechain":true,"uuid":"22222222-2222-2222-2222-222222222222","timestamp":"2026-05-01T00:00:01Z","cwd":"D:\\vlog","message":{"role":"assistant","id":"msg_1","content":[{"type":"text","text":"I will inspect the scripts now."},{"type":"tool_use","id":"call_1","name":"Bash","input":{"command":"ls -la /d/vlog"}}]}}"#,
                r#"{"type":"user","isSidechain":true,"uuid":"33333333-3333-3333-3333-333333333333","timestamp":"2026-05-01T00:00:02Z","cwd":"D:\\vlog","message":{"role":"user","content":[{"tool_use_id":"call_1","type":"tool_result","content":"process_audio.py\nvoice_clone.py"}]}}"#,
            ],
        );

        let adapter = ZCodeClaudeAdapter::with_root_dir(root.to_path_buf());
        let summaries = adapter.list_conversations().unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].message_count, 2);
        assert_eq!(
            summaries[0].summary.as_deref(),
            Some("Look at the D:\\vlog directory and understand the existing audio processing scripts.")
        );

        let conversation = adapter
            .read_conversation("p1:session-1:subagent:agent-1")
            .unwrap();
        assert_eq!(conversation.messages.len(), 2);
        assert_eq!(conversation.messages[0].role, Role::User);
        assert!(conversation.messages[0]
            .content
            .starts_with("Look at the D:\\vlog directory"));
        assert_eq!(conversation.messages[1].tool_calls.len(), 1);
        assert_eq!(
            conversation.messages[1].tool_calls[0].output.as_deref(),
            Some("process_audio.py\nvoice_clone.py")
        );
    }

    #[test]
    fn zcode_claude_summary_uses_first_meaningful_content_not_command_caveats() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let session_path = root
            .join("claude")
            .join("p1")
            .join("projects")
            .join("D--VSP")
            .join("session-1.jsonl");
        write_jsonl(
            &session_path,
            &[
                r#"{"type":"user","timestamp":"2026-05-01T00:00:00Z","cwd":"D:\\VSP","message":{"role":"user","content":"<local-command-caveat>Cached command output from a previous shell invocation.</local-command-caveat>"}}"#,
                r#"{"type":"user","timestamp":"2026-05-01T00:00:01Z","cwd":"D:\\VSP","message":{"role":"user","content":"Review the ChatMem migration flow and summarize the next fix."}}"#,
            ],
        );

        let adapter = ZCodeClaudeAdapter::with_root_dir(root.to_path_buf());
        let summaries = adapter.list_conversations().unwrap();

        assert_eq!(
            summaries[0].summary.as_deref(),
            Some("Review the ChatMem migration flow and summarize the next fix.")
        );
    }

    #[test]
    fn zcode_claude_conversation_filters_local_command_noise() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let session_path = root
            .join("claude")
            .join("p1")
            .join("projects")
            .join("D--VSP")
            .join("session-1.jsonl");
        write_jsonl(
            &session_path,
            &[
                r#"{"type":"user","timestamp":"2026-05-01T00:00:00Z","cwd":"D:\\VSP","message":{"role":"user","content":"<local-command-caveat>Caveat: generated while running local commands.</local-command-caveat>"}}"#,
                r#"{"type":"user","timestamp":"2026-05-01T00:00:01Z","cwd":"D:\\VSP","message":{"role":"user","content":"<command-name>/model</command-name>\n<command-message>model</command-message>\n<command-args>mimo-v2.5-pro</command-args>"}}"#,
                r#"{"type":"user","timestamp":"2026-05-01T00:00:02Z","cwd":"D:\\VSP","message":{"role":"user","content":"<local-command-stdout>Set model to mimo-v2.5-pro</local-command-stdout>"}}"#,
                r#"{"type":"assistant","timestamp":"2026-05-01T00:00:03Z","cwd":"D:\\VSP","message":{"role":"assistant","content":[{"type":"text","text":"No response requested."}]}}"#,
                r#"{"type":"user","timestamp":"2026-05-01T00:00:04Z","cwd":"D:\\VSP","message":{"role":"user","content":"Please inspect my real ChatMem conversation."}}"#,
                r#"{"type":"assistant","timestamp":"2026-05-01T00:00:05Z","cwd":"D:\\VSP","message":{"role":"assistant","content":[{"type":"text","text":"I found the real request."}]}}"#,
            ],
        );

        let adapter = ZCodeClaudeAdapter::with_root_dir(root.to_path_buf());
        let summaries = adapter.list_conversations().unwrap();
        assert_eq!(
            summaries[0].summary.as_deref(),
            Some("Please inspect my real ChatMem conversation.")
        );
        assert_eq!(summaries[0].message_count, 2);

        let conversation = adapter.read_conversation("p1:session-1").unwrap();
        assert_eq!(conversation.messages.len(), 2);
        assert_eq!(
            conversation.messages[0].content,
            "Please inspect my real ChatMem conversation."
        );
        assert_eq!(
            conversation.messages[1].content,
            "I found the real request."
        );
    }

    #[test]
    fn zcode_top_level_adapter_lists_cli_prefixed_conversations() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_jsonl(
            &root
                .join("claude")
                .join("p1")
                .join("projects")
                .join("D--VSP")
                .join("session-1.jsonl"),
            &[
                r#"{"type":"user","timestamp":"2026-05-01T00:00:00Z","cwd":"D:\\VSP","message":{"role":"user","content":"Fix the ZCode source hierarchy."}}"#,
            ],
        );

        let adapter = ZCodeAdapter::with_root_dir(root.to_path_buf());
        let summaries = adapter.list_conversations().unwrap();

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, "claude:p1:session-1");
        assert_eq!(summaries[0].source_agent, AgentKind::ZCode);

        let conversation = adapter.read_conversation("claude:p1:session-1").unwrap();
        assert_eq!(conversation.id, "claude:p1:session-1");
        assert_eq!(conversation.source_agent, AgentKind::ZCodeClaude);
        assert_eq!(
            conversation.messages[0].content,
            "Fix the ZCode source hierarchy."
        );
    }

    #[test]
    fn zcode_top_level_adapter_hides_subagents_and_command_only_sessions() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        write_jsonl(
            &root
                .join("claude")
                .join("p1")
                .join("projects")
                .join("D--VSP")
                .join("real-session.jsonl"),
            &[
                r#"{"type":"user","timestamp":"2026-05-01T00:00:00Z","cwd":"D:\\VSP","message":{"role":"user","content":"This is the real human request."}}"#,
            ],
        );
        write_jsonl(
            &root
                .join("claude")
                .join("p1")
                .join("projects")
                .join("D--VSP")
                .join("real-session")
                .join("subagents")
                .join("agent-1.jsonl"),
            &[
                r#"{"type":"user","isSidechain":true,"timestamp":"2026-05-01T00:00:01Z","cwd":"D:\\VSP","message":{"role":"user","content":"Research implementation details for the parent agent."}}"#,
            ],
        );
        write_jsonl(
            &root
                .join("claude")
                .join("p1")
                .join("projects")
                .join("D--VSP")
                .join("command-only.jsonl"),
            &[
                r#"{"type":"user","timestamp":"2026-05-01T00:00:02Z","cwd":"D:\\VSP","message":{"role":"user","content":"<local-command-caveat>Caveat: generated while running local commands.</local-command-caveat>"}}"#,
                r#"{"type":"user","timestamp":"2026-05-01T00:00:03Z","cwd":"D:\\VSP","message":{"role":"user","content":"<command-name>/model</command-name>\n<command-message>model</command-message>\n<command-args>mimo-v2.5-pro</command-args>"}}"#,
                r#"{"type":"assistant","timestamp":"2026-05-01T00:00:04Z","cwd":"D:\\VSP","message":{"role":"assistant","content":[{"type":"text","text":"No response requested."}]}}"#,
            ],
        );

        let adapter = ZCodeAdapter::with_root_dir(root.to_path_buf());
        let summaries = adapter.list_conversations().unwrap();

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, "claude:p1:real-session");
        assert_eq!(
            summaries[0].summary.as_deref(),
            Some("This is the real human request.")
        );
    }

    #[test]
    fn zcode_top_level_adapter_reads_task_session_json_with_chinese_messages() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let task_path = root
            .join("sessions")
            .join("p1")
            .join("task-1.json");
        fs::create_dir_all(task_path.parent().unwrap()).unwrap();
        let task_json = json!({
            "meta": {
                "taskId": "task-1",
                "acpSessionId": "session-1",
                "title": "\u{4f60}\u{597d}",
                "workspacePath": r"D:\VSP",
                "createdAt": 1778859901472_i64,
                "updatedAt": 1778859920861_i64,
                "provider": "claude",
                "model": "mimo-v2.5-pro",
            },
            "messages": [
                {
                    "role": "user",
                    "content": "\u{4f60}\u{597d}",
                    "timestamp": 1778859901472_i64,
                    "turnIndex": 0,
                },
                {
                    "role": "assistant",
                    "content": "\u{4f60}\u{597d}\u{ff0c}\u{6211}\u{53ef}\u{4ee5}\u{5e2e}\u{4f60}\u{7ee7}\u{7eed}\u{3002}",
                    "timestamp": 1778859920861_i64,
                    "tools": [
                        {
                            "title": r"Read D:\VSP\README.md",
                            "kind": "read",
                            "status": "completed",
                            "input": {"path": r"D:\VSP\README.md"},
                            "output": "README",
                        },
                        {
                            "title": r"List D:\VSP",
                            "kind": "execute",
                            "status": "completed",
                            "input": {"command": "dir"},
                            "output": "chatmem",
                        },
                    ],
                    "turnIndex": 0,
                },
            ],
            "fileChanges": [],
        });
        fs::write(&task_path, serde_json::to_vec(&task_json).unwrap()).unwrap();
        /*
        fs::write(
            &task_path,
            r#"{
              "meta": {
                "taskId": "task-1",
                "acpSessionId": "session-1",
                "title": "你好",
                "workspacePath": "D:\\VSP",
                "createdAt": 1778859901472,
                "updatedAt": 1778859920861,
                "provider": "claude",
                "model": "mimo-v2.5-pro"
              },
              "messages": [
                {
                  "role": "user",
                  "content": "你好",
                  "timestamp": 1778859901472,
                  "turnIndex": 0
                },
                {
                  "role": "assistant",
                  "content": "你好，我可以帮你继续。",
                  "timestamp": 1778859920861,
                  "tools": [
                    {
                      "title": "Read D:\\VSP\\README.md",
                      "kind": "read",
                      "status": "completed",
                      "input": {"path": "D:\\VSP\\README.md"},
                      "output": "README"
                    },
                    {
                      "title": "List D:\\VSP",
                      "kind": "execute",
                      "status": "completed",
                      "input": {"command": "dir"},
                      "output": "chatmem"
                    }
                  ],
                  "turnIndex": 0
                }
              ],
              "fileChanges": []
            }"#,
        )
        .unwrap();
        */

        let adapter = ZCodeAdapter::with_root_dir(root.to_path_buf());
        let summaries = adapter.list_conversations().unwrap();

        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].id, "claude:task:p1:task-1");
        assert_eq!(summaries[0].source_agent, AgentKind::ZCode);
        assert_eq!(summaries[0].summary.as_deref(), Some("\u{4f60}\u{597d}"));
        /*
        assert_eq!(summaries[0].summary.as_deref(), Some("你好"));
        */
        assert_eq!(summaries[0].project_dir, r"D:\VSP");
        assert_eq!(summaries[0].message_count, 2);
        assert_eq!(summaries[0].file_count, 0);

        let conversation = adapter.read_conversation("claude:task:p1:task-1").unwrap();
        assert_eq!(conversation.id, "claude:task:p1:task-1");
        assert_eq!(conversation.source_agent, AgentKind::ZCode);
        assert_eq!(conversation.messages.len(), 2);
        assert_eq!(conversation.messages[0].content, "\u{4f60}\u{597d}");
        assert_eq!(
            conversation.messages[1].content,
            "\u{4f60}\u{597d}\u{ff0c}\u{6211}\u{53ef}\u{4ee5}\u{5e2e}\u{4f60}\u{7ee7}\u{7eed}\u{3002}"
        );
        /*
        assert_eq!(conversation.messages[0].content, "你好");
        assert_eq!(conversation.messages[1].content, "你好，我可以帮你继续。");
        */
        assert_eq!(conversation.messages[1].tool_calls.len(), 2);
        assert_eq!(
            conversation.messages[1].tool_calls[0].output.as_deref(),
            Some("README")
        );
    }

    #[test]
    fn zcode_codex_wraps_profile_ids_and_preserves_project_dir() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path();
        let profile_dir = root.join("codex").join("p1");
        fs::create_dir_all(profile_dir.join("sessions")).unwrap();
        let rollout_path = profile_dir.join("sessions").join("rollout.jsonl");
        write_jsonl(
            &rollout_path,
            &[
                r#"{"timestamp":"2026-05-02T00:00:00Z","type":"event_msg","payload":{"type":"user_message","message":"hello zcode codex"}}"#,
                r#"{"timestamp":"2026-05-02T00:00:01Z","type":"event_msg","payload":{"type":"agent_message","message":"hi"}}"#,
            ],
        );

        let conn = Connection::open(profile_dir.join("state_5.sqlite")).unwrap();
        conn.execute_batch(
            "CREATE TABLE threads (
                id TEXT PRIMARY KEY,
                rollout_path TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                source TEXT NOT NULL,
                model_provider TEXT NOT NULL,
                cwd TEXT NOT NULL,
                title TEXT NOT NULL,
                sandbox_policy TEXT NOT NULL,
                approval_mode TEXT NOT NULL,
                tokens_used INTEGER NOT NULL DEFAULT 0,
                has_user_event INTEGER NOT NULL DEFAULT 0,
                archived INTEGER NOT NULL DEFAULT 0,
                archived_at INTEGER,
                git_sha TEXT,
                git_branch TEXT,
                git_origin_url TEXT,
                cli_version TEXT NOT NULL DEFAULT '',
                first_user_message TEXT NOT NULL DEFAULT ''
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO threads (
                id, rollout_path, created_at, updated_at, source, model_provider, cwd, title,
                sandbox_policy, approval_mode, tokens_used, has_user_event, archived,
                cli_version, first_user_message
            ) VALUES (?1, ?2, 1770000000, 1770000001, 'vscode', 'openai', ?3, 'ZCode title',
                '{}', 'never', 0, 1, 0, 'test', 'hello zcode codex')",
            params![
                "thread-1",
                rollout_path.to_string_lossy().to_string(),
                r"\\?\D:\VSP"
            ],
        )
        .unwrap();

        let adapter = ZCodeCodexAdapter::with_root_dir(root.to_path_buf());
        let summaries = adapter.list_conversations().unwrap();
        assert_eq!(summaries[0].id, "p1:thread-1");
        assert_eq!(summaries[0].source_agent, AgentKind::ZCodeCodex);

        let conversation = adapter.read_conversation("p1:thread-1").unwrap();
        assert_eq!(conversation.project_dir, r"D:\VSP");
        assert_eq!(conversation.source_agent, AgentKind::ZCodeCodex);
        assert!(conversation
            .messages
            .iter()
            .any(|message| message.content == "hello zcode codex"));
    }

    #[test]
    fn zcode_config_only_adapters_are_available_but_empty() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("gemini").join("p1")).unwrap();
        fs::create_dir_all(tmp.path().join("opencode").join("p1")).unwrap();

        let gemini = ZCodeGeminiAdapter::with_root_dir(tmp.path().to_path_buf());
        let opencode = ZCodeOpenCodeAdapter::with_root_dir(tmp.path().to_path_buf());

        assert!(gemini.is_available());
        assert!(opencode.is_available());
        assert!(gemini.list_conversations().unwrap().is_empty());
        assert!(opencode.list_conversations().unwrap().is_empty());
        assert!(gemini.write_conversation(&empty_conversation()).is_err());
        assert!(opencode.write_conversation(&empty_conversation()).is_err());
    }

    fn empty_conversation() -> Conversation {
        let now = Utc::now();
        Conversation {
            id: "empty".to_string(),
            source_agent: AgentKind::Claude,
            project_dir: "D:/VSP".to_string(),
            created_at: now,
            updated_at: now,
            summary: None,
            messages: Vec::new(),
            file_changes: Vec::new(),
        }
    }
}
