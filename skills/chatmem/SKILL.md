---
name: chatmem
description: "Use ChatMem for repository recall, continuation, handoff, migration, startup rules, wiki/context, local history search, and durable memory. 当用户问“记得吗、之前讨论过、回忆、继续做、迁移、交接、项目历史、本地历史、启动规则、记忆”等问题时使用；先查 ChatMem MCP，不要让用户重新描述已有对话。"
---

# ChatMem

## Core Principle

ChatMem is an MCP-first local-history, startup-rule, and continuation layer. Indexed local history is the evidence layer; approved startup rules are compact durable guidance for new agent sessions; generated wiki pages are readable projections rebuilt from approved rules and episodes. This skill is only the operating guide: use it to decide when and how to call the ChatMem MCP server.

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
2. If the user did not explicitly ask for recall or continuation, ask once in the user's language before loading project memory: `要我先用 ChatMem 低成本回忆一下这个项目吗？我会只加载启动规则、最近交接和少量相关历史，不展开完整对话。`
3. If the user asks a recall/continuation question, pastes a ChatMem continuation prompt, or answers yes to the recall question, proceed without asking again.
4. If this is a fresh install, a recall question misses obvious prior discussion, or diagnostics show no indexed history, call `import_all_local_history` once before concluding history is absent.
5. Call `get_project_context` for substantial repo work when available. Use `intent="startup"` for compact startup rules, `intent="recall"` when the user asks whether something was discussed before, and `intent="continue_work"` when resuming.
6. Start with `limit=3` unless the user asks for a broad review. Read approved rules, recent handoff metadata, diagnostics, and the top relevant history summaries first; do not load or paste raw transcripts.
7. Treat approved startup rules as durable project guidance. Treat history evidence as local evidence that may be stale or unapproved. Never treat absence from approved rules as evidence that a past discussion did not happen.
8. If `get_project_context` or `get_repo_memory_health` reports unmatched project roots that clearly belong to the same repo, use `merge_repo_alias`, then run `scan_repo_conversations` and retry recall.
9. If `get_project_context` is unavailable, fall back to `get_repo_memory` and then call `search_repo_history` for specific gaps: prior decisions, commands, key files, errors, earlier attempts, or recall questions. When a hit includes `conversation_id`, use `read_history_conversation` only after the user asks you to read/expand that found conversation.
10. Use the smallest useful context. Prefer approved startup rules, generated wiki pages, checkpoints, handoffs, targeted history evidence, and pending candidate summaries over replaying raw conversation logs.
11. Use `list_repo_wiki_pages` or `rebuild_repo_wiki` when the user asks for a readable project wiki, commands, gotchas, or recent-work pages. Treat those pages as generated projections, not editable source material.
12. Use `list_entity_graph` when a task depends on related systems, symbols, protocols, agents, or release concepts.
13. Use `list_memory_conflicts` before approving a surprising candidate, especially when it negates an existing command or convention.
14. When a stable rule should be injected at future startup, call `create_memory_candidate` with concise text and evidence.
15. When a pending candidate should update an existing approved startup rule, use `propose_memory_merge` to submit an agent-authored rewrite proposal for human review. Do not approve or silently overwrite approved rules yourself.
16. Before another agent continues the task, call `build_handoff_packet` instead of asking the user to copy the full conversation.

## Low-Token Project Recall

Use this ladder whenever the user agrees to project recall, asks "do you remember...", or needs to continue prior work:

1. First call `get_project_context` with `limit=3`. Pick the intent from the user's goal:
   - `intent="startup"` for general project orientation.
   - `intent="recall"` for "did we discuss X?".
   - `intent="continue_work"` for resuming implementation.
2. If the compact context contains plausible `relevant_history`, summarize only the source agent, conversation title/date, one-line summary, and evidence label. Say that this is indexed local history, not an approved startup rule. Ask whether the user wants you to read the most relevant conversation with `read_history_conversation` before increasing the limit or requesting broader history.
3. If compact context has startup rules but no history hit, say "启动规则没有命中，我再查本地历史证据" and call `search_repo_history` with the exact topic and `limit=3`.
4. If targeted search still misses but diagnostics show indexed conversations/chunks, broaden carefully: try parent/child repo roots, scan repo conversations, or merge obvious aliases before saying history is absent.
5. Increase to `limit=10` or more only after the user asks for a broader review or after the first compact pass proves the topic is relevant.
6. Never say "we did not discuss this" only because approved startup rules are empty. Say whether approved rules, indexed history, wiki projections, or diagnostics were checked.

