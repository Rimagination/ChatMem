# ChatMem Cross-Agent Control Plane Design

**Date:** 2026-04-20

## Goal

Design ChatMem V2 as a local-first cross-agent control plane that helps Claude, Codex, Gemini, and future hosts share repository memory, structured handoffs, execution state, and reviewable artifacts without relying on raw transcript replay.

## Context

ChatMem V1 already establishes the core repository memory stack:

- conversation import and browsing
- repository-scoped approved memory
- memory candidate review
- handoff packet generation
- MCP access through five agent-facing tools

That foundation is good, but the broader agent ecosystem has shifted. Recent official product and protocol work now emphasizes:

- structured agent-to-agent handoff instead of free-form summaries
- project memory with user control and governance
- parallel agents with isolated context windows
- task state, artifacts, and resumable execution
- human review and observability
- protocol separation between tool context (`MCP`) and agent-to-agent task exchange (`A2A`)

ChatMem should evolve accordingly.

## Product Decision

### Chosen direction

ChatMem V2 will become a cross-agent control plane, not just a conversation archive or migration utility.

This means ChatMem should own four product responsibilities:

1. durable repository memory
2. structured handoff and checkpoint assets
3. run and artifact observability across agents
4. human governance over promotion, review, and transfer

### Alternatives considered

#### 1. Archive-first product

Keep focusing on browsing, search, and migration of native conversations.

Why not chosen:

- easy to build but strategically shallow
- increasingly overlaps with host-native features
- does not solve the hard part of multi-agent work: continuation and control

#### 2. Control-plane-first product

Use the existing memory and MCP foundation to coordinate how agents resume work, transfer state, and surface evidence.

Why chosen:

- builds directly on current ChatMem assets and UI
- aligns with emerging MCP + A2A ecosystem boundaries
- solves a harder and more durable user problem than transcript browsing alone

#### 3. Full orchestration platform

Turn ChatMem into a scheduler, planner, and remote execution manager for many agents.

Why not chosen now:

- too large for the current product boundary
- would add major reliability and runtime concerns
- risks diluting the product before the control plane layer is mature

## User Jobs

ChatMem V2 should solve six concrete jobs.

1. Before work starts, give an agent the minimum repository memory and recent execution state needed for a good first move.
2. When one agent hands off to another, transfer goal, status, evidence, files, commands, and open questions in a structured form.
3. When multiple agents work in parallel, let the user see who is doing what, what completed, and what remains blocked.
4. When an agent proposes durable knowledge, route it through human review rather than silently turning transient output into memory.
5. When a task needs to be paused and resumed later, preserve checkpoints rather than forcing a cold restart from chat history.
6. When the user asks what happened, show a trustworthy run timeline with evidence and artifacts instead of opaque summaries.

## Non-Goals

V2 still avoids several categories of scope.

Out of scope:

- cloud-hosted multi-tenant orchestration as a required dependency
- direct mutation of Claude, Codex, or Gemini native memory stores
- embedding-first or vector-only retrieval as a requirement
- autonomous approval of formal memory
- remote code execution infrastructure
- replacing host-specific chat UX
- generalized enterprise workflow automation outside repository work

## Core Principles

### Local first

The primary deployment model remains a local desktop app plus local MCP server and local database.

### Repository scope first

Repository identity remains the default boundary for memory, runs, handoffs, checkpoints, and artifacts.

### Human governance

Agents may propose, assemble, and summarize. Humans approve, merge, edit, freeze, resume, or reject.

### Structured assets over transcript replay

ChatMem should prefer concise, typed assets such as `memory`, `episode`, `handoff`, `checkpoint`, `artifact`, and `approval` instead of re-injecting large raw transcripts.

### Explainability before magic

Every surfaced item should answer:

- why it was selected
- when it was last verified
- what evidence supports it
- which agent produced it

### Host-agnostic model

Codex, Claude, Gemini, and future hosts should share one mental model even when trigger hooks or UI wrappers differ.

## Overall Architecture

### Layer 1: Evidence Ingestion

ChatMem continues importing source conversations read-only into canonical tables.

It should normalize:

- messages
- tool calls
- file changes
- timestamps
- source agent identity
- repository identity

