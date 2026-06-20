# ChatMem v1.1.3 Windows 端开发报告 — 供 macOS 端对等实现

> **写给 macOS 开发者**：本文档详细列出 Windows 端在 v1.1.2 → v1.1.3 期间做的所有改动。每个改动都有完整的代码和修改位置，你需要在 macOS 端做对等实现。
>
> **前提**：上游仓库 `Rimagination/ChatMem` 的 main 分支目前是 v1.1.2。Windows 端的 fork `douxy1994/ChatMem` 在此基础上做了以下所有改动。
>
> **参考 PR**：https://github.com/Rimagination/ChatMem/pull/13

---

## 一、新增文件清单

以下文件在上游不存在，需要全部新建：

| 文件路径 | 用途 |
|----------|------|
| `crates/agentswap-hermes/Cargo.toml` | Hermes 适配器 crate 配置 |
| `crates/agentswap-hermes/src/lib.rs` | `pub mod adapter;` |
| `crates/agentswap-hermes/src/adapter.rs` | Hermes Agent 适配器（读取 state.db） |
| `src-tauri/src/local_sync.rs` | OneDrive 双向同步模块（488 行） |
| `docs/cross-platform-filename-encoding.md` | 跨平台文件名编码规范 |
| `docs/macos-sync-adaptation-guide.md` | 本文档 |

---

## 二、需要修改的现有文件

| 文件 | 改动量 | 说明 |
|------|--------|------|
| `src-tauri/Cargo.toml` | +5 行 | 添加 hermes 依赖、dialog-open、system-tray feature |
| `src-tauri/Cargo.lock` | 自动生成 | cargo 会自动更新 |
| `src-tauri/tauri.conf.json` | +14 行 | 添加 dialog allowlist、systemTray 配置、decorations/transparent 修复 |
| `src-tauri/src/main.rs` | +261 行 | 同步命令、托盘、来源视图增强、对话读取回退 |
| `src-tauri/src/agent_integration.rs` | +125 行 | Hermes Agent 集成（MCP 配置安装/卸载） |
| `src-tauri/src/chatmem_memory/store.rs` | +31 行 | `list_store_conversations` 方法 |
| `crates/agentswap-core/src/types.rs` | +2 行 | `AgentKind::Hermes` 枚举 |
| `crates/agentswap-core/src/tool_mapping.rs` | +5 行 | Hermes 工具映射 |
| `crates/agentswap-zcode/src/adapter.rs` | +3 行 | Hermes write bail |
| `package.json` | 版本号改为 1.1.3 |
| `src/settings/storage.ts` | +34 行 | 新增设置字段 |
| `src/App.tsx` | +530 行 | 机器分组、来源视图合并 |
| `src/styles.css` | +244 行 | 机器分组样式 |
| `src/components/SettingsPanel.tsx` | +247 行 | OneDrive 同步设置 UI |
| `src/components/MigrateModal.tsx` | +3 行 | Hermes 迁移支持 |

---

## 三、逐功能详细实现

### 功能 1：Hermes Agent 适配器

**做什么**：让 ChatMem 能读取 Hermes Agent 的对话记录。

**为什么**：Hermes Agent 把对话存在 `~/.hermes/state.db`（macOS）或 `AppData/Local/hermes/state.db`（Windows）的 SQLite 数据库中。ChatMem 需要一个适配器来读取它。

#### 1.1 创建 crate `agentswap-hermes`

**新建文件**：`crates/agentswap-hermes/Cargo.toml`

```toml
[package]
name = "agentswap-hermes"
version = "0.1.0"
edition = "2021"

[dependencies]
agentswap-core = { path = "../agentswap-core" }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "serde"] }
anyhow = "1"
dirs = "5"
rusqlite = { version = "0.31", features = ["bundled"] }
```

**新建文件**：`crates/agentswap-hermes/src/lib.rs`

```rust
pub mod adapter;
```

**新建文件**：`crates/agentswap-hermes/src/adapter.rs`（完整代码见 PR #13 中的同名文件，共 251 行）

