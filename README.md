# ChatMem

ChatMem 是一个本地优先的 AI 编程记忆与迁移层。它把 Claude、Codex、Gemini、OpenCode、ZCode 等本地对话历史整理成可搜索、可恢复、可迁移、可继续使用的项目上下文。

它不是另一个聊天客户端。ChatMem 解决的是 AI 编程里最容易断线的部分：换 agent、换窗口、换机器、隔几天回来，模型不知道之前发生过什么。ChatMem 会把本地对话作为证据层索引，再把稳定知识沉淀为启动规则、Wiki、checkpoint 和 handoff，并通过桌面端与 MCP 把这些上下文带回新的 agent 会话。

## 当前版本

最新版本：`v1.1.0`

1.1.0 的重点更新：

- 新增 ZCode 顶层来源：ZCode 下按 CLI 分组，CLI 下再按项目分组，支持 ZCode 内的 Claude、Codex、Gemini、OpenCode 等会话结构。
- 对话标题更贴近任务内容：优先使用用户真实输入的任务文字，而不是原始 UUID、命令提示或工具调用字符串。
- 完整对话支持 Markdown 渲染：长回答、列表、代码块、链接会以更可读的方式显示。
- 工具调用历史更安静：多个工具调用默认折叠为小字号灰色信息层，让“用户说了什么、agent 回答了什么”成为阅读重点。
- 更适合长会话延续：低 token 历史检索、对话证据窗口、checkpoint、handoff 和 Wiki 可以帮助新窗口接续，而不是重新读取整段超长对话。
- UI 层级优化：来源选择、搜索、项目/对话列表、对话操作、关于页都按 Codex 桌面端方向重新梳理，并修复右侧对话区横向溢出。

## 下载

正式下载入口：

