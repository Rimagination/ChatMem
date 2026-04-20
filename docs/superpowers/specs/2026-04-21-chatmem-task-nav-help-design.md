# ChatMem Task-Oriented Navigation and Help Design

## Goal

Refocus the ChatMem desktop UI around work continuity instead of exposing internal memory objects. The app should help returning users continue work quickly, approve the few things that require a human decision, and only reveal technical details when they are actively needed.

This design specifically defines:

- the global top navigation
- the purpose and structure of the Help page
- front-end terminology for user-facing surfaces
- low-fidelity page wireframes for the main navigation areas

## Product Positioning

ChatMem is agent infrastructure first and a human review console second.

That means:

- agents should continue using MCP and CLI-oriented flows for read, write, handoff, and checkpoint operations
- humans should see a simplified task-oriented interface
- the desktop app should only surface actions that require understanding, approval, recovery, or troubleshooting

The core user value is not "memory management." The core user value is: continue work across agents without losing context.

## Primary Audience

The primary audience for this design is the returning user who wants to resume work.

Audience priority:

1. Returning users who want to pick work back up
2. First-time users who need a clear starting point
3. Power users who may need diagnostics and low-level detail

This ordering drives the navigation. Returning users are the main path. First-time users are supported through Help. Power-user tooling remains available, but it should not dominate the surface.

## Problem Statement

The current UI exposes too much of the internal system model directly to users. Labels such as checkpoints, repo memory, memory inbox, artifacts, and handoffs make sense to developers and agent infrastructure, but they ask ordinary users to learn the storage model before they can act.

This creates three product problems:

1. Users do not know where to click first when they come back
2. The UI teaches internal nouns instead of next actions
3. Help and recovery are mixed with implementation-level concepts

## Design Principles

### 1. Lead with tasks, not objects

Top-level navigation should answer "what can I do now?" rather than "which system object do I want to inspect?"

### 2. Keep agent concepts backstage

Checkpoint, repo memory, artifact, and handoff may still exist in the model, but they should appear in secondary areas, not in the first layer of navigation.

### 3. Default to continuity

The default experience should always help the user continue previous work or start the next sensible action.

### 4. Only surface human decisions

The desktop app should become the place where a human reviews, confirms, resumes, or troubleshoots. Everything else should stay automated.

### 5. Hide complexity until needed

Advanced diagnostics and operational detail belong in History and Advanced Troubleshooting, not on the main path.

## Navigation Strategy

Three navigation approaches were considered:

### Option A: Task-oriented navigation

Examples:

- Continue Work
- Needs Review
- History
- Help

Pros:

- easiest for returning users
- aligns with user intent
- hides unnecessary internal concepts

Cons:

- less explicit about internal system structure

### Option B: Product-style navigation

Examples:

- Home
- Projects
- Help
- Settings

Pros:

- familiar application pattern

Cons:

- too generic
- does not guide the user toward the next action

### Option C: Expert-object navigation

Examples:

- Conversations
- Memory
- Checkpoints
- Handoffs
- Help

Pros:

- mirrors the internal system model

Cons:

- steep learning curve
- weak fit for ordinary users

## Recommendation

Use Option A: task-oriented navigation.

Global top navigation should be:

- Continue Work
- Needs Review
- History
- Help

Right-side utility actions should be:

- Search
- Current Agent
- Settings

This keeps the main navigation under five items and reserves the primary surface for work continuation.

## Final Information Architecture

### Global Navigation

Left:

- ChatMem brand

Center:

- Continue Work
- Needs Review
- History
- Help

Right:

- Search
- Current Agent
- Settings

### Page Model

Only one main page should be visible at a time beneath the top navigation.

Do not combine:

- a global navigation bar
- a second row of equal-weight object tabs

If internal object views still need to exist, they should appear as filters, sections, or secondary panels inside History or detailed screens.

## Terminology Strategy

Replace system-facing labels with user-facing task language at the first layer.

Recommended replacements:

- Checkpoints -> Recoverable Progress
- Repo Memory -> Project Rules
- Memory Inbox -> Pending Memory
- Handoff Packet -> Transfer Summary
- Runs / Artifacts -> Advanced Records

These underlying concepts can still appear in secondary detail views, but not in the main navigation.

## Page Specifications

## 1. Continue Work

### Purpose

Help returning users resume work as quickly as possible.

### Content Order

1. Recent Tasks
2. Recoverable Progress
3. Suggested Next Step
4. Recent Transfers

### Primary Action

- Resume

### Secondary Actions

- View Summary
- Transfer to Another Agent

### Empty State

Title:

- No recoverable progress yet

Body:

- You can start with a new task.

## 2. Needs Review

### Purpose

Collect every item that requires an explicit human decision.

### Content Order

1. Suggested conclusions to remember
2. Proposed project rules
3. Transfer summaries waiting for confirmation

### Primary Action

- Confirm

### Secondary Actions

- Review Later
- Do Not Keep

### Empty State

Title:

- Nothing needs your review

Body:

- Items that need your decision will appear here.

## 3. History

### Purpose

Provide traceability and context without interrupting the main recovery path.

### Content Order

1. Filter row
2. Reverse-chronological list
3. Detail drawer

### Filters

- Conversations
- Recovery
- Transfers
- Outputs

### Actions

- View Details
- Copy Command
- Copy Location

### Empty State

Title:

- No history yet

Body:

- Records will appear here after work begins.

## 4. Help

### Purpose

Act as a triage surface, not as a long documentation center.

### Page Header

Title:

- Need help?

Subtitle:

- Start with the most common questions.

Search placeholder:

- Search questions

### First-Screen Help Cards

1. Continue Previous Work
2. Switch Agent
3. Why wasn't this remembered?
4. Why can't I find @chatmem?
5. Where should I start?

