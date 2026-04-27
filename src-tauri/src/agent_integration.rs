use std::{
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use toml_edit::{value, Array, DocumentMut, Item, Table, Value as TomlValue};

const CHATMEM_SKILL: &str = include_str!("../../skills/chatmem/SKILL.md");
const CHATMEM_OPENAI_AGENT: &str = include_str!("../../skills/chatmem/agents/openai.yaml");
const MANAGED_BLOCK_START: &str = "<!-- CHATMEM-INTEGRATION:START -->";
const MANAGED_BLOCK_END: &str = "<!-- CHATMEM-INTEGRATION:END -->";

#[derive(Debug, Clone)]
struct IntegrationPaths {
    home_dir: PathBuf,
    executable_path: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IntegrationAgent {
    Claude,
    Codex,
    Gemini,
    OpenCode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentIntegrationStatus {
    agent: String,
    label: String,
    config_path: String,
    instructions_path: String,
    mcp_installed: bool,
    instructions_installed: bool,
    config_exists: bool,
    status: String,
    status_label: String,
    command_preview: String,
    details: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentIntegrationOperationResult {
    agent: String,
    label: String,
    changed: bool,
    message: String,
    backup_paths: Vec<String>,
    status: AgentIntegrationStatus,
}

impl IntegrationAgent {
    fn all() -> [Self; 4] {
        [Self::Claude, Self::Codex, Self::Gemini, Self::OpenCode]
    }

    fn from_key(key: &str) -> Option<Self> {
        match key {
            "claude" => Some(Self::Claude),
            "codex" => Some(Self::Codex),
            "gemini" => Some(Self::Gemini),
            "opencode" => Some(Self::OpenCode),
            _ => None,
        }
    }

    fn key(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
            Self::Gemini => "gemini",
            Self::OpenCode => "opencode",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Claude => "Claude",
            Self::Codex => "Codex",
            Self::Gemini => "Gemini",
            Self::OpenCode => "OpenCode",
        }
    }

    fn config_path(self, paths: &IntegrationPaths) -> PathBuf {
        match self {
            Self::Claude => paths.home_dir.join(".claude.json"),
            Self::Codex => paths.home_dir.join(".codex").join("config.toml"),
            Self::Gemini => paths.home_dir.join(".gemini").join("settings.json"),
            Self::OpenCode => paths
                .home_dir
                .join(".config")
                .join("opencode")
                .join("opencode.json"),
        }
    }

    fn instructions_path(self, paths: &IntegrationPaths) -> PathBuf {
        match self {
            Self::Claude => paths
                .home_dir
                .join(".claude")
                .join("skills")
                .join("chatmem")
                .join("SKILL.md"),
            Self::Codex => paths
                .home_dir
                .join(".agents")
                .join("skills")
                .join("chatmem")
                .join("SKILL.md"),
            Self::Gemini => paths.home_dir.join(".gemini").join("GEMINI.md"),
            Self::OpenCode => paths
                .home_dir
                .join(".config")
                .join("opencode")
                .join("skills")
                .join("chatmem")
                .join("SKILL.md"),
        }
    }
}

fn default_paths() -> Result<IntegrationPaths, String> {
    let home_dir =
        dirs::home_dir().ok_or_else(|| "Cannot resolve user home directory".to_string())?;
    let executable_path = std::env::current_exe()
        .map_err(|error| format!("Cannot resolve ChatMem executable: {error}"))?;

    Ok(IntegrationPaths {
        home_dir,
        executable_path,
    })
}

fn path_to_string(path: &Path) -> String {
    path.to_string_lossy().to_string()
}

fn command_preview(paths: &IntegrationPaths) -> String {
    format!("\"{}\" --mcp", path_to_string(&paths.executable_path))
}

fn opencode_rules_path(paths: &IntegrationPaths) -> PathBuf {
    paths
        .home_dir
        .join(".config")
        .join("opencode")
        .join("AGENTS.md")
}

fn claude_rules_path(paths: &IntegrationPaths) -> PathBuf {
    paths.home_dir.join(".claude").join("CLAUDE.md")
}

fn codex_rules_path(paths: &IntegrationPaths) -> PathBuf {
    paths.home_dir.join(".codex").join("AGENTS.md")
}

fn codex_legacy_skill_path(paths: &IntegrationPaths) -> PathBuf {
    paths
        .home_dir
        .join(".codex")
        .join("skills")
        .join("chatmem")
        .join("SKILL.md")
}

fn backup_path(path: &Path) -> PathBuf {
    let timestamp = Utc::now().format("%Y%m%d-%H%M%S");
    let file_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .unwrap_or_else(|| "config".to_string());
    path.with_file_name(format!("{file_name}.bak-{timestamp}"))
}

fn ensure_parent(path: &Path) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Cannot create {}: {error}", parent.display()))?;
    }
    Ok(())
}

fn write_text_if_changed(path: &Path, content: &str) -> Result<Option<PathBuf>, String> {
    if path.exists() {
        let existing = fs::read_to_string(path)
            .map_err(|error| format!("Cannot read {}: {error}", path.display()))?;
        if existing == content {
            return Ok(None);
        }

        let backup = backup_path(path);
        fs::copy(path, &backup).map_err(|error| {
            format!(
                "Cannot back up {} to {}: {error}",
                path.display(),
                backup.display()
            )
        })?;
        ensure_parent(path)?;
        fs::write(path, content)
            .map_err(|error| format!("Cannot write {}: {error}", path.display()))?;
        return Ok(Some(backup));
    }

    ensure_parent(path)?;
    fs::write(path, content)
        .map_err(|error| format!("Cannot write {}: {error}", path.display()))?;
    Ok(None)
}

fn remove_json_comments(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    let mut in_string = false;
    let mut escaped = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    while let Some(ch) = chars.next() {
        if in_line_comment {
            if ch == '\n' {
                in_line_comment = false;
                output.push(ch);
            }
            continue;
        }

        if in_block_comment {
            if ch == '*' && chars.peek() == Some(&'/') {
                let _ = chars.next();
                in_block_comment = false;
            }
            continue;
        }

        if in_string {
            output.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }

        if ch == '"' {
            in_string = true;
            output.push(ch);
            continue;
        }

        if ch == '/' && chars.peek() == Some(&'/') {
            let _ = chars.next();
            in_line_comment = true;
            continue;
        }

        if ch == '/' && chars.peek() == Some(&'*') {
            let _ = chars.next();
            in_block_comment = true;
            continue;
        }

        output.push(ch);
    }

    output
}

fn read_json_object(path: &Path) -> Result<Value, String> {
    if !path.exists() {
        return Ok(Value::Object(Map::new()));
    }

    let raw = fs::read_to_string(path)
        .map_err(|error| format!("Cannot read {}: {error}", path.display()))?;
    let raw = raw.trim_start_matches('\u{feff}');
    let stripped = remove_json_comments(raw);
    serde_json::from_str::<Value>(&stripped)
        .map_err(|error| format!("Cannot parse {}: {error}", path.display()))
}

fn ensure_json_object(value: &mut Value) -> &mut Map<String, Value> {
    if !value.is_object() {
        *value = Value::Object(Map::new());
    }
    value
        .as_object_mut()
        .expect("value was just made an object")
}

fn ensure_child_object<'a>(
    object: &'a mut Map<String, Value>,
    key: &str,
) -> &'a mut Map<String, Value> {
    let value = object
        .entry(key.to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    ensure_json_object(value)
}