- [GitHub Releases](https://github.com/Rimagination/ChatMem/releases)

Windows 推荐下载：

- `ChatMem_<version>_x64-setup.exe`：普通用户推荐安装包。
- `ChatMem_<version>_x64_en-US.msi`：适合企业化或脚本化安装环境。
- `ChatMem-v<version>-portable.zip`：免安装便携版，如果发布页提供该文件。

macOS 推荐下载：

- `ChatMem-v<version>-macOS-Apple-Silicon.dmg`：M1 / M2 / M3 / M4 等 Apple Silicon Mac。
- `ChatMem-v<version>-macOS-Intel.dmg`：Intel Mac。

不知道自己的 Mac 属于哪一种时，点屏幕左上角苹果菜单，选择“关于本机”。如果显示“芯片 Apple M1/M2/M3/M4”，下载 Apple Silicon 版；如果显示“处理器 Intel”，下载 Intel 版。

当前 macOS 包暂未做 Apple Developer ID 签名和 notarization。首次打开时，系统可能需要你在“系统设置”中允许打开，或者通过右键菜单打开。

## 支持的本地历史来源

| 来源 | ChatMem 中的层级 | 说明 |
| --- | --- | --- |
| Claude | 来源 -> 项目 -> 对话 | 解析本机 Claude Code 项目对话和子代理任务。 |
| Codex | 来源 -> 项目/本地历史 -> 对话 | 解析 Codex CLI / Codex 桌面端 rollout 与会话历史。 |
| Gemini | 来源 -> 项目 -> 对话 | 解析 Gemini CLI 本地历史，并兼容哈希项目路径。 |
| OpenCode | 来源 -> 项目/本地历史 -> 对话 | 解析 OpenCode 本地会话、工具调用和项目路径。 |
| ZCode | 来源 -> CLI -> 项目 -> 对话 | 解析 `~/.zcode/v2/acp-config`，把 ZCode 作为顶层来源，再按内部 CLI 分组。 |

ZCode Windows 默认位置示例：

```text
C:\Users\<you>\.zcode\v2\acp-config\
```

## 核心能力

- 本地对话浏览、归类、全文搜索和标题清洗
- 对话详情、Markdown 正文、工具调用折叠、文件变更查看
- 一键复制会话文件位置与恢复命令
- Claude / Codex / Gemini / OpenCode / ZCode 之间的对话迁移
- 删除前确认、批量选择、垃圾箱保留与恢复
- 全量本地历史导入、当前项目扫描、路径别名修复
- 低 token 历史检索、对话证据读取、Wiki 投影、启动规则
- checkpoint、handoff、run、artifact 等继续工作记录
- 设置页一键安装 ChatMem MCP 与各平台原生引导入口
- WebDAV 可选备份与同步
- 简体中文 / English 切换
- 应用内检查更新

## 推荐工作流

1. 打开 ChatMem，选择左侧来源。
2. 对 ZCode，先选 CLI 分组，再进入项目和对话；其他来源直接按项目或本地历史浏览。
3. 在对话详情里重点阅读用户消息和 agent 回复，需要时再展开工具调用。
4. 对很长的旧会话，优先用本地历史检索、checkpoint 或 handoff 接续，不要让新窗口整段读取超长 transcript。
5. 在“设置 -> Agent 集成”里安装 ChatMem MCP，让 Claude Code、Codex、Gemini CLI、OpenCode 等 agent 能主动读取项目记忆。

可以在新线程里这样提示 agent：

```text
Use ChatMem to load repo memory for D:\your\repo, then continue from the latest checkpoint or handoff if one exists.
```

中文场景也可以直接说：

```text
请用 ChatMem 读取这个仓库的项目记忆，并从最近的检查点或交接包继续。
```

## ChatMem MCP

ChatMem 可以作为本地 MCP 记忆服务使用。桌面应用负责查看、搜索、迁移、审批和安装；MCP 负责让 agent 读取项目记忆、搜索历史、生成交接包。

MCP 能力包括：

- `get_project_context`：读取紧凑项目上下文、启动规则和相关历史。
- `search_repo_history`：低 token 搜索本地历史。
- `read_history_conversation`：按需读取对话证据窗口，而不是整段 transcript。
- `import_all_local_history`：导入 Claude、Codex、Gemini、OpenCode、ZCode 等本地历史。
- 记忆候选、冲突检查、规则合并、Wiki 重建、checkpoint、handoff 等工具。

推荐查看完整说明：

- [ChatMem MCP Setup](./docs/CHATMEM_MCP_SETUP.md)
- [ChatMem Architecture and Features](./docs/CHATMEM_ARCHITECTURE_AND_FEATURES.md)
- [ChatMem Product Strategy](./docs/CHATMEM_PRODUCT_STRATEGY.md)

## Agent 接入

推荐方式是在 ChatMem 桌面应用中打开“设置 -> Agent 集成”，点击“一键安装到全部”。ChatMem 会自动：

- 检测各类 agent 的用户级配置位置
- 写入 `chatmem` MCP server
- 安装 ChatMem skill 或平台等价的原生引导入口
- 在覆盖配置前生成 `.bak-YYYYMMDD-HHMMSS` 备份

安装后完全退出并重新打开对应 agent。ChatMem 通常不会出现在 `@chatmem` 这种对话提及列表里，它是 agent 后台可调用的 MCP 工具。

安装版优先使用 `ChatMem.exe --mcp` 启动 MCP，这样升级后不会依赖旧仓库路径。开发模式仍保留 `mcp/run-chatmem-mcp.ps1` 作为手动排障入口。

## 数据与隐私

ChatMem 默认本地优先：

- 本地历史和记忆索引存放在用户机器上的 SQLite 数据库中。
- 对话原文仍以本地 agent 的原始历史文件作为证据来源。
- WebDAV 是可选备份，不是日常检索和记忆能力的前提。
- MCP 工具会尽量返回紧凑上下文，避免把超长历史一次性塞进新窗口。

## 本地开发

环境要求：

- Node.js 20+
- Rust stable
- 对应平台可用的 Tauri 构建环境

常用命令：

```powershell
npm ci
npm run test:run
cargo test --manifest-path .\src-tauri\Cargo.toml
npm run tauri build
```

## 发布

发布由 GitHub Actions 处理。推送形如 `v1.1.0` 的 tag 后，工作流会自动构建并上传：

- Windows NSIS 安装包
- Windows MSI 安装包
- Windows 便携版 zip
- updater 所需的 `latest.json` 和签名文件
- macOS Apple Silicon / Intel dmg
- macOS app updater 包

应用内更新依赖 Tauri updater，更新源指向：

```text
https://github.com/Rimagination/ChatMem/releases/latest/download/latest.json
```

发布前需要在 GitHub 仓库里配置：

- `TAURI_PRIVATE_KEY`
- `TAURI_KEY_PASSWORD`

细节见 [DEVELOPMENT.md](./DEVELOPMENT.md)。
