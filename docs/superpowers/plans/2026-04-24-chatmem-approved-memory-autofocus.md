# ChatMem Approved Memory Autofocus Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** After automatic local-history bootstrap makes a conversation ready, make the next `Memory` drawer open guide the user directly to the first approved memory card once, without changing inbox-first behavior.

**Architecture:** `App.tsx` will own a transient, conversation-scoped autofocus intent that is created alongside the existing bootstrap-ready event and consumed on the next relevant drawer open. `RepoMemoryPanel.tsx` will execute the focus itself: it will make the first approved memory card programmatically focusable, scroll it into view, focus it once, and report completion back to `App` so the behavior cannot replay.

**Tech Stack:** React, TypeScript, CSS, Vitest, Testing Library.

---

## Scope

This plan implements `docs/superpowers/specs/2026-04-23-chatmem-approved-memory-autofocus-design.md`.

Included:

- one-time autofocus intent owned by `App.tsx`
- autofocus of the first approved memory card in `RepoMemoryPanel.tsx`
- quiet cancel/consume behavior when the drawer opens to `Inbox`
- card-level focus styling
- focused tests for component autofocus mechanics and workspace lifecycle behavior

Not included:

- scan behavior changes
- changes to `Memory` button tab-selection logic
- new toasts, banners, or onboarding copy
- persistent onboarding state across repo revisits or app restarts

## File Structure

- `src/components/RepoMemoryPanel.tsx`
  - Accept the autofocus execution props from `App`.
  - Focus the first approved memory card once when asked.
  - Clear the autofocus request when the list is empty.
- `src/App.tsx`
  - Own the one-time autofocus intent.
  - Seed the intent when auto bootstrap succeeds.
  - Consume or clear the intent on drawer open, drawer close, and conversation changes.
  - Pass the execution props into `RepoMemoryPanel`.
- `src/styles.css`
  - Add a visible but restrained focus treatment for a programmatically focused approved memory card.
- `src/__tests__/RepoMemoryPanel.test.tsx`
  - New component-level regression coverage for focus execution, loading deferral, and empty-state no-op behavior.
- `src/__tests__/MemoryWorkspace.test.tsx`
  - Real workspace coverage for the one-time autofocus lifecycle and inbox-first consume path.

## Task 1: Add Repo Memory Panel Autofocus Mechanics

**Files:**
- Create: `src/__tests__/RepoMemoryPanel.test.tsx`
- Modify: `src/components/RepoMemoryPanel.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: Write failing component tests for focus execution**

Create `src/__tests__/RepoMemoryPanel.test.tsx` with:

```tsx
import { render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import RepoMemoryPanel from "../components/RepoMemoryPanel";
import type { ApprovedMemory } from "../chatmem-memory/types";

const scrollIntoViewMock = vi.fn();
const onReverify = vi.fn();
const onAutoFocusHandled = vi.fn();

const approvedMemory: ApprovedMemory = {
  memory_id: "mem-001",
  kind: "command",
  title: "Primary verification",
  value: "npm run test:run",
  usage_hint: "Use before handoff",
  status: "active",
  freshness_status: "fresh",
  freshness_score: 0.93,
  last_verified_at: "2026-04-19T09:00:00Z",
  verified_by: "codex",
  selected_because: null,
  evidence_refs: [],
};

describe("RepoMemoryPanel", () => {
  beforeEach(() => {
    onReverify.mockReset();
    onAutoFocusHandled.mockReset();
    scrollIntoViewMock.mockReset();
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: scrollIntoViewMock,
    });
  });

  it("focuses the first approved memory card when autofocus is requested", async () => {
    render(
      <RepoMemoryPanel
        memories={[approvedMemory]}
        loading={false}
        locale="en"
        onReverify={onReverify}
        autoFocusFirstMemory={true}
        onAutoFocusHandled={onAutoFocusHandled}
      />,
    );

    const firstCard = screen.getByText("Primary verification").closest("article");
    expect(firstCard).toBeTruthy();

    await waitFor(() => {
      expect(document.activeElement).toBe(firstCard);
    });

    expect(scrollIntoViewMock).toHaveBeenCalledTimes(1);
    expect(onAutoFocusHandled).toHaveBeenCalledTimes(1);
  });

  it("waits for loading to finish before focusing the first approved memory card", async () => {
    const { rerender } = render(
      <RepoMemoryPanel
        memories={[approvedMemory]}
        loading={true}
        locale="en"
        onReverify={onReverify}
        autoFocusFirstMemory={true}
        onAutoFocusHandled={onAutoFocusHandled}
      />,
    );

    expect(onAutoFocusHandled).not.toHaveBeenCalled();

    rerender(
      <RepoMemoryPanel
        memories={[approvedMemory]}
        loading={false}
        locale="en"
        onReverify={onReverify}
        autoFocusFirstMemory={true}
        onAutoFocusHandled={onAutoFocusHandled}
      />,
    );

    const firstCard = screen.getByText("Primary verification").closest("article");
    expect(firstCard).toBeTruthy();

    await waitFor(() => {
      expect(document.activeElement).toBe(firstCard);
    });

    expect(onAutoFocusHandled).toHaveBeenCalledTimes(1);
  });

  it("clears autofocus quietly when approved memory is empty", async () => {
    render(
      <RepoMemoryPanel
        memories={[]}
        loading={false}
        locale="en"
        onReverify={onReverify}
        autoFocusFirstMemory={true}
        onAutoFocusHandled={onAutoFocusHandled}
      />,
    );

    expect(await screen.findByText("No approved repository memory yet.")).toBeTruthy();

    await waitFor(() => {
      expect(onAutoFocusHandled).toHaveBeenCalledTimes(1);
    });

    expect(scrollIntoViewMock).not.toHaveBeenCalled();
  });
});
```

- [ ] **Step 2: Run the new component test file and verify it fails**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/RepoMemoryPanel.test.tsx
```

