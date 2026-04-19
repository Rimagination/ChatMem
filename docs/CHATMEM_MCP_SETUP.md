# ChatMem MCP Setup

## Codex App

1. Build the `chatmem-mcp` binary from `D:\VSP\agentswap-gui\src-tauri`.
2. Open this workspace in Codex App.
3. Install the local `ChatMem` plugin from `D:\VSP\agentswap-gui\.agents\plugins\marketplace.json`.
4. Start work with the prompt: "Load ChatMem repo memory for this workspace."

The Codex plugin bundles both the shared skill and the local MCP launcher. The MCP entry runs `plugins/chatmem/scripts/run-chatmem-mcp.ps1`, which looks for `chatmem-mcp.exe` in the Tauri build output or `CHATMEM_MCP_BIN`.

## Claude Code

Use `plugins/chatmem/.claude-plugin/plugin.json` as the shared skill bundle for Claude-side workflows. The MCP process is the same local `chatmem-mcp` server; Claude can point at the same launcher script if you want parity with Codex.

## Review Flow

- `get_repo_memory`: load startup memory before coding
- `search_repo_history`: pull prior episodes and commands instead of replaying transcripts
- `create_memory_candidate`: propose repo facts as pending candidates
- `build_handoff_packet`: create a cross-agent handoff packet

All candidate writes are review-gated. The desktop app exposes this in `Memory Inbox`, where a human approves, edits, rejects, or snoozes candidate memory before it becomes startup context.

## Optional Local Install

Run `scripts/sync-chatmem-plugin.ps1` to copy the Codex plugin bundle into `~/plugins/chatmem`, register it in `~/.agents/plugins/marketplace.json`, and copy the Claude bundle into `~/.claude/plugins/chatmem` for home-level use. The sync step also writes `repo-root.txt` so the copied launcher still points back at this repo checkout. Restart Codex App after syncing so the local marketplace is reloaded.