核心逻辑：
- `new()`：查找 state.db，优先 `dirs::data_local_dir()/hermes/state.db`，回退 `~/.hermes/state.db`
- `is_available()`：检查 db_path 是否存在
- `list_conversations()`：查询 `sessions` 表（`WHERE archived = 0`）
- `read_conversation()`：查询 `sessions` + `messages` 表，解析 tool_calls JSON
- `render_prompt()`：生成 Markdown 格式的对话文本
- `data_dir()`：返回 hermes 数据目录

**macOS 特别注意**：macOS 上 Hermes 数据在 `~/.hermes/state.db`，不需要 `data_local_dir` 逻辑。但为了跨平台兼容，保留优先检查 `data_local_dir` 的逻辑。

#### 1.2 注册 Hermes 到 AgentKind

**修改文件**：`crates/agentswap-core/src/types.rs`

在 `AgentKind` 枚举中添加：
```rust
pub enum AgentKind {
    // ... 现有变体
    ZCodeOpenCode,
    Hermes,  // ← 新增
}
```

在 `Conversation::source_label` 的 match 中添加：
```rust
AgentKind::Hermes => "Hermes Agent",
```

#### 1.3 注册 Hermes 到 tool_mapping

**修改文件**：`crates/agentswap-core/src/tool_mapping.rs`

在 `agent_family` 函数中添加：
```rust
AgentKind::Hermes => AgentKind::Claude,  // Hermes 工具映射归入 Claude 族
```

在 `to_canonical` 和 `from_canonical` 的 match 中添加 `AgentKind::Hermes`。

#### 1.4 注册 Hermes 到 ZCode adapter

**修改文件**：`crates/agentswap-zcode/src/adapter.rs`

在 write match 中添加：
```rust
AgentKind::Hermes => {
    anyhow::bail!("Hermes write is not supported (read-only adapter)")
}
```

#### 1.5 添加依赖和注册

**修改文件**：`src-tauri/Cargo.toml`

在 `[dependencies]` 中添加：
```toml
agentswap-hermes = { path = "../crates/agentswap-hermes" }
```

**修改文件**：`src-tauri/src/main.rs`

在文件顶部添加 import：
```rust
use agentswap_hermes::adapter::HermesAdapter;
```

在 `AGENT_KEYS` 中添加 `"hermes"`（同时移除 `"gemini"` 和 `"opencode"`）：
```rust
const AGENT_KEYS: &[&str] = &["claude", "codex", "zcode", "hermes"];
```

在 `get_adapter` 函数中添加：
```rust
"hermes" => Ok(Box::new(HermesAdapter::new())),
```

在 `agent_key` 函数中添加：
```rust
AgentKind::Hermes => "hermes",
```

---

### 功能 2：Hermes Agent 集成（MCP 配置）

**做什么**：在设置 → Agent 集成中，支持一键安装/卸载 Hermes 的 MCP 配置和 Skill。

**修改文件**：`src-tauri/src/agent_integration.rs`

#### 2.1 添加 IntegrationAgent::Hermes

在 `IntegrationAgent` 枚举中添加 `Hermes`。

在 `all()` 中返回 5 个变体（包含 Hermes）。

在 `from_key` / `key` / `display_name` 中添加 `"hermes"` 映射。

#### 2.2 添加路径配置

```rust
fn config_path(self, paths: &IntegrationPaths) -> PathBuf {
    match self {
        // ... 现有
        Self::Hermes => {
            // macOS: ~/.hermes/config.yaml
            // Windows: AppData/Local/hermes/config.yaml
            let base = dirs::data_local_dir().unwrap_or_else(|| paths.home_dir.clone());
            let appdata = base.join("hermes").join("config.yaml");
            let home = paths.home_dir.join(".hermes").join("config.yaml");
            if appdata.exists() || !home.exists() { appdata } else { home }
        }
    }
}

fn instructions_path(self, paths: &IntegrationPaths) -> PathBuf {
    match self {
        // ... 现有
        Self::Hermes => {
            let base = dirs::data_local_dir().unwrap_or_else(|| paths.home_dir.clone());
            let appdata = base.join("hermes").join("skills").join("chatmem").join("SKILL.md");
            let home = paths.home_dir.join(".hermes").join("skills").join("chatmem").join("SKILL.md");
            if appdata.exists() || !home.exists() { appdata } else { home }
        }
    }
}
```