fn write_json_value(path: &Path, value: &Value) -> Result<Option<PathBuf>, String> {
    let content = serde_json::to_string_pretty(value)
        .map_err(|error| format!("Cannot serialize {}: {error}", path.display()))?;
    write_text_if_changed(path, &(content + "\n"))
}

fn chatmem_stdio_json(paths: &IntegrationPaths) -> Value {
    json!({
        "command": path_to_string(&paths.executable_path),
        "args": ["--mcp"],
        "env": {}
    })
}

fn chatmem_gemini_json(paths: &IntegrationPaths) -> Value {
    json!({
        "command": path_to_string(&paths.executable_path),
        "args": ["--mcp"],
        "timeout": 30000,
        "trust": true
    })
}

fn chatmem_opencode_json(paths: &IntegrationPaths) -> Value {
    json!({
        "type": "local",
        "command": [path_to_string(&paths.executable_path), "--mcp"],
        "enabled": true,
        "timeout": 30000
    })
}

fn json_has_server(value: &Value, parent_key: &str) -> bool {
    value
        .get(parent_key)
        .and_then(Value::as_object)
        .and_then(|servers| servers.get("chatmem"))
        .is_some()
}

fn install_json_server(
    path: &Path,
    parent_key: &str,
    server_value: Value,
) -> Result<Option<PathBuf>, String> {
    let mut value = read_json_object(path)?;
    let root = ensure_json_object(&mut value);
    let servers = ensure_child_object(root, parent_key);
    servers.insert("chatmem".to_string(), server_value);

    if parent_key == "mcp" {
        root.entry("$schema".to_string())
            .or_insert_with(|| json!("https://opencode.ai/config.json"));
    }

    write_json_value(path, &value)
}