Expected:

- FAIL because `RepoMemoryPanel` does not yet accept `autoFocusFirstMemory` or `onAutoFocusHandled`
- FAIL because the first approved memory card is not programmatically focusable or focused

- [ ] **Step 3: Implement autofocus props and focus execution in `RepoMemoryPanel.tsx`**

In `src/components/RepoMemoryPanel.tsx`, update the imports and prop type:

```tsx
import { useEffect, useRef } from "react";
import type { ApprovedMemory } from "../chatmem-memory/types";
import type { Locale } from "../i18n/types";

type RepoMemoryPanelProps = {
  memories: ApprovedMemory[];
  loading: boolean;
  locale: Locale;
  onReverify: (memoryId: string) => void;
  autoFocusFirstMemory?: boolean;
  onAutoFocusHandled?: () => void;
};
```

Then update the component signature and add the ref/effect near the top of the component body:

```tsx
export default function RepoMemoryPanel({
  memories,
  loading,
  locale,
  onReverify,
  autoFocusFirstMemory = false,
  onAutoFocusHandled,
}: RepoMemoryPanelProps) {
  const firstMemoryCardRef = useRef<HTMLElement | null>(null);
  const isEnglish = locale === "en";
  const copy = {
    empty: isEnglish
      ? "No approved repository memory yet."
      : "\u6682\u65e0\u5df2\u6279\u51c6\u7684\u4ed3\u5e93\u8bb0\u5fc6\u3002",
    heading: isEnglish ? "Repo Memory" : "\u4ed3\u5e93\u8bb0\u5fc6",
    subtitle: isEnglish
      ? "Approved repository memory that can be used for startup context and handoffs."
      : "\u5df2\u6279\u51c6\u7684\u4ed3\u5e93\u8bb0\u5fc6\uff0c\u53ef\u7528\u4e8e\u542f\u52a8\u4e0a\u4e0b\u6587\u548c\u4ea4\u63a5\u3002",
    freshnessScore: isEnglish ? "Freshness score" : "\u65b0\u9c9c\u5ea6\u5206\u6570",
    reverify: isEnglish ? "Re-verify" : "\u91cd\u65b0\u9a8c\u8bc1",
  };

  useEffect(() => {
    if (!autoFocusFirstMemory || loading) {
      return;
    }

    if (memories.length === 0) {
      onAutoFocusHandled?.();
      return;
    }

    const firstCard = firstMemoryCardRef.current;
    if (!firstCard) {
      return;
    }

    firstCard.scrollIntoView({ block: "nearest" });
    firstCard.focus();
    onAutoFocusHandled?.();
  }, [autoFocusFirstMemory, loading, memories, onAutoFocusHandled]);
```