#### 2.3 添加安装/卸载函数

```rust
fn chatmem_hermes_yaml(paths: &IntegrationPaths) -> String {
    format!(
        " chatmem:\n args:\n - --mcp\n command: {}\n connect_timeout: 30\n",
        path_to_string(&paths.executable_path)
    )
}

fn hermes_config_has_chatmem(path: &Path) -> bool {
    fs::read_to_string(path)
        .map(|content| content.contains("chatmem:"))
        .unwrap_or(false)
}

fn install_hermes_config(path: &Path, paths: &IntegrationPaths) -> Result<Option<PathBuf>, String> {
    // 检查文件是否存在
    // 检查是否已安装
    // 读取现有内容
    // 在 "plugins:" 之前插入 chatmem 配置块
    // 写回文件
}

fn uninstall_hermes_config(path: &Path) -> Result<Option<PathBuf>, String> {
    // 读取文件
    // 移除 chatmem: 及其子行
    // 写回文件
}
```

#### 2.4 注册到 install_one / uninstall_one / status_for_agent

在 `install_one`、`uninstall_one`、`mcp_installed`、`instructions_installed`、`status_for_agent` 的 match 中添加 `IntegrationAgent::Hermes` 分支。

---

### 功能 3：OneDrive 双向同步

**做什么**：用户选择一个同步文件夹（如 OneDrive 目录），ChatMem 自动将本地对话双向同步到该文件夹。其他机器的 ChatMem 也同步到同一文件夹，实现跨机器对话共享。

**这是全新模块，需要新建 `src-tauri/src/local_sync.rs`（488 行）**。

#### 3.1 新建 local_sync.rs

完整代码见 PR #13 中的 `src-tauri/src/local_sync.rs`。核心结构：

```rust
// === 文件名编码（跨平台必须一致）===
pub fn id_to_filename(id: &str) -> String { /* 编码 :<>"|?*\/ 为 HTML 实体 */ }
pub fn filename_to_id(name: &str) -> String { /* 逆向解码 */ }

// === 数据结构 ===
pub struct SyncConversationMeta { id, agent, updated_at, source }
pub struct SyncItem { agent, id, updated_at, file_name, body }
pub struct SyncResult { uploaded, downloaded, skipped, conflicts_resolved, folder_path }
pub struct SyncStatus { available, folder_path, remote_conversation_count, last_sync_info }
pub struct CloudSyncReadiness { folder_exists, is_quiet, has_lock_files, recommended_action }

// === 核心函数 ===
pub fn check_sync_status(folder: &Path) -> SyncStatus
pub fn read_remote_conversations(folder: &Path) -> HashMap<(String, String), (String, Vec<u8>)>
pub fn bidirectional_sync(local_items: &[SyncItem], folder: &Path) -> Result<SyncResult>
pub fn is_folder_quiet(folder: &Path, quiet_seconds: u64) -> bool
pub fn check_cloud_readiness(folder: &Path, quiet_seconds: u64) -> CloudSyncReadiness
```

**bidirectional_sync 逻辑**：
1. 确保 `conversations/{agent}/` 目录结构存在
2. 读取远程对话（`read_remote_conversations`）
3. 构建本地对话查找表
4. 遍历所有对话 key：
   - 仅本地 → 写入同步文件夹（`id_to_filename` 编码文件名）
   - 仅远程 → 计数（已存在于同步文件夹）
   - 两者都有 → 比较时间戳，保留更新的
5. 写入 manifest.json

#### 3.2 在 main.rs 中注册同步命令

添加 `mod local_sync;`。

