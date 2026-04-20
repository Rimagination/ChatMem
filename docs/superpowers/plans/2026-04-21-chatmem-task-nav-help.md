# ChatMem Task-Oriented Navigation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the object-oriented workspace tab row with task-oriented navigation (`Continue Work`, `Needs Review`, `History`, `Help`) and add a user-facing Help surface that answers common return-user questions without exposing the internal ChatMem model.

**Architecture:** Keep the existing left sidebar, Tauri-backed conversation loading, and ChatMem memory APIs. Move navigation to a new global top bar, introduce dedicated page components for each top-level task, and reuse the existing object panels behind `History` and review summaries so the data model stays intact while the user-facing flow becomes task-first.

**Tech Stack:** React, TypeScript, Vite, Vitest, Tauri, existing `chatmem-memory` API helpers

---

## Planned File Changes

- Create: `src/components/TopNavigation.tsx`
  - Own the top-level task buttons plus the lightweight utility actions (`Search`, current agent indicator, `Settings`).
- Create: `src/components/ContinueWorkPage.tsx`
  - Render the default landing page with recent tasks, recoverable progress, next-step guidance, and recent transfers.
- Create: `src/components/ConversationSourceCard.tsx`
  - Centralize conversation file path and resume command UI that is currently hard-coded in `App.tsx`.
- Create: `src/components/NeedsReviewPage.tsx`
  - Summarize memory candidates, stale project rules, and transfer summaries that need a human decision.
- Create: `src/components/HistoryPage.tsx`
  - Provide the filter row (`Conversations`, `Recovery`, `Transfers`, `Outputs`) and host existing panels behind one secondary history surface.
- Create: `src/components/HelpPage.tsx`
  - Render the FAQ cards, quick actions, expanded answers, and advanced troubleshooting section.
- Modify: `src/App.tsx`
  - Replace `workspaceView` with top-level task state, load page data bundles, wire navigation actions, and compose the new page components.
- Modify: `src/styles.css`
  - Add the top-nav layout and task-page styles, and remove the old `workspace-mode-tabs` styles.
- Modify: `src/i18n/types.ts`
  - Add translation keys for the new navigation labels, page headings, buttons, and empty states.
- Modify: `src/i18n/strings.ts`
  - Add matching `zh-CN` and `en` copy for the task-oriented shell and Help page.
- Modify: `src/components/ConversationDetail.tsx`
  - Route remaining hard-coded copy through i18n so the new `History` surface does not fall back to mojibake.
- Test: `src/__tests__/App.test.tsx`
  - Cover the new navigation shell, `Continue Work`, `Needs Review`, `History`, `Help`, and the bilingual shell copy.

## Shared Test Scaffolding

Before implementing page-specific tests, extend the existing Vitest setup so the app can exercise the memory APIs without hitting the real filesystem.

Add this near the top of `src/__tests__/App.test.tsx`:

```tsx
const mockListCheckpoints = vi.fn();
const mockListHandoffs = vi.fn();
const mockListRepoMemories = vi.fn();
const mockListMemoryCandidates = vi.fn();
const mockListEpisodes = vi.fn();
const mockListRuns = vi.fn();
const mockListArtifacts = vi.fn();
const mockCreateCheckpoint = vi.fn();
const mockCreateHandoffPacket = vi.fn();
const mockReviewMemoryCandidate = vi.fn();
const mockReverifyMemory = vi.fn();
const mockMarkHandoffConsumed = vi.fn();

vi.mock("../chatmem-memory/api", async () => {
  const actual = await vi.importActual("../chatmem-memory/api");
  return {
    ...actual,
    listCheckpoints: (...args: unknown[]) => mockListCheckpoints(...args),
    listHandoffs: (...args: unknown[]) => mockListHandoffs(...args),
    listRepoMemories: (...args: unknown[]) => mockListRepoMemories(...args),
    listMemoryCandidates: (...args: unknown[]) => mockListMemoryCandidates(...args),
    listEpisodes: (...args: unknown[]) => mockListEpisodes(...args),
    listRuns: (...args: unknown[]) => mockListRuns(...args),
    listArtifacts: (...args: unknown[]) => mockListArtifacts(...args),
    createCheckpoint: (...args: unknown[]) => mockCreateCheckpoint(...args),
    createHandoffPacket: (...args: unknown[]) => mockCreateHandoffPacket(...args),
    reviewMemoryCandidate: (...args: unknown[]) => mockReviewMemoryCandidate(...args),
    reverifyMemory: (...args: unknown[]) => mockReverifyMemory(...args),
    markHandoffConsumed: (...args: unknown[]) => mockMarkHandoffConsumed(...args),
  };
});
```

And initialize the default responses inside `beforeEach`:

```tsx
mockListCheckpoints.mockResolvedValue([]);
mockListHandoffs.mockResolvedValue([]);
mockListRepoMemories.mockResolvedValue([]);
mockListMemoryCandidates.mockResolvedValue([]);
mockListEpisodes.mockResolvedValue([]);
mockListRuns.mockResolvedValue([]);
mockListArtifacts.mockResolvedValue([]);
mockCreateCheckpoint.mockResolvedValue({});
mockCreateHandoffPacket.mockResolvedValue({});
mockReviewMemoryCandidate.mockResolvedValue(undefined);
mockReverifyMemory.mockResolvedValue(undefined);
mockMarkHandoffConsumed.mockResolvedValue(undefined);
```

This scaffolding is a prerequisite for Tasks 2-5.

### Task 1: Replace the workspace tab row with a task-oriented app shell

**Files:**
- Create: `src/components/TopNavigation.tsx`
- Create: `src/components/ContinueWorkPage.tsx`
- Modify: `src/App.tsx`
- Modify: `src/styles.css`
- Modify: `src/i18n/types.ts`
- Modify: `src/i18n/strings.ts`
- Test: `src/__tests__/App.test.tsx`

- [ ] **Step 1: Write the failing test**

Append this test to `src/__tests__/App.test.tsx` and run it in English mode so the assertions stay ASCII-clean:

```tsx
it("renders task-oriented navigation and defaults to Continue Work", async () => {
  localStorage.setItem(
    "chatmem.settings",
    JSON.stringify({ locale: "en", autoCheckUpdates: false }),
  );

  renderApp();

  expect(await screen.findByRole("button", { name: "Continue Work" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Needs Review" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "History" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Help" })).toBeTruthy();
  expect(screen.getByRole("heading", { name: "Continue Work" })).toBeTruthy();
  expect(screen.queryByRole("button", { name: "Checkpoints" })).toBeNull();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/App.test.tsx
```

Expected: FAIL because the app still renders the old `workspace-mode-tabs` row and has no global task navigation.

- [ ] **Step 3: Write minimal implementation**

Add the new task-view types and top navigation component.

Create `src/components/TopNavigation.tsx`:

