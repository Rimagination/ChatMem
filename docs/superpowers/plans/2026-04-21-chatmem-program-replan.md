# ChatMem Program Replan

> **For agentic workers:** This document is a program-level sequencing plan, not a code-task execution plan. Use it to choose the next clean workstream before dispatching task-level implementation plans.

**Goal:** Untangle the unfinished ChatMem work into clean, independent tracks so UI restructuring, Zotero-style product design, and skill creation can proceed without contaminating each other.

**Architecture:** Treat the current unfinished work as three separate tracks: `task-oriented UI`, `Zotero-style knowledge/session manager`, and `ChatMem skill`. Stabilize or restart the broken UI worktree first, then write the product spec for the Zotero-style direction, and only then create the skill against the clarified model.

**Tech Stack:** React, TypeScript, Vite, Vitest, Tauri, ChatMem MCP surfaces, local skills

---

## Current State Audit

### What already exists

- Task-oriented UI design spec:
  - `docs/superpowers/specs/2026-04-21-chatmem-task-nav-help-design.md`
- Codex-style shell supplement spec:
  - `docs/superpowers/specs/2026-04-21-chatmem-codex-shell-design.md`
- Task-oriented UI rollout plan:
  - `docs/superpowers/plans/2026-04-21-chatmem-task-nav-help.md`
- Partially executed worktree:
  - `.worktrees/chatmem-task-nav-help`
- Product direction discussion:
  - ChatMem should evolve toward a Zotero-style knowledge and session manager, not just a memory helper
- Skill creation intent:
  - A ChatMem skill should eventually be created, but it should match the clarified product model, not the old one

### What is unfinished

#### Track A: Task-oriented UI

- The task-oriented top navigation and Help redesign have a spec and an execution plan
- A partial implementation exists in `.worktrees/chatmem-task-nav-help`
- That implementation was found non-compliant during review because the old object-based workspace path was not fully replaced
- The worktree is now in an unsafe intermediate state

#### Track B: Zotero-style manager

- The product direction has been discussed but not formalized as a written spec
- There is no current plan for:
  - local library structure
  - sync object model
  - WebDAV/cloud adapter boundaries
  - conflict handling
  - library UI

#### Track C: ChatMem skill

- The intent is clear: create a skill designed around ChatMem's real strengths
- The target behavior is not stable enough yet because Track B has not been specified

## Key Planning Decisions

### 1. Do not continue from the dirty UI worktree blindly

The current `.worktrees/chatmem-task-nav-help` state should not be treated as an ordinary branch in progress. It contains a partial fix chain and at least one broken intermediate diff. It must be either:

- salvaged deliberately, or
- discarded and restarted from a clean worktree

### 2. Split the work into separate tracks

The three tracks below should not be executed as one continuous stream:

1. Task-oriented UI restructuring
2. Zotero-style product architecture
3. Skill creation

Each has a different design surface and different stopping points.

### 3. Finish UI cleanup before opening the deeper product buildout

The UI restructuring is already designed and partially attempted. It is the shortest path to turning current thinking into visible product progress. The Zotero-style manager should be specified next, then the skill should be created against that clarified model.

## Recommended Sequence

## Phase 0: Stabilize the UI Track

### Objective

Get back to one trustworthy baseline for the task-oriented UI effort.

### Required actions

1. Inspect `.worktrees/chatmem-task-nav-help` and choose one path:
   - salvage the existing branch if the diff is recoverable in under one focused repair session
   - otherwise abandon it and start a fresh worktree from `main`
2. Restore a clean baseline:
   - `src/__tests__/App.test.tsx` must exist and pass again
   - `npm.cmd run test:run` must be green before any new UI execution resumes
3. Record the decision:
   - if salvaged, continue on the same branch
   - if abandoned, keep the old branch for reference only and create a new clean worktree for the re-run

### Exit criteria

- clean git state in the active UI worktree
- passing frontend test baseline
- one approved execution path for the UI redesign

## Phase 1: Re-run the Task-Oriented UI Rollout

### Objective

Complete the task-oriented shell that was already specified:

- Continue Work
- Needs Review
- History
- Help

### Source of truth

- `docs/superpowers/specs/2026-04-21-chatmem-task-nav-help-design.md`
- `docs/superpowers/specs/2026-04-21-chatmem-codex-shell-design.md`
- `docs/superpowers/plans/2026-04-21-chatmem-task-nav-help.md`

### Milestones

#### Milestone 1: Task-oriented shell

