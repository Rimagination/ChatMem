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
2. Call `get_project_context` for substantial repo work when available. Use `intent="startup"` for compact startup context, `intent="recall"` when the user asks whether something was discussed before, and `intent="continue_work"` when resuming.
3. Treat approved memory as durable project guidance. Treat history evidence as local evidence that may be stale or unapproved.
4. If `get_project_context` is unavailable, fall back to `get_repo_memory` and then call `search_repo_history` for specific gaps: prior decisions, commands, key files, errors, earlier attempts, or recall questions.
5. Use the smallest useful context. Prefer approved memory, generated wiki pages, checkpoints, handoffs, targeted history evidence, and pending candidate summaries over replaying raw conversation logs.
6. Use `list_repo_wiki_pages` or `rebuild_repo_wiki` when the user asks for a readable project wiki, commands, gotchas, or recent-work pages. Treat those pages as generated projections, not editable source material.
7. Use `list_entity_graph` when a task depends on related systems, symbols, protocols, agents, or release concepts.
8. Use `list_memory_conflicts` before approving a surprising candidate, especially when it negates an existing command or convention.
9. When a stable fact should survive this thread, call `create_memory_candidate` with concise text and evidence.
10. When a pending candidate should update an existing approved memory, use `propose_memory_merge` to submit an agent-authored rewrite proposal for human review. Do not approve or silently overwrite approved memory yourself.
11. Before another agent continues the task, call `build_handoff_packet` instead of asking the user to copy the full conversation.

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

Language rule:

- Prefer Chinese for memory titles, values, usage hints, merge proposals, checkpoints, and handoff summaries when the user works in Chinese.
- Preserve exact English identifiers such as file paths, commands, function names, env vars, config keys, model names, and API/tool names.
- A good durable memory usually reads as Chinese prose with embedded technical tokens, for example: `跨 agent 记忆依赖 repo_root 归一化；继续使用 canonical_repo_root 匹配 .git 根目录。`
- If the source evidence is entirely English and the user's language is unknown, keep the original wording instead of guessing.

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

For recall questions, never answer from `get_repo_memory` alone. If approved memory does not contain the answer, search history and clearly label matches as history evidence rather than approved memory.

## Extraction And Conflict Rule

ChatMem may auto-create pending candidates only from explicit durable-memory wording such as "Remember:", "Rule:", "Gotcha:", "Always", or "Do not". Auto-extracted candidates are not approved automatically. Conflicts are review signals attached to candidates when new wording appears to negate an active approved memory.

## Merge Proposal Rule

ChatMem does not need an internal LLM API to rewrite memory. The active agent can draft a better merge proposal through `propose_memory_merge` after reading the candidate, approved memory, conflicts, and evidence. The desktop memory inbox remains the human approval surface. If the rewrite is uncertain, ask the user instead of submitting a confident proposal.

## Handoff Rules

- Use a checkpoint when the same agent or user wants to resume current work with minimal drift.
- Use a handoff packet when another agent needs a concise transfer asset.
- Choose a target profile that matches the next agent when available.
- Include current goal, completed work, next steps, key files, useful commands, risks, and related memory ids; write prose fields in Chinese by default while preserving exact technical tokens.
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