```tsx
export type PrimaryView = "continue" | "review" | "history" | "help";

type TopNavigationProps = {
  currentView: PrimaryView;
  currentAgent: string;
  onChange: (view: PrimaryView) => void;
  onFocusSearch: () => void;
  onOpenSettings: () => void;
  t: (key: string) => string;
};

const NAV_ITEMS: Array<{ view: PrimaryView; labelKey: string }> = [
  { view: "continue", labelKey: "nav.continue" },
  { view: "review", labelKey: "nav.review" },
  { view: "history", labelKey: "nav.history" },
  { view: "help", labelKey: "nav.help" },
];

export default function TopNavigation({
  currentView,
  currentAgent,
  onChange,
  onFocusSearch,
  onOpenSettings,
  t,
}: TopNavigationProps) {
  return (
    <header className="top-navigation">
      <div className="top-navigation-brand">ChatMem</div>
      <nav className="top-navigation-links" aria-label={t("nav.aria")}>
        {NAV_ITEMS.map((item) => (
          <button
            key={item.view}
            type="button"
            className={`top-navigation-link ${currentView === item.view ? "active" : ""}`}
            onClick={() => onChange(item.view)}
          >
            {t(item.labelKey)}
          </button>
        ))}
      </nav>
      <div className="top-navigation-tools">
        <button type="button" className="toolbar-button" onClick={onFocusSearch}>
          {t("toolbar.search")}
        </button>
        <span className="agent-chip">{currentAgent}</span>
        <button type="button" className="toolbar-button" onClick={onOpenSettings}>
          {t("settings.short")}
        </button>
      </div>
    </header>
  );
}
```

Create the first minimal page shell in `src/components/ContinueWorkPage.tsx`:

```tsx
type ContinueWorkPageProps = {
  title: string;
  subtitle: string;
};

export default function ContinueWorkPage({ title, subtitle }: ContinueWorkPageProps) {
  return (
    <section className="task-page">
      <div className="task-page-header">
        <h2>{title}</h2>
        <p>{subtitle}</p>
      </div>
    </section>
  );
}
```

Replace the old `workspaceView` state in `src/App.tsx`:

```tsx
import TopNavigation, { type PrimaryView } from "./components/TopNavigation";
import ContinueWorkPage from "./components/ContinueWorkPage";

const [primaryView, setPrimaryView] = useState<PrimaryView>("continue");

useEffect(() => {
  setSelectedConversation(null);
  setCopyState({ target: null, status: "idle" });
  setPrimaryView("continue");
}, [selectedAgent]);
```

And replace the old tab row in the render tree:

```tsx
return (
  <div className="app-frame">
    <TopNavigation
      currentView={primaryView}
      currentAgent={selectedAgent}
      onChange={setPrimaryView}
      onFocusSearch={() => document.getElementById("conversation-search")?.focus()}
      onOpenSettings={() => setShowSettings(true)}
      t={t}
    />
    <div className="app-shell">
      <aside className="sidebar">
        <input
          id="conversation-search"
          type="text"
          className="search-box"
          placeholder={t("search.placeholder")}
          value={searchQuery}
          onChange={(event) => setSearchQuery(event.target.value)}
        />
        <ConversationList
          conversations={conversations}
          selectedId={selectedConversation?.id || null}
          onSelect={loadConversationDetail}
          loading={listLoading}
        />
      </aside>
      <section className="workspace">
        {primaryView === "continue" ? (
          <ContinueWorkPage
            title={t("continue.title")}
            subtitle={t("continue.subtitle")}
          />
        ) : null}
      </section>
    </div>
  </div>
);
```

Add the new translation keys in `src/i18n/types.ts`:

```ts
export type TranslationKey =
  | "brand.subtitle"
  | "search.placeholder"
  | "settings.open"
  | "settings.short"
  | "settings.title"
  | "settings.language"
  | "settings.checkUpdates"
  | "settings.autoCheck"
  | "settings.checking"
  | "settings.upToDate"
  | "settings.updateAvailablePrefix"
  | "settings.updateNow"
  | "settings.installing"
  | "settings.updateError"
  | "common.close"
  | "nav.aria"
  | "nav.continue"
  | "nav.review"
  | "nav.history"
  | "nav.help"
  | "toolbar.search"
  | "continue.title"
  | "continue.subtitle";
```

And add matching entries to `src/i18n/strings.ts`:

```ts
"zh-CN": {
  "nav.aria": "主导航",
  "nav.continue": "继续工作",
  "nav.review": "待确认",
  "nav.history": "历史",
  "nav.help": "帮助",
  "toolbar.search": "搜索",
  "continue.title": "继续工作",
  "continue.subtitle": "从最近的任务继续。",
},
en: {
  "nav.aria": "Primary navigation",
  "nav.continue": "Continue Work",
  "nav.review": "Needs Review",
  "nav.history": "History",
  "nav.help": "Help",
  "toolbar.search": "Search",
  "continue.title": "Continue Work",
  "continue.subtitle": "Pick up where you left off.",
},
```

Finally, add the shell styles in `src/styles.css`:

```css
.app-frame {
  min-height: 100vh;
  display: flex;
  flex-direction: column;
  gap: 16px;
  padding: 16px;
}

.top-navigation {
  display: grid;
  grid-template-columns: auto 1fr auto;
  align-items: center;
  gap: 16px;
  padding: 14px 18px;
  background: var(--bg-panel);
  border: 1px solid var(--border-soft);
  border-radius: var(--radius-xl);
  backdrop-filter: blur(18px);
}

.top-navigation-links {
  display: inline-flex;
  gap: 8px;
}

.top-navigation-link.active {
  background: var(--accent-soft);
  color: var(--accent-strong);
}

.task-page {
  padding: 28px 30px;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/App.test.tsx
```

Expected: PASS for the new navigation test, with the old workspace object tab row gone.

- [ ] **Step 5: Commit**

```powershell
git add src/components/TopNavigation.tsx src/components/ContinueWorkPage.tsx src/App.tsx src/styles.css src/i18n/types.ts src/i18n/strings.ts src/__tests__/App.test.tsx
git commit -m "feat: add task-oriented navigation shell"
```

### Task 2: Build the Continue Work landing page around recent tasks and recoverable progress

**Files:**
- Create: `src/components/ConversationSourceCard.tsx`
- Modify: `src/components/ContinueWorkPage.tsx`
- Modify: `src/App.tsx`
- Modify: `src/styles.css`
- Modify: `src/i18n/types.ts`
- Modify: `src/i18n/strings.ts`
- Test: `src/__tests__/App.test.tsx`

- [ ] **Step 1: Write the failing test**

Append this test:

```tsx
it("shows recent tasks, recoverable progress, and a suggested next step", async () => {
  localStorage.setItem(
    "chatmem.settings",
    JSON.stringify({ locale: "en", autoCheckUpdates: false }),
  );

  mockListCheckpoints.mockResolvedValue([
    {
      checkpoint_id: "cp-001",
      repo_root: "D:/VSP/demo",
      source_agent: "claude",
      conversation_id: "claude:conv-001",
      summary: "Debug session",
      status: "active",
      created_at: "2026-04-08T09:00:00Z",
      resume_command: "codex resume conv-001",
      handoff_id: null,
    },
  ]);
  mockListHandoffs.mockResolvedValue([
    {
      handoff_id: "handoff-001",
      from_agent: "claude",
      to_agent: "codex",
      current_goal: "Continue the rollout",
      status: "pending",
      done_items: ["Imported the latest session"],
      next_items: ["Verify the checkpoint"],
      key_files: [],
      useful_commands: [],
      checkpoint_id: "cp-001",
      target_profile: null,
      consumed_at: null,
      consumed_by: null,
    },
  ]);

  renderApp();
  fireEvent.click(await screen.findByText("Debug session"));

  expect(await screen.findByText("Recent Tasks")).toBeTruthy();
  expect(screen.getByText("Recoverable Progress")).toBeTruthy();
  expect(screen.getByText("Suggested Next Step")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Resume" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "View Summary" })).toBeTruthy();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/App.test.tsx
```