添加 `SyncSettingsPayload` 结构体（含 `sync_folder` 字段）。

添加 `AppSettingsPayload` 结构体（含 `auto_backup_enabled`、`auto_backup_interval_minutes` 字段）。

添加 Tauri 命令：
```rust
#[command]
fn local_sync_status(folder_path: String) -> local_sync::SyncStatus { ... }

#[command]
fn check_cloud_readiness(folder_path: String) -> local_sync::CloudSyncReadiness { ... }

#[command]
fn sync_local_now(folder_path: String) -> Result<local_sync::SyncResult, String> {
    // 1. 收集所有本地对话
    // 2. 调用 bidirectional_sync
    // 3. 导入远程对话到记忆库（见功能 4）
}
```

在 `invoke_handler` 中注册这三个命令。

#### 3.3 添加 Cargo 依赖

在 `src-tauri/Cargo.toml` 中添加 `dialog-open` feature：
```toml
tauri = { version = "1", features = ["dialog-open", "window-all", "shell-open", "updater"] }
```

#### 3.4 前端：设置面板 UI

**修改文件**：`src/components/SettingsPanel.tsx`

在 OneDrive 同步设置区域添加：
- 同步文件夹选择器（使用 Tauri `openDialog({ directory: true })`）
- 显示当前同步路径
- 清除按钮
- 已同步对话数
- 手动同步按钮
- 自动备份开关和间隔选择（5/15/30/60/120 分钟）

---

### 功能 4：同步自动导入远程对话

**做什么**：同步完成后，自动将远程对话写入 ChatMem 记忆库，这样在来源视图中就能看到其他机器的对话。

**修改文件**：`src-tauri/src/main.rs` — `sync_local_now` 函数

在 `bidirectional_sync` 返回后添加：
```rust
if result.downloaded > 0 {
    if let Ok(store) = open_memory_store() {
        let remote = local_sync::read_remote_conversations(&path);
        let mut local_ids = std::collections::HashSet::new();
        for item in &items {
            local_ids.insert((item.agent.clone(), item.id.clone()));
        }
        for ((agent, id), (_updated_at, body)) in &remote {
            if local_ids.contains(&(agent.clone(), id.clone())) { continue; }
            if let Ok(conversation) = serde_json::from_slice::<Conversation>(&body) {
                let _ = sync_conversation_into_store(&store, agent, &conversation);
            }
        }
    }
}
```

---

### 功能 5：来源视图显示同步对话

**做什么**：切换到某个来源（如 Hermes）时，不仅显示本地适配器的对话，还显示从其他机器同步过来的对话。

#### 5.1 MemoryStore 新增方法

**修改文件**：`src-tauri/src/chatmem_memory/store.rs`

添加 `list_store_conversations` 方法（31 行），查询 `conversations` + `repos` 表，按 `source_agent` 过滤。

#### 5.2 修改 list_conversations

**修改文件**：`src-tauri/src/main.rs`

原代码只从适配器读取。新代码合并两部分：
1. 适配器结果（本地原生存储）
2. 记忆库结果（`store.list_store_conversations(&agent)`）

用 `seen_ids` HashSet 去重。

---

### 功能 6：对话读取回退到同步文件夹

**做什么**：点击一个从其他机器同步过来的对话时，适配器读不到（因为不在本地），自动从 OneDrive 同步文件夹读取原始 JSON。

**修改文件**：`src-tauri/src/main.rs` — `read_conversation`

原代码直接调用 `adapter.read_conversation(&id)`，失败就报错。新代码：
1. 先试适配器
2. 失败 → 读取 settings 获取 sync_folder 路径
3. 构造文件路径：`{sync_folder}/conversations/{agent}/{id_to_filename(id)}.json`
4. 读取并反序列化为 `Conversation`

---

### 功能 7：系统托盘 / macOS Dock 行为

**做什么**：
- macOS：关闭按钮隐藏窗口（不退出），dock 图标点击重新显示，dock 右键退出
- 所有平台：托盘菜单（打开主界面 / 同步 / 退出）

