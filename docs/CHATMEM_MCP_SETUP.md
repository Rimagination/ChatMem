# ChatMem MCP Setup

ChatMem has two integration surfaces:

- MCP: the actual local tools for memory, history, checkpoints, and handoffs
- Skill: a thin operating guide that tells agents when and how to use the MCP tools

There is no local plugin wrapper in this repo.

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
command = "powershell"
args = [
  "-NoProfile",
  "-ExecutionPolicy",
  "Bypass",
  "-File",
  "D:\\VSP\\agentswap-gui\\mcp\\run-chatmem-mcp.ps1",
]
startup_timeout_sec = 20
tool_timeout_sec = 120
enabled = true
```

After changing MCP config, fully quit Codex App and open it again.

## Skill

The ChatMem skill lives at:

- `D:\VSP\agentswap-gui\skills\chatmem\SKILL.md`

The skill does not replace MCP. It only teaches the agent to:

- call `get_repo_memory` before substantial repo work
- search targeted history with `search_repo_history`
- create durable candidates with `create_memory_candidate`
- use checkpoints and handoff packets instead of raw transcript transfer
- avoid assuming ChatMem appears as an `@chatmem` chat mention

## Tool Surface

The core MCP tools include:

- `get_repo_memory`
- `search_repo_history`
- `create_memory_candidate`
- `list_memory_candidates`
- `build_handoff_packet`
- `create_checkpoint`
- `list_active_runs`
- `list_run_artifacts`
- `resume_from_checkpoint`

## Smoke Test

Build the binary, then run:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File D:\VSP\agentswap-gui\mcp\run-chatmem-mcp.ps1
```

If the process stays alive and waits on stdio, the MCP server is healthy.

## Usage Prompt

Use a short prompt like this in a new agent thread:

```text
Use ChatMem to load repo memory for D:\VSP\agentswap-gui, then continue from the latest checkpoint or handoff if one exists.
```

Do not paste full historical transcripts unless MCP is unavailable and there is no smaller memory export.
