# ChatMem Bootstrap Ready Notice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Show a lightweight one-time "local history is ready" notice after automatic ChatMem bootstrap succeeds for the currently selected conversation, then clear that notice when the user leaves the conversation context.

**Architecture:** `App.tsx` will own the transient ready state because it is a session-local UI event, not durable repo health. `ProjectIndexStatus.tsx` will receive a new `bootstrapReady` prop and render a ready notice only when effective indexed history is nonzero and the active conversation just completed automatic bootstrap. The existing zero-chunk idle/scanning notes keep higher priority, and the ready notice clears on conversation change instead of persisting across repo revisits.

**Tech Stack:** React, TypeScript, CSS, Vitest, Testing Library.

---

## Scope

This plan implements `docs/superpowers/specs/2026-04-23-chatmem-bootstrap-ready-notice-design.md`.

Included:

- one-time ready notice after successful automatic bootstrap
- transient ready state owned by `App.tsx`
- `ProjectIndexStatus` prop and rendering updates
- restrained visual variant for the ready notice
- component tests for note priority rules
- workspace tests for scanning -> ready transition, conversation-switch clearing, and manual-rescan exclusion

Not included:

- toasts
- timers or auto-dismiss behavior
- persistent ready notices across repo switches or app restarts
- changes to backend scan behavior
- changes to manual rescan semantics

## File Structure

- `src/App.tsx`
  - Track which selected conversation just completed automatic bootstrap.
  - Pass a transient `bootstrapReady` boolean into `ProjectIndexStatus`.
  - Clear the ready notice when the user changes selected conversation.
  - Ensure only automatic bootstrap scans can raise the ready notice.
- `src/components/ProjectIndexStatus.tsx`
  - Accept the new `bootstrapReady` prop.
  - Add ready-copy rendering with explicit state priority over existing zero-index copy.
- `src/styles.css`
  - Add a restrained visual variant for the ready notice.
- `src/__tests__/ProjectIndexStatus.test.tsx`
  - Cover ready-copy visibility, hidden state, and zero-index priority.
- `src/__tests__/MemoryWorkspace.test.tsx`
  - Cover real workspace transition from scanning note to ready notice.
  - Cover clearing the ready notice after switching conversations.
  - Cover that manual rescan does not create the ready notice.

## Task 1: Add Ready Notice Rendering To ProjectIndexStatus

**Files:**
- Modify: `src/__tests__/ProjectIndexStatus.test.tsx`
- Modify: `src/components/ProjectIndexStatus.tsx`
- Modify: `src/styles.css`

- [ ] **Step 1: Write the failing component tests**

In `src/__tests__/ProjectIndexStatus.test.tsx`, update every existing `ProjectIndexStatus` render call to pass `bootstrapReady={false}`.

In the first test, keep the existing idle-note absence assertion and add this ready-note absence assertion:

```tsx
    expect(
      screen.queryByText(
        "Local history is ready for this project. You can now ask what was discussed before.",
      ),
    ).toBeNull();
```

Then add these two new tests after the existing scanning-note test:

```tsx
  it("shows the ready notice when automatic bootstrap just completed", () => {
    const onScan = vi.fn();

    render(
      <ProjectIndexStatus
        health={healthFixture}
        loading={false}
        scanning={false}
        bootstrapReady
        locale="en"
        onScan={onScan}
      />,
    );

    expect(
      screen.getByText(
        "Local history is ready for this project. You can now ask what was discussed before.",
      ),
    ).toBeTruthy();
    expect(
      screen.queryByText(
        "Local history has not been indexed for this project yet, so older conversations may not be fully searchable. After indexing, you can ask what was discussed before.",
      ),
    ).toBeNull();
  });

  it("keeps the zero-index note instead of the ready notice when chunks are still empty", () => {
    const onScan = vi.fn();

    render(
      <ProjectIndexStatus
        health={emptyHealthFixture}
        loading={false}
        scanning={false}
        bootstrapReady
        locale="en"
        onScan={onScan}
      />,
    );

    expect(
      screen.queryByText(
        "Local history is ready for this project. You can now ask what was discussed before.",
      ),
    ).toBeNull();
    expect(
      screen.getByText(
        "Local history has not been indexed for this project yet, so older conversations may not be fully searchable. After indexing, you can ask what was discussed before.",
      ),
    ).toBeTruthy();
  });
```