## Conversation Evidence Follow-Up

If `get_project_context.relevant_history` or `search_repo_history.matches` returns any plausible match:

- Do not ask the user to redescribe the topic as the next step.
- Do not say the current session has no context just because the match is not an approved startup rule. Say: "启动规则没有命中，但本地历史里找到了相关对话证据。"
- Name where it was found: source agent (`source_agent` or the prefix in `conversation_id`), conversation title, updated date, and evidence excerpt when available.
- Ask a concrete follow-up in the user's language, for example: `我在 Codex 和 Claude 的本地历史里找到了关于 Pmodel 的对话证据。要我读取最相关的那段对话，还是先列出命中的对话让你选？`
- If the user says yes, call `read_history_conversation` with the returned `conversation_id`, the evidence `message_id` when present, the original query, and a compact `limit` such as 8-12.
- If `read_history_conversation` is not available in the current MCP tool list, say the installed ChatMem MCP is older and continue with the existing search evidence: offer to list the matched conversations, run `search_repo_history` with a slightly larger limit, or ask the user to open the conversation in ChatMem. Do not ask the user to redescribe the topic.
- Only ask the user to explain the topic again after approved rules, indexed history, aliases, and fresh import/scan all miss.

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

Good candidates are durable, repo-scoped, and useful as startup rules:

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

Search history is indexed as FTS plus vectors. Local `chatmem-local-hash-v1` vectors are always kept as a fallback. When `CHATMEM_EMBEDDING_PROVIDER=openai-compatible` and the matching base URL/model/dimensions/API key environment variables are present, search can use real provider embeddings stored side-by-side with the local fallback. Call `rebuild_repo_embeddings` after changing embedding provider settings. Treat vector hits as retrieval candidates, not final truth: prefer approved startup rules and evidence refs for durable facts, and verify stale or surprising matches before acting.

For recall questions, never answer from `get_repo_memory` alone. If approved startup rules do not contain the answer, search history and clearly label matches as history evidence rather than approved rules. If history evidence exists, offer to read/expand the found conversation before asking the user to explain the topic. If exact repo search misses, broaden to the repo family: parent roots and child roots can contain the real conversation.

## Local History And Alias Rule

- `import_all_local_history` is the broad bootstrap tool. Use it sparingly: first install, major source-path changes, or suspicious recall misses.
- `scan_repo_conversations` is the repo-scoped maintenance tool. It links conversations that match the repo and returns unmatched project roots for diagnosis.
- `merge_repo_alias` is the repair tool for path drift: old cwd, file cwd, renamed folder, generated Codex project path, or related subproject that should belong to the current repo.
- After merging an alias, run `scan_repo_conversations` again before answering recall questions.
- Do not merge unrelated repositories just to get more search hits.

## Extraction And Conflict Rule

ChatMem may auto-create pending startup-rule candidates only from explicit durable-memory markers such as "Remember:", "Rule:", "Gotcha:", "记住：", or "规则：". Bare imperative lines such as "Always ..." or "Do not ..." are not enough because they often come from agent task instructions. Auto-extracted candidates are not approved automatically and are not required for local-history search. Conflicts are review signals attached to candidates when new wording appears to negate an active approved rule.

## Merge Proposal Rule

ChatMem does not need an internal LLM API to rewrite startup rules. The active agent can draft a better merge proposal through `propose_memory_merge` after reading the candidate, approved rule, conflicts, and evidence. The desktop review inbox remains the human approval surface. If the rewrite is uncertain, ask the user instead of submitting a confident proposal.

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

- "I will search local history first."
- "I found a recent handoff; I will continue from that."
- "The memory is stale, so I will verify before relying on it."
- "I will create a memory candidate for this durable rule."

Do not expose internal memory mechanics unless the user asks. The user experience should feel like "continue this project", not "manage a memory database".
