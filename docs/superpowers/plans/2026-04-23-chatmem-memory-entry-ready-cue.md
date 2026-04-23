# ChatMem Memory Entry Ready Cue Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a lightweight ready cue to the `Memory` toolbar button when the current conversation has just completed automatic ChatMem bootstrap, while keeping the inbox count badge as the dominant signal when pending memory review exists.

**Architecture:** This is a small frontend-only extension of the existing conversation-scoped bootstrap-ready event already owned by `App.tsx`. The `Memory` button will derive its cue directly from `bootstrapReadyConversationId === selectedConversation.id`, render a small inline `Ready` label only when inbox attention is zero, and use a restrained `is-ready` button style when the ready event is active. Toolbar-level regression coverage will live in `App.test.tsx`, including the async conversation-switch case so the cue clears on the same lifecycle boundary as the panel notice.

**Tech Stack:** React, TypeScript, CSS, Vitest, Testing Library.

---

## Scope

This plan implements `docs/superpowers/specs/2026-04-23-chatmem-memory-entry-ready-cue-design.md`.

Included:

- `Memory` button ready cue
- restrained `is-ready` button styling
- inbox-badge precedence over the ready cue text
- toolbar-level tests for no-inbox ready state, combined ready+inbox state, and async conversation-switch clearing

Not included:

- changes to scan behavior
- changes to the memory drawer contents or tab logic
- changes to the `ProjectIndexStatus` ready notice
- toasts, banners, or modals

## File Structure

- `src/App.tsx`
  - Derive toolbar cue state from the existing conversation-scoped bootstrap-ready event.
  - Render the inline `Ready` cue only when inbox attention is zero.
  - Apply `is-ready` styling even when the inbox badge wins, so readiness is still signaled without a second badge.
- `src/styles.css`
  - Add the visual treatment for the ready-state button and its inline cue.
- `src/__tests__/App.test.tsx`
  - Cover the toolbar state matrix and the async conversation-switch clearing behavior.