### Layer 2: Knowledge Assets

This layer stores durable reusable assets:

- approved memories
- memory candidates
- episodes
- handoff packets

### Layer 3: Execution State

This new V2 layer stores working-state transfer assets:

- agent runs
- run events
- checkpoints
- artifacts
- approvals

### Layer 4: Protocol Surfaces

ChatMem exposes two protocol surfaces:

1. `MCP` for tool access, retrieval, and local host integrations
2. `A2A-lite` for task, status, and artifact exchange between agents or future external orchestrators

### Layer 5: Human Control Surface

The desktop app remains the main governance UI for:

- browsing conversations
- reviewing memory candidates
- composing and inspecting handoffs
- inspecting run timelines
- freezing and resuming checkpoints
- reviewing artifacts and approval requests

## Canonical Assets

V2 uses nine primary asset types.

### 1. Raw Conversation

The evidence layer imported from native agent stores.

### 2. Episode

A distilled prior work card optimized for search and reuse.

### 3. Memory Candidate

An agent-proposed repository fact awaiting review.

### 4. Approved Repo Memory

Human-governed durable memory eligible for startup retrieval and handoff enrichment.

### 5. Handoff Packet

A structured transfer asset for moving active work from one agent to another.

### 6. Checkpoint

A frozen working-state asset that captures the latest trustworthy continuation point for a task.

A checkpoint should include:

- current goal
- active branch or workspace hint
- completed items
- pending items
- open questions
- key files
- useful commands
- related memories
- related artifacts
- source run reference

### 7. Run

A logical unit of work produced by one agent for one task in one repository context.

### 8. Artifact

A typed output produced during a run.

Supported initial artifact kinds:

- `summary`
- `patch_set`
- `test_result`
- `build_output`
- `research_note`
- `decision_record`
- `handoff_attachment`

### 9. Approval Request

A human review object representing a decision point such as:

- approve memory
- accept checkpoint
- publish handoff
- mark artifact as canonical

## Data Model Additions

V2 should preserve the existing V1 tables and add the following new tables.

### New tables

- `agent_runs`
- `run_events`
- `checkpoints`
- `artifacts`
- `artifact_links`
- `approval_requests`
- `approval_actions`
- `run_subscriptions`

### Important new fields on existing tables

#### `approved_memories`

Add:

- `freshness_status`
- `freshness_score`
- `verified_at`
- `verified_by`
- `usage_count`
- `last_used_at`
- `target_host_scope`

#### `handoff_packets`

Add:

- `status`
- `checkpoint_id`
- `target_profile`
- `compression_strategy`
- `consumed_at`
- `consumed_by`

#### `episodes`

Add:

- `outcome_type`
- `usefulness_score`
- `last_referenced_at`

## V2 Retrieval Model

### Retrieval surfaces

V2 retrieval has three distinct surfaces:

1. startup context retrieval
2. active task continuation retrieval
3. historical research retrieval

These surfaces should remain separate because their ranking logic differs.

### Startup context retrieval

Purpose:

- orient the agent quickly
- minimize prompt budget

Preferred sources:

1. approved memories
2. freshest relevant checkpoint summary
3. one recent handoff if present

### Active task continuation retrieval

Purpose:

- resume or redirect active work

Preferred sources:

1. latest checkpoint
2. unconsumed handoff
3. recent run artifacts
4. supporting approved memories

### Historical research retrieval

Purpose:

- find precedent without polluting the working context

Preferred sources:

1. episodes
2. relevant artifacts
3. approved memories
4. conversation summaries

### Ranking factors

V2 should rank with explicit rule-based signals first.

Primary signals:

- repository identity match
- active task match
- freshness
- evidence strength
- asset type priority
- target host compatibility
- recent usefulness signal
- human-pinned priority

## Memory Freshness and Governance

ChatMem V2 should treat stale memory as a first-class product concern.

### Freshness states

- `fresh`
- `needs_review`
- `stale`
- `archived`

### Freshness triggers

Memories should move toward review when:

- relevant files changed materially
- commands fail repeatedly
- the same memory is manually corrected
- the repository branch or toolchain changed
- the memory has not been verified for a long time

