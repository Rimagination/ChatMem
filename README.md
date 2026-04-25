# ChatMem

ChatMem 是一个面向 Windows 和 macOS 的桌面应用，用来浏览、搜索、恢复和迁移本地 AI 编程对话记录。当前支持 Claude、Codex 和 Gemini 三类来源。

更准确地说，ChatMem 是一个本地优先的 AI 编程项目记忆控制台：先把用户机器上已经存在的历史对话变成可检索资源，再把稳定知识沉淀为 Wiki、启动规则和交接包，让不同 agent 可以带着证据继续工作。

产品定位、记忆分层和设计参考见：

- [ChatMem Product Strategy](./docs/CHATMEM_PRODUCT_STRATEGY.md)

## 下载

发布页会提供 Windows 和 macOS 包。

Windows：

- `ChatMem_<version>_x64-setup.exe`
  推荐给普通用户的安装包。
- `ChatMem_<version>_x64_en-US.msi`
  适合偏企业化的 Windows 环境。
- `ChatMem-v<version>-portable.zip`
  免安装便携版。

macOS：

- `ChatMem-v<version>-macOS-Apple-Silicon.dmg`
  推荐给 M1 / M2 / M3 / M4 等 Apple Silicon Mac 用户。
- `ChatMem-v<version>-macOS-Intel.dmg`
  推荐给 Intel Mac 用户。
- `ChatMem_<version>_<arch>.dmg`
  Tauri 自动生成的原始 dmg，内容与上面对应的用户版 dmg 相同。
- `ChatMem_<arch>.app.tar.gz`
  供应用内更新使用的 macOS updater 包。

不知道自己的 Mac 属于哪一种时，点屏幕左上角苹果菜单，选择“关于本机”。如果显示“芯片 Apple M1/M2/M3/M4”，下载 Apple Silicon 版；如果显示“处理器 Intel”，下载 Intel 版。Apple Silicon 电脑也可能通过 Rosetta 运行 Intel 版，但不推荐，优先下载 Apple Silicon 版。

当前 macOS 包暂未做 Apple Developer ID 签名和 notarization。首次打开时，系统可能需要你在“系统设置”中允许打开，或者通过右键菜单打开。

正式下载入口：

- [GitHub Releases](https://github.com/Rimagination/ChatMem/releases)

## 主要功能

- 对话列表浏览与全文搜索
- 对话详情、工具调用和文件变更查看
- 一键复制会话文件位置与恢复命令
- Claude / Codex / Gemini 之间的对话迁移
- 本地历史索引、项目 Wiki、启动规则和 agent 交接包
- 简体中文 / English 切换
- 应用内检查更新

## 设计参考与致谢

ChatMem 参考了许多记忆、agent、知识库和代码 Wiki 项目的思路，但不是任何单一项目的复刻。相关方向包括 mem0、Letta / MemGPT、Zep、Cognee、LangGraph / LangMem、LLM Wiki / DeepWiki / CodeWiki，以及 OpenAI / Claude 原生记忆。完整说明见 [ChatMem Product Strategy](./docs/CHATMEM_PRODUCT_STRATEGY.md#设计参考与致谢)。

## ChatMem MCP

ChatMem 现在可以作为 Codex 的本地 MCP 记忆服务使用，用来给仓库工作流提供：

- 仓库启动记忆
- 全量本地历史导入、当前项目扫描和路径别名修复
- 历史工作搜索
- 记忆候选沉淀
- agent handoff 交接包

推荐查看完整说明：

- [ChatMem MCP Setup](./docs/CHATMEM_MCP_SETUP.md)

## Codex / Agent 接入

桌面应用负责查看、搜索、迁移和管理本地对话；MCP 负责让 Codex 这类 agent 读取项目记忆、搜索历史、生成交接包。两者可以一起用，也可以只用桌面应用。

如果要让 Codex 调用 ChatMem，先构建本地 MCP：

```powershell
cd C:\path\to\ChatMem\src-tauri
cargo build --release --bin chatmem-mcp
```

然后把下面的配置加入 `%USERPROFILE%\.codex\config.toml`，注意把路径换成你自己的 ChatMem 仓库位置：

```toml
[mcp_servers.chatmem]
command = "powershell"
args = [
  "-NoProfile",
  "-ExecutionPolicy",
  "Bypass",
  "-File",
  "C:\\path\\to\\ChatMem\\mcp\\run-chatmem-mcp.ps1",
]
startup_timeout_sec = 20
tool_timeout_sec = 120
enabled = true
```

改完后完全退出并重新打开 Codex。ChatMem 不会出现在 `@chatmem` 这种对话提及列表里，它是 agent 后台可调用的 MCP 工具。

Skill 是可选增强：MCP 提供工具，skill 告诉 agent 什么时候应该读取记忆、什么时候应该生成交接包。安装方式是把 `skills/chatmem` 复制到 `%USERPROFILE%\.codex\skills\chatmem`。

可以在新线程里这样提示 agent：

```text
Use ChatMem to load repo memory for D:\your\repo, then continue from the latest checkpoint or handoff if one exists.
```

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

发布由 GitHub Actions 处理。推送形如 `v0.1.8` 的 tag 后，工作流会自动：

- 构建 Windows NSIS 安装包
- 构建 Windows MSI 安装包
- 生成 updater 所需的 `latest.json` 和签名文件
- 生成 Windows 便携版 zip
- 构建 macOS dmg 和 app updater 包
- 额外上传面向用户下载的 macOS Apple Silicon / Intel dmg 文件名
- 上传所有发布资产到 GitHub Release

## 更新机制

应用内更新依赖 Tauri updater，更新源指向：

- `https://github.com/Rimagination/ChatMem/releases/latest/download/latest.json`

第一次接入发布前，需要先在 GitHub 仓库里配置：

- `TAURI_PRIVATE_KEY`
- `TAURI_KEY_PASSWORD`

细节见 [DEVELOPMENT.md](./DEVELOPMENT.md)。
