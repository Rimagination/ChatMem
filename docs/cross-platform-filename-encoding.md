# ChatMem 跨平台同步文件名编码规范

## 问题背景

ZCode 适配器的 `encode_zcode_cli_id()` 会生成含冒号的对话 ID，例如：

```
claude:task:06dd00ba9c77:claude-import-585747f263a1544ace36845a
```

各平台文件名限制不同：

| 字符 | Windows | macOS (HFS+/APFS) | Linux |
|------|---------|-------------------|-------|
| `:`  | ❌ 禁止 | ✅ 允许 | ✅ 允许 |
| `<`  | ❌ 禁止 | ✅ 允许 | ✅ 允许 |
| `>`  | ❌ 禁止 | ✅ 允许 | ✅ 允许 |
| `"`  | ❌ 禁止 | ✅ 允许 | ✅ 允许 |
| `\|` | ❌ 禁止 | ✅ 允许 | ✅ 允许 |
| `?`  | ❌ 禁止 | ✅ 允许 | ✅ 允许 |
| `*`  | ❌ 禁止 | ✅ 允许 | ✅ 允许 |
| `/`  | ❌ 禁止 | ❌ 禁止 | ❌ 禁止 |
| `\`  | ❌ 禁止 | ✅ 允许 | ✅ 允许 |
| NUL  | ❌ 禁止 | ❌ 禁止 | ❌ 禁止 |

**核心矛盾**：macOS 允许冒号出现在文件名中（`claude:task:xxx.json`），但 Windows 不允许。如果两个平台各自用自己的规则写文件名，双向同步时同一段对话会产生两个不同文件名，导致同步冲突或数据重复。

## 解决方案：HTML 实体编码

**原则：写入同步文件夹时，所有平台必须使用同一套编码规则。**

### 编码表（写入同步文件夹时使用）

| 原始字符 | 编码为    | 说明              |
|----------|-----------|-------------------|
| `:`      | `&#x3a;`  | 冒号（ZCode 常用）|
| `<`      | `&#x3c;`  | 小于号            |
| `>`      | `&#x3e;`  | 大于号            |
| `"`      | `&#x22;`  | 双引号            |
| `\|`     | `&#x7c;`  | 竖线              |
| `?`      | `&#x3f;`  | 问号              |
| `*`      | `&#x2a;`  | 星号              |
| `/`      | `&#x2f;`  | 正斜杠            |
| `\`      | `&#x5c;`  | 反斜杠            |

### 解码规则（读取同步文件夹时使用）

将上述编码逆向还原。编码格式为 `&#x` + 两位十六进制 + `;`。

### 示例

| 对话 ID                                              | 同步文件名                                                        |
|------------------------------------------------------|-------------------------------------------------------------------|
| `claude:task:abc:def`                                | `claude&#x3a;task&#x3a;abc&#x3a;def.json`                         |
| `012bf8f0-d5d6-481d-8274-e90a57f73024`               | `012bf8f0-d5d6-481d-8274-e90a57f73024.json`（无特殊字符，不变）     |
| `20260612_235040_90dd58`                              | `20260012_235040_90dd58.json`（无特殊字符，不变）                   |

## 需要修改的代码

### Windows 端（已完成）

文件：`src-tauri/src/local_sync.rs`

```rust
fn id_to_filename(id: &str) -> String {
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

fn filename_to_id(name: &str) -> String {
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

调用位置：
- `bidirectional_sync()` 写入文件时：`id_to_filename(id)` → `{safe_name}.json`
- `read_remote_conversations()` 读取文件时：`filename_to_id(file_stem)` → 还原为原始 ID

### macOS 端（需要修改）

需要在 `local_sync.rs` 中添加**完全相同**的 `id_to_filename()` 和 `filename_to_id()` 函数，并在相同位置调用：

1. **写入时**（`bidirectional_sync` 函数中两处 `fs::write`）：
   ```rust
   let safe_name = id_to_filename(id);
   let file_path = conversations_dir.join(agent).join(format!("{safe_name}.json"));
   fs::write(&file_path, local_body)?;
   ```

2. **读取时**（`read_remote_conversations` 函数中）：
   ```rust
   let file_name = path.file_stem().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
   let id = filename_to_id(&file_name);
   remote.insert((agent.to_string(), id), (updated_at, body));
   ```

3. **计数时**（`count_remote_conversations` 函数）：无需修改，只计算 `.json` 文件数量。

## 向后兼容性

- 已有的 `&#x3a;` 编码文件可以被新代码正确读取（`filename_to_id` 会还原为原始 ID）
- macOS 端如果之前用原始冒号写入了文件（如 `claude:task:xxx.json`），升级后：
  - **读取**：旧文件名不含 `&#x3a;`，`filename_to_id` 不会做任何替换，原始冒号保留 → 正确匹配
  - **写入**：新文件会用 `&#x3a;` 编码 → 与 Windows 一致
  - **冲突处理**：如果同一对话既有 `claude:task:xxx.json` 又有 `claude&#x3a;task&#x3a;xxx.json`，双向同步的时间戳比较会保留更新的那个，旧文件名的不会被删除但也不会重复同步

## 测试要点

1. Windows 写入 → macOS 读取：确认 `&#x3a;` 编码文件在 macOS 上能正确解码
2. macOS 写入 → Windows 读取：确认 macOS 新编码的文件在 Windows 上能正确读取
3. 混合旧文件：确认旧的冒号文件名和新的 `&#x3a;` 文件名不产生重复
4. 无特殊字符的 ID（UUID、时间戳格式）：确认不做任何变换