fn install_opencode_config(
    path: &Path,
    paths: &IntegrationPaths,
) -> Result<Option<PathBuf>, String> {
    let mut value = read_json_object(path)?;
    let root = ensure_json_object(&mut value);

    root.entry("$schema".to_string())
        .or_insert_with(|| json!("https://opencode.ai/config.json"));

    {
        let servers = ensure_child_object(root, "mcp");
        servers.insert("chatmem".to_string(), chatmem_opencode_json(paths));
    }

    {
        let tools = ensure_child_object(root, "tools");
        tools.insert("chatmem_*".to_string(), json!(true));
    }

    {
        let permission = ensure_child_object(root, "permission");
        let skill = ensure_child_object(permission, "skill");
        skill.insert("chatmem".to_string(), json!("allow"));
    }

    write_json_value(path, &value)
}

fn uninstall_json_server(path: &Path, parent_key: &str) -> Result<Option<PathBuf>, String> {
    if !path.exists() {
        return Ok(None);
    }

    let mut value = read_json_object(path)?;
    let Some(root) = value.as_object_mut() else {
        return Ok(None);
    };
    let Some(servers) = root.get_mut(parent_key).and_then(Value::as_object_mut) else {
        return Ok(None);
    };

    if servers.remove("chatmem").is_none() {
        return Ok(None);
    }

    write_json_value(path, &value)
}

fn uninstall_opencode_config(path: &Path) -> Result<Option<PathBuf>, String> {
    if !path.exists() {
        return Ok(None);
    }

    let mut value = read_json_object(path)?;
    let Some(root) = value.as_object_mut() else {
        return Ok(None);
    };

    let mut changed = false;
    if let Some(servers) = root.get_mut("mcp").and_then(Value::as_object_mut) {
        changed |= servers.remove("chatmem").is_some();
    }
    if let Some(tools) = root.get_mut("tools").and_then(Value::as_object_mut) {
        changed |= tools.remove("chatmem_*").is_some();
    }
    if let Some(skill) = root
        .get_mut("permission")
        .and_then(Value::as_object_mut)
        .and_then(|permission| permission.get_mut("skill"))
        .and_then(Value::as_object_mut)
    {
        changed |= skill.remove("chatmem").is_some();
    }

    if !changed {
        return Ok(None);
    }

    write_json_value(path, &value)
}

fn read_codex_config(path: &Path) -> Result<DocumentMut, String> {
    if !path.exists() {
        return Ok(DocumentMut::new());
    }

    let raw = fs::read_to_string(path)
        .map_err(|error| format!("Cannot read {}: {error}", path.display()))?;
    raw.parse::<DocumentMut>()
        .map_err(|error| format!("Cannot parse {}: {error}", path.display()))
}

fn codex_has_chatmem(path: &Path) -> bool {
    read_codex_config(path)
        .ok()
        .and_then(|doc| {
            let servers = doc.get("mcp_servers")?;
            if servers
                .as_table()
                .and_then(|table| table.get("chatmem"))
                .is_some()
            {
                return Some(());
            }
            servers
                .as_inline_table()
                .and_then(|table| table.get("chatmem"))
                .map(|_| ())
        })
        .is_some()
}

fn install_codex_config(path: &Path, paths: &IntegrationPaths) -> Result<Option<PathBuf>, String> {
    let mut doc = read_codex_config(path)?;
    let mut args = Array::new();
    args.push("--mcp");

    let root = doc.as_table_mut();
    let servers_item = root
        .entry("mcp_servers")
        .or_insert_with(|| Item::Table(Table::new()));
    if !servers_item.is_table() {
        let mut converted = Table::new();
        if let Some(inline) = servers_item.as_inline_table() {
            for (key, value) in inline.iter() {
                converted.insert(key, Item::Value(value.clone()));
            }
        }
        *servers_item = Item::Table(converted);
    }

    let servers = servers_item
        .as_table_mut()
        .ok_or_else(|| "Cannot create Codex mcp_servers table".to_string())?;
    let mut chatmem = Table::new();
    chatmem["command"] = value(path_to_string(&paths.executable_path));
    chatmem["args"] = Item::Value(TomlValue::Array(args));
    chatmem["startup_timeout_sec"] = value(20);
    chatmem["tool_timeout_sec"] = value(120);
    chatmem["enabled"] = value(true);
    servers.insert("chatmem", Item::Table(chatmem));

    write_text_if_changed(path, &doc.to_string())
}