Each card should include:

- a short title
- a one-line explanation
- a single action button

### Recommended Card Copy

#### Continue Previous Work

Description:

- Go back to the latest progress.

Button:

- View Progress

#### Switch Agent

Description:

- Pass the current task to another agent.

Button:

- Start Transfer

#### Why wasn't this remembered?

Description:

- Some information needs review before it is kept.

Button:

- View Review Queue

#### Why can't I find @chatmem?

Description:

- ChatMem often works in the background.

Button:

- See How It Works

#### Where should I start?

Description:

- Start with Continue Work.

Button:

- Go to Continue Work

### Expanded Help Sections

1. Return to recent progress
2. Transfer to another agent
3. What gets remembered
4. Why you do not need @chatmem
5. The best place to begin

### Advanced Troubleshooting

Default state: collapsed

Sections:

- Connection Status
- Configuration Locations
- Resume Commands
- Related Paths

This area is for advanced users and debugging. It should not compete visually with the first-screen help actions.

## Low-Fidelity Wireframes

## App Shell

```text
+----------------------------------------------------------------------------------+
| ChatMem | Continue Work | Needs Review | History | Help     Search  Agent  Settings |
+----------------------------------------------------------------------------------+
|                                                                                  |
|  Main page content for the selected top-level destination                        |
|                                                                                  |
+----------------------------------------------------------------------------------+
```

## Continue Work

```text
+----------------------------------------------------------------------------------+
| Continue Work                                                                    |
| Pick up where you left off.                                                      |
+----------------------------------------------------------------------------------+
| Recent Tasks                                                                     |
| - Project A                                                                      |
| - Project B                                                                      |
+----------------------------------------------------------------------------------+
| Recoverable Progress                                                             |
| Last activity summary                                                            |
| [Resume] [View Summary]                                                          |
+----------------------------------------------------------------------------------+
| Suggested Next Step                                                              |
| One clear recommendation                                                         |
+----------------------------------------------------------------------------------+
| Recent Transfers                                                                 |
| Latest cross-agent transfer summaries                                            |
+----------------------------------------------------------------------------------+
```

## Needs Review

```text
+----------------------------------------------------------------------------------+
| Needs Review                                                                     |
| Confirm the few things that require your decision.                               |
+----------------------------------------------------------------------------------+
| Suggested Conclusions to Remember                                                |
| [Confirm] [Review Later] [Do Not Keep]                                           |
+----------------------------------------------------------------------------------+
| Proposed Project Rules                                                           |
| [Confirm] [Review Later] [Do Not Keep]                                           |
+----------------------------------------------------------------------------------+
| Pending Transfer Summaries                                                       |
| [Confirm] [Review Later]                                                         |
+----------------------------------------------------------------------------------+
```

## History

```text
+----------------------------------------------------------------------------------+
| History                                                                          |
| Review past work only when you need it.                                          |
+----------------------------------------------------------------------------------+
| Filters: [Conversations] [Recovery] [Transfers] [Outputs]                        |
+----------------------------------------------------------------------------------+
| Timeline list                                                                    |
| - Item                                                                           |
| - Item                                                                           |
| - Item                                                                           |
|                                            +-----------------------------------+ |
|                                            | Detail Drawer                     | |
|                                            | View Details                      | |
|                                            | Copy Command                      | |
|                                            | Copy Location                     | |
|                                            +-----------------------------------+ |
+----------------------------------------------------------------------------------+
```

## Help

```text
+----------------------------------------------------------------------------------+
| Need help?                                                                       |
| Start with the most common questions.                                            |
| [ Search questions............................................................ ]  |
+----------------------------------------------------------------------------------+
| [Continue Previous Work] [Switch Agent] [Why wasn't this remembered?]            |
| [Why can't I find @chatmem?] [Where should I start?]                             |
+----------------------------------------------------------------------------------+
| Quick Actions: [View Progress] [Start Transfer] [View Review Queue]              |
+----------------------------------------------------------------------------------+
| Expanded Answer                                                                  |
| Title                                                                            |
| Short explanation                                                                |
| [Primary Action]                                                                 |
+----------------------------------------------------------------------------------+
| Advanced Troubleshooting (collapsed)                                              |
+----------------------------------------------------------------------------------+
```

## Interaction Rules

1. Each page should have one visually dominant action
2. Destructive actions should never be primary by default
3. The app should not require users to understand the memory model before acting
4. Advanced detail should be hidden until requested
5. Help should route users back into the correct task flow, not trap them in documentation

## States

Each main page should support four shared states:

- Loading
- Empty
- Actionable
- Error

### Error Copy Principle

Show the user outcome first, then the likely reason, then the technical detail only on request.

For example:

- We could not load recent progress.
- Check your connection or try again.
- View technical details

Do not lead with MCP, schema, or filesystem errors in the primary message.

## What This Design Intentionally Does Not Do

- It does not turn memory objects into first-class navigation items
- It does not ask ordinary users to maintain a memory graph manually
- It does not make Help a documentation dump
- It does not remove advanced capability; it relocates it

## Success Criteria

This design succeeds if:

- a returning user can identify where to click first within a few seconds
- Help answers common usage questions without teaching the internal model
- review actions are centralized instead of scattered across object tabs
- advanced operational surfaces remain accessible without dominating the UI

## Implementation Notes

When implemented, the existing object-based areas should likely be absorbed like this:

- checkpoints -> Continue Work / History
- repo memory + memory inbox + approvals -> Needs Review
- runs + artifacts -> History or Advanced Troubleshooting
- handoffs -> Continue Work, Needs Review, and History depending on state

This is a restructuring of the user experience, not a removal of the underlying data model.
