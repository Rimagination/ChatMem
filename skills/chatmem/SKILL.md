---
name: chatmem
description: Use when entering a repository that relies on ChatMem, resuming interrupted local agent work, continuing a project in another agent, searching prior repository context, creating durable memory candidates, or preparing a handoff.
---

# ChatMem

## Core Principle

ChatMem is an MCP-first memory and continuation layer. Approved memory is the source of truth; generated wiki pages are readable projections rebuilt from approved memory and episodes. This skill is only the operating guide: use it to decide when and how to call the ChatMem MCP server.

Do not ask the user to paste full historical transcripts when ChatMem can provide repo memory, checkpoints, handoffs, or targeted history.

Do not assume ChatMem appears as an `@chatmem` mention in the chat UI. For agents, ChatMem should normally be reached through MCP tools; the desktop app is the human-facing conversation and memory control surface.

## When to Use

Use this skill when the user:

- asks to continue, resume, migrate, or hand off work between Claude, Codex, Gemini, or another local agent
- pastes a ChatMem continuation prompt from the desktop app
- asks what was done earlier in the current repository
- needs project rules, prior decisions, commands, gotchas, or recent work context
- wants to create durable memory from the current conversation

Do not use it for general web search, unrelated memory, or one-off notes that will not help future work in the same repo.

## MCP-First Workflow

1. Identify the repo root from the current workspace, user prompt, or ChatMem continuation prompt.
2. Call `get_repo_memory` for the repo before substantial work.
3. If the goal is continuation, inspect the freshest checkpoint or handoff returned by ChatMem before searching raw history.
4. Call `search_repo_history` only for specific gaps: prior decisions, commands, key files, errors, or earlier attempts. It uses hybrid keyword/vector search, so concise semantic queries such as "cloud backup sync" or "release signing" are acceptable.
5. Use the smallest useful context. Prefer approved memory, generated wiki pages, checkpoints, handoffs, and targeted search results over replaying raw conversation logs.
6. Use `list_repo_wiki_pages` or `rebuild_repo_wiki` when the user asks for a readable project wiki, commands, gotchas, or recent-work pages. Treat those pages as generated projections, not editable source material.
7. Use `list_entity_graph` when a task depends on related systems, symbols, protocols, agents, or release concepts.
8. Use `list_memory_conflicts` before approving a surprising candidate, especially when it negates an existing command or convention.
9. When a stable fact should survive this thread, call `create_memory_candidate` with concise text and evidence.
10. Before another agent continues the task, call `build_handoff_packet` instead of asking the user to copy the full conversation.

## Continuation Prompts

The ChatMem desktop app may copy a short prompt like:

```text
Use ChatMem to load memory for this project:
repo: D:\VSP
conversation: 019dab66-4427-7d93-a7dc-6cb90a1a4a74
Prefer ChatMem MCP over pasted history.
```

When you see this, do not treat it as context by itself. Treat it as an instruction to call ChatMem MCP for the repo and optional conversation id.

## Memory Candidate Rules

Good candidates are durable, repo-scoped, and useful at startup:

- commands that must be run before packaging or release
- repo conventions, architectural decisions, or gotchas
- known compatibility rules
- stable user preferences for this repo
- repeatable handoff or verification requirements
- sync/release caveats that affect future local work

Bad candidates:

- secrets, tokens, credentials, or private account details
- full conversation summaries
- temporary TODO lists
- speculative ideas without evidence
- edits directly to generated wiki output instead of creating or approving memory
- personal chatter or one-off debugging noise

## WebDAV Sync Rule

The desktop settings screen owns WebDAV credentials and cloud upload. MCP tools do not silently write to the user's cloud storage. When a user expects files to appear in their netdisk, the app must run the explicit WebDAV sync action, which creates the remote `chatmem` folder and uploads JSON conversation snapshots plus a manifest.

## Retrieval Rule

Search history is indexed as FTS plus vectors. Local `chatmem-local-hash-v1` vectors are always kept as a fallback. When `CHATMEM_EMBEDDING_PROVIDER=openai-compatible` and the matching base URL/model/dimensions/API key environment variables are present, search can use real provider embeddings stored side-by-side with the local fallback. Call `rebuild_repo_embeddings` after changing embedding provider settings. Treat vector hits as retrieval candidates, not final truth: prefer approved memories and evidence refs for durable facts, and verify stale or surprising matches before acting.

## Extraction And Conflict Rule

ChatMem may auto-create pending candidates only from explicit durable-memory wording such as "Remember:", "Rule:", "Gotcha:", "Always", or "Do not". Auto-extracted candidates are not approved automatically. Conflicts are review signals attached to candidates when new wording appears to negate an active approved memory.

## Handoff Rules

- Use a checkpoint when the same agent or user wants to resume current work with minimal drift.
- Use a handoff packet when another agent needs a concise transfer asset.
- Choose a target profile that matches the next agent when available.
- Include current goal, completed work, next steps, key files, useful commands, risks, and related memory ids.
- Keep handoffs compact; they should replace raw transcript transfer, not become another transcript.

## If MCP Is Unavailable

If ChatMem MCP tools are not available:

1. Say that ChatMem MCP is not currently callable in this environment.
2. Ask the user for the short ChatMem continuation prompt or plain memory export from the desktop app.
3. Use that export as a fallback only for this turn.
4. Do not invent memories, checkpoints, handoffs, or prior decisions.

Avoid telling the user to paste an entire historical conversation unless there is no other workable path.

## User-Facing Behavior

When you use ChatMem, be brief:

- "I will load the repo memory first."
- "I found a recent handoff; I will continue from that."
- "The memory is stale, so I will verify before relying on it."
- "I will create a memory candidate for this durable rule."

Do not expose internal memory mechanics unless the user asks. The user experience should feel like "continue this project", not "manage a memory database".