### Governance actions

The UI should support:

- approve
- approve with edit
- merge
- reject
- snooze
- disable
- re-verify
- archive

## Handoff 2.0

Handoff is the highest-priority V2 upgrade area.

### Handoff design goal

A handoff should let another agent continue usefully in one pass, without reading the full prior conversation unless deeper evidence is needed.

### Handoff packet structure

Each packet should contain:

- `current_goal`
- `task_status`
- `completed_items`
- `next_items`
- `blocked_items`
- `open_questions`
- `key_files`
- `useful_commands`
- `related_memories`
- `related_artifacts`
- `related_episode_refs`
- `checkpoint_ref`
- `confidence`
- `target_profile`

### Target profiles

V2 should support host-aware formatting profiles:

- `codex_compact`
- `claude_contextual`
- `gemini_brief`
- `generic_json`

The underlying information stays shared. Only compression and presentation differ.

### Handoff lifecycle

1. draft
2. reviewed
3. published
4. consumed
5. superseded

### Design rule

Only one handoff should be marked as the preferred live continuation asset for a specific active task at a time.

## Checkpoint System

Checkpoints are the second major V2 upgrade area.

### Why checkpoints matter

Memory captures durable facts. Handoffs capture transfer intent. Checkpoints capture resumable working state.

Without checkpoints, agents still lose too much context between sessions.

### Checkpoint creation moments

Suggested trigger moments:

- before agent switch
- before long-running task pause
- after a meaningful milestone
- before risky refactor or migration
- when a human explicitly freezes state

### Checkpoint behaviors

The user should be able to:

- freeze a checkpoint
- resume from a checkpoint
- compare two checkpoints
- promote a checkpoint into a handoff
- attach artifacts and approvals

## Runs and Timeline

Runs and timeline are the third major V2 upgrade area.

### Run model

A run represents one bounded work attempt by one agent.

Each run should include:

- run id
- repository id
- task id or task hint
- source agent
- start time
- end time
- status
- summary
- related checkpoint
- related handoff

### Run statuses

- `running`
- `waiting_for_input`
- `waiting_for_review`
- `blocked`
- `completed`
- `failed`
- `canceled`

### Run events

Events should be small and typed:

- started
- memory_loaded
- search_performed
- candidate_proposed
- artifact_created
- approval_requested
- handoff_published
- checkpoint_frozen
- resumed
- completed
- failed

### Timeline UI value

This gives the user a single place to answer:

- what is active now
- what changed recently
- where the baton currently is
- which outputs are safe to trust

## Artifact System

Artifacts should become first-class assets rather than hidden strings inside run logs.

### Artifact requirements

Each artifact should store:

- artifact id
- type
- title
- short summary
- producing run
- producing agent
- body or file reference
- evidence references
- created at
- trust state

### Trust states

- `generated`
- `reviewed`
- `approved`
- `rejected`

### Artifact uses

Artifacts support:

- run timeline inspection
- handoff enrichment
- checkpoint packaging
- future export and sync

## Protocol Strategy

### MCP strategy

V2 should preserve the current five MCP tools for compatibility:

- `get_repo_memory`
- `search_repo_history`
- `create_memory_candidate`
- `list_memory_candidates`
- `build_handoff_packet`

These remain the stable base contract.

### New optional MCP tools

After compatibility is stable, add a V2 tool set:

- `get_active_run_context`
- `create_checkpoint`
- `list_checkpoints`
- `resume_from_checkpoint`
- `list_run_artifacts`
- `request_human_approval`
- `list_active_runs`

Design rule:

These tools are additive. Existing integrations must not break.

### A2A-lite strategy

ChatMem should add a narrow A2A-style surface after the MCP layer matures.

Initial A2A-lite capabilities:

- `Agent Card` for ChatMem as a control-plane service
- create task
- query task status
- push or pull task updates
- fetch artifact metadata
- fetch checkpoint metadata

This is intentionally narrower than full orchestration.

## Desktop UI Information Architecture

The current V1 views should remain and expand.

### Existing views to keep

- `Conversations`
- `Repo Memory`
- `Memory Inbox`
- `Episodes`
- `Handoffs`

