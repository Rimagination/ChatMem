# ChatMem Bootstrap Status Copy Design

## Goal

Make ChatMem's first-run local-history bootstrap state legible to the user.

When a repository does not yet have a usable local history index, the project history panel should explain two things at the same time:

- why older conversations may not be fully searchable yet
- what the user will be able to do once indexing finishes

The desired tone is persistent and mixed:

- explain the current limitation
- explain the expected outcome
- keep the surface calm and operational, not promotional or alarmist

## Scope

This design changes only the copy and state presentation inside the existing `ProjectIndexStatus` panel.

Included:

- persistent explanatory message when the effective indexed history count is zero
- one copy variant for "currently scanning"
- one copy variant for "not scanning yet / scan finished with zero indexed chunks"
- message disappears once the repo has a usable history index

Not included:

- new onboarding modals
- toasts
- banners outside the project history panel
- new settings
- background job architecture
- changes to scan behavior itself

## Problem

The current `ProjectIndexStatus` panel shows counts and a scan button, but for a fresh repo with `indexed_chunk_count === 0` it can still read as a normal dashboard panel with empty metrics.

That leaves an important ambiguity:

- is ChatMem finished and simply knows nothing?
- or is local history not ready yet?

Without explicit explanation, users can still interpret "0 chunks" as "this system has no memory" rather than "this repo's local history has not been indexed yet."

## Approaches Considered

### Option A: Inline explanation inside `ProjectIndexStatus`

Add a short explanatory copy block between the panel header and the metric grid.

Pros:

- closest to the scan button and chunk counts
- minimal layout churn
- preserves the current utilitarian workspace shape
- easiest to keep persistent without becoming noisy

Cons:

- less attention-grabbing than a page-level banner

### Option B: Workspace-level banner

Show a full-width notice above the project history panel.

Pros:

- very visible

Cons:

- visually louder than the current workspace
- duplicates information that already belongs to the local history panel
- risks making the conversation workspace feel like an onboarding flow

### Option C: Zero-index special empty-state card

Replace the upper part of `ProjectIndexStatus` with a dedicated empty-state experience whenever chunks are zero.

Pros:

- strongest explanation

Cons:

- heavier than needed
- turns a small operational panel into a mini flow
- over-rotates for a status message that should disappear naturally once indexing succeeds

## Recommendation

Use **Option A**.

This keeps the explanation anchored to the exact surface that owns the scan state, without adding a new visual layer to the workspace.

## Selected Design

### Placement

Keep the current panel structure:

1. panel label and repo root
2. scan button
3. explanatory note
4. metric grid
5. warnings list

The new note sits between the header and the metric grid.

### Visibility Rule

Show the explanatory note when:

```text
effectiveIndexedChunkCount === 0
```

Where:

```text
effectiveIndexedChunkCount =
  health?.indexed_chunk_count ?? health?.search_document_count ?? 0
```

This keeps the copy aligned with the count the panel already displays.

Hide the note when:

```text
effectiveIndexedChunkCount > 0
```

### State Matrix

#### 1. Loading with no health payload yet

Keep the current loading-only state.

No new explanatory note is shown yet because the panel does not know whether the repo is empty or already indexed.

#### 2. Zero chunks and currently scanning

Show a persistent mixed-message note that says:

- local history is being imported now
- older conversations may not be fully searchable until indexing finishes
- after indexing, the user can directly ask what was discussed before

Recommended English copy:

```text
Importing local history for this project. Older conversations may not be fully searchable yet. When indexing finishes, you can ask what was discussed before.
```

Recommended Chinese copy:

```text
正在导入这个项目的本地历史。索引完成前，旧对话可能还找不全。完成后，你可以直接问以前讨论过什么。
```

#### 3. Zero chunks and not currently scanning

Show a persistent mixed-message note that says:

- this repo still does not have a local history index
- older conversations may not be fully searchable yet
- after indexing, the user can directly ask what was discussed before

Recommended English copy:

```text
Local history has not been indexed for this project yet, so older conversations may not be fully searchable. After indexing, you can ask what was discussed before.
```

Recommended Chinese copy:

```text
这个项目的本地历史还没有建立索引，所以旧对话暂时可能找不全。完成导入后，你可以直接问以前讨论过什么。
```

#### 4. Indexed history exists

Hide the explanatory note completely.

The panel returns to the compact metrics-only state.

## Interaction Model

No new actions are introduced.

The existing scan button remains the only explicit action:

- `Scanning...` while active
- `Rescan local history` when idle

The note is explanatory only. It should not contain inline links, secondary buttons, or dismiss controls.

## Visual Treatment

The message should read as guidance, not as an error.

Use a restrained note style inside the panel:

- neutral or subtly tinted background
- compact padding
- body-sized text
- no nested card framing
- no warning red unless the existing `warnings` list is also present

This should feel like a quiet operational note, not like a system alert.

## Content Principles

The copy should:

- avoid saying ChatMem "does not remember"
- avoid implying that approved memory is missing or broken
- explicitly tie the limitation to local history indexing
- explicitly state the future benefit in plain language

The copy should not:

- mention internal APIs
- mention MCP
- mention `indexed_chunk_count`
- talk about embeddings, chunks, or database details

## Testing

Add or update component tests for `ProjectIndexStatus` to cover:

1. zero chunks + `scanning={false}` shows the idle explanatory copy
2. zero chunks + `scanning={true}` shows the scanning explanatory copy
3. nonzero chunks hides the explanatory copy
4. existing scan button behavior still works

If needed, keep one integration test at the workspace level to ensure the new note appears in the real conversation workspace when local history is still empty.

## Success Criteria

This design is successful when:

- a first-time user can immediately tell that local history is not ready yet
- the panel explains why old conversations may not be fully searchable
- the panel tells the user what will become possible after indexing
- the workspace remains quiet and tool-like instead of becoming an onboarding flow
- the explanatory note disappears automatically once history indexing is available
