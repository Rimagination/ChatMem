# ChatMem MCP Memory Design

**Date:** 2026-04-19

## Goal

Design a local-first ChatMem memory system where Codex app and Claude Code both access the same repository-scoped memory, history, and handoff workflows through a single ChatMem MCP server.

## Product Decision

### Chosen architecture

ChatMem V1 will follow an MCP-first architecture:

1. `ChatMem app` remains the human control surface for browsing conversations, reviewing memory candidates, and managing approved memories.
2. `ChatMem MCP server` becomes the only agent-facing interface for memory retrieval, history search, candidate creation, and handoff generation.
3. `Codex plugin/skill` and `Claude plugin/skill` remain thin entry shells that call ChatMem MCP tools instead of implementing their own memory logic.

This explicitly rejects a plugin-first architecture where each agent integration owns search, storage, and memory behavior separately.

### Scope model

V1 is repository-scoped first.

- Memory is keyed to a canonical repository identity.
- Automatic startup injection only uses repository-scoped approved memories.
- User-level memory is out of scope as a first-class system in V1.

### Trust model

Agents may propose memory, but they may not directly create or update approved memory.

- agent writes -> `memory candidate`
- human review in ChatMem app -> `approved repo memory`

This is a hard rule in V1.

### Trigger model

ChatMem V1 uses a mixed trigger model:

- explicit invocation remains available at all times
- automatic behavior is limited to a few high-value moments
- automatic behavior must stay small, quiet, and explainable

## User Goals

The system should solve four concrete jobs:

1. Before work starts in a repository, give the agent the few repository memories that materially improve the odds of a good first move.
2. While work is in progress, let the user or agent retrieve relevant prior work from repository history without dumping raw transcripts into context.
3. When switching from one agent to another, produce a usable handoff packet rather than forcing the next agent to rediscover the state.
4. Let the user approve, edit, merge, disable, or reject repository memory from a dedicated management surface.

## Non-Goals

V1 does not attempt to be a full general-purpose memory platform.

Out of scope:

- cross-repository global memory graph
- automatic approval of formal memory
- remote sync or cloud-hosted storage
- multi-user permissions
- embedding-first retrieval as a required dependency
- platform-specific memory logic that diverges between Codex and Claude
- direct mutation of Claude or Codex native storage formats

## Overall Architecture

### Layer 1: ChatMem Core

ChatMem Core owns local storage, canonical conversation import, asset persistence, approval workflows, and desktop UI.

It remains the source of truth for:

- repository identity
- canonical conversation records
- episodes
- memory candidates
- approved repository memory
- handoff packets
- evidence references

### Layer 2: ChatMem MCP Server

The MCP server is the only supported agent interface.

It is responsible for:

- exposing repository memory and history as agent tools
- enforcing read vs candidate-write boundaries
- applying retrieval filtering and ranking
- assembling compact startup memory payloads
- assembling handoff packets

It is not responsible for:

- rich visual review workflows
- long-form manual browsing
- candidate approval UI

### Layer 3: Thin Agent Entry Shells

Codex app and Claude Code each get a thin integration layer consisting of:

- MCP configuration
- small skill/prompt wrappers
- platform-specific trigger hooks
- formatting of MCP results into the host agent's preferred context block

These shells must not duplicate:

- repository identity logic
- search ranking logic
- approval logic
- schema ownership

## Canonical Assets

V1 uses five primary asset types.

### 1. Raw Conversation

Raw conversation is the evidence layer.

It includes:

- messages
- tool calls
- file changes
- source agent metadata
- timestamps

Raw conversation is not automatically injected as working context.

### 2. Episode

An episode is a reusable experience card derived from a prior conversation.

It should summarize:

- task goal
- outcome
- key steps
- key files
- useful commands
- notable failures or gotchas
- evidence links

Episodes primarily support search and handoff assembly.

### 3. Memory Candidate

A memory candidate is the only memory object an agent may create directly.

Supported V1 kinds:

- `command`
- `convention`
- `gotcha`
- `preference`
- `strategy`
- `path_or_entrypoint`

Each candidate must include:

- repository scope
- short summary
- concrete value
- explanation of why it matters
- evidence references
- confidence
- proposing agent

### 4. Approved Repo Memory

Approved memory is the only memory class eligible for startup injection.

It must be:

- repository-scoped
- short
- stable enough to reuse
- evidence-backed
- reviewable and editable by the user

Examples:

- canonical test command
- important build gotcha
- repository-specific convention
- stable workflow preference that clearly belongs to the repository

### 5. Handoff Packet

A handoff packet is a working-state transfer asset for agent switching.

It includes:

- current goal
- completed items
- next recommended items
- key files
- useful commands
- related memories
- related episodes

It is not treated as durable formal memory by default.

## MCP Tool Contract

V1 exposes five MCP tools only.

### `get_repo_memory`

Purpose: startup injection

Inputs:

- `repo_root`
- `agent`
- optional `task_hint`

Outputs:

- `repo_summary`
- up to three `approved_memories`
- up to two `priority_gotchas`
- optional `recent_handoff`

Design rule: the result is a compact startup packet, not a long document dump.

### `search_repo_history`

Purpose: history retrieval when context is missing or prior work is relevant

Inputs:

- `repo_root`
- `query`
- optional `limit`
- optional filters by type

Outputs:

- `matches[]`

Each match contains:

- `type`
- `title`
- `summary`
- `why_matched`
- `evidence_refs`
- `score`

### `create_memory_candidate`

Purpose: let an agent propose repository memory

Inputs:

- `repo_root`
- `kind`
- `summary`
- `value`
- `why_it_matters`
- `evidence_refs[]`
- `confidence`
- `proposed_by`

Outputs:

- `candidate_id`
- `status = pending_review`

Design rule: this tool may not create approved memory.

### `list_memory_candidates`

Purpose: show pending or recent candidate memory for review and awareness

Inputs:

- `repo_root`
- optional `status`

Outputs:

- `candidates[]`

Each item contains:

- `candidate_id`
- `kind`
- `summary`
- `confidence`
- `proposed_by`
- `evidence_refs`

### `build_handoff_packet`

Purpose: transfer working state from one agent to another

Inputs:

- `repo_root`
- `from_agent`
- `to_agent`
- optional `goal_hint`

Outputs:

- `current_goal`
- `done_items[]`
- `next_items[]`
- `key_files[]`
- `useful_commands[]`
- `related_memories[]`
- `related_episodes[]`

## Automatic Trigger and Injection Strategy

### Automatic injection

Automatic injection is limited to two moments:

1. an agent enters an integrated repository
2. an agent receives a handoff packet

The startup packet uses `get_repo_memory`.

Injection limits:

- approved memories: at most 3
- gotchas: at most 1 by default
- recent handoff: at most 1

Automatic injection must always be concise and deterministic.

### Automatic suggestion

Automatic suggestion is allowed when:

- the user asks how something was done before
- the agent appears blocked on missing repository context
- a relevant prior episode is likely useful
- an agent switch is being prepared

Suggested actions include:

- search history
- generate handoff
- review pending candidate memory

Suggestions must remain opt-in.

### Explicit invocation

The user must always be able to invoke ChatMem explicitly through host-specific entry points such as:

- `@chatmem`
- `$chatmem`
- natural-language requests to search ChatMem or build a handoff

### Injection budget

V1 should use strict budget limits:

- startup packet: approximately 600-900 tokens
- handoff packet: approximately 800-1200 tokens
- single approved memory: short enough to fit in one compact paragraph
- single episode summary: short enough to fit in one compact paragraph

If the budget is exceeded, trim in this order:

1. lower-priority preferences
2. lower-priority strategies
3. older handoff details
4. long episode detail

### Suppression rules

Automatic behavior should be suppressed when:

- the repository has no approved memory
- the same repository was already auto-injected recently
- the user has just declined ChatMem assistance in the active interaction
- retrieval confidence is low
- the current task is clearly unrelated to repository work

## Approval Flow and Management UI

The ChatMem app becomes the governance surface for repository memory.

### Required V1 views

1. `Repo Memory`
2. `Memory Inbox`
3. `Episodes`
4. `Handoffs`

### Memory Inbox behavior

The inbox displays pending candidates with:

- summary
- kind
- confidence
- proposing agent
- why it matters
- evidence preview

Allowed review actions:

- approve
- approve with edit
- reject
- merge into existing memory
- snooze

### Repo Memory behavior

Approved memory must support:

- edit
- disable
- delete
- evidence inspection

Each memory should also show whether it is used for:

- startup injection
- search only
- handoff support

### Evidence inspection

Every candidate and approved memory must support traceable evidence inspection including:

- source conversation
- source message excerpt
- source tool call when relevant
- related file change when relevant
- source agent and timestamp

## Repository Identity