Finally, inside the `memories.map(...)` render, replace the opening `<article>` tag with:

```tsx
            <article
              key={memory.memory_id}
              className="memory-card"
              ref={index === 0 ? firstMemoryCardRef : undefined}
              tabIndex={index === 0 ? -1 : undefined}
            >
```

and update the map signature to capture `index`:

```tsx
        {memories.map((memory, index) => {
```

- [ ] **Step 4: Add the approved-memory focus style**

In `src/styles.css`, directly after the existing `.memory-card` block, add:

```css
.memory-card:focus,
.memory-card:focus-visible {
  outline: none;
  border-color: rgba(58, 143, 100, 0.34);
  box-shadow: 0 0 0 3px rgba(58, 143, 100, 0.14);
}
```

- [ ] **Step 5: Re-run the new component tests**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/RepoMemoryPanel.test.tsx
```

Expected: PASS with all `RepoMemoryPanel` autofocus tests green.

- [ ] **Step 6: Commit Task 1**

```powershell
git add src/components/RepoMemoryPanel.tsx src/styles.css src/__tests__/RepoMemoryPanel.test.tsx
git commit -m "feat: autofocus first approved ChatMem memory"
```

## Task 2: Wire The One-Time Autofocus Intent Through App

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/__tests__/MemoryWorkspace.test.tsx`

- [ ] **Step 1: Write failing workspace tests for one-time autofocus lifecycle**

In `src/__tests__/MemoryWorkspace.test.tsx`, add these three tests near the existing bootstrap-ready coverage:

```tsx
  it("autofocuses the first approved memory card on the first post-bootstrap Memory open", async () => {
    let healthCallCount = 0;
    const baseImplementation = mockInvoke.getMockImplementation();
    if (!baseImplementation) {
      throw new Error("Missing base invoke mock implementation");
    }

    mockInvoke.mockImplementation((command: string, payload?: Record<string, unknown>) => {
      if (command === "list_memory_candidates") {
        return Promise.resolve([]);
      }

      if (
        command === "get_repo_memory_health" &&
        payload?.repoRoot === "D:/VSP/agentswap-gui"
      ) {
        healthCallCount += 1;
        return Promise.resolve({
          repo_root: "D:/VSP/agentswap-gui",
          canonical_repo_root: "D:/VSP/agentswap-gui",
          approved_memory_count: 1,
          pending_candidate_count: 0,
          search_document_count: healthCallCount === 1 ? 0 : 4,
          indexed_chunk_count: healthCallCount === 1 ? 0 : 8,
          inherited_repo_roots: [],
          conversation_counts_by_agent:
            healthCallCount === 1 ? [] : [{ source_agent: "claude", conversation_count: 1 }],
          repo_aliases: [],
          warnings: [],
        });
      }

      return baseImplementation(command, payload);
    });

    renderApp();

    fireEvent.click((await screen.findAllByText("Memory workflow"))[0]);

    const memoryButton = await screen.findByRole("button", { name: "Memory" });
    fireEvent.click(memoryButton);

    const firstCard = (await screen.findByText("Primary verification")).closest("article");
    expect(firstCard).toBeTruthy();

    await waitFor(() => {
      expect(document.activeElement).toBe(firstCard);
    });
  });

  it("does not replay approved-memory autofocus after the drawer is closed and reopened", async () => {
    let healthCallCount = 0;
    const baseImplementation = mockInvoke.getMockImplementation();
    if (!baseImplementation) {
      throw new Error("Missing base invoke mock implementation");
    }

    mockInvoke.mockImplementation((command: string, payload?: Record<string, unknown>) => {
      if (command === "list_memory_candidates") {
        return Promise.resolve([]);
      }

      if (
        command === "get_repo_memory_health" &&
        payload?.repoRoot === "D:/VSP/agentswap-gui"
      ) {
        healthCallCount += 1;
        return Promise.resolve({
          repo_root: "D:/VSP/agentswap-gui",
          canonical_repo_root: "D:/VSP/agentswap-gui",
          approved_memory_count: 1,
          pending_candidate_count: 0,
          search_document_count: healthCallCount === 1 ? 0 : 4,
          indexed_chunk_count: healthCallCount === 1 ? 0 : 8,
          inherited_repo_roots: [],
          conversation_counts_by_agent:
            healthCallCount === 1 ? [] : [{ source_agent: "claude", conversation_count: 1 }],
          repo_aliases: [],
          warnings: [],
        });
      }

      return baseImplementation(command, payload);
    });

    renderApp();

    fireEvent.click((await screen.findAllByText("Memory workflow"))[0]);
    fireEvent.click(await screen.findByRole("button", { name: "Memory" }));

    const firstCard = (await screen.findByText("Primary verification")).closest("article");
    expect(firstCard).toBeTruthy();

    await waitFor(() => {
      expect(document.activeElement).toBe(firstCard);
    });

    fireEvent.click(screen.getByRole("button", { name: "Close memory drawer" }));
    expect(screen.queryByRole("complementary", { name: "Project Memory" })).toBeNull();

    const memoryButton = screen.getByRole("button", { name: "Memory" });
    memoryButton.focus();
    fireEvent.click(memoryButton);

    expect(await screen.findByRole("complementary", { name: "Project Memory" })).toBeTruthy();
    expect(document.activeElement).toBe(memoryButton);
  });

  it("consumes approved-memory autofocus when inbox attention forces the drawer to open in Inbox", async () => {
    let healthCallCount = 0;
    const baseImplementation = mockInvoke.getMockImplementation();
    if (!baseImplementation) {
      throw new Error("Missing base invoke mock implementation");
    }

    mockInvoke.mockImplementation((command: string, payload?: Record<string, unknown>) => {
      if (
        command === "get_repo_memory_health" &&
        payload?.repoRoot === "D:/VSP/agentswap-gui"
      ) {
        healthCallCount += 1;
        return Promise.resolve({
          repo_root: "D:/VSP/agentswap-gui",
          canonical_repo_root: "D:/VSP/agentswap-gui",
          approved_memory_count: 1,
          pending_candidate_count: 1,
          search_document_count: healthCallCount === 1 ? 0 : 4,
          indexed_chunk_count: healthCallCount === 1 ? 0 : 8,
          inherited_repo_roots: [],
          conversation_counts_by_agent:
            healthCallCount === 1 ? [] : [{ source_agent: "claude", conversation_count: 1 }],
          repo_aliases: [],
          warnings: [],
        });
      }

      return baseImplementation(command, payload);
    });

    renderApp();

    fireEvent.click((await screen.findAllByText("Memory workflow"))[0]);

    const memoryButton = await screen.findByRole("button", { name: "Memory" });
    expect(memoryButton.className).toContain("is-ready");

    fireEvent.click(memoryButton);
    expect(await screen.findByText("Review pending memory")).toBeTruthy();

    const approvedTab = screen.getByRole("tab", { name: "Approved 1" });
    approvedTab.focus();
    fireEvent.click(approvedTab);

    expect(await screen.findByText("Primary verification")).toBeTruthy();
    expect(document.activeElement).toBe(approvedTab);
  });
```

- [ ] **Step 2: Run the workspace test file and verify it fails**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/MemoryWorkspace.test.tsx
```

Expected:

- FAIL because `App.tsx` does not yet track a one-time approved-memory autofocus intent
- FAIL because the drawer close and inbox-open paths do not yet consume that intent

- [ ] **Step 3: Add the one-time autofocus intent state and lifecycle to `App.tsx`**

In `src/App.tsx`, add the new state immediately after `bootstrapReadyConversationId`:

```tsx
  const [pendingApprovedMemoryAutofocusConversationId, setPendingApprovedMemoryAutofocusConversationId] =
    useState<string | null>(null);
```

Update the existing `activeConversationId` cleanup effect to also clear the new intent when the conversation changes:

```tsx
  useEffect(() => {
    setCopyState({ target: null, status: "idle" });
    setBootstrapReadyConversationId((current) =>
      current === activeConversationId ? current : null,
    );
    setPendingApprovedMemoryAutofocusConversationId((current) =>
      current === activeConversationId ? current : null,
    );
  }, [activeConversationId]);
```

In the `!activeRepoRoot` branch of the repo-root effect, add:

```tsx
      setPendingApprovedMemoryAutofocusConversationId(null);