Expected: FAIL because the current `ContinueWorkPage` is only a heading/subtitle shell.

- [ ] **Step 3: Write minimal implementation**

Create `src/components/ConversationSourceCard.tsx` to replace the hard-coded meta strip from `App.tsx`:

```tsx
type ConversationSourceCardProps = {
  storagePath: string | null | undefined;
  resumeCommand: string | null | undefined;
  locationLabel: string;
  resumeLabel: string;
  unavailableLocationLabel: string;
  onCopyLocation: () => void;
  onCopyResume: () => void;
};

export default function ConversationSourceCard({
  storagePath,
  resumeCommand,
  locationLabel,
  resumeLabel,
  unavailableLocationLabel,
  onCopyLocation,
  onCopyResume,
}: ConversationSourceCardProps) {
  return (
    <article className="task-card">
      <div className="task-card-header">
        <h3>{locationLabel}</h3>
      </div>
      <p className={`task-card-path ${storagePath ? "" : "is-muted"}`}>
        {storagePath || unavailableLocationLabel}
      </p>
      <div className="task-card-actions">
        <button type="button" className="btn btn-secondary" onClick={onCopyLocation} disabled={!storagePath}>
          Copy Location
        </button>
        <button type="button" className="btn btn-secondary" onClick={onCopyResume} disabled={!resumeCommand}>
          {resumeLabel}
        </button>
      </div>
    </article>
  );
}
```

Expand `src/components/ContinueWorkPage.tsx`:

```tsx
import ConversationSourceCard from "./ConversationSourceCard";
import type { CheckpointRecord, HandoffPacket } from "../chatmem-memory/types";

type ConversationSummary = {
  id: string;
  summary: string | null;
  updated_at: string;
};

type SelectedConversation = {
  summary: string | null;
  storage_path?: string | null;
  resume_command?: string | null;
};

type ContinueWorkPageProps = {
  title: string;
  subtitle: string;
  conversations: ConversationSummary[];
  selectedConversation: SelectedConversation | null;
  checkpoints: CheckpointRecord[];
  handoffs: HandoffPacket[];
  onResume: () => void;
  onViewHistory: () => void;
  onStartTransfer: () => void;
  onCopyLocation: () => void;
  onCopyResume: () => void;
  t: (key: string) => string;
};

export default function ContinueWorkPage(props: ContinueWorkPageProps) {
  const latestCheckpoint = props.checkpoints[0];
  const latestHandoff = props.handoffs[0];

  return (
    <section className="task-page">
      <div className="task-page-header">
        <h2>{props.title}</h2>
        <p>{props.subtitle}</p>
      </div>

      <div className="task-grid">
        <article className="task-card">
          <div className="task-card-header">
            <h3>{props.t("continue.section.tasks")}</h3>
          </div>
          <ul className="task-list">
            {props.conversations.slice(0, 5).map((conversation) => (
              <li key={conversation.id}>{conversation.summary || conversation.id}</li>
            ))}
          </ul>
        </article>

        <article className="task-card task-card-primary">
          <div className="task-card-header">
            <h3>{props.t("continue.section.progress")}</h3>
          </div>
          <p>{latestCheckpoint?.summary || props.t("continue.empty.progress")}</p>
          <div className="task-card-actions">
            <button type="button" className="btn btn-primary" onClick={props.onResume}>
              {props.t("continue.action.resume")}
            </button>
            <button type="button" className="btn btn-secondary" onClick={props.onViewHistory}>
              {props.t("continue.action.summary")}
            </button>
          </div>
        </article>

        <article className="task-card">
          <div className="task-card-header">
            <h3>{props.t("continue.section.next")}</h3>
          </div>
          <p>
            {latestHandoff?.next_items?.[0] ||
              props.selectedConversation?.summary ||
              props.t("continue.empty.next")}
          </p>
          <button type="button" className="btn btn-secondary" onClick={props.onStartTransfer}>
            {props.t("continue.action.transfer")}
          </button>
        </article>

        <ConversationSourceCard
          storagePath={props.selectedConversation?.storage_path}
          resumeCommand={props.selectedConversation?.resume_command}
          locationLabel={props.t("continue.section.source")}
          resumeLabel={props.t("continue.action.copyResume")}
          unavailableLocationLabel={props.t("continue.empty.location")}
          onCopyLocation={props.onCopyLocation}
          onCopyResume={props.onCopyResume}
        />
      </div>
    </section>
  );
}
```

Update `src/App.tsx` so `Continue Work` loads only the needed data bundle:

```tsx
useEffect(() => {
  if (!activeRepoRoot) {
    setCheckpoints([]);
    setHandoffs([]);
    return;
  }

  if (primaryView !== "continue") {
    return;
  }

  const loadContinueData = async () => {
    setMemoryLoading(true);
    try {
      const [nextCheckpoints, nextHandoffs] = await Promise.all([
        listCheckpoints(activeRepoRoot),
        listHandoffs(activeRepoRoot),
      ]);
      setCheckpoints(nextCheckpoints);
      setHandoffs(nextHandoffs);
    } catch (error) {
      console.error("Failed to load continue-work data:", error);
    } finally {
      setMemoryLoading(false);
    }
  };

  void loadContinueData();
}, [activeRepoRoot, primaryView]);
```

And render it:

```tsx
{primaryView === "continue" ? (
  <ContinueWorkPage
    title={t("continue.title")}
    subtitle={t("continue.subtitle")}
    conversations={conversations}
    selectedConversation={selectedConversation}
    checkpoints={checkpoints}
    handoffs={handoffs}
    onResume={() => handleCopy("resume", selectedConversation?.resume_command)}
    onViewHistory={() => setPrimaryView("history")}
    onStartTransfer={() => handleCreateHandoff(availableHandoffTargets[0] ?? selectedAgent)}
    onCopyLocation={() => handleCopy("location", selectedConversation?.storage_path)}
    onCopyResume={() => handleCopy("resume", selectedConversation?.resume_command)}
    t={t}
  />
) : null}
```

Add the new translation keys:

```ts
| "continue.section.tasks"
| "continue.section.progress"
| "continue.section.next"
| "continue.section.source"
| "continue.empty.progress"
| "continue.empty.next"
| "continue.empty.location"
| "continue.action.resume"
| "continue.action.summary"
| "continue.action.transfer"
| "continue.action.copyResume";
```

And the copy:

```ts
"zh-CN": {
  "continue.section.tasks": "最近任务",
  "continue.section.progress": "可恢复进度",
  "continue.section.next": "建议下一步",
  "continue.section.source": "当前来源",
  "continue.empty.progress": "还没有可恢复进度。",
  "continue.empty.next": "先从左侧选择一个任务。",
  "continue.empty.location": "当前来源不可提供文件位置",
  "continue.action.resume": "恢复继续",
  "continue.action.summary": "查看摘要",
  "continue.action.transfer": "开始转交",
  "continue.action.copyResume": "复制恢复命令",
},
en: {
  "continue.section.tasks": "Recent Tasks",
  "continue.section.progress": "Recoverable Progress",
  "continue.section.next": "Suggested Next Step",
  "continue.section.source": "Current Source",
  "continue.empty.progress": "No recoverable progress yet.",
  "continue.empty.next": "Select a task from the left to continue.",
  "continue.empty.location": "No file location is available for this source.",
  "continue.action.resume": "Resume",
  "continue.action.summary": "View Summary",
  "continue.action.transfer": "Start Transfer",
  "continue.action.copyResume": "Copy Resume Command",
},
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/App.test.tsx
```

Expected: PASS for the new `Continue Work` test, with checkpoint and handoff data appearing on the landing page.

- [ ] **Step 5: Commit**

```powershell
git add src/components/ConversationSourceCard.tsx src/components/ContinueWorkPage.tsx src/App.tsx src/styles.css src/i18n/types.ts src/i18n/strings.ts src/__tests__/App.test.tsx
git commit -m "feat: build continue work page"
```

### Task 3: Turn memory review into a dedicated Needs Review page

**Files:**
- Create: `src/components/NeedsReviewPage.tsx`
- Modify: `src/App.tsx`
- Modify: `src/styles.css`
- Modify: `src/i18n/types.ts`
- Modify: `src/i18n/strings.ts`
- Test: `src/__tests__/App.test.tsx`

- [ ] **Step 1: Write the failing test**

Append this test:

```tsx
it("shows pending memory decisions on the Needs Review page", async () => {
  localStorage.setItem(
    "chatmem.settings",
    JSON.stringify({ locale: "en", autoCheckUpdates: false }),
  );

  mockListMemoryCandidates.mockResolvedValue([
    {
      candidate_id: "cand-001",
      repo_root: "D:/VSP/demo",
      summary: "Run tests before packaging",
      kind: "repo_rule",
      value: "Run npm.cmd run test:run before shipping the desktop build.",
      why_it_matters: "This catches the common shipping regressions.",
      proposed_by: "codex",
      confidence: 0.92,
      status: "pending_review",
      evidence_refs: [],
      merge_suggestion: null,
    },
  ]);
  mockListRepoMemories.mockResolvedValue([
    {
      memory_id: "mem-001",
      title: "Packaging rule",
      kind: "repo_rule",
      value: "Run tests before packaging.",
      usage_hint: "Mention this before release work.",
      status: "approved",
      freshness_status: "needs_review",
      freshness_score: 0.42,
      last_verified_at: null,
      verified_by: null,
      evidence_refs: [],
    },
  ]);

  renderApp();
  fireEvent.click(await screen.findByRole("button", { name: "Needs Review" }));

  expect(await screen.findByRole("heading", { name: "Needs Review" })).toBeTruthy();
  expect(screen.getByText("Suggested Conclusions to Remember")).toBeTruthy();
  expect(screen.getByText("Proposed Project Rules")).toBeTruthy();
  expect(screen.getByRole("button", { name: "Confirm" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Review Later" })).toBeTruthy();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/App.test.tsx
```

Expected: FAIL because there is no `NeedsReviewPage` and the review data bundle is not loaded through a dedicated task page.

- [ ] **Step 3: Write minimal implementation**

Create `src/components/NeedsReviewPage.tsx`:

```tsx
import type { ApprovedMemory, MemoryCandidate, HandoffPacket } from "../chatmem-memory/types";

type NeedsReviewPageProps = {
  candidates: MemoryCandidate[];
  memories: ApprovedMemory[];
  handoffs: HandoffPacket[];
  onApproveCandidate: (candidate: MemoryCandidate) => void;
  onRejectCandidate: (candidateId: string) => void;
  onReverify: (memoryId: string) => void;
  onReviewLater: () => void;
  t: (key: string) => string;
};

export default function NeedsReviewPage({
  candidates,
  memories,
  handoffs,
  onApproveCandidate,
  onRejectCandidate,
  onReverify,
  onReviewLater,
  t,
}: NeedsReviewPageProps) {
  const staleMemories = memories.filter(
    (memory) => memory.freshness_status === "needs_review" || memory.freshness_status === "stale",
  );
  const pendingTransfers = handoffs.filter((handoff) => handoff.status === "pending");

  return (
    <section className="task-page">
      <div className="task-page-header">
        <h2>{t("review.title")}</h2>
        <p>{t("review.subtitle")}</p>
      </div>

      <div className="task-grid">
        <article className="task-card">
          <div className="task-card-header">
            <h3>{t("review.section.candidates")}</h3>
          </div>
          {candidates.slice(0, 3).map((candidate) => (
            <div key={candidate.candidate_id} className="review-item">
              <strong>{candidate.summary}</strong>
              <p>{candidate.why_it_matters}</p>
              <div className="task-card-actions">
                <button type="button" className="btn btn-primary" onClick={() => onApproveCandidate(candidate)}>
                  {t("review.action.confirm")}
                </button>
                <button type="button" className="btn btn-secondary" onClick={onReviewLater}>
                  {t("review.action.later")}
                </button>
                <button type="button" className="btn btn-secondary" onClick={() => onRejectCandidate(candidate.candidate_id)}>
                  {t("review.action.reject")}
                </button>
              </div>
            </div>
          ))}
        </article>

        <article className="task-card">
          <div className="task-card-header">
            <h3>{t("review.section.rules")}</h3>
          </div>
          {staleMemories.slice(0, 3).map((memory) => (
            <div key={memory.memory_id} className="review-item">
              <strong>{memory.title}</strong>
              <p>{memory.usage_hint}</p>
              <button type="button" className="btn btn-secondary" onClick={() => onReverify(memory.memory_id)}>
                {t("review.action.confirm")}
              </button>
            </div>
          ))}
        </article>

        <article className="task-card">
          <div className="task-card-header">
            <h3>{t("review.section.transfers")}</h3>
          </div>
          {pendingTransfers.length === 0 ? (
            <p>{t("review.empty.transfers")}</p>
          ) : (
            pendingTransfers.slice(0, 3).map((handoff) => (
              <div key={handoff.handoff_id} className="review-item">
                <strong>{handoff.current_goal}</strong>
                <p>{handoff.next_items[0] || t("review.empty.transfers")}</p>
              </div>
            ))
          )}
        </article>
      </div>
    </section>
  );
}
```

Load the review bundle in `src/App.tsx`:

```tsx
useEffect(() => {
  if (!activeRepoRoot || primaryView !== "review") {
    return;
  }

  const loadReviewData = async () => {
    setMemoryLoading(true);
    try {
      const [nextMemories, nextCandidates, nextHandoffs] = await Promise.all([
        listRepoMemories(activeRepoRoot),
        listMemoryCandidates(activeRepoRoot, "pending_review"),
        listHandoffs(activeRepoRoot),
      ]);
      setRepoMemories(nextMemories);
      setMemoryCandidates(nextCandidates);
      setHandoffs(nextHandoffs);
    } catch (error) {
      console.error("Failed to load review data:", error);
    } finally {
      setMemoryLoading(false);
    }
  };

  void loadReviewData();
}, [activeRepoRoot, primaryView]);
```

And render the page:

```tsx
{primaryView === "review" ? (
  <NeedsReviewPage
    candidates={memoryCandidates}
    memories={repoMemories}
    handoffs={handoffs}
    onApproveCandidate={handleApproveCandidate}
    onRejectCandidate={handleRejectCandidate}
    onReverify={handleReverifyMemory}
    onReviewLater={() => setPrimaryView("continue")}
    t={t}
  />
) : null}
```

Add the keys:

```ts
| "review.title"
| "review.subtitle"
| "review.section.candidates"
| "review.section.rules"
| "review.section.transfers"
| "review.empty.transfers"
| "review.action.confirm"
| "review.action.later"
| "review.action.reject";
```

And the copy:

```ts
"zh-CN": {
  "review.title": "待确认",
  "review.subtitle": "把需要你拍板的内容集中处理。",
  "review.section.candidates": "建议记住的结论",
  "review.section.rules": "待采用的项目规则",
  "review.section.transfers": "待确认的转交摘要",
  "review.empty.transfers": "暂时没有待确认的转交内容。",
  "review.action.confirm": "确认",
  "review.action.later": "稍后处理",
  "review.action.reject": "不保留",
},
en: {
  "review.title": "Needs Review",
  "review.subtitle": "Handle the few things that need your decision.",
  "review.section.candidates": "Suggested Conclusions to Remember",
  "review.section.rules": "Proposed Project Rules",
  "review.section.transfers": "Transfer Summaries Waiting for Confirmation",
  "review.empty.transfers": "No transfers are waiting for review.",
  "review.action.confirm": "Confirm",
  "review.action.later": "Review Later",
  "review.action.reject": "Do Not Keep",
},
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/App.test.tsx
```

Expected: PASS with the review queue surfaced through `Needs Review` instead of a dedicated object tab.

- [ ] **Step 5: Commit**

```powershell
git add src/components/NeedsReviewPage.tsx src/App.tsx src/styles.css src/i18n/types.ts src/i18n/strings.ts src/__tests__/App.test.tsx
git commit -m "feat: add needs review workspace"
```

### Task 4: Consolidate existing object panels behind a History page with filters

**Files:**
- Create: `src/components/HistoryPage.tsx`
- Modify: `src/App.tsx`
- Modify: `src/styles.css`
- Modify: `src/i18n/types.ts`
- Modify: `src/i18n/strings.ts`
- Test: `src/__tests__/App.test.tsx`

- [ ] **Step 1: Write the failing test**

Append this test:

```tsx
it("routes the old object views through History filters", async () => {
  localStorage.setItem(
    "chatmem.settings",
    JSON.stringify({ locale: "en", autoCheckUpdates: false }),
  );

  mockListCheckpoints.mockResolvedValue([
    {
      checkpoint_id: "cp-001",
      repo_root: "D:/VSP/demo",
      source_agent: "claude",
      conversation_id: "claude:conv-001",
      summary: "Debug session",
      status: "active",
      created_at: "2026-04-08T09:00:00Z",
      resume_command: "codex resume conv-001",
      handoff_id: null,
    },
  ]);

  renderApp();
  fireEvent.click(await screen.findByRole("button", { name: "History" }));

  expect(await screen.findByRole("heading", { name: "History" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Conversations" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Recovery" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Transfers" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Outputs" })).toBeTruthy();

  fireEvent.click(screen.getByRole("button", { name: "Recovery" }));
  expect(await screen.findByText("Checkpoints")).toBeTruthy();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/App.test.tsx
```

Expected: FAIL because the existing panels are still only reachable through the removed `workspace-mode-tabs` row.

- [ ] **Step 3: Write minimal implementation**

Create `src/components/HistoryPage.tsx`:

```tsx
import { useState } from "react";
import ConversationDetail from "./ConversationDetail";
import CheckpointsPanel from "./CheckpointsPanel";
import EpisodesPanel from "./EpisodesPanel";
import RunsPanel from "./RunsPanel";
import ArtifactsPanel from "./ArtifactsPanel";
import HandoffsPanel from "./HandoffsPanel";
import ConversationSourceCard from "./ConversationSourceCard";
import type {
  ArtifactRecord,
  CheckpointRecord,
  EpisodeRecord,
  HandoffPacket,
  RunRecord,
} from "../chatmem-memory/types";

export type HistoryFilter = "conversations" | "recovery" | "transfers" | "outputs";

type HistoryPageProps = {
  conversation: any | null;
  checkpoints: CheckpointRecord[];
  episodes: EpisodeRecord[];
  runs: RunRecord[];
  artifacts: ArtifactRecord[];
  handoffs: HandoffPacket[];
  loading: boolean;
  currentAgent: string;
  availableTargets: string[];
  allAgents: string[];
  onCreateCheckpoint: () => void;
  onPromoteCheckpoint: (checkpoint: CheckpointRecord, targetAgent: string) => void;
  onCreateHandoff: (targetAgent: string) => void;
  onMarkConsumed: (handoffId: string) => void;
  onCopyLocation: () => void;
  onCopyResume: () => void;
  t: (key: string) => string;
};

const FILTERS: Array<{ value: HistoryFilter; key: string }> = [
  { value: "conversations", key: "history.filter.conversations" },
  { value: "recovery", key: "history.filter.recovery" },
  { value: "transfers", key: "history.filter.transfers" },
  { value: "outputs", key: "history.filter.outputs" },
];

export default function HistoryPage(props: HistoryPageProps) {
  const [filter, setFilter] = useState<HistoryFilter>("conversations");

  return (
    <section className="task-page">
      <div className="task-page-header">
        <h2>{props.t("history.title")}</h2>
        <p>{props.t("history.subtitle")}</p>
      </div>

      <div className="history-filters">
        {FILTERS.map((item) => (
          <button
            key={item.value}
            type="button"
            className={`history-filter ${filter === item.value ? "active" : ""}`}
            onClick={() => setFilter(item.value)}
          >
            {props.t(item.key)}
          </button>
        ))}
      </div>

      <div className="history-body">
        {filter === "conversations" && props.conversation ? (
          <>
            <ConversationSourceCard
              storagePath={props.conversation.storage_path}
              resumeCommand={props.conversation.resume_command}
              locationLabel={props.t("continue.section.source")}
              resumeLabel={props.t("continue.action.copyResume")}
              unavailableLocationLabel={props.t("continue.empty.location")}
              onCopyLocation={props.onCopyLocation}
              onCopyResume={props.onCopyResume}
            />
            <ConversationDetail conversation={props.conversation} />
          </>
        ) : null}

        {filter === "recovery" ? (
          <>
            <CheckpointsPanel
              checkpoints={props.checkpoints}
              loading={props.loading}
              allAgents={props.allAgents}
              onCreate={props.onCreateCheckpoint}
              onPromote={props.onPromoteCheckpoint}
            />
            <EpisodesPanel episodes={props.episodes} loading={props.loading} />
          </>
        ) : null}

        {filter === "transfers" ? (
          <HandoffsPanel
            handoffs={props.handoffs}
            loading={props.loading}
            currentAgent={props.currentAgent}
            availableTargets={props.availableTargets}
            onCreate={props.onCreateHandoff}
            onMarkConsumed={props.onMarkConsumed}
          />
        ) : null}

        {filter === "outputs" ? (
          <>
            <RunsPanel runs={props.runs} loading={props.loading} />
            <ArtifactsPanel artifacts={props.artifacts} loading={props.loading} />
          </>
        ) : null}
      </div>
    </section>
  );
}
```