fn uninstall_codex_config(path: &Path) -> Result<Option<PathBuf>, String> {
    if !path.exists() {
        return Ok(None);
    }

    let mut doc = read_codex_config(path)?;
    let Some(servers) = doc
        .get_mut("mcp_servers")
        .and_then(|item| item.as_table_mut())
    else {
        return Ok(None);
    };

    if servers.remove("chatmem").is_none() {
        return Ok(None);
    }

    write_text_if_changed(path, &doc.to_string())
}

fn managed_instruction_block() -> String {
    format!(
        "{MANAGED_BLOCK_START}\n## ChatMem\nUse ChatMem before answering repository recall, continuation, migration, handoff, or memory questions. If the `chatmem` skill is available, load it first; then call the `chatmem` MCP tools. If the user has not explicitly asked for recall, ask once whether to load a compact project recollection first. Prefer `get_project_context` with `limit=3` for startup, recall, and continuation; use `search_repo_history` as a targeted second step with `limit<=3`. When history hits appear, name the source agent/conversation, say they are indexed local-history evidence rather than approved startup rules, and ask whether to call `read_history_conversation` before expanding. Do not ask the user to redescribe the topic while plausible history hits exist. Use `import_all_local_history` after a fresh install or suspicious recall miss. 中文用户问“记得吗、之前聊过、回忆、继续、迁移、交接、项目历史、本地历史、启动规则、记忆”时，先查 ChatMem，再用中文回答。\n{MANAGED_BLOCK_END}\n"
    )
}

fn upsert_managed_block(existing: &str, block: &str) -> String {
    let Some(start) = existing.find(MANAGED_BLOCK_START) else {
        let trimmed = existing.trim_end();
        if trimmed.is_empty() {
            return block.to_string();
        }
        return format!("{trimmed}\n\n{block}");
    };
    let Some(relative_end) = existing[start..].find(MANAGED_BLOCK_END) else {
        let trimmed = existing.trim_end();
        return format!("{trimmed}\n\n{block}");
    };
    let end = start + relative_end + MANAGED_BLOCK_END.len();
    let mut updated = String::new();
    updated.push_str(existing[..start].trim_end());
    if !updated.is_empty() {
        updated.push_str("\n\n");
    }
    updated.push_str(block.trim_end());
    updated.push_str(existing[end..].trim_start_matches(['\r', '\n']));
    if !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated
}

fn remove_managed_block(existing: &str) -> String {
    let Some(start) = existing.find(MANAGED_BLOCK_START) else {
        return existing.to_string();
    };
    let Some(relative_end) = existing[start..].find(MANAGED_BLOCK_END) else {
        return existing.to_string();
    };
    let end = start + relative_end + MANAGED_BLOCK_END.len();
    let mut updated = String::new();
    updated.push_str(existing[..start].trim_end());
    let tail = existing[end..].trim_start_matches(['\r', '\n']);
    if !updated.is_empty() && !tail.is_empty() {
        updated.push_str("\n\n");
    }
    updated.push_str(tail);
    if !updated.is_empty() && !updated.ends_with('\n') {
        updated.push('\n');
    }
    updated
}

fn install_skill_tree(
    agent: IntegrationAgent,
    paths: &IntegrationPaths,
) -> Result<Vec<PathBuf>, String> {
    let skill_path = agent.instructions_path(paths);
    let mut backups = Vec::new();

    if let Some(backup) = write_text_if_changed(&skill_path, CHATMEM_SKILL)? {
        backups.push(backup);
    }

    if agent == IntegrationAgent::Codex {
        let legacy_skill_path = codex_legacy_skill_path(paths);
        if let Some(backup) = write_text_if_changed(&legacy_skill_path, CHATMEM_SKILL)? {
            backups.push(backup);
        }

        let agent_yaml_path = skill_path
            .parent()
            .ok_or_else(|| "Invalid Codex skill path".to_string())?
            .join("agents")
            .join("openai.yaml");
        if let Some(backup) = write_text_if_changed(&agent_yaml_path, CHATMEM_OPENAI_AGENT)? {
            backups.push(backup);
        }

        let legacy_agent_yaml_path = legacy_skill_path
            .parent()
            .ok_or_else(|| "Invalid legacy Codex skill path".to_string())?
            .join("agents")
            .join("openai.yaml");
        if let Some(backup) = write_text_if_changed(&legacy_agent_yaml_path, CHATMEM_OPENAI_AGENT)?
        {
            backups.push(backup);
        }
    }

    Ok(backups)
}

fn install_managed_instructions(path: &Path) -> Result<Vec<PathBuf>, String> {
    let existing = if path.exists() {
        fs::read_to_string(path)
            .map_err(|error| format!("Cannot read {}: {error}", path.display()))?
    } else {
        String::new()
    };
    let updated = upsert_managed_block(&existing, &managed_instruction_block());
    Ok(write_text_if_changed(path, &updated)?.into_iter().collect())
}

