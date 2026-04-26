# ChatMem MCP Setup

ChatMem has two integration surfaces:

- MCP: the actual local tools for memory, history, checkpoints, and handoffs
- Skill: a thin operating guide that tells agents when and how to use the MCP tools

There is no local plugin wrapper. ChatMem stays intentionally simple here: one MCP server plus one lightweight skill/instruction surface.

The desktop app now includes an Agent integration installer. Manual config is still useful for development and debugging.

## Recommended Setup

Open ChatMem and go to:

```text
Settings -> Agent integration -> Install all
```

This installs both surfaces for Claude Code, Codex, Gemini CLI, and OpenCode when their user-level config locations are available. The installer writes backups such as `.bak-YYYYMMDD-HHMMSS` before changing existing config files.

Installed app builds use:

```powershell
ChatMem.exe --mcp
```

That keeps MCP launch stable after app upgrades because the agent points at the installed ChatMem executable instead of a development checkout.

The installer now treats "installed" as more than "MCP exists". Every supported agent gets an explicit recall rule in the native place it reads at startup:

- Claude: `C:\Users\Liang\.claude\CLAUDE.md` plus `C:\Users\Liang\.claude\skills\chatmem\SKILL.md`
- Codex: `C:\Users\Liang\.codex\AGENTS.md` plus the official `C:\Users\Liang\.agents\skills\chatmem\SKILL.md`; the older `C:\Users\Liang\.codex\skills\chatmem\SKILL.md` path is also written for desktop compatibility
- Gemini: `C:\Users\Liang\.gemini\GEMINI.md`
- OpenCode: `C:\Users\Liang\.config\opencode\AGENTS.md` plus `C:\Users\Liang\.config\opencode\skills\chatmem\SKILL.md`

Those rule files tell the agent to check ChatMem before asking the user to redescribe a topic that may already exist in local history.

OpenCode needs one extra nudge beyond MCP. The installer writes:

- `C:\Users\Liang\.config\opencode\opencode.json`: MCP server, `chatmem_*` tool enablement, and `chatmem` skill permission
- `C:\Users\Liang\.config\opencode\skills\chatmem\SKILL.md`: the ChatMem skill
- `C:\Users\Liang\.config\opencode\AGENTS.md`: a small global rule that tells OpenCode to load ChatMem for recall, continuation, migration, and memory questions

Without `AGENTS.md`, OpenCode may have the skill installed but still answer from the current conversation only.

## Build the MCP Binary

From the repo root:

```powershell
cd D:\VSP\agentswap-gui\src-tauri
cargo build --release --bin chatmem-mcp
```

Expected output:

- `D:\VSP\agentswap-gui\src-tauri\target\release\chatmem-mcp.exe`

The launcher also checks `.tauri-target-build\release\chatmem-mcp.exe` for release builds that use a custom Cargo target directory.

## Repo MCP Config

The repo-level MCP config lives at:

- `D:\VSP\agentswap-gui\.mcp.json`

It starts ChatMem with:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\mcp\run-chatmem-mcp.ps1
```

The launcher lives at:

- `D:\VSP\agentswap-gui\mcp\run-chatmem-mcp.ps1`

Environment overrides:

- `CHATMEM_REPO_ROOT`: override the repo root used to find the binary
- `CHATMEM_MCP_BIN`: override the exact MCP binary path

## Codex App Config

Codex App can also read MCP servers from `config.toml`.

User-level path:

- `C:\Users\Liang\.codex\config.toml`

Example:

```toml
[mcp_servers.chatmem]
command = "C:\\Program Files\\ChatMem\\ChatMem.exe"
args = ["--mcp"]
startup_timeout_sec = 20
tool_timeout_sec = 120
enabled = true
```

After changing MCP config, fully quit Codex App and open it again.

## Skill

The ChatMem skill lives at:

- `D:\VSP\agentswap-gui\skills\chatmem\SKILL.md`

The skill does not replace MCP. It only teaches the agent to:

- ask once whether to load compact project recall when the user has not explicitly asked for recall or continuation
- call `get_project_context` before substantial repo work, using `intent="startup"`, `intent="recall"`, or `intent="continue_work"` as appropriate, and start with `limit=3` for a low-token first pass
- run `import_all_local_history` when a fresh install or suspicious recall miss suggests local history has not been indexed yet
- run `scan_repo_conversations` and, when the diagnostic clearly points at the same project, `merge_repo_alias` to repair cwd/path drift
- treat approved memory as durable guidance and history hits as evidence that may still need verification
- search targeted history with `search_repo_history`, which now uses hybrid keyword/vector retrieval; start with `limit<=3`, summarize source agent/conversation evidence, and ask before expanding
- read a found conversation with `read_history_conversation` only after the user asks to read or expand that history hit
- inspect related concepts with `list_entity_graph`
- review contradictory pending candidates with `list_memory_conflicts`
- create durable candidates with `create_memory_candidate`
- draft memory rewrites with `propose_memory_merge` when a candidate should update an approved memory
- use checkpoints and handoff packets instead of raw transcript transfer
- avoid assuming ChatMem appears as an `@chatmem` chat mention

Language convention:

- For Chinese-speaking users, write durable memory titles, values, usage hints, merge proposals, checkpoints, and handoffs in Chinese.
- Preserve exact technical tokens in English, including commands, paths, function names, config keys, model names, and MCP tool names.
- Prefer mixed wording such as: `跨 agent 记忆依赖 repo_root 归一化；继续使用 canonical_repo_root 匹配 .git 根目录。`

## Tool Surface

The core MCP tools include:

- `get_project_context`
- `get_repo_memory`
- `get_repo_memory_health`
- `import_all_local_history`
- `scan_repo_conversations`
- `merge_repo_alias`
- `search_repo_history`
- `read_history_conversation`
- `list_entity_graph`
- `create_memory_candidate`
- `propose_memory_merge`
- `list_memory_candidates`
- `list_memory_conflicts`
- `build_handoff_packet`
- `create_checkpoint`
- `list_active_runs`
- `list_run_artifacts`
- `resume_from_checkpoint`

## Smoke Test

For installed builds:

```powershell
& "C:\Program Files\ChatMem\ChatMem.exe" --mcp
```

For development builds, build the binary, then run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File D:\VSP\agentswap-gui\mcp\run-chatmem-mcp.ps1
```

If the process stays alive and waits on stdio, the MCP server is healthy.

## Usage Prompt

Use a short prompt like this in a new agent thread:

```text
Use ChatMem to load project context for D:\VSP\agentswap-gui with intent continue_work, then continue from the latest checkpoint or handoff if one exists.
```

Do not paste full historical transcripts unless MCP is unavailable and there is no smaller memory export.

Low-token recall protocol:

1. Ask: `要我先用 ChatMem 低成本回忆一下这个项目吗？我会只加载启动规则、最近交接和少量相关历史，不展开完整对话。`
2. If the user agrees, call `get_project_context` with the right intent and `limit=3`.
3. If plausible history appears, summarize source agent, conversation title/date, and evidence label, then ask whether to read a specific hit with `read_history_conversation`.
4. If startup rules miss, say so clearly and run targeted `search_repo_history` instead of concluding that no prior discussion exists.
5. Never ask the user to redescribe the topic while there are plausible local-history conversation hits. Say that startup rules missed but indexed history found conversation evidence, then offer to read the relevant conversation.
