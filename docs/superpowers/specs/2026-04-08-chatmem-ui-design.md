# ChatMem UI Refresh Design

**Date:** 2026-04-08

## Goal

Refresh ChatMem into a cleaner Codex App-inspired desktop workspace while adding one practical utility: surface where the underlying conversation file lives and make it easy to hand that location or a native resume command to another agent.

## Context

The current app already has the right product structure: a left conversation list and a right conversation detail area. The weak point is presentation. The detail view still reads like a data inspector instead of a serious chat workspace, and the "how do I continue or hand this off?" workflow is missing.

The user wants:

- a more polished UI, closer to Codex App than to a generic React admin tool
- explicit display of where the conversation file is stored
- one-click copying for the actual handoff workflow
- clearer user-side message bubbles
- an application icon unique to ChatMem

## Product Decision

### Primary handoff model

There are two distinct user jobs:

1. Ask another agent to read the old conversation content from disk
2. Resume the same conversation in its native agent

Those jobs should not be merged into one ambiguous "copy address" action.

The UI will therefore expose:

- **Conversation file location**: the concrete on-disk file path for the selected conversation
- **Resume command**: the exact native command to continue that conversation in the source agent

The UI will not expose a separate "conversation address" concept such as `project_dir + id`, because that is an internal index, not the most useful end-user artifact for the handoff workflow.

### Copy actions

The language will be:

- label: `对话文件位置`
- action: `复制位置`
- secondary action: `复制恢复命令`

This is more task-oriented than "copy path" or "copy session address".

## Visual Direction

### Visual thesis

Quiet, professional, shallow-depth desktop workspace with Codex-like reading hierarchy: bright surface, soft structure, high legibility, and deliberate emphasis on the message stream instead of decorative chrome.

### Layout thesis

Keep the existing two-pane architecture, but change the balance of emphasis:

- the left pane becomes a denser navigation rail
- the right pane becomes a true conversation workspace
- the conversation header gains a compact metadata strip for file location and actions

### Interaction thesis

Use restrained motion only where it sharpens hierarchy:

- subtle list-row emphasis on hover/select
- message reveal transitions when switching conversations
- feedback state on copy actions

## Information Architecture

### Left pane

The left pane remains responsible for:

- source-agent switching
- conversation search
- conversation selection

Changes:

- reduce the heavy card feeling of conversation rows
- tighten spacing and improve scan order
- keep title first, metadata second
- preserve current functionality and filtering behavior

### Right pane

The right pane becomes a stacked workspace:

1. conversation title row
2. conversation metadata strip
3. lightweight stats row
4. message stream
5. file changes section

The metadata strip is the new functional center for handoff tasks.

## Metadata Strip Design

### Contents

The metadata strip will display:

- `对话文件位置`
- the resolved file path for the selected conversation
- a `复制位置` button
- a `复制恢复命令` button

### Behavior

- if file location exists, show the full value in a truncating but selectable layout
- if file location is unavailable, show a clear unavailable state rather than leaving the area blank
- copy actions provide immediate local feedback such as temporary button state or inline success text
- copy actions do not use blocking alerts

### Resume command rules

Resume commands are generated from the selected conversation's source agent and ID:

- Claude: `claude --resume <id>`
- Codex: `codex resume <id>`
- Gemini: `gemini --resume <id>`

This follows AgentSwap's native resume model.

## Message Stream Design

### Message hierarchy

The message stream should read like a conversation first and a debug object viewer second.

- `user` messages: right aligned, filled bubble, stronger color block, compact metadata
- `assistant` messages: left aligned, light surface, minimal border, more neutral tone
- `system` messages: visually subordinate to assistant messages

### Bubble rules

User bubbles should feel intentionally authored, not merely tinted:

- strong rounded rectangle shape
- narrower max width than the full reading column
- clear separation from assistant content
- timestamp and role labels should support the bubble, not dominate it

### Tool calls

Tool calls remain visible, but as secondary supporting artifacts:

- grouped beneath the owning message
- reduced visual contrast compared with primary message content
- more compact spacing and framing

## File Changes Section

Keep file changes in the detail view, but visually subordinate them beneath the message stream. This remains useful product information, but should not compete with the conversation itself.

## App Icon

### Icon direction

Create a simple ChatMem identity that combines:

- a chat bubble silhouette
- a memory cue such as a node, notch, or signal mark

The icon should:

- work at small desktop sizes
- avoid illustration complexity
- use one primary accent family consistent with the refreshed UI

### Scope

Update the Tauri application icon assets so the packaged app has a real ChatMem icon, not just in-app branding text.

## Data and Backend Changes

### Required backend additions

The current frontend payload does not include source file location. The selected conversation response must be extended with:

- `storage_path`: resolved absolute path to the backing conversation file, when available
- `resume_command`: generated native resume command for the selected agent and conversation ID

These fields should be computed in the Tauri backend rather than assembled in scattered frontend logic.

### Adapter expectations

- Claude adapter can resolve the backing `.jsonl` path from the session ID
- Codex adapter can resolve the rollout path from its thread metadata
- Gemini adapter can resolve the backing session JSON path

If a path cannot be resolved, the backend should return `null` and the UI should render the unavailable state.

## Frontend Scope

### Files expected to change

- `src/App.tsx`
- `src/components/ConversationDetail.tsx`
- `src/components/ConversationList.tsx`
- `src/styles.css`
- `src/__tests__/App.test.tsx`
- additional focused component tests if needed
- `src-tauri/src/main.rs`
- Tauri icon assets and config references

### Out of scope

- no search behavior redesign
- no migration workflow redesign
- no deletion workflow redesign
- no move to a single-column chat app
- no persistence or schema changes beyond exposing path/command metadata

## Error Handling

- If clipboard copy fails, show non-blocking inline failure feedback
- If file location is unavailable, keep the metadata row visible with a disabled copy action
- If resume command cannot be built, disable only that action while keeping file location visible

## Testing Strategy

Implementation must follow TDD for new behavior.

Coverage should include:

- metadata strip renders selected conversation file location
- unavailable state renders correctly when storage path is missing
- copy buttons render with the approved labels
- resume command text is derived from agent + conversation ID
- user messages render with the dedicated bubble treatment class

## Acceptance Criteria

- the refreshed UI reads as a polished desktop chat workspace rather than a generic inspector
- the selected conversation clearly displays `对话文件位置`
- users can click `复制位置`
- users can click `复制恢复命令`
- user-authored messages visibly use a stronger chat bubble treatment
- the packaged app has a dedicated ChatMem icon