fn uninstall_skill_tree(agent: IntegrationAgent, paths: &IntegrationPaths) -> Result<bool, String> {
    let skill_path = agent.instructions_path(paths);
    let mut removed = false;
    let Some(skill_dir) = skill_path.parent() else {
        return Ok(removed);
    };
    if skill_dir.exists() {
        fs::remove_dir_all(skill_dir)
            .map_err(|error| format!("Cannot remove {}: {error}", skill_dir.display()))?;
        removed = true;
    }

    if agent == IntegrationAgent::Codex {
        let legacy_skill_path = codex_legacy_skill_path(paths);
        if let Some(legacy_skill_dir) = legacy_skill_path.parent() {
            if legacy_skill_dir.exists() {
                fs::remove_dir_all(legacy_skill_dir).map_err(|error| {
                    format!("Cannot remove {}: {error}", legacy_skill_dir.display())
                })?;
                removed = true;
            }
        }
    }

    Ok(removed)
}

fn uninstall_managed_instructions(path: &Path) -> Result<Option<PathBuf>, String> {
    if !path.exists() {
        return Ok(None);
    }
    let existing = fs::read_to_string(path)
        .map_err(|error| format!("Cannot read {}: {error}", path.display()))?;
    let updated = remove_managed_block(&existing);
    if updated == existing {
        return Ok(None);
    }
    write_text_if_changed(path, &updated)
}

fn instructions_installed(agent: IntegrationAgent, paths: &IntegrationPaths) -> bool {
    let path = agent.instructions_path(paths);
    match agent {
        IntegrationAgent::Claude => {
            path.exists()
                && fs::read_to_string(claude_rules_path(paths))
                    .map(|content| content.contains(MANAGED_BLOCK_START))
                    .unwrap_or(false)
        }
        IntegrationAgent::Codex => {
            path.exists()
                && fs::read_to_string(codex_rules_path(paths))
                    .map(|content| content.contains(MANAGED_BLOCK_START))
                    .unwrap_or(false)
        }
        IntegrationAgent::Gemini => fs::read_to_string(path)
            .map(|content| content.contains(MANAGED_BLOCK_START))
            .unwrap_or(false),
        IntegrationAgent::OpenCode => {
            path.exists()
                && fs::read_to_string(opencode_rules_path(paths))
                    .map(|content| content.contains(MANAGED_BLOCK_START))
                    .unwrap_or(false)
        }
    }
}

fn mcp_installed(agent: IntegrationAgent, paths: &IntegrationPaths) -> bool {
    let path = agent.config_path(paths);
    match agent {
        IntegrationAgent::Codex => codex_has_chatmem(&path),
        IntegrationAgent::Claude | IntegrationAgent::Gemini => read_json_object(&path)
            .map(|value| json_has_server(&value, "mcpServers"))
            .unwrap_or(false),
        IntegrationAgent::OpenCode => read_json_object(&path)
            .map(|value| json_has_server(&value, "mcp"))
            .unwrap_or(false),
    }
}

