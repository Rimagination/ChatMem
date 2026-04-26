# ChatMem

ChatMem 是一个本地优先的 AI 编程记忆层，用来把已经发生过的 Claude、Codex、Gemini 和 OpenCode 对话变成可搜索、可恢复、可迁移、可继续使用的项目上下文。

它的目标不是再做一个聊天客户端，而是解决 AI 编程里最容易断线的部分：换 agent、换会话、换机器或隔几天回来时，模型不知道之前发生过什么。ChatMem 会先索引本地历史，把对话作为证据层；再把稳定知识沉淀为启动规则、Wiki 和交接包；最后通过 MCP 与各平台的原生引导入口，把这些上下文带回新的 agent 会话。

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

- Claude / Codex / Gemini / OpenCode 本地对话浏览、归类与全文搜索
- 对话详情、工具调用和文件变更查看
- 一键复制会话文件位置与恢复命令
- 跨 agent 对话迁移与恢复
- 删除前确认、批量选择、垃圾箱保留与恢复
- 本地历史索引、项目 Wiki、启动规则、实体图和 agent 交接包
- 设置页一键安装 ChatMem MCP 与各平台原生引导
- 简体中文 / English 切换
- 应用内检查更新

## ChatMem MCP

ChatMem 可以作为本地 MCP 记忆服务使用，用来给仓库工作流提供：

- 仓库启动记忆
- 全量本地历史导入、当前项目扫描和路径别名修复
- 低 token 历史检索与对话证据读取
- 记忆候选沉淀、冲突检查和规则合并建议
- agent handoff 交接包与检查点

推荐查看完整说明：

- [ChatMem MCP Setup](./docs/CHATMEM_MCP_SETUP.md)

## Agent 接入

桌面应用负责查看、搜索、迁移和管理本地对话；MCP 负责让 Claude Code、Codex、Gemini CLI、OpenCode 这类 agent 读取项目记忆、搜索历史、生成交接包。两者可以一起用，也可以只用桌面应用。

推荐方式是在 ChatMem 桌面应用中打开“设置 -> Agent 集成”，点击“一键安装到全部”。ChatMem 会自动：

- 检测四类 agent 的用户级配置位置
- 写入 `chatmem` MCP server
- 安装 ChatMem skill 或平台等价的原生引导入口
- 在覆盖配置前生成 `.bak-YYYYMMDD-HHMMSS` 备份

安装后完全退出并重新打开对应 agent。ChatMem 通常不会出现在 `@chatmem` 这种对话提及列表里，它是 agent 后台可调用的 MCP 工具。

安装版优先使用 `ChatMem.exe --mcp` 启动 MCP，这样升级后不会依赖旧仓库路径。开发模式仍保留 `mcp/run-chatmem-mcp.ps1` 作为手动排障入口。

可以在新线程里这样提示 agent：

```text
Use ChatMem to load repo memory for D:\your\repo, then continue from the latest checkpoint or handoff if one exists.
```

## 关于 ChatMem

ChatMem 是本地优先的 AI 编程对话记忆工具。应用内的“关于我们”会放置产品说明、设计参考与致谢，避免把低频信息混进主要工作流。

ChatMem 参考了许多记忆、agent、知识库和代码 Wiki 项目的思路，但不是任何单一项目的复刻。完整设计说明见 [ChatMem Product Strategy](./docs/CHATMEM_PRODUCT_STRATEGY.md#设计参考与致谢)。

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

发布由 GitHub Actions 处理。推送形如 `v1.0.24` 的 tag 后，工作流会自动：

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
