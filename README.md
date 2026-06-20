# ChatMem

ChatMem 是一个本地优先的 AI 编程记忆与迁移层。它会把 Claude、Codex、Gemini、OpenCode、ZCode 等本地对话历史整理成可搜索、可恢复、可迁移、可继续使用的项目上下文。

它不是另一个聊天客户端。ChatMem 解决的是 AI 编程里最容易断线的部分：换 agent、换窗口、换机器、隔几天回来，模型不知道之前发生过什么。ChatMem 会把本地对话作为证据层索引，再把稳定知识沉淀为启动规则、Wiki、checkpoint 和 handoff，并通过桌面端与 MCP 把这些上下文带回新的 agent 会话。

## 当前版本

最新版本：`v1.1.4`

### v1.1.4 重点更新

**对话阅读和性能**
- 对话详情先展示最开始的消息，长对话再通过“显示后续消息”逐步加载，避免倒序阅读。
- 小对话直接完整读取，大对话才按消息数分段加载，减少点击对话后的等待和空白详情页。
- 对话列表和消息卡片增加轻量渲染约束，降低长列表滚动卡顿。

**布局和设置**
- 收起侧边栏按钮移到顶栏 Logo 左侧，并与 Logo 中线对齐。
- 最大化窗口下保留右侧对话区的圆角边界，避免内容向屏幕外偏移。
- “关于 ChatMem”并入设置页，侧栏底部只保留收藏夹、垃圾箱、设置和版本号。

**继续、收藏和迁移**
- `Migrate` 弹窗支持“总结式迁移”，可复制 source-backed continuation brief；完整对话迁移仍保留。
- 对话工具栏继续保留低 token 续接提示，不替代原功能。
- 重要对话可以一键收藏，并在侧栏收藏夹入口集中查看。
- Release 说明见 `docs/releases/v1.1.4.md`。
- Windows 端总结式迁移实现指南见 `docs/windows-summary-migration-implementation.md`。

---

### v1.1.3 重点更新

**macOS 26 (Tahoe) 兼容修复**
- 修复在 macOS 26 上窗口完全卡死无法操作的问题
- 改用原生窗口装饰（`decorations: true`），移除自定义透明窗口和拖拽区域
- 关闭按钮改为隐藏窗口（标准 macOS 行为），dock 栏右键退出才真正退出

**OneDrive 双向同步**
- 新增本地文件夹同步功能，支持 OneDrive、Google Drive、Dropbox 等任意同步目录
- 双向合并算法：按 `updated_at` 时间戳自动判断上传/下载/跳过
- 云端同步状态检测：自动识别 `.tmp`、`.partial`、`~$` 锁文件，避免与云盘客户端冲突
- 定时自动备份：可配置间隔（5/15/30/60/120 分钟），云盘忙碌时自动跳过
- 用户自选文件夹路径，通过系统原生文件夹选择器设置
- **修复 Windows error 123**：ZCode 对话 ID 含冒号，Windows 不允许冒号作文件名。写入时编码为 `&#x3a;`，读取时解码还原
- **同步自动导入**：远程对话同步后自动写入 ChatMem 记忆库，切换来源即可查看跨机器对话
- 跨平台文件名编码规范见 `docs/cross-platform-filename-encoding.md`

**系统托盘（Windows）/ Dock 行为优化（macOS）**
- 关闭按钮最小化到系统托盘（不退出应用）
- 托盘右键菜单：打开主界面 / 同步 / 退出
- 单击托盘图标恢复窗口

**Hermes Agent 支持**
- 新增 Hermes Agent 适配器，从 `~/.hermes/state.db` SQLite 数据库读取对话
- Windows 端使用 `AppData/Local/hermes/state.db`
- 设置 → Agent 集成中支持一键安装/卸载 Hermes MCP 配置和 Skill
- 修复工具调用显示 unknown：正确解析 OpenAI 格式（function.name/arguments）

**ZCode 原生集成**
- 新增 ZCode 集成支持，设置 → Agent 集成可管理
- MCP 自动安装到 `~/.zcode/v2/config.json`
- Skill 通过 skills-manager 中央仓库软链接到 `~/.zcode/skills/chatmem/`

**机器分组**
- 自动检测对话来源机器（Windows / Mac / Linux）
- 多台电脑时显示机器分组层，单台时不显示
- 双击分组名称可自定义重命名，保存到设置
- 支持合并电脑分组、移动对话到其他分组

**对话来源视图增强**
- 来源视图合并本地适配器 + 记忆库数据，同步的对话在来源视图中可见
- 点击同步的对话可正常查看详情，适配器失败时自动从同步文件夹读取

**统一 Skill 管理**
- ChatMem skill 统一存放在 `~/.skills-manager/skills/chatmem/`
- Claude、Codex、Hermes、ZCode 通过软链接/Junction 共享同一份 SKILL.md
- Gemini CLI 和 OpenCode 继续作为本地历史来源保留，MCP/Skill 接入按各自原生配置方式处理

**设置持久化**
- 同步文件夹路径、自动备份开关、备份间隔等设置保存到 settings.json
- Windows：`AppData/Roaming/ChatMem/settings.json`；macOS：`~/Library/Application Support/ChatMem/settings.json`
- 重新安装后无需重新配置，设置自动恢复

**跨平台构建修复**
- macOS 专用依赖（cocoa/objc）改为平台条件依赖，Windows 编译不再报错
- Windows x64 构建通过 `npx tauri build --target x86_64-pc-windows-msvc`

### v1.1.2 重点更新