fn status_for_agent(agent: IntegrationAgent, paths: &IntegrationPaths) -> AgentIntegrationStatus {
    let config_path = agent.config_path(paths);
    let instructions_path = agent.instructions_path(paths);
    let mcp_installed = mcp_installed(agent, paths);
    let instructions_installed = instructions_installed(agent, paths);
    let status = match (mcp_installed, instructions_installed) {
        (true, true) => "ready",
        (true, false) | (false, true) => "partial",
        (false, false) => "not_installed",
    };
    let status_label = match status {
        "ready" => "已就绪",
        "partial" => "需修复",
        _ => "未安装",
    };
    let mut details = Vec::new();
    if !config_path.exists() {
        details.push("未找到配置文件，安装时会自动创建。".to_string());
    }
    if mcp_installed {
        details.push("MCP 已写入 agent 配置。".to_string());
    }
    if instructions_installed {
        match agent {
            IntegrationAgent::Claude => {
                details.push("ChatMem skill 和 Claude 全局 CLAUDE.md 规则已安装。".to_string());
            }
            IntegrationAgent::Codex => {
                details.push(
                    "ChatMem skill 和 Codex 全局 AGENTS.md 规则已安装；同时保留旧版 Codex skill 路径兼容。"
                        .to_string(),
                );
            }
            IntegrationAgent::Gemini => {
                details.push("ChatMem GEMINI.md 规则已安装。".to_string());
            }
            IntegrationAgent::OpenCode => {
                details.push("ChatMem skill 和 OpenCode 全局 AGENTS.md 规则已安装。".to_string());
            }
        }
    } else {
        match agent {
            IntegrationAgent::Claude => details.push(
                "Claude 需要同时安装 ChatMem skill 和全局 CLAUDE.md 规则；缺任一项都可能不会自动触发。"
                    .to_string(),
            ),
            IntegrationAgent::Codex => details.push(
                "Codex 需要同时安装 ChatMem skill 和全局 AGENTS.md 规则；缺任一项都可能不会自动触发。"
                    .to_string(),
            ),
            IntegrationAgent::Gemini => details.push(
                "Gemini 主要依赖 GEMINI.md 规则引导调用 MCP；未安装时不会主动回忆。".to_string(),
            ),
            IntegrationAgent::OpenCode => details.push(
                "OpenCode 需要同时安装 ChatMem skill 和全局 AGENTS.md 规则；缺任一项都可能不会自动触发。"
                    .to_string(),
            ),
        }
    }
    if !details.iter().any(|item| item.contains("重启")) {
        details.push("安装或修复后，请重启对应 agent。".to_string());
    }

    AgentIntegrationStatus {
        agent: agent.key().to_string(),
        label: agent.label().to_string(),
        config_path: path_to_string(&config_path),
        instructions_path: path_to_string(&instructions_path),
        mcp_installed,
        instructions_installed,
        config_exists: config_path.exists(),
        status: status.to_string(),
        status_label: status_label.to_string(),
        command_preview: command_preview(paths),
        details,
    }
}

fn install_one(
    agent: IntegrationAgent,
    paths: &IntegrationPaths,
) -> Result<AgentIntegrationOperationResult, String> {
    let config_path = agent.config_path(paths);
    let mut backups = Vec::new();

    let config_backup = match agent {
        IntegrationAgent::Claude => {
            install_json_server(&config_path, "mcpServers", chatmem_stdio_json(paths))?
        }
        IntegrationAgent::Codex => install_codex_config(&config_path, paths)?,
        IntegrationAgent::Gemini => {
            install_json_server(&config_path, "mcpServers", chatmem_gemini_json(paths))?
        }
        IntegrationAgent::OpenCode => install_opencode_config(&config_path, paths)?,
    };
    backups.extend(config_backup);

    let instruction_backups = match agent {
        IntegrationAgent::Claude => {
            let mut backups = install_skill_tree(agent, paths)?;
            backups.extend(install_managed_instructions(&claude_rules_path(paths))?);
            backups
        }
        IntegrationAgent::Codex => {
            let mut backups = install_skill_tree(agent, paths)?;
            backups.extend(install_managed_instructions(&codex_rules_path(paths))?);
            backups
        }
        IntegrationAgent::Gemini => install_managed_instructions(&agent.instructions_path(paths))?,
        IntegrationAgent::OpenCode => {
            let mut backups = install_skill_tree(agent, paths)?;
            backups.extend(install_managed_instructions(&opencode_rules_path(paths))?);
            backups
        }
    };
    backups.extend(instruction_backups);

    let status = status_for_agent(agent, paths);
    Ok(AgentIntegrationOperationResult {
        agent: agent.key().to_string(),
        label: agent.label().to_string(),
        changed: true,
        message: format!("{} 集成已安装或修复。", agent.label()),
        backup_paths: backups.iter().map(|path| path_to_string(path)).collect(),
        status,
    })
}

fn uninstall_one(
    agent: IntegrationAgent,
    paths: &IntegrationPaths,
) -> Result<AgentIntegrationOperationResult, String> {
    let config_path = agent.config_path(paths);
    let mut backups = Vec::new();

    let config_backup = match agent {
        IntegrationAgent::Claude | IntegrationAgent::Gemini => {
            uninstall_json_server(&config_path, "mcpServers")?
        }
        IntegrationAgent::Codex => uninstall_codex_config(&config_path)?,
        IntegrationAgent::OpenCode => uninstall_opencode_config(&config_path)?,
    };
    backups.extend(config_backup);

    let removed_instructions = match agent {
        IntegrationAgent::Claude => {
            let mut removed = uninstall_skill_tree(agent, paths)?;
            if let Some(backup) = uninstall_managed_instructions(&claude_rules_path(paths))? {
                backups.push(backup);
                removed = true;
            }
            removed
        }
        IntegrationAgent::Codex => {
            let mut removed = uninstall_skill_tree(agent, paths)?;
            if let Some(backup) = uninstall_managed_instructions(&codex_rules_path(paths))? {
                backups.push(backup);
                removed = true;
            }
            removed
        }
        IntegrationAgent::Gemini => {
            let backup = uninstall_managed_instructions(&agent.instructions_path(paths))?;
            backups.extend(backup);
            backups.last().is_some()
        }
        IntegrationAgent::OpenCode => {
            let mut removed = uninstall_skill_tree(agent, paths)?;
            if let Some(backup) = uninstall_managed_instructions(&opencode_rules_path(paths))? {
                backups.push(backup);
                removed = true;
            }
            removed
        }
    };

    let status = status_for_agent(agent, paths);
    Ok(AgentIntegrationOperationResult {
        agent: agent.key().to_string(),
        label: agent.label().to_string(),
        changed: !backups.is_empty() || removed_instructions,
        message: format!("{} 集成已卸载，历史和记忆数据不会被删除。", agent.label()),
        backup_paths: backups.iter().map(|path| path_to_string(path)).collect(),
        status,
    })
}

