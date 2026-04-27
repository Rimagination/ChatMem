# ChatMem Bootstrap Ready Notice Design

## Goal

Add a lightweight success confirmation when ChatMem finishes the first automatic local-history bootstrap for a repo.

The message should answer a simple user question:

- did ChatMem just finish importing this project's local history?

It should do that without turning the project memory panel into a persistent status feed.

## Scope

This design extends the existing `ProjectIndexStatus` panel and the repo-level state already maintained by `App`.

Included:

- a one-time "local history is ready" notice after automatic bootstrap succeeds
- notice shown only for the current repo in the current view
- notice cleared when the user leaves that repo or conversation context
- tests for component state and real workspace flow

Not included:

- toasts
- timers or auto-dismiss animation
- persistent "ready" state across repo switches or app restarts
- changes to scan behavior
- changes to manual rescan behavior
- onboarding modals or global banners

## Problem

The current bootstrap copy handles two important states well:

- local history is not indexed yet
- local history is being imported now

But when the first automatic bootstrap succeeds, the panel quietly drops back to its normal metrics-only state.

That makes the transition easy to miss. A first-time user may not realize that the repo has just crossed from "older conversations may not be searchable yet" into "you can ask about prior discussion now."

## Approaches Considered

### Option A: Session-scoped inline ready notice

When auto bootstrap completes successfully for the active repo, show a single inline confirmation inside `ProjectIndexStatus`.

Pros:

- uses the same panel that already owns bootstrap state
- no new visual layer
- easy to reason about and test
- clears naturally when the user leaves the repo context

Cons:

- does not persist if the user leaves and comes back later

### Option B: Timed flash message

Show the ready notice for a few seconds, then hide it automatically.

Pros:

- very light visual footprint

Cons:

- easy to miss
- adds timer-driven behavior and brittle tests
- not ideal for a status change the user may notice a beat later

### Option C: Persistent indexed-state message

Whenever a repo has indexed history, always show a "history is ready" note.

Pros:

- impossible to miss

Cons:

- too sticky
- turns a one-time transition into permanent chrome
- weakens the calm operational feel of the panel

## Recommendation

Use **Option A**.

This keeps the feedback visible at the moment it matters, without making the success state permanently occupy space in the UI.

## Selected Design

### Ownership

The "just finished bootstrapping" signal should live in `App`, not be derived from `RepoMemoryHealth` alone.

Reason:

- indexed chunks are a durable repo fact
- "just became ready because of this session's auto bootstrap" is a transient UI event

`ProjectIndexStatus` should receive that transient event as an explicit prop, such as:

```text
bootstrapReady: boolean
```

### Trigger Conditions

Set the ready notice only when all of the following are true:

1. the repo entered the auto-bootstrap path because its effective indexed chunk count was zero
2. the scan was started automatically, not by the user pressing the manual rescan button
3. the scan completed successfully
4. the follow-up health refresh shows a usable indexed history count

Where usable indexed history means:

```text
effectiveIndexedChunkCount > 0
```

and

```text
effectiveIndexedChunkCount =
  health?.indexed_chunk_count ?? health?.search_document_count ?? 0
```

Do not set the ready notice when:

- the scan fails
- the scan completes but refreshed health still reports zero usable chunks
- the user manually triggers `Rescan local history`

### Visibility Rules

#### 1. Loading with no health payload yet

Keep the current loading-only state.

No ready notice is shown.

#### 2. Zero chunks and currently scanning

Keep the existing bootstrap scanning note:

```text
Importing local history for this project. Older conversations may not be fully searchable yet. When indexing finishes, you can ask what was discussed before.
```

#### 3. Zero chunks and not scanning

Keep the existing idle zero-index note:

```text
Local history has not been indexed for this project yet, so older conversations may not be fully searchable. After indexing, you can ask what was discussed before.
```

#### 4. Auto bootstrap just completed successfully

Show a lightweight ready notice:

Recommended English copy:

```text
Local history is ready for this project. You can now ask what was discussed before.
```

Recommended Simplified Chinese copy:

```text
这个项目的本地历史已经就绪，现在可以直接问以前讨论过什么。
```

#### 5. Indexed history exists without a fresh auto-bootstrap completion event

Show no notice.

The panel stays in its normal compact metrics state.

### Clearing Rules

The ready notice is one-time and local to the current repo view.

Clear it when:

- the user switches to a conversation in another repo
- the user leaves the current repo-backed conversation context
- the current selection is replaced by a repo that did not just complete auto bootstrap

Do not replay it when:

- the user later returns to the same repo in the same session
- the repo already had indexed history before selection
- the user performs a manual rescan

### Interaction Model

No new buttons or links are added.

The existing scan button remains unchanged:

- `Scanning...` while a scan is active
- `Rescan local history` when idle

The ready notice is informational only.

### Visual Treatment

Use the same restrained note container family as the existing bootstrap note.

The ready notice should feel slightly more affirmative than the zero-index note, but still quiet:

- compact padding
- low-contrast border/background
- standard body text
- no success badge
- no toast styling
- no animation required

This is confirmation, not celebration.

### State Priority

When multiple states could compete, render in this order:

1. loading without health
2. zero chunks + scanning note
3. zero chunks + idle note
4. bootstrap ready notice
5. no note

That means the ready notice only appears after the repo has moved out of the zero-index states.

## Testing

Update tests to cover:

1. `ProjectIndexStatus` shows the ready notice only when `bootstrapReady` is true and effective indexed chunks are nonzero
2. `ProjectIndexStatus` does not show the ready notice when indexed chunks are zero
3. real workspace flow: auto bootstrap transitions from the scanning note to the ready notice after scan success and refreshed health
4. real workspace flow: leaving the repo context clears the ready notice
5. manual rescan does not produce the ready notice

## Success Criteria

This design is successful when:

- a first-time user can see that automatic import has completed
- the user gets a direct cue that prior-discussion recall is now available
- the confirmation appears only at the moment of transition
- the panel does not accumulate permanent success chrome
- returning to the repo later does not replay the message unless a future design explicitly changes that behavior