#### 7.1 Cargo.toml

添加 `system-tray` feature：
```toml
tauri = { version = "1", features = ["dialog-open", "window-all", "shell-open", "updater", "system-tray"] }
```

#### 7.2 tauri.conf.json

添加 `systemTray` 配置：
```json
"systemTray": {
  "iconPath": "icons/icon.icns",
  "iconAsTemplate": true
}
```

修改窗口配置（修复 macOS 26 卡死）：
```json
"decorations": true,
"transparent": false
```

添加 dialog allowlist：
```json
"dialog": { "all": false, "open": true }
```

#### 7.3 main.rs

添加 `use tauri::Manager;`。

在 `Builder` 中添加 `.system_tray(system_tray)` 和 `.on_system_tray_event(...)`。

托盘菜单：
```rust
let tray_menu = tauri::SystemTrayMenu::new()
    .add_item(tauri::CustomMenuItem::new("open", "打开主界面").accelerator("Cmd+Shift+M"))
    .add_native_item(tauri::SystemTrayMenuItem::Separator)
    .add_item(tauri::CustomMenuItem::new("sync", "同步"))
    .add_native_item(tauri::SystemTrayMenuItem::Separator)
    .add_item(tauri::CustomMenuItem::new("quit", "退出"));
```

托盘事件处理：
- `"open"` → `window.show()` + `window.set_focus()`
- `"sync"` → `window.emit("tray-sync", ())`
- `"quit"` → `app.exit(0)`
- `LeftClick` → 同 "open"

关闭按钮行为（所有平台）：
```rust
.on_window_event(|event| {
    if let tauri::WindowEvent::CloseRequested { api, .. } = event.event() {
        event.window().hide().unwrap_or(());
        api.prevent_close();
    }
})
```

macOS dock 点击重新显示（保留原有的 `setup_macos_dock_handler`）。

---

### 功能 8：设置持久化补充

**做什么**：新增的设置字段（sync_folder、auto_backup_enabled、auto_backup_interval_minutes）需要持久化到 settings.json。

**修改文件**：`src-tauri/src/main.rs`

在 `SyncSettingsPayload` 中添加：
```rust
#[serde(default)]
sync_folder: String,
```

在 `AppSettingsPayload` 中添加：
```rust
#[serde(default)]
auto_backup_enabled: bool,
#[serde(default = "default_auto_backup_interval")]
auto_backup_interval_minutes: i64,
```

添加默认值函数：
```rust
fn default_auto_backup_interval() -> i64 { 30 }
```

**修改文件**：`src/settings/storage.ts`

在 `SyncSettings` 类型中添加 `syncFolder: string;`。

在 `AppSettings` 类型中添加：
```typescript
autoBackupEnabled: boolean;
autoBackupIntervalMinutes: number;
```

在 `DEFAULT_SYNC_SETTINGS` 中添加 `syncFolder: ""`。

在 `normalizeSyncSettings` 中添加 syncFolder 解析。

---

### 功能 9：机器分组

**做什么**：当有来自多台电脑的对话时，在项目列表上方显示机器分组层。支持自定义重命名、合并分组、移动对话。

**修改文件**：`src/settings/storage.ts`

在 `AppSettings` 中添加：
```typescript
machineGroupNames: Record<string, string>;
machineGroupOverrides: Record<string, string>;
```

默认值：`{}`

**修改文件**：`src/App.tsx`

#### 9.1 detectMachineId 函数

```typescript
function detectMachineId(projectDir: string): string {
  const normalized = projectDir.replace(/\\/g, "/");
  if (/^[a-zA-Z]:\//.test(normalized)) return "windows";
  if (/^\/(Users|Volumes|Applications)\//i.test(normalized)) return "macos";
  if (/^\/(home|root|usr|opt|tmp)\//.test(normalized)) return "linux";
  if (normalized.startsWith("chatmem://")) return "internal";
  return "other";
}
```

**注意**：不要按用户名拆分！同一台机器上 `/users/alvis` 和 `/volumes/douxy` 是同一台 Mac。

