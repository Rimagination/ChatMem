# ChatMem Bootstrap Status Copy Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the `ProjectIndexStatus` panel clearly explain why older conversations may not be fully searchable yet when a repo still has zero indexed local history, and what becomes possible after indexing finishes.

**Architecture:** This is a small frontend-only change. `ProjectIndexStatus.tsx` gains one persistent explanatory note that appears only when the effective indexed history count is zero, with separate copy for idle and scanning states. The existing workspace keeps the same layout and scan actions; only the panel copy and note styling change, with one component-level test layer and one workspace-level integration test layer.

**Tech Stack:** React, TypeScript, CSS, Vitest, Testing Library.

---

## Scope

This plan implements `docs/superpowers/specs/2026-04-23-chatmem-bootstrap-status-copy-design.md`.

Included:

- Idle and scanning explanatory copy inside `ProjectIndexStatus`
- Hidden note when indexed history exists
- Restrained panel note styling
- Component tests for note visibility rules
- One workspace-level integration test proving the note appears during auto bootstrap

Not included:

- New scan behavior
- New onboarding surfaces
- Banners, modals, or toasts
- Additional state in `App.tsx`

## File Structure

- `src/components/ProjectIndexStatus.tsx`
  - Add effective indexed-count calculation and conditional bootstrap note copy.
- `src/styles.css`
  - Add styling for the explanatory note block within the existing panel.
- `src/__tests__/ProjectIndexStatus.test.tsx`
  - Cover idle note, scanning note, hidden note, and existing scan button behavior.
- `src/__tests__/MemoryWorkspace.test.tsx`
  - Verify the explanatory note appears in the real workspace when auto bootstrap starts for an empty-history repo.

## Task 1: Add Bootstrap Note To ProjectIndexStatus

**Files:**
- Modify: `src/__tests__/ProjectIndexStatus.test.tsx`
- Modify: `src/components/ProjectIndexStatus.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: Write the failing component tests**

In `src/__tests__/ProjectIndexStatus.test.tsx`, add this fixture below `healthFixture`:

```tsx
const emptyHealthFixture: RepoMemoryHealth = {
  repo_root: "D:/VSP/agentswap-gui",
  canonical_repo_root: "D:/VSP/agentswap-gui",
  approved_memory_count: 0,
  pending_candidate_count: 0,
  search_document_count: 0,
  indexed_chunk_count: 0,
  inherited_repo_roots: [],
  conversation_counts_by_agent: [],
  repo_aliases: [],
  warnings: [],
};
```

Update the first test so it also asserts the explanatory note is hidden when indexed history exists:

```tsx
    expect(
      screen.queryByText(
        "Local history has not been indexed for this project yet, so older conversations may not be fully searchable. After indexing, you can ask what was discussed before.",
      ),
    ).toBeNull();
```

Then add these two new tests after the first test:

```tsx
  it("shows the idle bootstrap note when local history is still empty", () => {
    const onScan = vi.fn();

    render(
      <ProjectIndexStatus
        health={emptyHealthFixture}
        loading={false}
        scanning={false}
        locale="en"
        onScan={onScan}
      />,
    );

    expect(
      screen.getByText(
        "Local history has not been indexed for this project yet, so older conversations may not be fully searchable. After indexing, you can ask what was discussed before.",
      ),
    ).toBeTruthy();
    expect(screen.getByRole("button", { name: "Rescan local history" })).toBeTruthy();
  });

  it("shows the scanning bootstrap note while local history is importing", () => {
    const onScan = vi.fn();

    render(
      <ProjectIndexStatus
        health={emptyHealthFixture}
        loading={false}
        scanning={true}
        locale="en"
        onScan={onScan}
      />,
    );

    expect(
      screen.getByText(
        "Importing local history for this project. Older conversations may not be fully searchable yet. When indexing finishes, you can ask what was discussed before.",
      ),
    ).toBeTruthy();
    expect(screen.getByRole("button", { name: "Scanning..." })).toBeDisabled();
  });
```

- [ ] **Step 2: Run the focused test file to verify it fails**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/ProjectIndexStatus.test.tsx
```

Expected: FAIL because `ProjectIndexStatus` does not yet render the new explanatory note copy.

- [ ] **Step 3: Add the explanatory note in `ProjectIndexStatus.tsx`**

In `src/components/ProjectIndexStatus.tsx`, add these derived values after `totalConversations`:

```tsx
  const effectiveIndexedChunkCount =
    health?.indexed_chunk_count ?? health?.search_document_count ?? 0;
  const showBootstrapNote = effectiveIndexedChunkCount === 0;
```

Extend the `copy` object in the English branch with:

```tsx
        bootstrapIdle:
          "Local history has not been indexed for this project yet, so older conversations may not be fully searchable. After indexing, you can ask what was discussed before.",
        bootstrapScanning:
          "Importing local history for this project. Older conversations may not be fully searchable yet. When indexing finishes, you can ask what was discussed before.",
```

Extend the non-English branch with:

```tsx
        bootstrapIdle:
          "这个项目的本地历史还没有建立索引，所以旧对话暂时可能找不全。完成导入后，你可以直接问以前讨论过什么。",
        bootstrapScanning:
          "正在导入这个项目的本地历史。索引完成前，旧对话可能还找不全。完成后，你可以直接问以前讨论过什么。",
```