Load the history bundle in parallel in `src/App.tsx`:

```tsx
useEffect(() => {
  if (!activeRepoRoot || primaryView !== "history") {
    return;
  }

  const loadHistoryData = async () => {
    setMemoryLoading(true);
    try {
      const [nextCheckpoints, nextEpisodes, nextRuns, nextArtifacts, nextHandoffs] =
        await Promise.all([
          listCheckpoints(activeRepoRoot),
          listEpisodes(activeRepoRoot),
          listRuns(activeRepoRoot),
          listArtifacts(activeRepoRoot),
          listHandoffs(activeRepoRoot),
        ]);
      setCheckpoints(nextCheckpoints);
      setEpisodes(nextEpisodes);
      setRuns(nextRuns);
      setArtifacts(nextArtifacts);
      setHandoffs(nextHandoffs);
    } catch (error) {
      console.error("Failed to load history data:", error);
    } finally {
      setMemoryLoading(false);
    }
  };

  void loadHistoryData();
}, [activeRepoRoot, primaryView]);
```

And render the page:

```tsx
{primaryView === "history" ? (
  <HistoryPage
    conversation={selectedConversation}
    checkpoints={checkpoints}
    episodes={episodes}
    runs={runs}
    artifacts={artifacts}
    handoffs={handoffs}
    loading={memoryLoading}
    currentAgent={selectedAgent}
    availableTargets={availableHandoffTargets}
    allAgents={allAgents}
    onCreateCheckpoint={handleCreateCheckpoint}
    onPromoteCheckpoint={handlePromoteCheckpoint}
    onCreateHandoff={handleCreateHandoff}
    onMarkConsumed={handleMarkHandoffConsumed}
    onCopyLocation={() => handleCopy("location", selectedConversation?.storage_path)}
    onCopyResume={() => handleCopy("resume", selectedConversation?.resume_command)}
    t={t}
  />
) : null}
```

Add the keys:

```ts
| "history.title"
| "history.subtitle"
| "history.filter.conversations"
| "history.filter.recovery"
| "history.filter.transfers"
| "history.filter.outputs";
```

Add the copy:

```ts
"zh-CN": {
  "history.title": "历史",
  "history.subtitle": "只在需要时回看过去的工作。",
  "history.filter.conversations": "对话",
  "history.filter.recovery": "恢复",
  "history.filter.transfers": "转交",
  "history.filter.outputs": "产出",
},
en: {
  "history.title": "History",
  "history.subtitle": "Review past work only when you need it.",
  "history.filter.conversations": "Conversations",
  "history.filter.recovery": "Recovery",
  "history.filter.transfers": "Transfers",
  "history.filter.outputs": "Outputs",
},
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/App.test.tsx
```

Expected: PASS, with the old object views available only through `History` filters.

- [ ] **Step 5: Commit**

```powershell
git add src/components/HistoryPage.tsx src/App.tsx src/styles.css src/i18n/types.ts src/i18n/strings.ts src/__tests__/App.test.tsx
git commit -m "feat: consolidate history views"
```

### Task 5: Add the Help page with FAQ cards, quick actions, and advanced troubleshooting

**Files:**
- Create: `src/components/HelpPage.tsx`
- Modify: `src/App.tsx`
- Modify: `src/styles.css`
- Modify: `src/i18n/types.ts`
- Modify: `src/i18n/strings.ts`
- Test: `src/__tests__/App.test.tsx`

- [ ] **Step 1: Write the failing test**

Append this test:

```tsx
it("shows the Help page cards and advanced troubleshooting details", async () => {
  localStorage.setItem(
    "chatmem.settings",
    JSON.stringify({ locale: "en", autoCheckUpdates: false }),
  );

  renderApp();
  fireEvent.click(await screen.findByText("Debug session"));
  fireEvent.click(screen.getByRole("button", { name: "Help" }));

  expect(await screen.findByRole("heading", { name: "Need help?" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "View Progress" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Start Transfer" })).toBeTruthy();

  fireEvent.click(screen.getByRole("button", { name: "See How It Works" }));
  expect(
    screen.getByText("ChatMem often helps with recovery, transfer, and project memory in the background."),
  ).toBeTruthy();

  fireEvent.click(screen.getByText("Advanced Troubleshooting"));
  expect(screen.getByText("codex resume conv-001")).toBeTruthy();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/App.test.tsx
```

Expected: FAIL because there is no `HelpPage` and the FAQ/advanced sections do not exist.

- [ ] **Step 3: Write minimal implementation**

Create `src/components/HelpPage.tsx`:

```tsx
import { useState } from "react";
import type { PrimaryView } from "./TopNavigation";

type HelpTopic = "resume" | "transfer" | "memory" | "mention" | "start";

type HelpPageProps = {
  onNavigate: (view: PrimaryView) => void;
  onStartTransfer: () => void;
  activeRepoRoot: string | null;
  storagePath: string | null | undefined;
  resumeCommand: string | null | undefined;
  t: (key: string) => string;
};

export default function HelpPage({
  onNavigate,
  onStartTransfer,
  activeRepoRoot,
  storagePath,
  resumeCommand,
  t,
}: HelpPageProps) {
  const [topic, setTopic] = useState<HelpTopic>("resume");

  return (
    <section className="task-page">
      <div className="task-page-header">
        <h2>{t("help.title")}</h2>
        <p>{t("help.subtitle")}</p>
      </div>

      <div className="help-search-row">
        <input type="text" className="search-box" placeholder={t("help.search")} />
      </div>

      <div className="help-card-grid">
        <article className="task-card">
          <h3>{t("help.card.resume.title")}</h3>
          <p>{t("help.card.resume.body")}</p>
          <button type="button" className="btn btn-primary" onClick={() => onNavigate("continue")}>
            {t("help.card.resume.action")}
          </button>
        </article>

        <article className="task-card">
          <h3>{t("help.card.transfer.title")}</h3>
          <p>{t("help.card.transfer.body")}</p>
          <button type="button" className="btn btn-secondary" onClick={onStartTransfer}>
            {t("help.card.transfer.action")}
          </button>
        </article>

        <article className="task-card">
          <h3>{t("help.card.memory.title")}</h3>
          <p>{t("help.card.memory.body")}</p>
          <button type="button" className="btn btn-secondary" onClick={() => onNavigate("review")}>
            {t("help.card.memory.action")}
          </button>
        </article>

        <article className="task-card">
          <h3>{t("help.card.mention.title")}</h3>
          <p>{t("help.card.mention.body")}</p>
          <button type="button" className="btn btn-secondary" onClick={() => setTopic("mention")}>
            {t("help.card.mention.action")}
          </button>
        </article>

        <article className="task-card">
          <h3>{t("help.card.start.title")}</h3>
          <p>{t("help.card.start.body")}</p>
          <button type="button" className="btn btn-secondary" onClick={() => onNavigate("continue")}>
            {t("help.card.start.action")}
          </button>
        </article>
      </div>

      <article className="task-card help-answer-card">
        <h3>{t(`help.answer.${topic}.title`)}</h3>
        <p>{t(`help.answer.${topic}.body`)}</p>
      </article>

      <details className="help-advanced">
        <summary>{t("help.advanced.title")}</summary>
        <div className="help-advanced-grid">
          <div>
            <strong>{t("help.advanced.connection")}</strong>
            <p>{activeRepoRoot ? t("help.advanced.ready") : t("help.advanced.notReady")}</p>
          </div>
          <div>
            <strong>{t("help.advanced.repoPath")}</strong>
            <p>{activeRepoRoot || t("help.advanced.none")}</p>
          </div>
          <div>
            <strong>{t("help.advanced.storagePath")}</strong>
            <p>{storagePath || t("help.advanced.none")}</p>
          </div>
          <div>
            <strong>{t("help.advanced.resume")}</strong>
            <p>{resumeCommand || t("help.advanced.none")}</p>
          </div>
        </div>
      </details>
    </section>
  );
}
```

Render it from `src/App.tsx`:

```tsx
{primaryView === "help" ? (
  <HelpPage
    onNavigate={setPrimaryView}
    onStartTransfer={() => handleCreateHandoff(availableHandoffTargets[0] ?? selectedAgent)}
    activeRepoRoot={activeRepoRoot}
    storagePath={selectedConversation?.storage_path}
    resumeCommand={selectedConversation?.resume_command}
    t={t}
  />
) : null}
```

Add the translation keys:

```ts
| "help.title"
| "help.subtitle"
| "help.search"
| "help.card.resume.title"
| "help.card.resume.body"
| "help.card.resume.action"
| "help.card.transfer.title"
| "help.card.transfer.body"
| "help.card.transfer.action"
| "help.card.memory.title"
| "help.card.memory.body"
| "help.card.memory.action"
| "help.card.mention.title"
| "help.card.mention.body"
| "help.card.mention.action"
| "help.card.start.title"
| "help.card.start.body"
| "help.card.start.action"
| "help.answer.resume.title"
| "help.answer.resume.body"
| "help.answer.transfer.title"
| "help.answer.transfer.body"
| "help.answer.memory.title"
| "help.answer.memory.body"
| "help.answer.mention.title"
| "help.answer.mention.body"
| "help.answer.start.title"
| "help.answer.start.body"
| "help.advanced.title"
| "help.advanced.connection"
| "help.advanced.ready"
| "help.advanced.notReady"
| "help.advanced.repoPath"
| "help.advanced.storagePath"
| "help.advanced.resume"
| "help.advanced.none";
```

And the approved copy:

```ts
"zh-CN": {
  "help.title": "需要帮忙？",
  "help.subtitle": "从最常见的问题开始。",
  "help.search": "搜索问题",
  "help.card.resume.title": "继续上次工作",
  "help.card.resume.body": "回到最近一次进度。",
  "help.card.resume.action": "查看进度",
  "help.card.transfer.title": "切换 Agent",
  "help.card.transfer.body": "把当前任务交给另一个 Agent。",
  "help.card.transfer.action": "开始转交",
  "help.card.memory.title": "为什么没记住",
  "help.card.memory.body": "有些内容需要确认后才会保留。",
  "help.card.memory.action": "查看待确认",
  "help.card.mention.title": "为什么找不到 @chatmem",
  "help.card.mention.body": "ChatMem 往往在后台工作。",
  "help.card.mention.action": "查看用法",
  "help.card.start.title": "我该从哪里开始",
  "help.card.start.body": "先从“继续工作”开始。",
  "help.card.start.action": "返回继续工作",
  "help.answer.resume.title": "回到最近进度",
  "help.answer.resume.body": "这里会显示最近一次可以继续的位置。你可以直接恢复，也可以先查看摘要再决定。",
  "help.answer.transfer.title": "转交给下一个 Agent",
  "help.answer.transfer.body": "转交前会整理当前目标、已完成内容和下一步建议，减少重复解释。",
  "help.answer.memory.title": "哪些内容会被记住",
  "help.answer.memory.body": "只有适合长期保留的项目规则、稳定结论和可复用经验，才会进入项目记忆。",
  "help.answer.mention.title": "为什么不需要 @",
  "help.answer.mention.body": "ChatMem 通常在后台帮助恢复、转交和读取项目记忆。",
  "help.answer.start.title": "最推荐的起点",
  "help.answer.start.body": "大多数时候，从“继续工作”开始就够了。",
  "help.advanced.title": "高级排障",
  "help.advanced.connection": "连接状态",
  "help.advanced.ready": "当前工作区已准备就绪。",
  "help.advanced.notReady": "先选择一段对话来加载工作区。",
  "help.advanced.repoPath": "当前项目路径",
  "help.advanced.storagePath": "对话文件路径",
  "help.advanced.resume": "恢复命令",
  "help.advanced.none": "当前没有可显示的内容。",
},
en: {
  "help.title": "Need help?",
  "help.subtitle": "Start with the most common questions.",
  "help.search": "Search questions",
  "help.card.resume.title": "Continue Previous Work",
  "help.card.resume.body": "Go back to the latest progress.",
  "help.card.resume.action": "View Progress",
  "help.card.transfer.title": "Switch Agent",
  "help.card.transfer.body": "Pass the current task to another agent.",
  "help.card.transfer.action": "Start Transfer",
  "help.card.memory.title": "Why wasn't this remembered?",
  "help.card.memory.body": "Some information needs review before it is kept.",
  "help.card.memory.action": "View Review Queue",
  "help.card.mention.title": "Why can't I find @chatmem?",
  "help.card.mention.body": "ChatMem often works in the background.",
  "help.card.mention.action": "See How It Works",
  "help.card.start.title": "Where should I start?",
  "help.card.start.body": "Start with Continue Work.",
  "help.card.start.action": "Go to Continue Work",
  "help.answer.resume.title": "Return to recent progress",
  "help.answer.resume.body": "This page shows the latest progress you can continue. You can resume directly or inspect the summary first.",
  "help.answer.transfer.title": "Transfer to another agent",
  "help.answer.transfer.body": "Before a transfer, the app packages the current goal, completed work, and the next suggested steps.",
  "help.answer.memory.title": "What gets remembered",
  "help.answer.memory.body": "Only stable project rules, durable conclusions, and reusable experience should become project memory.",
  "help.answer.mention.title": "Why you do not need @chatmem",
  "help.answer.mention.body": "ChatMem often helps with recovery, transfer, and project memory in the background.",
  "help.answer.start.title": "The best place to begin",
  "help.answer.start.body": "Most of the time, starting with Continue Work is enough.",
  "help.advanced.title": "Advanced Troubleshooting",
  "help.advanced.connection": "Connection Status",
  "help.advanced.ready": "The current workspace is ready.",
  "help.advanced.notReady": "Select a conversation first to load workspace context.",
  "help.advanced.repoPath": "Current Project Path",
  "help.advanced.storagePath": "Conversation File Path",
  "help.advanced.resume": "Resume Command",
  "help.advanced.none": "Nothing is available here yet.",
},
```

