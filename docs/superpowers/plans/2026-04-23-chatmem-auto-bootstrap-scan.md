# ChatMem Auto Bootstrap Scan Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Automatically bootstrap a repo's local history index the first time a user opens a conversation for a repo whose ChatMem history index is still empty.

**Architecture:** This is a frontend orchestration change built on top of existing Tauri commands. `App.tsx` continues to own project-memory loading, but after `get_repo_memory_health(repoRoot)` returns, it will automatically call `scan_repo_conversations(repoRoot)` when `indexed_chunk_count === 0`, then refresh health. A second pass adds session-local deduplication so the same repo is not auto-scanned repeatedly in one app session.

**Tech Stack:** React, TypeScript, Tauri `invoke`, Vitest, Testing Library.

---

## Scope

This plan implements `docs/superpowers/specs/2026-04-23-chatmem-auto-bootstrap-scan-design.md`.

Included:

- Auto-bootstrap scan when repo health reports zero indexed chunks.
- Shared scan helper so auto and manual scans use the same refresh path.
- Protection against stale repo-state writes when scan results return after the user switches repos.
- Session-local deduplication so the same repo is not auto-bootstrapped more than once per app session.
- Regression tests for auto bootstrap, stale repo switching, health failure, and one-shot dedupe.

Not included:

- Global startup scanning across all repos.
- New background jobs, progress bars, or cancel/pause controls.
- Automatic candidate extraction.
- New backend commands or schema changes.

## File Structure

- `src/App.tsx`
  - Refactor the existing manual repo scan logic into a shared helper.
  - Trigger the helper automatically after repo-health load when `indexed_chunk_count === 0`.
  - Add session-local attempted-repo tracking for one-shot bootstrap behavior.
- `src/__tests__/MemoryWorkspace.test.tsx`
  - Add the failing/passing tests for auto bootstrap, one-shot dedupe, and health-failure non-bootstrap.
- `src/__tests__/App.test.tsx`
  - Add the repo-switch regression proving an in-flight bootstrap scan does not overwrite the visible repo state after the user switches contexts.

## Task 1: Auto Bootstrap Empty Repo History

**Files:**
- Modify: `src/__tests__/MemoryWorkspace.test.tsx`
- Modify: `src/__tests__/App.test.tsx`
- Modify: `src/App.tsx`

- [ ] **Step 1: Write the failing auto-bootstrap test**

In `src/__tests__/MemoryWorkspace.test.tsx`, add this test after `it("shows local history status and rescans the active repo", ...)`:

```tsx
  it("automatically bootstraps local history when repo health has no indexed chunks", async () => {
    let healthCalls = 0;
    const baseImplementation = mockInvoke.getMockImplementation();
    if (!baseImplementation) {
      throw new Error("Missing base invoke mock implementation");
    }

    mockInvoke.mockImplementation((command: string, payload?: Record<string, unknown>) => {
      if (command === "get_repo_memory_health") {
        healthCalls += 1;
        return Promise.resolve({
          repo_root: "D:/VSP/agentswap-gui",
          canonical_repo_root: "D:/VSP/agentswap-gui",
          approved_memory_count: 1,
          pending_candidate_count: 1,
          search_document_count: healthCalls > 1 ? 4 : 0,
          indexed_chunk_count: healthCalls > 1 ? 8 : 0,
          inherited_repo_roots: [],
          conversation_counts_by_agent:
            healthCalls > 1 ? [{ source_agent: "claude", conversation_count: 1 }] : [],
          repo_aliases: [],
          warnings: [],
        });
      }
      return baseImplementation(command, payload);
    });

    renderApp();

    fireEvent.click((await screen.findAllByText("Memory workflow"))[0]);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("scan_repo_conversations", {
        repoRoot: "D:/VSP/agentswap-gui",
      });
    });

    await waitFor(() => {
      const repoHealthCalls = mockInvoke.mock.calls.filter(
        ([command, args]) =>
          command === "get_repo_memory_health" &&
          args &&
          typeof args === "object" &&
          "repoRoot" in args &&
          args.repoRoot === "D:/VSP/agentswap-gui",
      );
      expect(repoHealthCalls).toHaveLength(2);
    });
  });
```

- [ ] **Step 2: Write the failing stale-repo regression test**

In `src/__tests__/App.test.tsx`, add this helper near the top of the file after `function renderApp()`:

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

Then add this test near the existing workspace tests:

```tsx
  it("does not overwrite visible repo health when an automatic scan resolves after switching repos", async () => {
    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    const scanDeferred = createDeferred<{
      repo_root: string;
      canonical_repo_root: string;
      scanned_conversation_count: number;
      linked_conversation_count: number;
      skipped_conversation_count: number;
      source_agents: Array<{ source_agent: string; conversation_count: number }>;
      warnings: string[];
    }>();

    mockInvoke.mockImplementation(async (command: string, payload?: Record<string, unknown>) => {
      if (command === "list_conversations") {
        return [
          {
            id: "conv-a",
            source_agent: "claude",
            project_dir: "D:/VSP/demo",
            created_at: "2026-04-23T08:00:00Z",
            updated_at: "2026-04-23T09:00:00Z",
            summary: "Repo A",
            message_count: 1,
            file_count: 0,
          },
          {
            id: "conv-b",
            source_agent: "claude",
            project_dir: "D:/PV/service",
            created_at: "2026-04-23T08:30:00Z",
            updated_at: "2026-04-23T09:30:00Z",
            summary: "Repo B",
            message_count: 1,
            file_count: 0,
          },
        ];
      }

      if (command === "read_conversation") {
        if (payload?.id === "conv-b") {
          return {
            id: "conv-b",
            source_agent: "claude",
            project_dir: "D:/PV/service",
            created_at: "2026-04-23T08:30:00Z",
            updated_at: "2026-04-23T09:30:00Z",
            summary: "Repo B",
            storage_path: "C:/Users/demo/.claude/projects/conv-b.jsonl",
            resume_command: "claude --resume conv-b",
            messages: [],
            file_changes: [],
          };
        }

        return {
          id: "conv-a",
          source_agent: "claude",
          project_dir: "D:/VSP/demo",
          created_at: "2026-04-23T08:00:00Z",
          updated_at: "2026-04-23T09:00:00Z",
          summary: "Repo A",
          storage_path: "C:/Users/demo/.claude/projects/conv-a.jsonl",
          resume_command: "claude --resume conv-a",
          messages: [],
          file_changes: [],
        };
      }

      if (
        command === "list_repo_memories" ||
        command === "list_memory_candidates" ||
        command === "list_handoffs" ||
        command === "list_checkpoints" ||
        command === "list_runs" ||
        command === "list_artifacts" ||
        command === "list_episodes"
      ) {
        return [];
      }

      if (command === "list_wiki_pages" || command === "rebuild_repo_wiki") {
        return [];
      }

      if (command === "get_repo_memory_health") {
        if (payload?.repoRoot === "D:/PV/service") {
          return {
            repo_root: "D:/PV/service",
            canonical_repo_root: "D:/PV/service",
            approved_memory_count: 0,
            pending_candidate_count: 0,
            search_document_count: 5,
            indexed_chunk_count: 3,
            inherited_repo_roots: [],
            conversation_counts_by_agent: [{ source_agent: "claude", conversation_count: 1 }],
            repo_aliases: [],
            warnings: [],
          };
        }

        return {
          repo_root: "D:/VSP/demo",
          canonical_repo_root: "D:/VSP/demo",
          approved_memory_count: 0,
          pending_candidate_count: 0,
          search_document_count: 0,
          indexed_chunk_count: 0,
          inherited_repo_roots: [],
          conversation_counts_by_agent: [],
          repo_aliases: [],
          warnings: [],
        };
      }

      if (command === "scan_repo_conversations") {
        return scanDeferred.promise;
      }

      return [];
    });

    renderApp();

    fireEvent.click((await screen.findAllByText("Repo A"))[0]);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("scan_repo_conversations", {
        repoRoot: "D:/VSP/demo",
      });
    });

    fireEvent.click((await screen.findAllByText("Repo B"))[0]);
    expect(await screen.findByRole("heading", { name: "D:/PV/service" })).toBeTruthy();

    await act(async () => {
      scanDeferred.resolve({
        repo_root: "D:/VSP/demo",
        canonical_repo_root: "D:/VSP/demo",
        scanned_conversation_count: 1,
        linked_conversation_count: 1,
        skipped_conversation_count: 0,
        source_agents: [{ source_agent: "claude", conversation_count: 1 }],
        warnings: [],
      });
      await scanDeferred.promise;
    });

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "D:/PV/service" })).toBeTruthy();
      expect(screen.queryByRole("heading", { name: "D:/VSP/demo" })).toBeNull();
    });
  });
```

