# ChatMem Memory Entry Ready Cue Design

## Goal

Add a lightweight readiness cue to the existing `Memory` entry button when the current conversation has just completed automatic local-history bootstrap.

The cue should help the user notice one concrete fact:

- this conversation's project history is now ready to query

It should do that before the user opens the memory drawer, without turning the toolbar into a notification strip.

## Scope

This design changes only the `Memory` button in the conversation toolbar.

Included:

- a small ready cue on the `Memory` button when the current conversation has just completed automatic bootstrap
- clear coexistence rules with the existing inbox count badge
- restrained button styling for the ready state
- tests for the toolbar-level state

Not included:

- changes to scan behavior
- changes to the memory drawer header or tabs
- new toasts, modals, or banners
- new backend state
- any persistence beyond the current conversation-scoped ready event

## Problem

The new ready notice inside `ProjectIndexStatus` works, but it still lives below the toolbar.

That means the user may understand that local history is ready only after their eyes have already moved into the panel body. The decision point for "should I open memory now?" still happens higher up, at the toolbar, where the current `Memory` button only communicates one thing:

- inbox attention count

There is no matching toolbar cue for:

- local history is now ready, and asking about prior discussion should work better

## Approaches Considered

### Option A: Add a small ready cue to the `Memory` button

When the current conversation has a fresh bootstrap-ready event, show a small inline readiness cue directly on the button.

Pros:

- closest to the existing memory entry point
- visible before drawer open
- easy to connect mentally with "open memory now"
- keeps the signal local to the existing toolbar action

Cons:

- needs careful coexistence with the inbox badge

### Option B: Ready styling only, no textual cue

Tint the button green when ready, but do not add any label.

Pros:

- very quiet

Cons:

- weaker meaning
- users may not infer that the repo is now queryable

### Option C: Put the cue only inside the drawer

Show a ready hint in the drawer header or the Approved tab after open.

Pros:

- minimal toolbar change

Cons:

- too late in the interaction
- does not help the user decide to click the memory entry in the first place

## Recommendation

Use **Option A**.

The button already represents the repo-memory entry point. A small inline ready cue there gives the user a timely signal without adding a second surface.

## Selected Design

### Ownership

This cue should reuse the existing conversation-scoped ready event already owned by `App`.

No new durable state is needed.

The button should derive a boolean such as:

```text
showMemoryReadyCue = bootstrapReadyConversationId === selectedConversation.id
```

### Placement

The cue appears inside the existing `Memory` button in the conversation toolbar.

It should sit to the right of the `Memory` label, in the same general zone where the inbox badge already appears.

### Content

Recommended English cue text:

```text
Ready
```

Recommended Simplified Chinese cue text:

```text
已就绪
```

This cue is intentionally shorter than the panel notice. It should act as a compact signal, not a sentence.

### Visual Form

Use a quiet inline cue, not a second heavy badge.

Recommended form when shown:

- a small green dot
- short text (`Ready` / `已就绪`)
- inline alignment with the button label

Avoid adding another rounded numeric-style pill if it can be expressed with a lighter inline cue.

### State Matrix

#### 1. No inbox attention, no bootstrap ready event

Show the current normal button:

```text
Memory
```

No cue, no count badge, no ready styling.

#### 2. No inbox attention, bootstrap ready event is active

Show:

```text
Memory  Ready
```

Where `Ready` is rendered as the small inline cue described above.

Also apply a restrained `is-ready` button style so the button reads as slightly more available than usual.

#### 3. Inbox attention exists, no bootstrap ready event

Keep the current count-badge behavior unchanged.

Example:

```text
Memory  1
```

The button should keep the existing inbox-attention treatment.

#### 4. Inbox attention exists, bootstrap ready event is also active

Keep the inbox count badge as the only visible badge.

Do **not** show both the numeric badge and the `Ready` cue at the same time.

In this combined state:

- keep the numeric inbox badge visible
- keep the existing inbox-first tab-opening behavior
- apply the same restrained `is-ready` button style so readiness is still present without adding a second badge

This prevents the button from becoming visually crowded.

### Interaction Rules

Click behavior must stay exactly as it is today:

- if `memoryAttentionCount > 0`, open the drawer to `Inbox`
- otherwise open the drawer to `Approved`

The ready cue is informational only. It must not change which tab opens or create a new action target.

### Relationship To The Panel Notice

The toolbar cue and the panel notice communicate the same transition at two different scales:

- toolbar cue: "this entry is ready now"
- panel notice: "local history is ready for this project"

They may coexist for the same conversation immediately after automatic bootstrap completes.

That is acceptable because:

- one is a compact action-adjacent signal
- one is a full explanatory sentence in the panel body

### Clearing Rules

The button cue must clear on the same lifecycle boundary as the panel ready notice:

- when the user switches to another conversation
- when the current conversation context is cleared

It must not persist across later revisits unless a future design changes that rule explicitly.

### Visual Treatment

The button should remain recognizably the same control.

Ready styling should be restrained:

- slightly adjusted border/background
- no strong glow
- no success-toast color treatment
- no animation

The cue should feel calm and operational, not celebratory.

## Testing

Add or update toolbar-level tests to cover:

1. ready cue appears on the `Memory` button when the current conversation has a bootstrap-ready event and inbox count is zero
2. inbox count badge still wins when inbox attention exists
3. combined state does not render both a numeric badge and a `Ready` cue simultaneously
4. switching conversations clears the toolbar cue with the same conversation-scoped rule as the panel ready notice

## Success Criteria

This design is successful when:

- users can tell from the toolbar that memory has become more useful now
- the `Memory` button communicates readiness before the drawer opens
- inbox count semantics remain clear
- the button never becomes visually crowded with two competing badges
- the cue disappears with the same one-time lifecycle as the panel ready notice
