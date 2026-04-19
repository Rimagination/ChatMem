---
name: chatmem
description: Use when working inside a repository that has ChatMem memory enabled, especially before starting work, when searching prior implementation context, or when handing work to another agent.
---

# ChatMem

## Overview

ChatMem turns prior conversations into repository memory that agents can reuse without replaying raw transcripts.

Use it to load startup context, search earlier work, propose memory candidates, and build handoff packets.

## When to Use

- Entering a repo and you want the smallest useful context first
- Needing precedent from earlier Codex, Claude, or Gemini sessions
- Capturing a durable repo fact that should survive the current thread
- Preparing to hand work from one agent to another

## Workflow

1. Call `get_repo_memory` with the current repo root before substantial work.
2. Call `search_repo_history` when you need precedent, commands, or prior fixes.
3. Call `create_memory_candidate` only for narrow, evidence-backed repo knowledge.
4. Route every candidate through the human-reviewed `Memory Inbox`; do not auto-approve writes.
5. Call `build_handoff_packet` before switching from Codex to Claude or the reverse.

## Candidate Rules

- Prefer concrete commands, gotchas, conventions, and stable preferences.
- Keep values short enough to inject at startup.
- Attach evidence excerpts from the current or prior conversations.
- Do not store secrets, transient logs, or one-off brainstorming noise.

## Good Targets

- "Run `npm run test:run` before packaging the Tauri app."
- "Release builds require the updater signing key."
- "This repo prefers repo-scoped memory with human approval."

## Bad Targets

- Full conversation summaries
- Personal chatter
- Temporary TODO lists
- Secrets or credentials