- [ ] **Step 3: Run the focused failing tests**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/MemoryWorkspace.test.tsx src/__tests__/App.test.tsx
```

Expected:

- FAIL because `scan_repo_conversations` is not called automatically when `indexed_chunk_count` is `0`
- FAIL because there is no shared auto-scan path to protect the visible repo state during repo switching

- [ ] **Step 4: Write the minimal implementation in `App.tsx`**

In `src/App.tsx`, add this shared helper near the existing scan refs:

```tsx
  const runRepoScan = useCallback(async (requestRepoRoot: string) => {
    const requestId = ++repoScanRequestIdRef.current;
    repoScanActiveCountRef.current += 1;
    setRepoScanRunning(true);
    try {
      await scanRepoConversations(requestRepoRoot);
      const nextHealth = await getRepoMemoryHealth(requestRepoRoot);
      if (
        activeRepoRootRef.current === requestRepoRoot &&
        requestId === repoScanRequestIdRef.current
      ) {
        setRepoMemoryHealth(nextHealth);
      }
      return nextHealth;
    } catch (error) {
      console.error("Failed to scan repo conversations:", error);
      return null;
    } finally {
      repoScanActiveCountRef.current = Math.max(0, repoScanActiveCountRef.current - 1);
      setRepoScanRunning(repoScanActiveCountRef.current > 0);
    }
  }, []);
```

Then replace the repo-health success block inside `loadProjectMemory` with:

```tsx
      try {
        const nextHealth = await getRepoMemoryHealth(requestRepoRoot);
        if (cancelled || activeRepoRootRef.current !== requestRepoRoot) {
          return;
        }
        setRepoMemoryHealth(nextHealth);

        const shouldAutoBootstrap =
          nextHealth.indexed_chunk_count === 0 && repoScanActiveCountRef.current === 0;

        if (shouldAutoBootstrap) {
          void runRepoScan(requestRepoRoot);
        }
      } catch (error) {
        console.error("Failed to load repo memory health:", error);
      } finally {
        if (!cancelled) {
          setRepoHealthLoading(false);
        }
      }
```

Update the effect dependency list to include `runRepoScan`:

```tsx
  }, [activeRepoRoot, runRepoScan]);
```

Finally, replace `handleScanRepoConversations` with:

```tsx
  const handleScanRepoConversations = async () => {
    if (!activeRepoRoot) {
      return;
    }
    await runRepoScan(activeRepoRoot);
  };
```

- [ ] **Step 5: Run the focused tests again**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/MemoryWorkspace.test.tsx src/__tests__/App.test.tsx
```

Expected:

- PASS for the new auto-bootstrap test
- PASS for the repo-switch regression
- Existing manual-scan and workspace tests in those files remain green

- [ ] **Step 6: Commit Task 1**

```powershell
git add src/App.tsx src/__tests__/MemoryWorkspace.test.tsx src/__tests__/App.test.tsx
git commit -m "feat: auto bootstrap empty repo history"
```

## Task 2: Deduplicate Bootstrap Attempts Per Repo Session

**Files:**
- Modify: `src/__tests__/MemoryWorkspace.test.tsx`
- Modify: `src/App.tsx`

- [ ] **Step 1: Write the failing one-shot bootstrap regression**

In `src/__tests__/MemoryWorkspace.test.tsx`, add this test after the new auto-bootstrap test:

```tsx
  it("only attempts automatic bootstrap once per repo in a session", async () => {
    const baseImplementation = mockInvoke.getMockImplementation();
    if (!baseImplementation) {
      throw new Error("Missing base invoke mock implementation");
    }

    mockInvoke.mockImplementation((command: string, payload?: Record<string, unknown>) => {
      if (command === "list_conversations") {
        return Promise.resolve([
          {
            id: "conv-001",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/VSP/agentswap-gui",
            created_at: "2026-04-19T08:00:00Z",
            updated_at: "2026-04-19T09:00:00Z",
            summary: "Memory workflow",
            message_count: 3,
            file_count: 2,
          },
          {
            id: "conv-002",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/VSP/agentswap-gui",
            created_at: "2026-04-19T10:00:00Z",
            updated_at: "2026-04-19T11:00:00Z",
            summary: "Second memory workflow",
            message_count: 1,
            file_count: 0,
          },
        ]);
      }

      if (command === "read_conversation") {
        const summary = payload?.id === "conv-002" ? "Second memory workflow" : "Memory workflow";
        return Promise.resolve({
          id: payload?.id,
          source_agent: payload?.agent ?? "claude",
          project_dir: "D:/VSP/agentswap-gui",
          created_at: "2026-04-19T08:00:00Z",
          updated_at: "2026-04-19T09:00:00Z",
          summary,
          storage_path: `C:/Users/demo/.claude/projects/${payload?.id}.jsonl`,
          resume_command: `claude --resume ${payload?.id}`,
          messages: [],
          file_changes: [],
        });
      }

      if (command === "get_repo_memory_health") {
        return Promise.resolve({
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
        });
      }

      return baseImplementation(command, payload);
    });

    renderApp();

    fireEvent.click((await screen.findAllByText("Memory workflow"))[0]);

    await waitFor(() => {
      const scanCalls = mockInvoke.mock.calls.filter(
        ([command, args]) =>
          command === "scan_repo_conversations" &&
          args &&
          typeof args === "object" &&
          "repoRoot" in args &&
          args.repoRoot === "D:/VSP/agentswap-gui",
      );
      expect(scanCalls).toHaveLength(1);
    });

    fireEvent.click(await screen.findByText("Second memory workflow"));

    await waitFor(() => {
      const scanCalls = mockInvoke.mock.calls.filter(
        ([command, args]) =>
          command === "scan_repo_conversations" &&
          args &&
          typeof args === "object" &&
          "repoRoot" in args &&
          args.repoRoot === "D:/VSP/agentswap-gui",
      );
      expect(scanCalls).toHaveLength(1);
    });
  });
```

