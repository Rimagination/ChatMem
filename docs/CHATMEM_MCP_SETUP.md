# ChatMem MCP Setup

This document covers the working ChatMem setup for Codex App after the MCP schema fix and the additive control-plane surfaces.

## What ChatMem Is

ChatMem is best used as a local MCP server that gives Codex repository memory tools and a small local control-plane layer.

The long-lived MCP surface includes the original repository memory tools plus checkpoint creation from the earlier checkpoint phase:

- `get_repo_memory`
- `search_repo_history`
- `create_memory_candidate`
- `list_memory_candidates`
- `build_handoff_packet`
- `create_checkpoint`

Task 6 adds additive control-plane tools on top of that existing surface:

- `list_active_runs`
- `list_run_artifacts`
- `resume_from_checkpoint`

The original five MCP tools remain supported, and `create_checkpoint` continues to work as the pre-existing checkpoint entry point. The Task 6 additions can be adopted incrementally without breaking existing clients.

In Codex App, this is an MCP integration first. Do not rely on the local plugin marketplace flow as the primary installation path.

## Build the MCP Binary

From the repo root:

```powershell
cd D:\VSP\agentswap-gui\src-tauri
cargo build --release --bin chatmem-mcp
```

Expected output:

- `D:\VSP\agentswap-gui\src-tauri\target\release\chatmem-mcp.exe`

## Recommended Codex App Setup

Codex App reads MCP configuration from `config.toml`.

### User-level config

Path:

- `C:\Users\Liang\.codex\config.toml`

Add:

```toml
[mcp_servers.chatmem]
command = "powershell"
args = [
  "-NoProfile",
  "-ExecutionPolicy",
  "Bypass",
  "-File",
  "D:\\VSP\\plugins\\chatmem\\scripts\\run-chatmem-mcp.ps1",
]
startup_timeout_sec = 20
tool_timeout_sec = 120
enabled = true
```

### Project-level config

Path:

- `D:\VSP\.codex\config.toml`

Add the same block there if you want the workspace to carry its own ChatMem MCP config.

## Launcher Script

The launcher used by Codex lives at:

- `D:\VSP\plugins\chatmem\scripts\run-chatmem-mcp.ps1`

It resolves the repo root from `repo-root.txt`, then tries:

1. `src-tauri\target\release\chatmem-mcp.exe`
2. `src-tauri\target\debug\chatmem-mcp.exe`

You can override the binary path with:

- `CHATMEM_MCP_BIN`

You can override the repo root with:

- `CHATMEM_REPO_ROOT`

## Restart Requirement

After changing MCP config, fully quit Codex App and open it again.

If ChatMem still does not appear, verify the binary starts cleanly:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File D:\VSP\plugins\chatmem\scripts\run-chatmem-mcp.ps1
```

If the process stays alive and waits on stdio, the MCP server is healthy.

## Control-Plane Surfaces

ChatMem now exposes three local surfaces:

- Desktop app for human review and operational visibility
- MCP for host integration and automation
- A2A-lite metadata for describing ChatMem as a local-first control plane

The metadata surface is intentionally lightweight. It does not introduce a network transport or a distributed A2A protocol.

## How To Use ChatMem In Codex

Once enabled, use ChatMem through natural prompts. The easiest pattern is to ask Codex to call the tools for you.

### Startup examples

Use these when opening a repo or starting a fresh thread:

- "Load ChatMem repo memory for this workspace."
- "Read the ChatMem memory for this repository before we start."
- "Search ChatMem history for previous MCP work in this repo."

### During development

Use these when you discover a reusable rule or want prior context:

- "Search ChatMem for earlier discussion about release packaging."
- "Save this conclusion as a memory candidate for the repo."
- "List pending memory candidates for this repository."
- Review proposed memories from the Codex App "Memory Inbox" before accepting them into the repo history.

### Handoff examples

Use these when switching agents or pausing work:

- "Build a ChatMem handoff packet for another agent."
- "Generate a handoff for this repo so we can resume later."

### Checkpoint and control-plane examples

Use these when you need checkpoint or Task 6 control-plane flows:

- "List active ChatMem runs for this repository."
- "Show the artifacts produced by recent runs."
- "Freeze the current state into a checkpoint."
- "Resume from checkpoint and build the next handoff packet."

## Practical Prompt Templates

### New thread template

```text
Load ChatMem repo memory for this workspace, summarize the key constraints, then continue with the task.
```

### Development template

```text
Use ChatMem while we work: read repo memory first, search prior history when needed, and save stable conclusions as memory candidates.
```

### Handoff template

```text
Create a ChatMem handoff packet for this repository with the current goal, completed items, next steps, and the key files to inspect first.
```

## Known Pitfall That Was Fixed

Earlier builds could fail to register in Codex because `list_memory_candidates` exposed an MCP output schema whose root type was `array`.

Codex expects MCP tool output schemas to use an object root. The working payload is now:

```json
{
  "candidates": []
}
```

That fix is implemented in:

- `src-tauri/src/chatmem_memory/models.rs`
- `src-tauri/src/chatmem_memory/mcp.rs`

## Optional Local Plugin Shell

The repo still contains local plugin shell files:

- `plugins/chatmem/.codex-plugin/plugin.json`
- `plugins/chatmem/.mcp.json`
- `.agents/plugins/marketplace.json`
- `~/.agents/plugins/marketplace.json`

These are useful as packaging artifacts, but the reliable Codex App setup is the MCP `config.toml` path above.