- [ ] **Step 2: Run the focused component test file to verify it fails**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/ProjectIndexStatus.test.tsx
```

Expected: FAIL because `ProjectIndexStatus` does not yet accept `bootstrapReady` or render the ready notice.

- [ ] **Step 3: Add the new prop and note priority logic in `ProjectIndexStatus.tsx`**

In `src/components/ProjectIndexStatus.tsx`, extend the prop type and destructuring:

```tsx
type ProjectIndexStatusProps = {
  health: RepoMemoryHealth | null;
  loading: boolean;
  scanning: boolean;
  bootstrapReady: boolean;
  locale: Locale;
  onScan: () => void;
};
```

```tsx
export default function ProjectIndexStatus({
  health,
  loading,
  scanning,
  bootstrapReady,
  locale,
  onScan,
}: ProjectIndexStatusProps) {
```

Below `showBootstrapNote`, add:

```tsx
  const showBootstrapReadyNotice =
    bootstrapReady && effectiveIndexedChunkCount > 0 && !showBootstrapNote;
```

Extend the English branch of `copy` with:

```tsx
        bootstrapReady:
          "Local history is ready for this project. You can now ask what was discussed before.",
```

Extend the non-English branch with:

```tsx
        bootstrapReady:
          "这个项目的本地历史已经就绪，现在可以直接问以前讨论过什么。",
```

Then replace the existing note rendering with this priority-based version:

```tsx
  const noteBody = showBootstrapNote
    ? scanning
      ? copy.bootstrapScanning
      : copy.bootstrapIdle
    : showBootstrapReadyNotice
      ? copy.bootstrapReady
      : null;

  const noteClassName = `project-index-note${showBootstrapReadyNotice ? " is-ready" : ""}`;
```

```tsx
      {noteBody ? (
        <div className={noteClassName}>
          <p>{noteBody}</p>
        </div>
      ) : null}
```

- [ ] **Step 4: Add the ready-note visual variant**

In `src/styles.css`, keep the existing `.project-index-note` block and add this variant immediately after it:

```css
.project-index-note.is-ready {
  border-color: rgba(52, 96, 70, 0.14);
  background: rgba(239, 247, 241, 0.96);
  color: var(--text-primary);
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
git commit -m "feat: show ChatMem bootstrap ready notice"
```

## Task 2: Wire App State And Workspace Flow

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/__tests__/MemoryWorkspace.test.tsx`

- [ ] **Step 1: Write the failing workspace tests**

In `src/__tests__/MemoryWorkspace.test.tsx`, add this test after `it("shows bootstrap scan status copy while auto-import is still running", ...)`:

```tsx
  it("shows a ready notice when automatic bootstrap finishes for the active conversation", async () => {
    const scanDeferred = createDeferred<{
      repo_root: string;
      canonical_repo_root: string;
      scanned_conversation_count: number;
      linked_conversation_count: number;
      skipped_conversation_count: number;
      source_agents: { source_agent: string; conversation_count: number }[];
      warnings: string[];
    }>();
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

      if (
        command === "scan_repo_conversations" &&
        payload?.repoRoot === "D:/VSP/agentswap-gui"
      ) {
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

    scanDeferred.resolve({
      repo_root: "D:/VSP/agentswap-gui",
      canonical_repo_root: "D:/VSP/agentswap-gui",
      scanned_conversation_count: 1,
      linked_conversation_count: 1,
      skipped_conversation_count: 0,
      source_agents: [{ source_agent: "claude", conversation_count: 1 }],
      warnings: [],
    });

    await waitFor(() => {
      expect(
        screen.getByText(
          "Local history is ready for this project. You can now ask what was discussed before.",
        ),
      ).toBeTruthy();
    });
  });
```

Then add this test after it:

```tsx
  it("clears the ready notice after switching to another conversation", async () => {
    let healthCallCount = 0;
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
            message_count: 2,
            file_count: 1,
          },
        ]);
      }

      if (command === "read_conversation" && payload?.id === "conv-002") {
        return Promise.resolve({
          id: "conv-002",
          source_agent: payload?.agent ?? "claude",
          project_dir: "D:/VSP/agentswap-gui",
          created_at: "2026-04-19T10:00:00Z",
          updated_at: "2026-04-19T11:00:00Z",
          summary: "Second memory workflow",
          storage_path: "C:/Users/demo/.claude/projects/conv-002.jsonl",
          resume_command: "claude --resume conv-002",
          messages: [],
          file_changes: [],
        });
      }

      if (command === "read_conversation" && payload?.id === "conv-001") {
        return Promise.resolve({
          id: "conv-001",
          source_agent: payload?.agent ?? "claude",
          project_dir: "D:/VSP/agentswap-gui",
          created_at: "2026-04-19T08:00:00Z",
          updated_at: "2026-04-19T09:00:00Z",
          summary: "Memory workflow",
          storage_path: "C:/Users/demo/.claude/projects/conv-001.jsonl",
          resume_command: "claude --resume conv-001",
          messages: [],
          file_changes: [],
        });
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

    await waitFor(() => {
      expect(
        screen.getByText(
          "Local history is ready for this project. You can now ask what was discussed before.",
        ),
      ).toBeTruthy();
    });

    fireEvent.click((await screen.findAllByText("Second memory workflow"))[0]);

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Second memory workflow" })).toBeTruthy();
    });

    expect(
      screen.queryByText(
        "Local history is ready for this project. You can now ask what was discussed before.",
      ),
    ).toBeNull();
  });
```

Finally, add this regression test after it:

```tsx
  it("does not show the ready notice after a manual rescan", async () => {
    renderApp();

    fireEvent.click((await screen.findAllByText("Memory workflow"))[0]);

    expect(await screen.findByText("Local history")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Rescan local history" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("scan_repo_conversations", {
        repoRoot: "D:/VSP/agentswap-gui",
      });
    });

    expect(
      screen.queryByText(
        "Local history is ready for this project. You can now ask what was discussed before.",
      ),
    ).toBeNull();
  });
```

- [ ] **Step 2: Run the focused workspace tests to verify the new feature fails**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/MemoryWorkspace.test.tsx
```

Expected:

- FAIL because the ready notice never appears after automatic bootstrap
- FAIL because switching conversations cannot clear a notice that is never created
- the manual-rescan regression may already be green

- [ ] **Step 3: Add transient ready state in `App.tsx`**

In `src/App.tsx`, add this state near the existing memory and repo health state:

```tsx
  const [bootstrapReadyConversationId, setBootstrapReadyConversationId] = useState<string | null>(
    null,
  );
```

Then add the active conversation id and ref next to `activeRepoRoot`:

```tsx
  const activeConversationId = selectedConversation?.id ?? null;
  const activeRepoRoot = selectedConversation?.project_dir ?? null;
  const activeConversationIdRef = useRef<string | null>(activeConversationId);
  const activeRepoRootRef = useRef<string | null>(activeRepoRoot);
```

Add this effect before the existing repo-root effect:

```tsx
  useEffect(() => {
    activeConversationIdRef.current = activeConversationId;
  }, [activeConversationId]);
```

Replace the existing selected-conversation copy-reset effect with:

```tsx
  useEffect(() => {
    setCopyState({ target: null, status: "idle" });
    setBootstrapReadyConversationId((current) =>
      current === activeConversationId ? current : null,
    );
  }, [activeConversationId]);
```

- [ ] **Step 4: Limit the ready notice to automatic bootstrap scans**

In `src/App.tsx`, update `runRepoScan` to accept options:

```tsx
  const runRepoScan = useCallback(
    async (
      requestRepoRoot: string,
      options: { announceBootstrapReady?: boolean; requestConversationId?: string | null } = {},
    ) => {
      const requestId = ++repoScanRequestIdRef.current;
      const requestConversationId =
        options.requestConversationId ?? activeConversationIdRef.current;
      repoScanActiveCountRef.current += 1;
      setRepoScanRunning(true);
      try {
        await scanRepoConversations(requestRepoRoot);
        const nextHealth = await getRepoMemoryHealth(requestRepoRoot);
        const effectiveIndexedChunkCount =
          nextHealth.indexed_chunk_count ?? nextHealth.search_document_count ?? 0;

        if (
          activeRepoRootRef.current === requestRepoRoot &&
          requestId === repoScanRequestIdRef.current
        ) {
          setRepoMemoryHealth(nextHealth);

          if (
            options.announceBootstrapReady &&
            requestConversationId &&
            activeConversationIdRef.current === requestConversationId &&
            effectiveIndexedChunkCount > 0
          ) {
            setBootstrapReadyConversationId(requestConversationId);
          }
        }

        return nextHealth;
      } catch (error) {
        console.error("Failed to scan repo conversations:", error);
        return null;
      } finally {
        repoScanActiveCountRef.current = Math.max(0, repoScanActiveCountRef.current - 1);
        setRepoScanRunning(repoScanActiveCountRef.current > 0);
      }
    },
    [],
  );
```

Inside `loadProjectMemory`, capture the current conversation id and use effective indexed chunk count:

```tsx
      const requestConversationId = activeConversationIdRef.current;
```

```tsx
        const effectiveIndexedChunkCount =
          nextHealth.indexed_chunk_count ?? nextHealth.search_document_count ?? 0;
        const bootstrapKey = nextHealth.canonical_repo_root || requestRepoRoot;
        if (
          effectiveIndexedChunkCount === 0 &&
          autoBootstrapAttemptedReposRef.current[bootstrapKey] !== true
        ) {
          autoBootstrapAttemptedReposRef.current[bootstrapKey] = true;
          void runRepoScan(requestRepoRoot, {
            announceBootstrapReady: true,
            requestConversationId,
          });
        }
```

Keep `handleScanRepoConversations` calling `await runRepoScan(activeRepoRoot);` with no options so manual rescan never sets the ready notice.

Finally, pass the new prop into `ProjectIndexStatus`:

```tsx
          <ProjectIndexStatus
            health={repoMemoryHealth}
            loading={repoHealthLoading}
            scanning={repoScanRunning}
            bootstrapReady={bootstrapReadyConversationId === selectedConversation.id}
            locale={locale}
            onScan={() => void handleScanRepoConversations()}
          />
```

- [ ] **Step 5: Run the focused workspace and component suites**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/MemoryWorkspace.test.tsx src/__tests__/ProjectIndexStatus.test.tsx
```

Expected:

- PASS for scanning -> ready transition
- PASS for conversation-switch clearing
- PASS for manual-rescan exclusion
- PASS for component note-priority coverage

- [ ] **Step 6: Commit Task 2**

```powershell
git add src/App.tsx src/__tests__/MemoryWorkspace.test.tsx
git commit -m "feat: show one-time ChatMem bootstrap ready notice"
```

## Final Verification

- [ ] **Step 1: Run the full frontend suite**

Run:

```powershell
npm.cmd run test:run
```

Expected: PASS with all Vitest files green.

- [ ] **Step 2: Inspect working tree and recent commits**

Run:

```powershell
git status --short
git log --oneline -8
```

Expected:

- working tree is clean
- the ready-notice commits appear at the top of the branch

## Spec Coverage Self-Review

- `App.tsx` owns the transient UI event instead of deriving it from durable health: Task 2.
- `ProjectIndexStatus` receives a `bootstrapReady` prop and shows the ready copy only when indexed history is usable: Task 1 and Task 2.
- Zero-index scanning and idle notes keep higher priority: Task 1.
- Ready notice appears only after automatic bootstrap success with nonzero effective indexed history: Task 2.
- Ready notice clears when the user leaves the conversation context: Task 2 conversation-switch test.
- Manual rescan does not create the ready notice: Task 2 regression test.
- No toasts, timers, or persistent replay behavior are introduced: enforced by file scope and state design.

## Placeholder Scan

- No `TODO`, `TBD`, or deferred implementation markers remain.
- Every code-changing step includes exact code or exact assertions.
- Every verification step includes an exact command and expected outcome.