Strengthen the existing health-failure test by appending this assertion at the end:

```tsx
    expect(mockInvoke).not.toHaveBeenCalledWith("scan_repo_conversations", {
      repoRoot: "D:/VSP/agentswap-gui",
    });
```

- [ ] **Step 2: Run the focused failing tests**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/MemoryWorkspace.test.tsx
```

Expected:

- FAIL because selecting another conversation in the same repo triggers a second automatic scan
- the health-failure assertion should remain green and protect the eligibility rule

- [ ] **Step 3: Add session-local attempted-repo tracking**

In `src/App.tsx`, add this ref near the other repo scan refs:

```tsx
  const autoBootstrapAttemptedReposRef = useRef<Record<string, true>>({});
```

Then replace the auto-bootstrap predicate in `loadProjectMemory` with:

```tsx
        const bootstrapKey = nextHealth.canonical_repo_root || requestRepoRoot;
        const shouldAutoBootstrap =
          nextHealth.indexed_chunk_count === 0 &&
          repoScanActiveCountRef.current === 0 &&
          autoBootstrapAttemptedReposRef.current[bootstrapKey] !== true;

        if (shouldAutoBootstrap) {
          autoBootstrapAttemptedReposRef.current[bootstrapKey] = true;
          void runRepoScan(requestRepoRoot);
        }
```

Do not reset `autoBootstrapAttemptedReposRef` when the user closes a drawer or switches conversations. It is session-scoped.

- [ ] **Step 4: Run the regression suite**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/MemoryWorkspace.test.tsx src/__tests__/App.test.tsx src/__tests__/ProjectIndexStatus.test.tsx
```

Expected:

- PASS for the one-shot bootstrap test
- PASS for the health-failure non-bootstrap assertion
- PASS for the stale repo-switch test
- PASS for the existing manual rescan and ProjectIndexStatus coverage

- [ ] **Step 5: Commit Task 2**

```powershell
git add src/App.tsx src/__tests__/MemoryWorkspace.test.tsx src/__tests__/App.test.tsx
git commit -m "fix: dedupe ChatMem auto bootstrap scans"
```

## Final Verification

- [ ] **Step 1: Run the full frontend test suite**

Run:

```powershell
npm.cmd run test:run
```

Expected: PASS with all Vitest files green.

- [ ] **Step 2: Inspect working tree and commits**

Run:

```powershell
git status --short
git log --oneline -6
```

Expected:

- no unexpected modified files
- two new commits for this feature work

## Spec Coverage Self-Review

- Auto-bootstrap on first open when the repo index is empty: Task 1.
- Refresh repo health after automatic scan: Task 1.
- Keep using the existing `ProjectIndexStatus` surface with existing scan UI: Task 1.
- Do not break visible repo state when scans resolve after repo switch: Task 1.
- Session-local one-shot behavior per repo: Task 2.
- Do not auto-bootstrap when repo health fails: Task 2.
- Keep manual rescan working: Task 1 and Task 2 regression commands.
- No new backend commands, jobs, or onboarding flows: enforced by file scope.

## Placeholder Scan

- No `TODO`, `TBD`, or deferred implementation markers remain.
- Every code-changing step includes concrete code or exact assertions.
- Every verification step includes an exact command and expected result.