### New V2 views

- `Runs`
- `Checkpoints`
- `Artifacts`
- `Approvals`

### Key UX changes

#### Repo Memory

Add freshness badges, target-host scope, usage history, and re-verify actions.

#### Memory Inbox

Add merge suggestions, duplicate detection, and richer evidence previews.

#### Handoffs

Add status, target profile, consume state, and conversion from checkpoint.

#### Runs

Show live and historical run cards, grouped by task.

#### Checkpoints

Show freeze/resume controls, diffs, and linked artifacts.

#### Approvals

Aggregate all human decision points in one place instead of scattering them across separate views.

## Privacy and Trust Model

### Sensitive content controls

V2 should support:

- per-memory exclusion
- per-artifact exclusion
- private checkpoint flag
- no-sync marker for future remote sync compatibility

### Default trust boundaries

- imported conversation data is read-only
- agents may propose but not self-approve durable knowledge
- human review gates all durable promotion
- host-native stores remain source evidence, not mutable ChatMem-owned state

## Rollout Plan

### Phase 1: Handoff 2.0

Ship first because it directly improves cross-agent continuation.

Scope:

- richer handoff schema
- target profiles
- handoff lifecycle states
- handoff review and consume tracking

### Phase 2: Memory Freshness and Governance

Ship second because stale memory destroys trust quickly.

Scope:

- freshness model
- verification fields
- duplicate and merge assistance
- re-verify actions

### Phase 3: Runs and Artifact Timeline

Ship third because it gives the user operational visibility.

Scope:

- run tables
- run timeline UI
- artifact storage and display
- task-level grouping

### Phase 4: Checkpoints

Ship fourth because it unlocks resumable state rather than summary-only transfer.

Scope:

- checkpoint creation
- resume flow
- checkpoint diff and promotion to handoff

### Phase 5: A2A-lite Bridge

Ship fifth after the internal model is stable.

Scope:

- agent card
- task state endpoints
- artifact and checkpoint metadata exchange

### Phase 6: Optional Remote Sync

Only consider this after the local control plane is trusted.

## Success Criteria

V2 is successful when:

1. A handoff between Codex and Claude is materially faster and more reliable than resuming from raw transcript history.
2. Users can see live or recent run state, not just final conversation output.
3. Stale memory is visible and reviewable instead of silently poisoning startup context.
4. Checkpoints let users resume interrupted work with less manual reconstruction.
5. Existing MCP integrations continue working while V2 capabilities ship incrementally.
6. ChatMem is recognized as the system of record for repository memory and cross-agent continuity, not just a transcript browser.

## Risks

### Scope risk

Runs, checkpoints, artifacts, and A2A-lite can expand quickly. V2 must prioritize sequence and keep each phase narrow.

### Noise risk

If every run and artifact becomes visible with no filtering, the product will become harder to use rather than easier.

### Consistency risk

Different hosts have different trigger hooks and context budgets. ChatMem must keep one product model while allowing host-specific wrappers.

### Trust risk

If stale memory and weak handoffs look authoritative, users will stop trusting the whole system. Freshness and evidence remain core, not optional.

## Open Questions

1. Should checkpoints be manually created first, or can ChatMem safely suggest them automatically at milestone boundaries?
2. Should artifact bodies live fully inside SQLite, or should larger artifacts move to file-backed storage with indexed metadata?
3. When multiple handoffs exist for one task, how aggressively should ChatMem enforce a single preferred continuation path?
4. At what maturity point should A2A-lite become a public external interface rather than an internal compatibility layer?

## References

- Google Developers Blog, "A2A: A New Era of Agent Interoperability", accessed 2026-04-20
- A2A Protocol documentation, key concepts and MCP relationship, accessed 2026-04-20
- Model Context Protocol official site, accessed 2026-04-20
- OpenAI Agents SDK documentation for handoffs, results, tracing, and human-in-the-loop, accessed 2026-04-20
- Anthropic Memory announcement, published 2025-09-11
- Claude Code documentation for sub-agents, accessed 2026-04-20
- Claude Code desktop redesign for parallel agents, published 2026-04-14