```

Inside `runRepoScan`, immediately after `setBootstrapReadyConversationId(...)`, seed the new intent:

```tsx
        if (shouldAnnounceBootstrapReady) {
          const readyConversationId = options?.requestConversationId ?? null;
          setBootstrapReadyConversationId(readyConversationId);
          setPendingApprovedMemoryAutofocusConversationId(readyConversationId);
        }
```

Add a close helper above `renderMemoryDrawer`:

```tsx
  const closeMemoryDrawer = useCallback(() => {
    setMemoryDrawerOpen(false);
    setPendingApprovedMemoryAutofocusConversationId((current) =>
      current === activeConversationIdRef.current ? null : current,
    );
  }, []);
```

Then update the `Memory` button click handler to consume the intent if the drawer opens into `Inbox`:

```tsx
              onClick={() => {
                const nextTab = memoryAttentionCount > 0 ? "inbox" : "approved";
                setMemoryDrawerTab(nextTab);
                setMemoryDrawerOpen(true);
                if (nextTab !== "approved") {
                  setPendingApprovedMemoryAutofocusConversationId((current) =>
                    current === selectedConversation.id ? null : current,
                  );
                }
              }}
```

- [ ] **Step 4: Pass the execution props into `RepoMemoryPanel` and use the close helper**

Still in `src/App.tsx`, add these values inside `renderMemoryDrawer`, before `renderDrawerTab`:

```tsx
    const shouldAutoFocusFirstApprovedMemory =
      memoryDrawerTab === "approved" &&
      pendingApprovedMemoryAutofocusConversationId === activeConversationId;
    const handleApprovedMemoryAutofocusHandled = () => {
      setPendingApprovedMemoryAutofocusConversationId((current) =>
        current === activeConversationId ? null : current,
      );
    };
```

Replace the existing `RepoMemoryPanel` usage with:

```tsx
          <RepoMemoryPanel
            memories={repoMemories}
            loading={memoryLoading}
            locale={locale}
            onReverify={(memoryId) => void handleReverifyMemory(memoryId)}
            autoFocusFirstMemory={shouldAutoFocusFirstApprovedMemory}
            onAutoFocusHandled={handleApprovedMemoryAutofocusHandled}
          />
```

Replace both drawer-close call sites with the helper:

```tsx
      <div className="memory-drawer-overlay" onMouseDown={closeMemoryDrawer}>
```

and

```tsx
              onClick={closeMemoryDrawer}
```

- [ ] **Step 5: Re-run the workspace tests**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/MemoryWorkspace.test.tsx
```

Expected: PASS with all memory-workspace tests green, including the new autofocus lifecycle coverage.

- [ ] **Step 6: Commit Task 2**

```powershell
git add src/App.tsx src/__tests__/MemoryWorkspace.test.tsx
git commit -m "feat: guide first Memory open to approved history"
```

## Final Verification

- [ ] **Step 1: Run the targeted test files together**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/RepoMemoryPanel.test.tsx src/__tests__/MemoryWorkspace.test.tsx
```

Expected: PASS with all new autofocus tests green.

- [ ] **Step 2: Run the full frontend suite**

Run:

```powershell
npm.cmd run test:run
```

Expected: PASS with all Vitest files green.

- [ ] **Step 3: Run the production build**

Run:

```powershell
npm.cmd run build
```

Expected: PASS with `tsc && vite build` completing successfully.

- [ ] **Step 4: Inspect the working tree and recent commits**

Run:

```powershell
git status --short
git log --oneline -8
```

Expected:

- working tree is clean
- the autofocus implementation commits are at the top of the branch

## Spec Coverage Self-Review

- One-time autofocus intent tied to the current conversation's ready event: Task 2.
- Focus lands on the first approved memory card, not on `Re-verify`: Task 1.
- Inbox-first tab behavior remains unchanged and consumes the intent instead of delaying it: Task 2.
- Empty-state no-op behavior is covered: Task 1 component test 3.
- Loading deferral behavior is covered: Task 1 component test 2.
- Drawer close prevents replay by clearing the intent: Task 2.
- No new banners, copy, or backend changes are introduced: enforced by file scope.

## Placeholder Scan

- No `TODO`, `TBD`, or deferred placeholders remain.
- Every code-changing step includes concrete code or markup.
- Every verification step includes exact commands and expected outcomes.
- All prop names introduced in Task 1 are reused consistently in Task 2.