V1 must establish a stable repository identity before retrieval or memory mutation.

Identity order:

1. git root, when available
2. normalized repository path
3. repository fingerprint derived from remote, root, and selected metadata
4. fallback to normalized directory scope when git is unavailable

Memory, history retrieval, and handoff generation must remain repository-bound.

Cross-repository retrieval is not allowed in V1.

## Retrieval and Ranking

### Retrieval pipeline

Each request follows three stages:

1. hard filtering
2. use-case-specific candidate pool selection
3. small-pool ranking

### Hard filtering

Always filter by:

- exact repository identity
- allowed status
- allowed asset type for the current request
- stale or disabled state

### Use-case-specific pools

- `get_repo_memory` -> approved memory + recent handoff only
- `search_repo_history` -> episodes + approved memory + conversation summaries
- `build_handoff_packet` -> recent conversations + useful memories + related episodes

### Ranking factors

V1 ranking should remain rule-based:

- asset type priority
- freshness / last verification
- evidence strength
- task hint overlap
- recent successful usefulness signal

Preferred type priority for startup injection:

1. gotcha
2. command
3. convention
4. strategy
5. preference

### Explainability

Returned items should include a short explanation such as:

- why selected
- what matched
- when last verified

This keeps retrieval legible to both the user and the host agent.

## Data Model and Storage

V1 should use a ChatMem-owned local SQLite database rather than mutating source agent stores.

### Canonical conversation tables

- `repos`
- `conversations`
- `messages`
- `tool_calls`
- `file_changes`

These tables store imported, normalized repository conversation evidence.

### Asset tables

- `episodes`
- `memory_candidates`
- `approved_memories`
- `handoff_packets`

These tables store the product-level reusable assets.

### Evidence and search tables

- `evidence_refs`
- `search_documents`
- `search_documents_fts`

`search_documents` provides a unified retrieval surface for:

- conversation summaries
- episodes
- approved memory

### Storage principle

- source data is imported read-only
- assets are stored in ChatMem-owned tables
- agent access always goes through MCP

## Codex and Claude Integration

### Codex app integration

The Codex-side integration should provide:

- MCP configuration
- small skill/prompt wrappers for startup, search, handoff, and candidate creation
- host-specific formatting for MCP results

### Claude Code integration

The Claude-side integration should provide the same logical behavior through:

- MCP configuration
- small skill/prompt wrappers
- host-specific formatting only where needed

### Consistency requirement

Codex and Claude should present the same mental model:

- entering a repository may load compact repository memory
- ChatMem can search prior repository work
- ChatMem can build a handoff packet
- ChatMem may suggest candidate memory, but the user governs promotion

Platform-specific differences are implementation details, not product differences.

## V1 Milestones

### Milestone 1: Core Schema and Repository Identity

- define repository identity
- define canonical tables
- define asset tables
- import source conversations into the canonical layer

### Milestone 2: MCP Server

- implement the five MCP tools
- validate local tool behavior manually

### Milestone 3: Management UI

- implement Repo Memory and Memory Inbox first
- expose Episodes and Handoffs views
- enable candidate review actions

### Milestone 4: Agent Integrations

- wire Codex app to the local MCP server
- wire Claude Code to the local MCP server
- support explicit invocation first
- then add startup injection and handoff triggers

### Milestone 5: Retrieval Quality and Noise Control

- ranking
- suppression rules
- de-duplication and merge support
- stale memory handling

## Success Criteria

V1 is successful when:

1. Codex app and Claude Code both retrieve the same repository-scoped memory through ChatMem MCP.
2. Approved memory is never directly mutated by agents.
3. The user can review, edit, approve, reject, merge, and disable repository memory candidates in ChatMem app.
4. Startup injection helps the agent begin work with repository-specific context while staying small and non-disruptive.
5. Handoff packets make cross-agent continuation materially easier than starting from raw history alone.

## Risks and Open Questions

### Trigger parity risk

Codex and Claude may not expose identical trigger hooks. The product behavior should stay aligned even if the implementation hooks differ.

### Repository identity drift

Copied worktrees, renamed directories, and partial checkouts may complicate stable repository identity. V1 should favor correctness over aggressive identity merging.

### Memory quality risk

The system must avoid promoting noisy or one-off observations into formal repository memory. The candidate-review boundary exists to protect against this failure mode.

### Startup noise risk

If startup injection becomes verbose or repetitive, users will quickly stop trusting it. Budget limits and suppression rules are first-order design constraints.