#### 9.2 machineGroups memo

按 `machineGroupOverrides[fullPath] || detectMachineId(fullPath)` 分组 `projectGroups`。

自动标签：一台 → "Windows"/"Mac"，多台 → "Windows-1"/"Windows-2"。

仅 `machineGroups.length > 1` 时渲染分组层。

#### 9.3 合并/移动功能

状态：`mgSelectMode`, `selectedMgIds`, `selectedConvKeysForMove`, `mergeTargetId`, `moveTargetId`

处理函数：
- `handleMergeMachineGroups(targetId)`：将选中分组的所有对话 project_dir override 到目标
- `handleMoveConversations(targetId)`：将选中对话的 project_dir override 到目标
- `handleResetGroupOverrides()`：清空所有 override

#### 9.4 渲染

- 管理分组按钮（侧边栏 action 区域，仅 `machineGroups.length > 1` 时显示）
- action bar：已选状态 + 合并/移动/选择全部/重置 按钮
- 复选框：机器分组头部（全选该组）+ 对话行
- 双击分组名称可内联编辑重命名

**修改文件**：`src/styles.css`

添加机器分组相关样式（约 100 行），使用浅色主题变量。

---

### 功能 10：移除 Gemini CLI 和 OpenCode

**做什么**：从 Agent 列表中移除 Gemini CLI 和 OpenCode。

**修改文件**：
- `src-tauri/src/main.rs`：`AGENT_KEYS` 从 `["claude", "codex", "gemini", "opencode", "zcode", "hermes"]` 改为 `["claude", "codex", "zcode", "hermes"]`
- `src-tauri/src/local_sync.rs`：`AGENT_LIST` 同步修改
- `src/App.tsx`：`AGENT_OPTIONS` 数组移除 gemini 和 opencode 条目

---

## 四、实现顺序建议

1. **Hermes 适配器**（功能 1）— 基础，其他功能依赖它
2. **Hermes 集成**（功能 2）— MCP 配置安装
3. **OneDrive 同步**（功能 3）— 核心新功能
4. **同步导入 + 来源视图 + 读取回退**（功能 4/5/6）— 同步的配套
5. **系统托盘**（功能 7）— 独立功能
6. **设置持久化**（功能 8）— 配合同步
7. **机器分组**（功能 9）— 最复杂的 UI 改动
8. **移除 Gemini/OpenCode**（功能 10）— 最简单

---

## 五、验证清单

- [ ] `cargo check` 通过
- [ ] `npm run build` 通过
- [ ] `npx tauri build` 成功生成 .dmg
- [ ] Hermes 来源显示本地 + 同步的对话
- [ ] 点击同步的对话能正常查看详情
- [ ] OneDrive 同步：上传/下载/跳过 三种情况正确
- [ ] ZCode 对话 ID 中的冒号被编码为 `&#x3a;`
- [ ] 系统托盘：关闭→隐藏到 dock，dock 点击恢复
- [ ] 托盘菜单：打开/同步/退出 正常工作
- [ ] 设置：syncFolder、autoBackup 重启后保留
- [ ] 机器分组：单台不显示，多台显示
- [ ] 机器分组：双击重命名、合并、移动 功能正常
- [ ] 机器分组 UI：浅色主题，文字清晰可读

---

## 六、已知问题

1. macOS 端 `decorations: false, transparent: true` 会导致 macOS 26 (Tahoe) 窗口卡死。需要改为 `decorations: true, transparent: false`。这在 tauri.conf.json 中修改。

2. macOS 端需要 `cocoa` 和 `objc` 依赖。在 Cargo.toml 中用平台条件依赖：
   ```toml
   [target.'cfg(target_os = "macos")'.dependencies]
   cocoa = "0.24"
   objc = "0.2"
   ```
   这样 Windows 编译时不会拉取这些 macOS 专用 crate。

3. 文件名编码必须两端完全一致。如果 macOS 端用不同的编码方式，同步文件会产生两份不同文件名的副本。