- 新增 ZCode 顶层来源：ZCode 下按 CLI 分组，CLI 下再按项目分组，支持 ZCode 内的 Claude、Codex、Gemini、OpenCode 等会话结构。
- 对话标题更贴近任务内容：优先使用用户真实输入的任务文字，而不是原始 UUID、命令提示或工具调用字符串。
- 完整对话支持 Markdown 渲染：长回答、列表、代码块、链接会以更可读的方式显示。
- 工具调用历史更安静：多个工具调用默认折叠为小字号灰色信息层，让"用户说了什么、agent 回答了什么"成为阅读重点。
- 更适合长会话延续：继续卡片会提取当前工作线、最新完成动作、权威文件和过期背景，配合低 token 历史检索、对话证据窗口、checkpoint、handoff 和 Wiki 接续，而不是重新读取整段超长对话。
- UI 层级优化：来源选择、搜索、项目/对话列表、对话操作、关于页都按 Codex 桌面端方向重新梳理，并修复右侧对话区横向溢出。

## 下载

推荐从正式 Release 下载，不要下载 GitHub 自动生成的 source code zip/tar.gz。

- [全部下载：ChatMem Releases](https://github.com/Rimagination/ChatMem/releases)
- Windows 安装包：下载发布页里的 `.exe` 安装包。
- Windows 便携版：下载 `ChatMem-v<version>-portable.zip`。
- macOS Apple Silicon：下载 `ChatMem-v<version>-macOS-Apple-Silicon.dmg`。
- macOS Intel：下载 `ChatMem-v<version>-macOS-Intel.dmg`。

不知道自己的 Mac 属于哪一种时，点屏幕左上角苹果菜单，选择“关于本机”。如果显示“芯片 Apple M1/M2/M3/M4”，下载 Apple Silicon 版；如果显示“处理器 Intel”，下载 Intel 版。

当前 macOS 包暂未做 Apple Developer ID 签名和 notarization。首次打开时，系统可能需要你在“系统设置”里允许打开，或者通过右键菜单打开。

## 支持的本地历史来源

| 来源 | ChatMem 中的层级 | 说明 |
| --- | --- | --- |
| Claude | 来源 -> 项目 -> 对话 | 解析本机 Claude Code 项目对话和子代理任务。 |
| Codex | 来源 -> 项目/本地历史 -> 对话 | 解析 Codex CLI / Codex 桌面端 rollout 与会话历史。 |
| Gemini | 来源 -> 项目 -> 对话 | 解析 Gemini CLI 本地历史。 |
| OpenCode | 来源 -> 项目 -> 对话 | 解析 OpenCode SQLite 会话库。 |
| Hermes | 来源 -> 项目 -> 对话 | 解析 Hermes Agent SQLite 数据库（`~/.hermes/state.db`）。 |
| ZCode | 来源 -> CLI -> 项目 -> 对话 | 解析 `~/.zcode/v2/acp-config`，把 ZCode 作为顶层来源，再按内部 CLI 分组。 |

ZCode Windows 默认位置示例：

```text
C:\Users\<you>\.zcode\v2\acp-config\
```

## 核心能力

- 本地对话浏览、归类、全文搜索和标题清洗
- Markdown 对话正文、工具调用折叠、文件变更查看
- 一键复制会话文件位置、恢复命令和低 token 续接提示
- Claude / Codex / Gemini / OpenCode / ZCode / Hermes 等来源的续接；可写 agent 之间支持完整对话迁移
- 删除前确认、批量选择、垃圾箱保留与恢复
- 全量本地历史导入、当前项目扫描、路径别名修复
- 低 token 历史检索、对话证据读取、Wiki 投影、启动规则
- checkpoint、handoff、run、artifact 等继续工作记录
- 设置页一键安装 ChatMem MCP 与各平台原生引导入口
- WebDAV 可选备份与同步
- 简体中文 / English 切换
- 应用内检查更新

## 自动恢复快照

ChatMem 支持自动捕获当前对话并创建恢复 checkpoint，但从 `v1.1.3` 起这是一个显式 opt-in 功能。

默认状态下，ChatMem 不会自动创建恢复快照。需要自动恢复时，可以在设置里开启“自动保存恢复快照”。这样做是为了避免刚安装或刚升级的用户在不知情时产生额外本地记忆记录。

## 推荐工作流

1. 打开 ChatMem，选择左侧来源。
2. 对 ZCode，先选 CLI 分组，再进入项目和对话；其他来源直接按项目或本地历史浏览。
3. 在对话详情里优先阅读用户消息和 agent 回复，需要时再展开工具调用。
4. 对很长的旧会话，优先复制低 token 续接提示、使用 checkpoint 或 handoff 接续，不要让新窗口整段读取超长 transcript。
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

完整说明：

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

安装版优先使用 `ChatMem.exe --mcp` 启动 MCP，这样升级后不会依赖旧仓库路径。开发模式保留 `mcp/run-chatmem-mcp.ps1` 作为手动排障入口。

## 数据与隐私

ChatMem 默认本地优先：

- 本地历史和记忆索引存放在用户机器上的 SQLite 数据库中。
- 对话原文件仍以本地 agent 的原始历史文件作为证据来源。
- WebDAV 是可选备份，不是日常检索和记忆能力的前提。
- MCP 工具会尽量返回紧凑上下文，避免把超长历史一次性塞进新窗口。
- 自动恢复快照默认关闭，必须由用户在设置中主动开启。

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

发布由 GitHub Actions 处理。推送形如 `v1.1.4` 的 tag 后，工作流会自动构建并上传：

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

更多开发和发布细节见 [DEVELOPMENT.md](./DEVELOPMENT.md)。