- [ ] **Step 4: Run test to verify it passes**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/App.test.tsx
```

Expected: PASS, including the FAQ answer and advanced troubleshooting content.

- [ ] **Step 5: Commit**

```powershell
git add src/components/HelpPage.tsx src/App.tsx src/styles.css src/i18n/types.ts src/i18n/strings.ts src/__tests__/App.test.tsx
git commit -m "feat: add help triage page"
```

### Task 6: Remove shell leftovers, localize the remaining detail copy, and run the full regression sweep

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/components/ConversationDetail.tsx`
- Modify: `src/styles.css`
- Modify: `src/i18n/types.ts`
- Modify: `src/i18n/strings.ts`
- Test: `src/__tests__/App.test.tsx`

- [ ] **Step 1: Write the failing regression tests**

Append these tests:

```tsx
it("localizes the new shell and detail copy in English mode", async () => {
  localStorage.setItem(
    "chatmem.settings",
    JSON.stringify({ locale: "en", autoCheckUpdates: false }),
  );

  renderApp();
  fireEvent.click(await screen.findByText("Debug session"));
  fireEvent.click(screen.getByRole("button", { name: "History" }));

  expect(await screen.findByText("Messages")).toBeTruthy();
  expect(screen.getByText("File Changes")).toBeTruthy();
  expect(screen.getByText("Tool Calls")).toBeTruthy();
});

it("does not render the old workspace object tabs anymore", async () => {
  renderApp();

  expect(screen.queryByRole("button", { name: "Conversation" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Repo Memory" })).toBeNull();
  expect(screen.queryByRole("button", { name: "Handoffs" })).toBeNull();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/App.test.tsx
```

Expected: FAIL because `ConversationDetail.tsx` still contains hard-coded, partially corrupted labels and `App.tsx` still carries the old workspace-tab variables and helpers.

- [ ] **Step 3: Write minimal implementation**

Route `ConversationDetail.tsx` through i18n:

```tsx
import { useI18n } from "../i18n/I18nProvider";

function formatRole(role: string, t: (key: string) => string) {
  switch (role) {
    case "user":
      return t("detail.role.user");
    case "assistant":
      return t("detail.role.assistant");
    case "system":
      return t("detail.role.system");
    default:
      return role;
  }
}

export default function ConversationDetail({ conversation }: ConversationDetailProps) {
  const { t } = useI18n();
  const toolCallCount = conversation.messages.reduce(
    (count, message) => count + message.tool_calls.length,
    0,
  );

  return (
    <div className="conversation-detail">
      <div className="stats">
        <div className="stat-item">
          <div className="stat-value">{conversation.messages.length}</div>
          <div className="stat-label">{t("detail.stats.messages")}</div>
        </div>
        <div className="stat-item">
          <div className="stat-value">{conversation.file_changes.length}</div>
          <div className="stat-label">{t("detail.stats.files")}</div>
        </div>
        <div className="stat-item">
          <div className="stat-value">{toolCallCount}</div>
          <div className="stat-label">{t("detail.stats.tools")}</div>
        </div>
      </div>
    </div>
  );
}
```

Add the detail keys:

```ts
| "detail.role.user"
| "detail.role.assistant"
| "detail.role.system"
| "detail.stats.messages"
| "detail.stats.files"
| "detail.stats.tools";
```

And the copy:

```ts
"zh-CN": {
  "detail.role.user": "用户",
  "detail.role.assistant": "助手",
  "detail.role.system": "系统",
  "detail.stats.messages": "消息数",
  "detail.stats.files": "文件变更",
  "detail.stats.tools": "工具调用",
},
en: {
  "detail.role.user": "User",
  "detail.role.assistant": "Assistant",
  "detail.role.system": "System",
  "detail.stats.messages": "Messages",
  "detail.stats.files": "File Changes",
  "detail.stats.tools": "Tool Calls",
},
```

Then remove the obsolete shell code from `src/App.tsx`:

```tsx
// delete:
type WorkspaceView =
  | "conversation"
  | "checkpoints"
  | "repo-memory"
  | "memory-inbox"
  | "approvals"
  | "episodes"
  | "runs"
  | "artifacts"
  | "handoffs";

const [workspaceView, setWorkspaceView] = useState<WorkspaceView>("conversation");

// delete the entire <div className="workspace-mode-tabs">...</div> block
// delete the old selectedConversation meta strip that is now owned by ConversationSourceCard
```

And trim the old styles:

```css
/* delete */
.workspace-mode-tabs { display: none; }
.workspace-mode-tab { display: none; }

/* add */
.help-card-grid,
.task-grid,
.history-body {
  display: grid;
  gap: 16px;
}
```

- [ ] **Step 4: Run the full regression suite and build**

Run:

```powershell
npm.cmd run test:run
npm.cmd run build
```

Expected:

- `vitest run` exits with PASS
- `tsc && vite build` exits successfully and emits the production bundle

- [ ] **Step 5: Commit**

```powershell
git add src/App.tsx src/components/ConversationDetail.tsx src/styles.css src/i18n/types.ts src/i18n/strings.ts src/__tests__/App.test.tsx
git commit -m "refactor: finalize task-oriented chatmem shell"
```

## Self-Review

### 1. Spec coverage

- Global top navigation: covered by Task 1
- Continue Work landing experience: covered by Task 2
- Needs Review page: covered by Task 3
- History surface with filters: covered by Task 4
- Help page with FAQ cards and advanced troubleshooting: covered by Task 5
- Terminology cleanup and removal of the old object-tab shell: covered by Task 6

No spec gaps remain.

### 2. Placeholder scan

Checked for `TODO`, `TBD`, "appropriate error handling", "similar to Task N", and steps without concrete commands or code. None remain.

### 3. Type consistency

- `PrimaryView` uses `continue`, `review`, `history`, `help` everywhere
- `HistoryFilter` uses `conversations`, `recovery`, `transfers`, `outputs` everywhere
- Continue Work button labels use the same `continue.action.*` keys across the plan
- Detail labels use `detail.stats.*` consistently in Task 6

## Execution Handoff

Plan complete and saved to `docs/superpowers/plans/2026-04-21-chatmem-task-nav-help.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

**Which approach?**