Replace the inline chunk count expression with `effectiveIndexedChunkCount`:

```tsx
            {effectiveIndexedChunkCount}
```

Then insert this note block between the header and the metric grid:

```tsx
      {showBootstrapNote ? (
        <div className="project-index-note">
          <p>{scanning ? copy.bootstrapScanning : copy.bootstrapIdle}</p>
        </div>
      ) : null}
```

- [ ] **Step 4: Add restrained note styling**

In `src/styles.css`, insert this block after `.project-index-status` and before `.project-index-grid`:

```css
.project-index-note {
  padding: 11px 12px;
  border-radius: var(--radius-md);
  border: 1px solid rgba(22, 32, 24, 0.08);
  background: rgba(244, 248, 244, 0.92);
  color: var(--text-secondary);
}

.project-index-note p {
  margin: 0;
  line-height: 1.5;
}
```

- [ ] **Step 5: Run the focused component tests again**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/ProjectIndexStatus.test.tsx
```

Expected: PASS with all `ProjectIndexStatus` tests green.

- [ ] **Step 6: Commit Task 1**

```powershell
git add src/components/ProjectIndexStatus.tsx src/styles.css src/__tests__/ProjectIndexStatus.test.tsx
git commit -m "feat: explain empty ChatMem history state"
```

## Task 2: Cover Bootstrap Copy In The Real Workspace

**Files:**
- Modify: `src/__tests__/MemoryWorkspace.test.tsx`

- [ ] **Step 1: Write the failing workspace integration test**

In `src/__tests__/MemoryWorkspace.test.tsx`, add this helper near the top of the file after `renderApp()`:

```tsx
function createDeferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}
```

Then add this test after `it("shows local history status and rescans the active repo", ...)`:

```tsx
  it("shows the bootstrap explanation while local history is auto-indexing", async () => {
    const scanDeferred = createDeferred<{
      repo_root: string;
      canonical_repo_root: string;
      scanned_conversation_count: number;
      linked_conversation_count: number;
      skipped_conversation_count: number;
      source_agents: Array<{ source_agent: string; conversation_count: number }>;
      warnings: string[];
    }>();

    const baseImplementation = mockInvoke.getMockImplementation();
    if (!baseImplementation) {
      throw new Error("Missing base invoke mock implementation");
    }

    mockInvoke.mockImplementation((command: string, payload?: Record<string, unknown>) => {
      if (command === "get_repo_memory_health") {
        return Promise.resolve({
          repo_root: "D:/VSP/agentswap-gui",
          canonical_repo_root: "D:/VSP/agentswap-gui",
          approved_memory_count: 1,
          pending_candidate_count: 1,
          search_document_count: 0,
          indexed_chunk_count: 0,
          inherited_repo_roots: [],
          conversation_counts_by_agent: [],
          repo_aliases: [],
          warnings: [],
        });
      }

      if (command === "scan_repo_conversations") {
        return scanDeferred.promise;
      }

      return baseImplementation(command, payload);
    });

    renderApp();

    fireEvent.click((await screen.findAllByText("Memory workflow"))[0]);

    expect(
      await screen.findByText(
        "Importing local history for this project. Older conversations may not be fully searchable yet. When indexing finishes, you can ask what was discussed before.",
      ),
    ).toBeTruthy();
    expect(screen.getByRole("button", { name: "Scanning..." })).toBeDisabled();
  });
```

- [ ] **Step 2: Run the focused workspace test to verify it fails**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/MemoryWorkspace.test.tsx
```

Expected: FAIL because the real workspace does not yet render the new explanatory note copy during auto bootstrap.

- [ ] **Step 3: Re-run the workspace test after Task 1 changes**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/MemoryWorkspace.test.tsx src/__tests__/ProjectIndexStatus.test.tsx
```

Expected: PASS. The workspace test should now see the scanning explanatory copy coming from `ProjectIndexStatus`.

- [ ] **Step 4: Commit Task 2**

```powershell
git add src/__tests__/MemoryWorkspace.test.tsx
git commit -m "test: cover ChatMem bootstrap status copy"
```

## Final Verification

- [ ] **Step 1: Run the full frontend suite**

Run:

```powershell
npm.cmd run test:run
```

Expected: PASS with all Vitest files green.

- [ ] **Step 2: Inspect the working tree and recent commits**

Run:

```powershell
git status --short
git log --oneline -8
```

Expected:

- working tree is clean
- the new status-copy commits appear on top of the current branch

## Spec Coverage Self-Review

- Persistent explanatory note for zero effective indexed history: Task 1.
- Separate idle and scanning copy variants: Task 1.
- Note disappears once indexed history exists: Task 1.
- Existing scan button remains the only action: Task 1 tests keep the button path intact.
- Restrained visual treatment inside the existing panel: Task 1.
- Workspace-level proof that the note appears during real auto bootstrap: Task 2.

## Placeholder Scan

- No `TODO`, `TBD`, or deferred implementation markers remain.
- Every code-changing step includes exact code.
- Every verification step includes an exact command and expected outcome.
