# ChatMem

ChatMem is a local-first memory layer for AI coding agents.

It keeps project history, useful rules, handoff notes, and searchable conversation sources on your own machine, so Claude, Codex, Gemini, OpenCode, ZCode, Hermes, and other supported agents can continue work without asking you to retell the same context.

ChatMem is not another chat client. The desktop app is for browsing, searching, syncing, and cleaning up local history. Agent continuation happens through the ChatMem MCP tools and the agent setup managed from Settings.

## Current Version

Latest version: `v1.2.5`

### Highlights

- Cleaner Settings pages with a left navigation and focused right-side panels.
- Simpler memory management: suggestions and startup rules are for viewing or deleting in the desktop app; supported agents handle saving useful suggestions as rules.
- Better Agent Integration setup for Claude, Codex, Gemini, OpenCode, ZCode, and Hermes.
- Fixed duplicate Codex ChatMem entries by keeping the shared skill in `.agents/skills/chatmem`.
- Faster, clearer local-history recall through the `recall_project_work` MCP entrypoint.
- Cleaner source display in local-history recall and memory suggestion surfaces.
- Improved update and local-data checks before and after upgrades.

Release notes: [docs/releases/v1.2.5.md](./docs/releases/v1.2.5.md)

## Download

Download ChatMem from the official GitHub Releases page:

- [ChatMem Releases](https://github.com/Rimagination/ChatMem/releases)
- Windows installer: download the `.exe` installer.
- Windows portable build: download `ChatMem-v<version>-portable.zip`.
- macOS Apple Silicon: download `ChatMem-v<version>-macOS-Apple-Silicon.dmg`.
- macOS Intel: download `ChatMem-v<version>-macOS-Intel.dmg`.

Do not use GitHub's automatically generated source code zip or tarball to install the app. Those archives are for developers.

macOS builds are currently unsigned and not notarized. On first launch, macOS may ask you to allow the app from Security settings or open it from the right-click menu.

## Supported Local Sources

| Source | ChatMem view | Notes |
| --- | --- | --- |
| Claude | Source -> Project -> Conversation | Reads local Claude Code project conversations and subagent tasks. |
| Codex | Source -> Project / Local History -> Conversation | Reads Codex CLI and Codex desktop rollout/session history. |
| Gemini | Source -> Project -> Conversation | Reads Gemini CLI local history. |
| OpenCode | Source -> Project -> Conversation | Reads OpenCode SQLite conversation history. |
| Hermes | Source -> Project -> Conversation | Reads Hermes Agent SQLite history. |
| ZCode | Source -> CLI -> Project -> Conversation | Reads ZCode ACP history and groups it by CLI and project. |

## What ChatMem Does

- Browse, search, and reopen local AI coding conversations.
- Group conversations by source, machine, project, CLI, and local history.
- Keep deleted conversations recoverable through Trash.
- Import local history and link it to the current repository.
- Provide low-token recall instead of rereading full transcripts.
- Keep startup rules, Wiki context, checkpoints, handoffs, runs, and artifacts separate.
- Let supported agents query project memory through MCP.
- Install or update agent setup from Settings -> Agent Integration.
- Sync conversation data through WebDAV or a local cloud-synced folder.
- Support Simplified Chinese and English.

## Recommended Workflow

1. Open ChatMem and choose a source from the left sidebar.
2. Pick the project or local-history group you want to inspect.
3. Use search or the local-history recall box when you need to remember previous work.
4. Keep the desktop app as a review surface: view, favorite, delete, recover, or sync history.
5. In Settings -> Agent Integration, set up the agents you use.
6. In a new agent session, ask it to use ChatMem before continuing a project.

Example prompt:

```text
Use ChatMem to load repo memory for D:\your\repo, then continue from the latest checkpoint or handoff if one exists.
```

中文也可以直接说：

```text
请用 ChatMem 读取这个仓库的项目记忆，并从最近的检查点或交接记录继续。
```

## Agent Integration

The recommended setup path is:

1. Open Settings.
2. Go to Agent Integration.
3. Click Set up all, or update a single agent.
4. Restart the corresponding agent after setup.

ChatMem usually does not appear as `@chatmem` in a chat mention list. It is available to agents as an MCP tool that can be called when repository memory, local history, continuation, migration, handoff, or startup rules are needed.

Installed builds prefer `ChatMem.exe --mcp` for MCP startup, so upgrades do not depend on a development checkout path.

## MCP Tools

ChatMem can run as a local MCP memory service. Common entrypoints include:

- `recall_project_work`: compact project recall with source snippets and next actions.
- `get_project_context`: startup rules, recent handoff, local history, and pending suggestions.
- `search_repo_history`: targeted search across indexed local history.
- `read_history_conversation`: read a focused source window when a match needs more context.
- `import_all_local_history`: import supported local histories.
- `scan_repo_conversations`: scan conversations for the current repository.

More details:

- [ChatMem MCP Setup](./docs/CHATMEM_MCP_SETUP.md)
- [ChatMem Architecture and Features](./docs/CHATMEM_ARCHITECTURE_AND_FEATURES.md)
- [ChatMem Product Strategy](./docs/CHATMEM_PRODUCT_STRATEGY.md)

## Data And Privacy

ChatMem is local-first by default.

- Conversation files stay anchored to the source agent's local history.
- Search and memory indexes are stored locally in SQLite.
- WebDAV and cloud-folder sync are optional.
- MCP tools return compact context where possible instead of dumping long histories into a new agent session.
- Automatic recovery checkpoints are opt-in.

## Local Development

Requirements:

- Node.js 20+
- Rust stable
- A working Tauri build environment for your platform

Common commands:

```powershell
npm ci
npm run test:run
cargo test --manifest-path .\src-tauri\Cargo.toml
npm run tauri -- build
```

Build the MCP binary explicitly when testing MCP packaging:

```powershell
cargo build --release --bin chatmem-mcp --manifest-path .\src-tauri\Cargo.toml
```

## Release

Releases are built by GitHub Actions when a tag like `v1.2.5` is pushed.

The release workflow builds and uploads:

- Windows NSIS installer
- Windows MSI installer
- Windows portable zip
- Tauri updater metadata
- macOS Apple Silicon DMG
- macOS Intel DMG
- macOS updater package

Required repository secrets:

- `TAURI_PRIVATE_KEY`
- `TAURI_KEY_PASSWORD`

Updater endpoint:

```text
https://github.com/Rimagination/ChatMem/releases/latest/download/latest.json
```

See [DEVELOPMENT.md](./DEVELOPMENT.md) and [docs/CHATMEM_RELEASE_CHECKLIST.md](./docs/CHATMEM_RELEASE_CHECKLIST.md) for release details.
