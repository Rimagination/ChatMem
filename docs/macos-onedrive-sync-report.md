# ChatMem macOS 修复与 OneDrive 同步功能 — 技术文档 & Windows 开发指南

> **版本**: ChatMem v1.1.2 (修改版)
> **日期**: 2026-06-14
> **适用平台**: macOS (Apple Silicon) / Windows 11
> **仓库**: https://github.com/Rimagination/ChatMem

---

## 目录

1. [概述](#1-概述)
2. [macOS 26 (Tahoe) 卡死修复](#2-macos-26-tahoe-卡死修复)
3. [OneDrive 双向同步功能](#3-onedrive-双向同步功能)
4. [云端同步状态检测](#4-云端同步状态检测)
5. [定时自动备份](#5-定时自动备份)
6. [Windows 11 端开发指南](#6-windows-11-端开发指南)
7. [附录：文件变更清单](#7-附录文件变更清单)

---

## 1. 概述

ChatMem 是一个基于 Tauri v1 (Rust + React) 的本地优先 AI 对话记忆管理工具。原始版本在 macOS 26 (Tahoe) 上存在严重的窗口卡死问题，且缺少跨设备对话同步功能。

本文档记录了以下修复和新增功能：

| 类别 | 内容 |
|------|------|
| **Bug 修复** | macOS 26 窗口卡死（Tauri v1 透明窗口兼容性） |
| **新功能** | OneDrive 双向同步（基于共享文件夹） |
| **新功能** | 云端同步状态检测（避免文件锁冲突） |
| **新功能** | 定时自动备份（可配置间隔，智能跳过忙碌状态） |

---

## 2. macOS 26 (Tahoe) 卡死修复

### 2.1 问题描述

原始 ChatMem 在 macOS 26 (Tahoe) 上启动后窗口完全无响应，系统报告"应用程序没有响应"。进程仍在运行（占用 1.4GB 内存），但 UI 无法交互。

### 2.2 根因分析

Tauri v1.8.3 使用 WRY (v0.24.12) 和 TAO (v0.16.11) 管理原生窗口和 WebView。原始配置：

```json
// src-tauri/tauri.conf.json (原始)
{
  "windows": [{
    "decorations": false,    // 无原生标题栏
    "transparent": true       // 透明窗口
  }]
}
```

配合前端的自定义标题栏（`data-tauri-drag-region` + 自定义最小化/最大化/关闭按钮），在 macOS 26 上触发了以下问题：

1. **透明窗口 + WKWebView 冲突**: `transparent: true` 导致 WRY 设置 `setOpaque(false)` + 透明背景。macOS 26 的新 compositor 与这种配置不兼容，WebView 渲染管线卡死。
2. **自定义拖拽区域阻塞事件循环**: `data-tauri-drag-region` 属性让 TAO 在原生层拦截鼠标事件实现窗口拖拽。macOS 26 的窗口管理器变更导致这些拦截阻塞了主线程。

### 2.3 修复方案

**核心原则**: 放弃自定义窗口装饰，使用 macOS 原生窗口管理。

#### 修改 1: 窗口配置 (`src-tauri/tauri.conf.json`)

```json
{
  "windows": [{
    "fullscreen": false,
    "resizable": true,
    "title": "ChatMem",
    "width": 1200,
    "height": 800,
    "minWidth": 900,
    "minHeight": 600,
    "decorations": true,
    "transparent": false
  }]
}
```

- `decorations`: `false` → `true` — 启用原生标题栏（包含 macOS 交通灯按钮）
- `transparent`: `true` → `false` — 禁用透明窗口

#### 修改 2: 移除自定义窗口控制 (`src/App.tsx`)

删除以下内容：
- `data-tauri-drag-region="true"` 属性（从 header 和子元素）
- `onMouseDown={handleTopbarMouseDown}` 事件处理
- 自定义最小化/最大化/关闭按钮的整个 `<div className="window-controls">`
- `handleTopbarMouseDown` 函数（调用 `appWindow.startDragging()`）
- `handleToggleWindowSize` 函数（调用 `appWindow.toggleMaximize()`）

保留顶部栏用于品牌标识和导航，但添加 `paddingLeft: 78` 为 macOS 交通灯按钮留出空间：

```tsx
<header className="app-topbar" style={{ paddingLeft: 78 }}>
  <div className="topbar-left">
    <img className="topbar-app-icon" src={brandIcon} alt="ChatMem icon" />
    <span className="topbar-version">ChatMem v{packageInfo.version}</span>
    {/* 侧边栏切换按钮保留 */}
  </div>
  <div className="topbar-drag-space" />
</header>
```

#### 修改 3: CSS 调整 (`src/styles.css`)

```css
/* 原始：无边框窗口的浮动卡片效果 */
.app-shell {
  width: calc(100% - 16px);
  height: calc(100% - 16px);
  margin: 8px;
  border: 1px solid rgba(22, 32, 24, 0.1);
  border-radius: 12px;
  background: linear-gradient(...), var(--bg-app);
  box-shadow: var(--shadow-window);
}

/* 修复后：填满窗口，实心背景 */
.app-shell {
  width: 100%;
  height: 100%;
  margin: 0;
  border: none;
  border-radius: 0;
  background: var(--bg-app);
  box-shadow: none;
}

body {
  background: var(--bg-app);   /* 原为 transparent */
}

#root {
  background: var(--bg-app);   /* 原为 transparent */
}
```

### 2.4 修复效果

| 指标 | 修复前 | 修复后 |
|------|--------|--------|
| 内存占用 | 1.4GB (卡死) | ~110MB (正常) |
| CPU | 持续高 | 0% (空闲) |
| 窗口响应 | 完全无响应 | 正常 |

### 2.5 Windows 端注意事项

- Windows 上 `transparent: true` + `decorations: false` **不会**导致卡死（使用 WebView2，不是 WKWebView）
- 如果 Windows 端想要保留自定义无边框窗口效果，可以保留原始配置
- 如果想要统一代码库，建议两端都使用 `decorations: true`

---

## 3. OneDrive 双向同步功能

### 3.1 设计理念

这不是简单的单向备份，而是**双向同步**：

```
Mac 上的 Agent 产生对话 → ChatMem 同步到共享文件夹 (OneDrive)
                                          ↓
Win 上的 ChatMem 读取 → Win 上的 Agent 继续对话 → 同步回共享文件夹
                                          ↓
Mac 再次同步 → 获取 Win 上的新对话内容
```

### 3.2 同步文件夹结构

```
<用户选择的文件夹>/
├── manifest.json                    # 同步元数据
└── conversations/
    ├── claude/
    │   ├── <conversation-id>.json   # 完整对话数据
    │   └── ...
    ├── codex/
    │   └── ...
    ├── gemini/
    │   └── ...
    ├── opencode/
    │   └── ...
    └── zcode/
        └── ...
```

#### manifest.json 格式

```json
{
  "schema_version": 2,
  "app_version": "1.1.2",
  "last_synced_at": "2026-06-14T22:30:00+00:00",
  "sync_direction": "bidirectional",
  "uploaded": 3,
  "downloaded": 2,
  "skipped": 5,
  "conflicts_resolved": 1,
  "total_local": 10,
  "total_remote": 9
}
```

### 3.3 双向同步算法

**Rust 实现**: `src-tauri/src/local_sync.rs` — `bidirectional_sync()`

```
输入: local_items (本地对话列表), folder (同步文件夹路径)

1. 确保 conversations/<agent>/ 目录结构存在
2. 读取远端对话 → HashMap<(agent, id), (updated_at, body)>
3. 构建本地 lookup → HashMap<(agent, id), (updated_at, body)>
4. 收集所有唯一的 (agent, id) 键
5. 对每个键:
   ├─ 只有本地 → 写入同步文件夹 (上传)
   ├─ 只有远端 → 标记为可用 (下载，已在同步文件夹中)
   ├─ 两端都有 → 比较 updated_at 时间戳:
   │   ├─ 本地更新 → 上传覆盖远端
   │   ├─ 远端更新 → 保留远端版本
   │   └─ 时间相同 → 跳过
   └─ 都不存在 → 不可能 (unreachable)
6. 写入 manifest.json
```

**时间戳解析** (`parse_timestamp`):
- 支持 RFC 3339 格式: `2026-06-14T22:30:00+08:00`
- 支持 ISO 8601 UTC 格式: `2026-06-14T14:30:00.000Z`
- 回退: 返回 0（最旧）

### 3.4 Tauri 命令

```rust
// 检查同步文件夹状态
#[command]
fn local_sync_status(folder_path: String) -> SyncStatus;

// 执行双向同步
#[command]
fn sync_local_now(folder_path: String) -> Result<SyncResult, String>;

// 检查云端就绪状态
#[command]
fn check_cloud_readiness(folder_path: String) -> CloudSyncReadiness;
```

### 3.5 前端实现

#### 文件夹选择器

使用 Tauri 的原生对话框 API：

```typescript
import { open as openDialog } from "@tauri-apps/api/dialog";

const selected = await openDialog({
  directory: true,
  title: "选择同步文件夹"
});
if (selected && typeof selected === "string") {
  handleSyncSettingsChange({ syncFolder: selected });
}
```

**需要在 `tauri.conf.json` 中启用**:
```json
{
  "allowlist": {
    "dialog": { "all": false, "open": true }
  }
}
```

#### 设置存储

`SyncSettings` 类型新增字段：

```typescript
type SyncSettings = {
  provider: "off" | "webdav" | "onedrive";
  // ... 其他 webdav 字段 ...
  syncFolder: string;  // 新增：用户选择的同步文件夹路径
};
```

路径持久化到 localStorage 和原生设置文件。

### 3.6 Windows 端实现要点

1. **文件夹检测**: Windows 上 OneDrive 路径通常在 `C:\Users\<username>\OneDrive\`，但用户应该可以自选任意文件夹
2. **文件路径**: Rust 的 `std::path::PathBuf` 跨平台兼容，无需特殊处理
3. **文件锁**: Windows 上文件锁比 macOS 更严格，需要确保同步时不会与 OneDrive 客户端冲突（见第 4 节）
4. **对话 ID**: 对话 ID 是 UUID v5（基于 agent + project_dir + session_id），跨平台一致

---

## 4. 云端同步状态检测

### 4.1 问题背景

OneDrive 客户端在同步文件时会创建临时文件和锁文件。如果 ChatMem 在此时执行同步，可能：
- 读取到不完整的文件
- 与 OneDrive 的文件锁冲突
- 上传的文件被 OneDrive 覆盖

### 4.2 检测逻辑

**Rust 实现**: `src-tauri/src/local_sync.rs` — `check_cloud_readiness()`

```rust
pub fn check_cloud_readiness(folder: &Path, quiet_seconds: u64) -> CloudSyncReadiness;
```

检测维度（递归扫描同步文件夹）：

| 检测项 | 模式 | 来源 |
|--------|------|------|
| 临时文件 | `*.tmp` | OneDrive |
| 部分下载 | `*.partial` | OneDrive |
| Office 锁文件 | `~$*` | Microsoft Office |
| Google Drive 临时文件 | `*.gdoc_tmp`, `*.crswap` | Google Drive |
| 同步状态目录 | `.odrive/`, `.sync/`, `.tmp.driveupload/` | 各种云盘 |
| 最近写入 | 任何 3 秒内被修改的文件 | 通用 |
| 文件夹活动 | 文件夹在 `quiet_seconds`（默认 10 秒）内被修改 | 通用 |

#### 返回值

```rust
struct CloudSyncReadiness {
    folder_exists: bool,        // 文件夹是否存在
    is_quiet: bool,             // 是否安静（无活动）
    has_lock_files: bool,       // 是否有锁文件
    recommended_action: String, // "safe_to_sync" | "wait" | "folder_missing"
}
```

### 4.3 Windows 端额外检测

Windows 上 OneDrive 有额外的状态指示：

1. **注册表状态**: `HKCU\Software\Microsoft\OneDrive\Accounts\Personal` 下有同步状态
2. **OneDrive 进程**: 可通过 `Get-Process OneDrive` 检查是否运行
3. **文件属性**: Windows 上 OneDrive 的 "按需文件" 使用 `FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS` 属性
4. **Named Pipe**: OneDrive 在 `\\.\pipe\OneDrive` 有 IPC 管道

建议 Windows 端额外实现：

```rust
// Windows 特有的 OneDrive 状态检测
#[cfg(target_os = "windows")]
fn check_onedrive_process_running() -> bool {
    // 检查 OneDrive.exe 进程
    std::process::Command::new("tasklist")
        .args(["/FI", "IMAGENAME eq OneDrive.exe"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).contains("OneDrive.exe"))
        .unwrap_or(false)
}
```

---

## 5. 定时自动备份

### 5.1 架构

```
┌─────────────────────────────────────────────────┐
│ React useEffect (auto-backup timer)             │
│                                                 │
│  setInterval(runAutoBackup, intervalMs)         │
│         │                                       │
│         ▼                                       │
│  ┌─ check_cloud_readiness(folder) ─┐            │
│  │                                  │            │
│  ├─ "safe_to_sync" → sync_local_now()           │
│  ├─ "wait" → skip (log "busy")     │            │
│  └─ "folder_missing" → skip        │            │
│                                     │            │
└─────────────────────────────────────┘            │
```

### 5.2 React 实现

```typescript
// src/App.tsx

const autoBackupRef = useRef<ReturnType<typeof setInterval> | null>(null);
const autoBackupRunningRef = useRef(false);

useEffect(() => {
  if (autoBackupRef.current) {
    clearInterval(autoBackupRef.current);
    autoBackupRef.current = null;
  }

  if (!appSettings.autoBackupEnabled || !appSettings.sync.syncFolder) {
    return;
  }

  const intervalMs = appSettings.autoBackupIntervalMinutes * 60 * 1000;

  const runAutoBackup = async () => {
    if (autoBackupRunningRef.current) return; // 防止重叠
    autoBackupRunningRef.current = true;
    try {
      const folder = appSettings.sync.syncFolder;
      if (!folder) return;

      // 1. 检查云盘状态
      const readiness = await invoke<CloudSyncReadiness>(
        "check_cloud_readiness",
        { folderPath: folder }
      );

      // 2. 只有空闲时才同步
      if (readiness.recommended_action === "safe_to_sync") {
        await invoke<LocalSyncResult>("sync_local_now", { folderPath: folder });
      }
    } finally {
      autoBackupRunningRef.current = false;
    }
  };

  autoBackupRef.current = setInterval(runAutoBackup, intervalMs);

  return () => {
    if (autoBackupRef.current) {
      clearInterval(autoBackupRef.current);
    }
  };
}, [
  appSettings.autoBackupEnabled,
  appSettings.autoBackupIntervalMinutes,
  appSettings.sync.syncFolder
]);
```

### 5.3 设置项

```typescript
type AppSettings = {
  // ...
  autoBackupEnabled: boolean;           // 默认 false
  autoBackupIntervalMinutes: number;    // 默认 30, 最小 5
};
```

UI 提供间隔选项: 5, 15, 30, 60, 120 分钟。

### 5.4 Windows 端注意事项

- `setInterval` 在 Tauri 的 WebView 中运行，窗口最小化时**可能**被节流
- 建议 Windows 端将定时器移到 Rust 后端，使用 `tokio::time::interval` 实现
- 或者使用 Tauri 的 `tauri::api::notification` 在同步完成后发送系统通知

---

## 6. Windows 11 端开发指南

### 6.1 环境准备

```powershell
# 1. 安装 Rust
winget install Rustlang.Rustup

# 2. 安装 Node.js
winget install OpenJS.NodeJS.LTS

# 3. 安装 Visual Studio Build Tools (WebView2 依赖)
winget install Microsoft.VisualStudio.2022.BuildTools
# 安装时选择 "Desktop development with C++"

# 4. 安装 Tauri CLI
cargo install tauri-cli
```

### 6.2 构建命令

```powershell
cd ChatMem
npm install
cargo tauri build --target x86_64-pc-windows-msvc
```

产出物：
- `src-tauri/target/x86_64-pc-windows-msvc/release/bundle/msi/ChatMem_1.1.2_x64_en-US.msi`
- `src-tauri/target/x86_64-pc-windows-msvc/release/bundle/nsis/ChatMem_1.1.2_x64-setup.exe`

### 6.3 需要修改的文件

#### 6.3.1 Agent 适配器路径

各 Agent 在 Windows 上的对话存储路径不同：

| Agent | macOS 路径 | Windows 路径 |
|-------|-----------|-------------|
| Claude | `~/.claude/projects/` | `%USERPROFILE%\.claude\projects\` |
| Codex | `~/.codex/sessions/` | `%USERPROFILE%\.codex\sessions\` |
| Gemini | `~/.gemini/sessions/` | `%USERPROFILE%\.gemini\sessions\` |
| OpenCode | `~/.opencode/sessions/` | `%USERPROFILE%\.opencode\sessions\` |

检查 `crates/agentswap-*/src/adapter.rs` 中的路径拼接逻辑，确保使用 `dirs::home_dir()` 而非硬编码 `~`。

#### 6.3.2 SQLite 数据库路径

```rust
// src-tauri/src/chatmem_memory/db.rs
// 检查数据库路径是否使用 dirs::data_dir()
// macOS: ~/Library/Application Support/com.chatmem.app/
// Windows: C:\Users\<user>\AppData\Roaming\com.chatmem.app\
```

#### 6.3.3 Keyring 存储

已使用的 `keyring` crate 跨平台兼容：
- macOS: 钥匙串 (Keychain)
- Windows: 凭据管理器 (Credential Manager)

无需修改。

#### 6.3.4 文件对话框

已使用的 `@tauri-apps/api/dialog` 跨平台兼容：
- macOS: NSOpenPanel
- Windows: IFileDialog (原生 Windows 文件对话框)

无需修改。

### 6.4 Windows 特有功能建议

1. **系统托盘**: 添加托盘图标，支持最小化到托盘
2. **开机自启**: 使用 `tauri-plugin-autostart`
3. **Windows 通知**: 同步完成时发送 Toast 通知
4. **OneDrive Known Folder**: 检测 `KnownFolder` API 获取 OneDrive 默认路径

```rust
// Windows 特有的 OneDrive 路径检测
#[cfg(target_os = "windows")]
fn detect_onedrive_folder() -> Option<PathBuf> {
    // 方法 1: 环境变量
    if let Ok(path) = std::env::var("OneDrive") {
        return Some(PathBuf::from(path));
    }
    // 方法 2: 注册表
    // HKCU\Environment\OneDrive
    None
}
```

5. **文件锁处理**: Windows 上 `std::fs::write` 对被锁定的文件会失败，需要重试机制：

```rust
fn write_with_retry(path: &Path, data: &[u8], max_retries: u32) -> Result<()> {
    for attempt in 0..max_retries {
        match std::fs::write(path, data) {
            Ok(_) => return Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                std::thread::sleep(std::time::Duration::from_millis(500 * (attempt + 1) as u64));
            }
            Err(e) => return Err(e.into()),
        }
    }
    anyhow::bail!("Failed to write after {} retries", max_retries)
}
```

### 6.5 跨平台代码组织建议

```
src-tauri/src/
├── main.rs              # 主入口，Tauri 命令注册
├── lib.rs               # 模块导出
├── local_sync.rs        # 同步逻辑（跨平台）
├── agent_integration.rs # Agent 集成
├── chatmem_memory/      # 记忆系统
│   ├── db.rs
│   ├── store.rs
│   └── ...
├── platform/
│   ├── mod.rs           # 平台抽象层
│   ├── macos.rs         # macOS 特有逻辑
│   └── windows.rs       # Windows 特有逻辑
└── ...
```

平台抽象层示例：

```rust
// src-tauri/src/platform/mod.rs
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(target_os = "windows")]
pub mod windows;

pub fn detect_cloud_folder() -> Option<std::path::PathBuf> {
    #[cfg(target_os = "macos")]
    { macos::detect_onedrive_folder() }
    #[cfg(target_os = "windows")]
    { windows::detect_onedrive_folder() }
}
```

---

## 7. 附录：文件变更清单

### 7.1 macOS 卡死修复涉及的文件

| 文件 | 变更类型 | 说明 |
|------|---------|------|
| `src-tauri/tauri.conf.json` | 修改 | `decorations: true`, `transparent: false` |
| `src/App.tsx` | 修改 | 移除自定义窗口控制、拖拽区域、相关函数 |
| `src/styles.css` | 修改 | `.app-shell` 实心背景、移除 margin/radius/shadow |

### 7.2 OneDrive 同步涉及的文件

| 文件 | 变更类型 | 说明 |
|------|---------|------|
| `src-tauri/src/local_sync.rs` | **新增** | 双向同步算法、云端状态检测 |
| `src-tauri/src/main.rs` | 修改 | 新增 3 个 Tauri 命令 |
| `src-tauri/Cargo.toml` | 修改 | 无需额外依赖（使用已有的 `chrono`, `serde_json`） |
| `src/settings/storage.ts` | 修改 | `SyncSettings` 新增 `syncFolder`，`AppSettings` 新增 `autoBackup*` |
| `src/components/SettingsPanel.tsx` | 修改 | 文件夹选择器、自动备份 UI、OneDrive 同步面板 |
| `src/App.tsx` | 修改 | 同步命令调用、自动备份定时器 |
| `src/styles.css` | 修改 | OneDrive 同步 UI 样式 |

### 7.3 新增的 Tauri 命令

| 命令 | 参数 | 返回值 | 说明 |
|------|------|--------|------|
| `local_sync_status` | `folder_path: String` | `SyncStatus` | 查询同步文件夹状态 |
| `sync_local_now` | `folder_path: String` | `SyncResult` | 执行双向同步 |
| `check_cloud_readiness` | `folder_path: String` | `CloudSyncReadiness` | 检查是否可以安全同步 |

### 7.4 Tauri allowlist 新增

```json
{
  "allowlist": {
    "dialog": { "all": false, "open": true }
  }
}
```

---

*文档完*