- adopt the Codex-style single-row top bar
- remove the large in-app logo treatment
- merge the left rail into the canvas and preserve the rounded right work surface
- add the Projects collapse/restore control and shared organize menu behavior
- replace the old object-tab shell
- add top navigation
- land a minimal Continue Work page

#### Milestone 2: Continue Work

- recent tasks
- recoverable progress
- next-step guidance
- source/resume card

#### Milestone 3: Needs Review

- pending memory proposals
- stale project rules
- transfer summaries waiting for human action

#### Milestone 4: History

- filters for conversations, recovery, transfers, outputs
- absorb the old object-based panels into secondary history views

#### Milestone 5: Help

- FAQ cards
- quick actions
- advanced troubleshooting

#### Milestone 6: Regression and polish

- full frontend test run
- build verification
- wording pass
- remove leftover object-first copy from the primary path

### Exit criteria

- task-oriented nav is the real primary shell
- no first-layer UI depends on users understanding internal object names
- Help is usable as a triage surface

## Phase 2: Write the Zotero-Style Product Spec

### Objective

Turn the “Zotero-style knowledge/session manager” direction into a real system spec before writing implementation code.

### Deliverables

1. A product spec that defines ChatMem as a library-centered system, not just a conversation viewer
2. A clear separation between:
   - local library
   - object storage
   - sync protocol
   - storage adapters
   - agent-facing APIs
3. A first-pass sync story that can support WebDAV-compatible backends such as Nutstore/Jianguoyun

### Questions the spec must answer

#### Library model

- What is a top-level object?
  - conversation
  - checkpoint
  - handoff
  - project memory
  - artifact
  - attachment
- What belongs in metadata versus object payload?
- What gets indexed locally?

#### Sync model

- Is sync object-based, log-based, or hybrid?
- What gets hashed?
- How are deletes represented?
- How are conflicts surfaced?
- What is the offline-first behavior?

#### Storage adapters

- What is the minimum adapter contract?
- Which capabilities are required for WebDAV?
- How should local folder sync differ from WebDAV?
- Which backend assumptions must never leak into the core model?

#### Product UX

- What is the library root screen?
- How are projects, conversations, and remembered knowledge organized?
- What is the equivalent of Zotero collections, items, attachments, tags, and notes in ChatMem?

### Exit criteria

- written spec approved
- explicit data model and sync model
- enough detail to write a real implementation plan

## Phase 3: Plan the Zotero-Style System Build

### Objective

Create the first real implementation plan for the Zotero-style manager after the spec is approved.

### Deliverables

- local data model plan
- sync engine plan
- adapter layer plan
- library UI plan
- migration plan from current ChatMem model to the new library-centered model

### Exit criteria

- one executable implementation plan, or a set of small plans if the work is split by subsystem

## Phase 4: Create the ChatMem Skill

### Objective

Create a skill that teaches the right workflow for ChatMem after the product boundaries are stable.

### Why this is last

If the skill is created too early, it will likely train agents on the current transitional model rather than the intended Zotero-style system.

### The skill should eventually cover

- when to load project/library context
- when to search prior conversations or memories
- when to create or promote checkpoints and handoffs
- when to suggest remembering durable conclusions
- how to treat the human UI as an approval layer instead of the primary operating surface

### Skill creation checklist

1. Choose final skill scope:
   - current ChatMem only
   - or library-centered ChatMem
2. Choose installation path
3. Initialize the skill with the proper template
4. Write `SKILL.md`
5. Add any bundled references if needed
6. Validate it with realistic prompts and pressure scenarios

### Exit criteria

- the skill triggers for the right situations
- the skill does not encode outdated assumptions
- the skill matches the product we actually want to build

## Recommended Immediate Next Step

Do **not** start the Zotero-style architecture build or the skill first.

The next clean move is:

1. resolve the `chatmem-task-nav-help` worktree state
2. re-run the task-oriented UI rollout from a trustworthy baseline
3. then write the Zotero-style product spec

## Stop/Go Gates

### Stop UI execution if

- the active worktree does not have a clean baseline
- tests are failing for unknown reasons
- implementation starts diverging from the approved task-nav spec

### Stop product architecture work if

- the Zotero-style model is still being described only in metaphors
- storage and sync boundaries are not explicit
- WebDAV adapter requirements are still mixed into the core model

### Stop skill creation if

- the skill would have to guess the future product model
- the trigger conditions are still tied to a temporary UI

## Program Definition of Done

This broader ChatMem replan is complete when:

- the task-oriented UI is real and shippable
- the Zotero-style manager has an approved architecture spec and implementation plan
- the ChatMem skill is created against that approved model

Until then, treat these as separate workstreams with deliberate handoffs, not one long continuous task.