## Task 1: Add The Memory Toolbar Ready Cue

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/styles.css`
- Modify: `src/__tests__/App.test.tsx`

- [ ] **Step 1: Write the failing toolbar tests**

In `src/__tests__/App.test.tsx`, update the Testing Library import so it includes `within`:

```tsx
import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
```

Then add these three tests after `it("shows conversation details, migration, copy actions, and memory drawer in one workspace", ...)`:

```tsx
  it("shows a ready cue on the Memory button after automatic bootstrap finishes", async () => {
    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    let healthCallCount = 0;
    const baseImplementation = mockInvoke.getMockImplementation();
    if (!baseImplementation) {
      throw new Error("Missing base invoke mock implementation");
    }

    mockInvoke.mockImplementation((command: string, payload?: Record<string, unknown>) => {
      if (command === "get_repo_memory_health" && payload?.repoRoot === "D:/VSP/demo") {
        healthCallCount += 1;
        return Promise.resolve({
          repo_root: "D:/VSP/demo",
          canonical_repo_root: "D:/VSP/demo",
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

      if (command === "scan_repo_conversations" && payload?.repoRoot === "D:/VSP/demo") {
        return Promise.resolve({
          repo_root: "D:/VSP/demo",
          canonical_repo_root: "D:/VSP/demo",
          scanned_conversation_count: 1,
          linked_conversation_count: 1,
          skipped_conversation_count: 0,
          source_agents: [{ source_agent: "claude", conversation_count: 1 }],
          warnings: [],
        });
      }

      return baseImplementation(command, payload);
    });

    renderApp();

    fireEvent.click((await screen.findAllByText("Debug session"))[0]);

    const memoryButton = await screen.findByRole("button", { name: "Memory Ready" });
    expect(memoryButton.className).toContain("is-ready");
    expect(within(memoryButton).getByText("Ready")).toBeTruthy();
    expect(within(memoryButton).queryByText("1")).toBeNull();
  });

  it("keeps the inbox badge instead of showing a Ready cue when pending memory exists", async () => {
    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    let healthCallCount = 0;
    const baseImplementation = mockInvoke.getMockImplementation();
    if (!baseImplementation) {
      throw new Error("Missing base invoke mock implementation");
    }

    mockInvoke.mockImplementation((command: string, payload?: Record<string, unknown>) => {
      if (command === "list_memory_candidates") {
        return Promise.resolve([
          {
            candidate_id: "cand-001",
            kind: "gotcha",
            summary: "Review pending memory",
            value: "Do not auto-approve candidate writes",
            why_it_matters: "Human review is required",
            confidence: 0.91,
            proposed_by: "codex",
            status: "pending_review",
            created_at: "2026-04-19T09:00:00Z",
            evidence_refs: [],
            merge_suggestion: null,
          },
        ]);
      }

      if (command === "get_repo_memory_health" && payload?.repoRoot === "D:/VSP/demo") {
        healthCallCount += 1;
        return Promise.resolve({
          repo_root: "D:/VSP/demo",
          canonical_repo_root: "D:/VSP/demo",
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

      if (command === "scan_repo_conversations" && payload?.repoRoot === "D:/VSP/demo") {
        return Promise.resolve({
          repo_root: "D:/VSP/demo",
          canonical_repo_root: "D:/VSP/demo",
          scanned_conversation_count: 1,
          linked_conversation_count: 1,
          skipped_conversation_count: 0,
          source_agents: [{ source_agent: "claude", conversation_count: 1 }],
          warnings: [],
        });
      }

      return baseImplementation(command, payload);
    });

    renderApp();

    fireEvent.click((await screen.findAllByText("Debug session"))[0]);

    const memoryButton = await screen.findByRole("button", { name: "Memory 1" });
    expect(memoryButton.className).toContain("is-ready");
    expect(within(memoryButton).getByText("1")).toBeTruthy();
    expect(within(memoryButton).queryByText("Ready")).toBeNull();
  });

  it("clears the Memory button Ready cue before an async conversation switch finishes loading", async () => {
    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    const deferredConversation = createDeferred<{
      id: string;
      source_agent: string;
      project_dir: string;
      created_at: string;
      updated_at: string;
      summary: string;
      storage_path: string;
      resume_command: string;
      messages: [];
      file_changes: [];
    }>();
    let healthCallCount = 0;
    const baseImplementation = mockInvoke.getMockImplementation();
    if (!baseImplementation) {
      throw new Error("Missing base invoke mock implementation");
    }

    mockInvoke.mockImplementation((command: string, payload?: Record<string, unknown>) => {
      if (command === "read_conversation" && payload?.id === "conv-002") {
        return deferredConversation.promise;
      }

      if (command === "get_repo_memory_health" && payload?.repoRoot === "D:/VSP/demo") {
        healthCallCount += 1;
        return Promise.resolve({
          repo_root: "D:/VSP/demo",
          canonical_repo_root: "D:/VSP/demo",
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

      if (command === "scan_repo_conversations" && payload?.repoRoot === "D:/VSP/demo") {
        return Promise.resolve({
          repo_root: "D:/VSP/demo",
          canonical_repo_root: "D:/VSP/demo",
          scanned_conversation_count: 1,
          linked_conversation_count: 1,
          skipped_conversation_count: 0,
          source_agents: [{ source_agent: "claude", conversation_count: 1 }],
          warnings: [],
        });
      }

      return baseImplementation(command, payload);
    });

    renderApp();

    fireEvent.click((await screen.findAllByText("Debug session"))[0]);
    expect(await screen.findByRole("button", { name: "Memory Ready" })).toBeTruthy();

    fireEvent.click((await screen.findAllByText("Memory investigation"))[0]);

    expect(screen.queryByRole("button", { name: "Memory Ready" })).toBeNull();
    expect(screen.getByRole("button", { name: "Memory" })).toBeTruthy();

    deferredConversation.resolve({
      id: "conv-002",
      source_agent: "claude",
      project_dir: "D:/PV/service",
      created_at: "2026-04-08T10:00:00Z",
      updated_at: "2026-04-08T11:00:00Z",
      summary: "Memory investigation",
      storage_path: "C:/Users/demo/.codex/sessions/2026/04/08/rollout-conv-002.jsonl",
      resume_command: "codex resume conv-002",
      messages: [],
      file_changes: [],
    });

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Memory investigation" })).toBeTruthy();
      expect(screen.getByRole("button", { name: "Memory" })).toBeTruthy();
    });
  });
```

- [ ] **Step 2: Run the focused App test file to verify it fails**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/App.test.tsx
```

Expected:

- FAIL because the `Memory` button does not yet render a `Ready` cue
- FAIL because there is no `is-ready` styling on the button
- FAIL because the async switch test cannot observe the cue clearing lifecycle yet

- [ ] **Step 3: Add the `Memory` button ready cue in `App.tsx`**

In `src/App.tsx`, directly after:

```tsx
    const memoryAttentionCount = memoryCandidates.length;
    const memoryButtonLabel = locale === "en" ? "Memory" : "\u8bb0\u5fc6";
```

add:

```tsx
    const showMemoryReadyCue = bootstrapReadyConversationId === selectedConversation.id;
    const showMemoryReadyText = showMemoryReadyCue && memoryAttentionCount === 0;
    const memoryReadyLabel = locale === "en" ? "Ready" : "\u5df2\u5c31\u7eea";
    const memoryButtonClassName = [
      "btn",
      "btn-secondary",
      "memory-drawer-trigger",
      memoryAttentionCount > 0 ? "has-memory-alert" : "",
      showMemoryReadyCue ? "is-ready" : "",
    ]
      .filter(Boolean)
      .join(" ");
```

Then replace the current `Memory` button block with:

```tsx
            <button
              type="button"
              className={memoryButtonClassName}
              onClick={() => {
                setMemoryDrawerTab(memoryAttentionCount > 0 ? "inbox" : "approved");
                setMemoryDrawerOpen(true);
              }}
            >
              <span>{memoryButtonLabel}</span>
              {showMemoryReadyText ? (
                <span className="memory-drawer-trigger-ready">
                  <span className="memory-drawer-trigger-ready-dot" aria-hidden="true" />
                  <span>{memoryReadyLabel}</span>
                </span>
              ) : null}
              {memoryAttentionCount > 0 ? (
                <span className="memory-drawer-trigger-badge">{memoryAttentionCount}</span>
              ) : null}
            </button>
```

- [ ] **Step 4: Add the ready-state button styles**

In `src/styles.css`, keep the existing `.memory-drawer-trigger` and `.memory-drawer-trigger-badge` rules, then add these blocks immediately after `.memory-drawer-trigger.has-memory-alert`:

```css
.memory-drawer-trigger.is-ready {
  border-color: rgba(58, 143, 100, 0.18);
  background: rgba(237, 248, 241, 0.9);
  color: var(--accent-strong);
}

.memory-drawer-trigger-ready {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  color: var(--accent-strong);
  font-size: 12px;
  font-weight: 700;
  line-height: 1;
}

.memory-drawer-trigger-ready-dot {
  width: 7px;
  height: 7px;
  border-radius: 999px;
  background: currentColor;
  flex: 0 0 auto;
}
```

- [ ] **Step 5: Run the focused App tests again**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/App.test.tsx
```

Expected: PASS with all `App.test.tsx` tests green, including the new `Memory` button ready-cue coverage.

- [ ] **Step 6: Commit Task 1**

```powershell
git add src/App.tsx src/styles.css src/__tests__/App.test.tsx
git commit -m "feat: add ChatMem memory entry ready cue"
```

## Final Verification

- [ ] **Step 1: Run the full frontend suite**

Run:

```powershell
npm.cmd run test:run
```

Expected: PASS with all Vitest files green.

- [ ] **Step 2: Run the production build**

Run:

```powershell
npm.cmd run build
```

Expected: PASS with `tsc && vite build` completing successfully.

- [ ] **Step 3: Inspect the working tree and recent commits**

Run:

```powershell
git status --short
git log --oneline -8
```

Expected:

- working tree is clean
- the memory-entry ready-cue commit is at the top of the branch

## Spec Coverage Self-Review

- Ready cue appears directly on the `Memory` button: Task 1.
- Cue is shown only for the current conversation-scoped bootstrap-ready event: Task 1.
- No-inbox state shows inline `Ready` text: Task 1 test 1 and implementation.
- Inbox badge wins over the ready cue text while the button still gets a ready style: Task 1 test 2 and implementation.
- Async conversation switching clears the cue immediately: Task 1 test 3 and implementation.
- Drawer-opening behavior stays unchanged: Task 1 keeps the existing click handler and tab-selection logic.
- No new drawer, panel, or scan behavior is introduced: enforced by file scope.

## Placeholder Scan

- No `TODO`, `TBD`, or deferred implementation markers remain.
- Every code-changing step includes exact code.
- Every verification step includes exact commands and expected outcomes.
