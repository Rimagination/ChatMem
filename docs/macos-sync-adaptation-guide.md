# ChatMem v1.1.3 macOS 端完整开发指南

> 本文档由 Windows 端开发者编写，供 macOS 端开发者参考实现对等功能。
> Windows 端 PR：https://github.com/Rimagination/ChatMem/pull/13

---

## 目录

1. [文件名编码（同步 error 123 修复）](#1-文件名编码)
2. [同步自动导入远程对话](#2-同步自动导入)
3. [来源视图显示同步对话](#3-来源视图增强)
4. [对话读取回退到同步文件夹](#4-对话读取回退)
5. [机器分组功能](#5-机器分组)
6. [系统托盘（macOS dock 行为）](#6-系统托盘)
7. [设置持久化补充字段](#7-设置持久化)
8. [验证清单](#8-验证清单)

---

## 1. 文件名编码

**文件**：`src-tauri/src/local_sync.rs`

**问题**：ZCode 的 `encode_zcode_cli_id()` 生成含冒号的 ID（如 `claude:task:xxx:yyy`）。macOS 允许冒号，但 Windows 不允许。两端必须使用同一编码规则，否则同步文件名不一致。

**添加两个函数**：

```rust
pub fn id_to_filename(id: &str) -> String {
    let mut out = String::with_capacity(id.len());
    for ch in id.chars() {
        match ch {
            ':' => out.push_str("&#x3a;"),
            '<' => out.push_str("&#x3c;"),
            '>' => out.push_str("&#x3e;"),
            '"' => out.push_str("&#x22;"),
            '|' => out.push_str("&#x7c;"),
            '?' => out.push_str("&#x3f;"),
            '*' => out.push_str("&#x2a;"),
            '/' => out.push_str("&#x2f;"),
            '\\' => out.push_str("&#x5c;"),
            _ => out.push(ch),
        }
    }
    out
}

pub fn filename_to_id(name: &str) -> String {
    name.replace("&#x3a;", ":")
        .replace("&#x3c;", "<")
        .replace("&#x3e;", ">")
        .replace("&#x22;", "\"")
        .replace("&#x7c;", "|")
        .replace("&#x3f;", "?")
        .replace("&#x2a;", "*")
        .replace("&#x2f;", "/")
        .replace("&#x5c;", "\\")
}
```

**调用位置（3 处）**：

1. `bidirectional_sync` 写入时（2 处 `fs::write`）：
   ```rust
   let safe_name = id_to_filename(id);
   let file_path = conversations_dir.join(agent).join(format!("{safe_name}.json"));
   ```

2. `read_remote_conversations` 读取时：
   ```rust
   let id = filename_to_id(&file_name);
   remote.insert((agent.to_string(), id), (updated_at, body));
   ```

同时将 `read_remote_conversations` 改为 `pub`。

---

## 2. 同步自动导入

**文件**：`src-tauri/src/main.rs` — `sync_local_now` 函数

**问题**：`bidirectional_sync` 只上传本地对话，不导入远程对话到记忆库。

**在 `bidirectional_sync` 返回后添加**：

```rust
let result = local_sync::bidirectional_sync(&items, &path).map_err(|e| e.to_string())?;

if result.downloaded > 0 {
    if let Ok(store) = open_memory_store() {
        let remote = local_sync::read_remote_conversations(&path);
        let mut local_ids: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
        for item in &items {
            local_ids.insert((item.agent.clone(), item.id.clone()));
        }
        for ((agent, id), (_updated_at, body)) in &remote {
            if local_ids.contains(&(agent.clone(), id.clone())) {
                continue;
            }
            match serde_json::from_slice::<Conversation>(body) {
                Ok(conversation) => {
                    if let Err(e) = sync_conversation_into_store(&store, agent, &conversation) {
                        eprintln!("Warning: failed to import synced {agent}/{id}: {e}");
                    }
                }
                Err(e) => {
                    eprintln!("Warning: failed to deserialize synced {agent}/{id}: {e}");
                }
            }
        }
    }
}

Ok(result)
```

---

## 3. 来源视图增强

### 3a. MemoryStore 新增方法

**文件**：`src-tauri/src/chatmem_memory/store.rs`

```rust
pub fn list_store_conversations(
    &self,
    source_agent: &str,
) -> Result<Vec<(String, String, Option<String>, String, String, usize)>> {
    let conn = self.conn()?;
    let mut stmt = conn.prepare(
        "SELECT c.source_conversation_id, r.repo_root, c.summary, c.started_at, c.updated_at,
                (SELECT COUNT(*) FROM messages m WHERE m.conversation_id = c.conversation_id) as msg_count
         FROM conversations c
         JOIN repos r ON c.repo_id = r.repo_id
         WHERE c.source_agent = ?1
         ORDER BY c.updated_at DESC",
    )?;
    let rows = stmt
        .query_map([source_agent], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, usize>(5)?,
            ))
        })?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}
```

### 3b. 修改 list_conversations

**文件**：`src-tauri/src/main.rs`

```rust
async fn list_conversations(agent: String) -> Result<Vec<ConversationSummaryResponse>, String> {
    let adapter = get_adapter(&agent)?;
    let mut seen_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut results: Vec<ConversationSummaryResponse> = Vec::new();

    if adapter.is_available() {
        if let Ok(conversations) = adapter.list_conversations() {
            for summary in conversations {
                seen_ids.insert(summary.id.clone());
                results.push(convert_summary(summary));
            }
        }
    }

    if let Ok(store) = open_memory_store() {
        if let Ok(store_convs) = store.list_store_conversations(&agent) {
            for (source_id, repo_root, summary, started_at, updated_at, msg_count) in store_convs {
                if seen_ids.contains(&source_id) { continue; }
                seen_ids.insert(source_id.clone());
                results.push(ConversationSummaryResponse {
                    id: source_id,
                    source_agent: agent.clone(),
                    project_dir: repo_root,
                    created_at: started_at,
                    updated_at,
                    summary,
                    message_count: msg_count,
                    file_count: 0,
                });
            }
        }
    }

    Ok(results)
}
```

---

## 4. 对话读取回退

**文件**：`src-tauri/src/main.rs` — `read_conversation`

适配器读不到时，从同步文件夹读取：

```rust
let conversation = match adapter.read_conversation(&id) {
    Ok(mut conv) => {
        conv.project_dir = normalize_project_dir(&conv.project_dir);
        conv
    }
    Err(_) => {
        let settings = read_app_settings_from_disk()?;
        let sync_folder = settings.as_ref().map(|s| s.sync.sync_folder.clone()).unwrap_or_default();
        if sync_folder.is_empty() {
            return Err(format!("Conversation {id} not found"));
        }
        let safe_name = local_sync::id_to_filename(&id);
        let file_path = std::path::PathBuf::from(&sync_folder)
            .join("conversations").join(&agent).join(format!("{safe_name}.json"));
        if !file_path.exists() {
            return Err(format!("Conversation {id} not found"));
        }
        let body = std::fs::read(&file_path).map_err(|e| format!("Read error: {e}"))?;
        serde_json::from_slice::<Conversation>(&body).map_err(|e| format!("Parse error: {e}"))?
    }
};
```

---

## 5. 机器分组

**文件**：`src/App.tsx`, `src/settings/storage.ts`, `src/styles.css`

### 5a. 检测函数（`src/App.tsx`）

```typescript
function detectMachineId(projectDir: string): string {
  const normalized = projectDir.replace(/\\/g, "/");
  if (/^[a-zA-Z]:\//.test(normalized)) return "windows";
  if (/^\/(Users|Volumes|Applications)\//i.test(normalized) || normalized === "/Applications") return "macos";
  if (/^\/(home|root|usr|opt|tmp)\//.test(normalized)) return "linux";
  if (normalized.startsWith("chatmem://")) return "internal";
  return "other";
}
```

**注意**：不要按用户名拆分！同一台 Mac 上 `/users/alvis` 和 `/volumes/douxy` 是同一台机器。

### 5b. 设置字段（`src/settings/storage.ts`）

```typescript
// AppSettings 类型中添加：
machineGroupNames: Record<string, string>;
machineGroupOverrides: Record<string, string>;  // project_dir → machine_id

// 默认值：
machineGroupNames: {},
machineGroupOverrides: {},
```

### 5c. 分组逻辑（`src/App.tsx`）

- `machineGroups` memo：按 `machineGroupOverrides[fullPath] || detectMachineId(fullPath)` 分组
- 自动标签：一台→"Windows"/"Mac"，多台→"Windows-1"/"Windows-2"
- 仅 `machineGroups.length > 1` 时渲染分组层

### 5d. 合并/移动功能

- `handleMergeMachineGroups(targetId)`：将选中分组的所有对话 override 到目标
- `handleMoveConversations(targetId)`：将选中对话 override 到目标分组
- `handleResetGroupOverrides()`：清空所有 override

### 5e. UI

- 管理分组按钮（侧边栏 action 区域）
- action bar：已选状态 + 合并/移动/选择全部/重置 按钮
- 复选框：机器分组头部 + 对话行
- 双击分组名称可重命名

### 5f. CSS（`src/styles.css`）

使用浅色主题变量：`--bg-surface`、`--bg-soft-strong`、`--text-primary`、`--border-soft`、`--accent`、`--shadow-float` 等。不要使用深色变量如 `#1e1e2e`、`#2a2a3a`。

---

## 6. 系统托盘

**文件**：`src-tauri/src/main.rs`, `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`

### Cargo.toml
```toml
tauri = { version = "1", features = [..., "system-tray"] }
```

### tauri.conf.json
```json
"systemTray": {
  "iconPath": "icons/icon.icns",
  "iconAsTemplate": true
}
```

### main.rs
```rust
use tauri::Manager;

fn main() {
    let tray_menu = tauri::SystemTrayMenu::new()
        .add_item(tauri::CustomMenuItem::new("open", "打开主界面").accelerator("Cmd+Shift+M"))
        .add_native_item(tauri::SystemTrayMenuItem::Separator)
        .add_item(tauri::CustomMenuItem::new("sync", "同步"))
        .add_native_item(tauri::SystemTrayMenuItem::Separator)
        .add_item(tauri::CustomMenuItem::new("quit", "退出"));

    let system_tray = tauri::SystemTray::new().with_menu(tray_menu);

    // ... on_system_tray_event 处理 open/sync/quit
    // ... on_window_event: CloseRequested → hide + prevent_close（所有平台）
}
```

---

## 7. 设置持久化补充字段

**文件**：`src-tauri/src/main.rs` — `SyncSettingsPayload` / `AppSettingsPayload`

```rust
struct SyncSettingsPayload {
    // ... 现有字段
    #[serde(default)]
    sync_folder: String,  // 新增
}

struct AppSettingsPayload {
    // ... 现有字段
    #[serde(default)]
    auto_backup_enabled: bool,  // 新增
    #[serde(default = "default_auto_backup_interval")]
    auto_backup_interval_minutes: i64,  // 新增
}

fn default_auto_backup_interval() -> i64 { 30 }
```

---

## 8. 验证清单

- [ ] `cargo check` 通过
- [ ] `npm run build` 通过
- [ ] 文件名编码：ZCode 对话 ID 中的冒号被编码为 `&#x3a;`
- [ ] 同步后远程对话出现在来源视图中
- [ ] 点击远程对话能正常查看详情
- [ ] 机器分组：单台不显示，多台显示
- [ ] 机器分组：双击可重命名
- [ ] 机器分组：合并/移动功能正常
- [ ] 系统托盘：关闭→最小化到 dock，dock 右键退出
- [ ] 设置持久化：重启后 syncFolder/autoBackup 设置保留

---

## Windows 端对应 PR

https://github.com/Rimagination/ChatMem/pull/13

包含所有上述功能的完整实现，可直接对照代码。
