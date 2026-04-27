# ChatMem Approved Memory Autofocus Design

## Goal

After a conversation's project has just completed automatic local-history bootstrap, make the first `Memory` drawer open feel more useful by guiding the user directly to readable approved memory.

The intended user feeling is simple:

- ChatMem just became more capable for this project
- opening `Memory` now takes me straight to something worth reading

This should happen once, quietly, without turning the drawer into a focus trap or changing existing tab priority rules.

## Scope

This design extends the one-time bootstrap-ready flow that already exists in the conversation toolbar and project index status.

Included:

- a one-time autofocus intent tied to the current conversation's fresh auto-bootstrap-ready state
- automatic focus on the first approved memory card when the drawer is opened into `Approved`
- cancellation and clearing rules so the behavior runs only once
- tests for drawer open, focus landing, and clearing behavior

Not included:

- changes to backend memory indexing
- changes to `Memory` button click routing
- changes to inbox priority
- new toasts, modals, or banners
- persistent per-repo onboarding state
- automatic scrolling or focus behavior outside the memory drawer

## Problem

The current flow now tells the user two helpful things:

- local history was imported
- the `Memory` entry is ready

But when the user acts on that cue, the drawer still opens like any other normal session. There is no extra help connecting "history is now useful" to "here is the first piece of repository memory you probably want to inspect."

That leaves a small but real gap between the readiness signal and the first meaningful reading target.

## Approaches Considered

### Option A: One-time autofocus to the first approved memory card

When the current conversation has a fresh auto-bootstrap-ready event, the next `Memory` drawer open may autofocus the first approved memory card.

Pros:

- carries the user directly to readable repository memory
- preserves the existing drawer structure
- feels like a one-time assist rather than a permanent behavior
- matches the existing one-time ready cue lifecycle

Cons:

- requires careful cancellation so focus does not replay or land after context changes
- can no-op when there are no approved memories yet

### Option B: Autofocus the `Approved` tab only

When the drawer opens, focus the `Approved` tab button instead of drawer content.

Pros:

- simpler
- very low risk

Cons:

- weaker payoff
- still requires another user step before they reach actual memory content

### Option C: Autofocus the first actionable button inside approved memory

Focus the first `Re-verify` button in the first memory card.

Pros:

- easy keyboard affordance

Cons:

- emphasizes memory maintenance instead of reading prior context
- feels too operational for a "history is ready" moment

## Recommendation

Use **Option A**.

This best matches the user's mental model: after bootstrap finishes, opening `Memory` should briefly feel like being brought to the project's readable memory, not just to another control surface.

## Selected Design

### Ownership

The one-time autofocus should be driven by transient drawer-open intent owned by `App`.

Reason:

- "this conversation just became ready" is already conversation-scoped UI state
- "the next drawer open should autofocus approved memory" is also a transient UI event
- the approved memory panel should execute focus, but it should not decide on its own when the one-time behavior is deserved

`App` should hold a one-time intent conceptually equivalent to:

```text
pendingApprovedMemoryAutofocusForConversationId
```

`RepoMemoryPanel` should receive a simple execution prop, such as:

```text
autoFocusFirstMemory: boolean
```

plus a callback to report that the attempt has completed or should be cleared.

### Trigger Conditions

Set the one-time autofocus intent only when all of the following are true:

1. the current conversation receives the existing bootstrap-ready event from automatic local-history bootstrap
2. the event came from auto bootstrap, not manual rescan
3. the user has not yet consumed or cleared this autofocus opportunity for that conversation

The autofocus attempt may execute only when:

1. the user opens the `Memory` drawer
2. the drawer opens to the `Approved` tab
3. the approved memory panel has at least one memory card available to focus

### Interaction Rules

The existing `Memory` button click behavior must remain unchanged:

- if `memoryAttentionCount > 0`, open `Inbox`
- otherwise open `Approved`

This design must not override inbox-first behavior just to make autofocus possible.

That means:

- if inbox attention exists and the drawer opens to `Inbox`, the autofocus does not run
- the one-time autofocus intent is cleared anyway

This keeps the behavior honest: the user got one assisted entry moment, not a delayed trap waiting for a later tab switch.

### Focus Target

The focus target should be the first approved memory card container itself, not a nested action button.

Why:

- the content is the destination
- focusing the card communicates "start reading here"
- it avoids framing the moment as a maintenance workflow

Implementation shape:

- the first memory card becomes programmatically focusable
- a ref is attached to that first card
- when autofocus is requested and the card is mounted, the card is scrolled into view if needed and then focused

Recommended mechanics:

```text
tabIndex={-1}
```

on the first card only when needed or when harmless to keep permanently.

### Loading and Empty States

If the drawer opens to `Approved` while approved memories are still loading:

- keep the autofocus intent alive until loading resolves or the attempt is canceled

If loading resolves and at least one approved memory exists:

- focus the first card once

If loading resolves and the approved memory list is empty:

- do not fallback to another target
- clear the autofocus intent silently

This is intentional. The feature should help when content exists, but it should not invent a second behavior in empty-state repos.

### One-Time Lifecycle

The autofocus opportunity is single-use.

Clear it when any of the following happens:

- autofocus successfully lands on the first approved memory card
- the drawer is opened into `Inbox` instead of `Approved`
- the drawer closes before the approved panel can complete the focus attempt
- the user switches to another conversation
- the current conversation context is cleared

Do not replay it when:

- the user closes and reopens the drawer later in the same conversation
- the user switches tabs within an already-open drawer
- the repo already had indexed history before this conversation selection
- the user performs manual rescan

### Accessibility

Programmatic focus must remain calm and legible:

- the focused memory card needs a visible focus style
- focus should land on a semantically understandable target, not an inert anonymous wrapper with no visible meaning
- focus movement should happen only once and only for the approved one-time bootstrap-assist case

The drawer must remain keyboard-usable after focus lands:

- tab should continue naturally into controls within the focused card and later cards

### Visual Treatment

No new banner or explanatory copy is required for this behavior.

The experience should rely on:

- the existing ready cue on the `Memory` button
- the existing drawer open
- a clear focus ring on the first approved memory card

The focus ring should match the existing UI language:

- restrained
- clearly visible
- not a neon onboarding highlight

## Testing

Update tests to cover:

1. after a conversation gets the one-time bootstrap-ready state, the first drawer open into `Approved` focuses the first approved memory card
2. the autofocus does not replay after closing and reopening the drawer
3. if inbox count forces the drawer to open to `Inbox`, autofocus does not run and the one-time intent is consumed
4. if the drawer closes or the conversation changes before approved content is ready, autofocus is canceled
5. if approved memory is empty, the autofocus attempt no-ops cleanly without throwing or focusing unrelated controls

## Success Criteria

This design is successful when:

- the first post-bootstrap `Memory` open feels guided rather than generic
- the user lands on readable approved memory, not on maintenance controls
- the behavior runs only once for that conversation's ready moment
- inbox priority remains unchanged
- empty and loading states fail quietly without awkward fallback focus jumps