fn selected_agents(agent: &str) -> Result<Vec<IntegrationAgent>, String> {
    if agent == "all" {
        return Ok(IntegrationAgent::all().to_vec());
    }
    IntegrationAgent::from_key(agent)
        .map(|agent| vec![agent])
        .ok_or_else(|| format!("Unknown agent integration target: {agent}"))
}

#[tauri::command]
pub async fn detect_agent_integrations() -> Result<Vec<AgentIntegrationStatus>, String> {
    let paths = default_paths()?;
    Ok(IntegrationAgent::all()
        .into_iter()
        .map(|agent| status_for_agent(agent, &paths))
        .collect())
}

#[tauri::command]
pub async fn install_agent_integration(
    agent: String,
) -> Result<Vec<AgentIntegrationOperationResult>, String> {
    let paths = default_paths()?;
    selected_agents(&agent)?
        .into_iter()
        .map(|agent| install_one(agent, &paths))
        .collect()
}

#[tauri::command]
pub async fn uninstall_agent_integration(
    agent: String,
) -> Result<Vec<AgentIntegrationOperationResult>, String> {
    let paths = default_paths()?;
    selected_agents(&agent)?
        .into_iter()
        .map(|agent| uninstall_one(agent, &paths))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_paths(name: &str) -> IntegrationPaths {
        let home_dir = std::env::temp_dir().join(format!(
            "chatmem-agent-integration-{name}-{}",
            uuid::Uuid::new_v4()
        ));
        IntegrationPaths {
            home_dir,
            executable_path: PathBuf::from(r"C:\Program Files\ChatMem\ChatMem.exe"),
        }
    }

    #[test]
    fn installs_codex_mcp_without_erasing_existing_config() {
        let paths = test_paths("codex");
        let config = IntegrationAgent::Codex.config_path(&paths);
        write_text_if_changed(
            &config,
            "model = \"gpt-5.5\"\n\n[projects.'D:\\\\VSP']\ntrust_level = \"trusted\"\n",
        )
        .unwrap();

        install_one(IntegrationAgent::Codex, &paths).unwrap();
        let updated = fs::read_to_string(config).unwrap();

        assert!(updated.contains("model = \"gpt-5.5\""));
        assert!(codex_has_chatmem(
            &IntegrationAgent::Codex.config_path(&paths)
        ));
        assert!(updated.contains(r"C:\Program Files\ChatMem\ChatMem.exe"));
        assert!(updated.contains("\"--mcp\""));
        assert!(IntegrationAgent::Codex.instructions_path(&paths).exists());
        assert!(codex_legacy_skill_path(&paths).exists());
        assert!(fs::read_to_string(codex_rules_path(&paths))
            .unwrap()
            .contains("中文用户问"));
        assert!(instructions_installed(IntegrationAgent::Codex, &paths));
    }

    #[test]
    fn installs_json_based_agents_with_expected_shapes() {
        let paths = test_paths("json-agents");

        install_one(IntegrationAgent::Claude, &paths).unwrap();
        let claude = read_json_object(&IntegrationAgent::Claude.config_path(&paths)).unwrap();
        assert_eq!(claude["mcpServers"]["chatmem"]["args"], json!(["--mcp"]));
        assert!(fs::read_to_string(claude_rules_path(&paths))
            .unwrap()
            .contains("read_history_conversation"));
        assert!(instructions_installed(IntegrationAgent::Claude, &paths));

        install_one(IntegrationAgent::Gemini, &paths).unwrap();
        let gemini = read_json_object(&IntegrationAgent::Gemini.config_path(&paths)).unwrap();
        assert_eq!(gemini["mcpServers"]["chatmem"]["trust"], json!(true));
        assert!(instructions_installed(IntegrationAgent::Gemini, &paths));

        install_one(IntegrationAgent::OpenCode, &paths).unwrap();
        let opencode = read_json_object(&IntegrationAgent::OpenCode.config_path(&paths)).unwrap();
        assert_eq!(opencode["mcp"]["chatmem"]["type"], json!("local"));
        assert_eq!(opencode["mcp"]["chatmem"]["timeout"], json!(30000));
        assert_eq!(opencode["tools"]["chatmem_*"], json!(true));
        assert_eq!(opencode["permission"]["skill"]["chatmem"], json!("allow"));
        assert_eq!(
            opencode["$schema"],
            json!("https://opencode.ai/config.json")
        );
        assert!(fs::read_to_string(opencode_rules_path(&paths))
            .unwrap()
            .contains("中文用户问"));
        assert!(instructions_installed(IntegrationAgent::OpenCode, &paths));
    }

    #[test]
    fn json_configs_with_utf8_bom_are_repaired_on_install() {
        let paths = test_paths("json-bom");
        let config = IntegrationAgent::OpenCode.config_path(&paths);
        ensure_parent(&config).unwrap();
        fs::write(
            &config,
            b"\xEF\xBB\xBF{\n  \"mcp\": {\n    \"other\": {\"type\": \"local\", \"command\": [\"node\", \"server.js\"]}\n  }\n}\n",
        )
        .unwrap();

        install_one(IntegrationAgent::OpenCode, &paths).unwrap();
        let bytes = fs::read(&config).unwrap();
        let updated = read_json_object(&config).unwrap();

        assert!(!bytes.starts_with(b"\xEF\xBB\xBF"));
        assert!(updated["mcp"].get("other").is_some());
        assert_eq!(updated["mcp"]["chatmem"]["type"], json!("local"));
    }

    #[test]
    fn managed_instruction_block_is_idempotent() {
        let block = managed_instruction_block();
        let once = upsert_managed_block("# User rules\n", &block);
        let twice = upsert_managed_block(&once, &block);

        assert_eq!(once, twice);
        assert!(block.contains("read_history_conversation"));
        assert!(block.contains("Do not ask the user to redescribe"));
        assert!(remove_managed_block(&twice).contains("# User rules"));
        assert!(!remove_managed_block(&twice).contains(MANAGED_BLOCK_START));
    }

    #[test]
    fn uninstall_removes_only_chatmem_json_server() {
        let paths = test_paths("uninstall");
        let config = IntegrationAgent::Gemini.config_path(&paths);
        let mut value = json!({
            "mcpServers": {
                "other": {"command": "node"},
                "chatmem": {"command": "ChatMem.exe", "args": ["--mcp"]}
            }
        });
        write_json_value(&config, &value).unwrap();

        uninstall_one(IntegrationAgent::Gemini, &paths).unwrap();
        value = read_json_object(&config).unwrap();

        assert!(value["mcpServers"].get("other").is_some());
        assert!(value["mcpServers"].get("chatmem").is_none());
    }

    #[test]
    fn opencode_uninstall_removes_only_chatmem_entries() {
        let paths = test_paths("opencode-uninstall");
        let config = IntegrationAgent::OpenCode.config_path(&paths);
        let value = json!({
            "mcp": {
                "other": {"type": "local", "command": ["node", "server.js"]},
                "chatmem": {"type": "local", "command": ["ChatMem.exe", "--mcp"]}
            },
            "tools": {
                "other_*": true,
                "chatmem_*": true
            },
            "permission": {
                "skill": {
                    "other": "ask",
                    "chatmem": "allow"
                }
            }
        });
        write_json_value(&config, &value).unwrap();
        write_text_if_changed(
            &IntegrationAgent::OpenCode.instructions_path(&paths),
            CHATMEM_SKILL,
        )
        .unwrap();
        install_managed_instructions(&opencode_rules_path(&paths)).unwrap();

        uninstall_one(IntegrationAgent::OpenCode, &paths).unwrap();
        let updated = read_json_object(&config).unwrap();

        assert!(updated["mcp"].get("other").is_some());
        assert!(updated["mcp"].get("chatmem").is_none());
        assert_eq!(updated["tools"]["other_*"], json!(true));
        assert!(updated["tools"].get("chatmem_*").is_none());
        assert_eq!(updated["permission"]["skill"]["other"], json!("ask"));
        assert!(updated["permission"]["skill"].get("chatmem").is_none());
        assert!(!IntegrationAgent::OpenCode
            .instructions_path(&paths)
            .exists());
        assert!(fs::read_to_string(opencode_rules_path(&paths))
            .unwrap_or_default()
            .is_empty());
    }
}
