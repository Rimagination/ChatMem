# ChatMem Memory Seeding Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the full memory seeding flow in one delivery: current conversation to candidate memory, project old-conversation batch prompt, review modal, and project-home memory ownership.

**Architecture:** Keep ChatMem desktop model agent-assisted: the app generates copyable prompts and reviews candidates; the current agent creates candidates through ChatMem MCP `create_memory_candidate`. Move memory visibility from the selected conversation side rail into a project-home surface and a review modal. Keep selected conversations full-width and use badges/buttons for memory actions.

**Tech Stack:** React, TypeScript, Vite, Vitest, Testing Library, Tauri invoke commands, existing ChatMem MCP/store APIs.

---

## Planned File Changes

- Create: `src/memory-seeding/types.ts`
  - Shared TypeScript types for seeding scopes, prompt inputs, project scan ranges, and review copy.
- Create: `src/memory-seeding/prompt.ts`
  - Pure prompt builders for current-conversation and project-scan agent-assisted tasks.
- Create: `src/components/ProjectHome.tsx`
  - Project-level surface for project memory, pending candidates, scan actions, and recent conversations.
- Create: `src/components/MemorySeedModal.tsx`
  - Modal that explains agent-assisted generation and lets users copy the current conversation or project scan prompt.
- Create: `src/components/MemoryReviewModal.tsx`
  - Modal for pending candidates with confirm, edit, review later, and reject actions.
- Modify: `src/App.tsx`
  - Add project-home routing, remove the persistent memory side panel from conversation view, wire seeding and review modals, and add prompt-copy state.
- Modify: `src/chatmem-memory/api.ts`
  - No backend call is needed for prompt generation; keep existing review/list calls. Add a small typed helper only if implementation needs a refresh function wrapper.
- Modify: `src/chatmem-memory/types.ts`
  - Add optional missing freshness fields to tests or normalize test fixtures so existing UI handles partial responses safely.
- Modify: `src/styles.css`
  - Add project home, modal, prompt box, badge, full-width conversation layout, and responsive styles.
- Test: `src/__tests__/memorySeedingPrompts.test.ts`
  - Unit tests for prompt content and safety rules.
- Test: `src/__tests__/MemoryWorkspace.test.tsx`
  - Update old side-panel tests to project-home and modal behavior.
- Test: `src/__tests__/App.test.tsx`
  - Update conversation workspace expectations so conversation view is full-width and exposes seeding controls.

## Task 1: Prompt Builders

**Files:**
- Create: `src/memory-seeding/types.ts`
- Create: `src/memory-seeding/prompt.ts`
- Test: `src/__tests__/memorySeedingPrompts.test.ts`

- [ ] **Step 1: Write the failing tests**

Create `src/__tests__/memorySeedingPrompts.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import {
  buildCurrentConversationMemoryPrompt,
  buildProjectScanMemoryPrompt,
} from "../memory-seeding/prompt";

describe("memory seeding prompts", () => {
  it("builds a current-conversation prompt that creates candidates without approving them", () => {
    const prompt = buildCurrentConversationMemoryPrompt({
      repoRoot: "D:/VSP/agentswap-gui",
      sourceAgent: "claude",
      conversationId: "conv-001",
      conversationTitle: "Release workflow notes",
      storagePath: "C:/Users/demo/.claude/projects/conv-001.jsonl",
      resumeCommand: "claude --resume conv-001",
    });

    expect(prompt).toContain("repo: D:/VSP/agentswap-gui");
    expect(prompt).toContain("conversation id: claude:conv-001");
    expect(prompt).toContain("storage path: C:/Users/demo/.claude/projects/conv-001.jsonl");
    expect(prompt).toContain("Use ChatMem MCP `create_memory_candidate`");
    expect(prompt).toContain("Do not approve candidates");
    expect(prompt).toContain("Do not record secrets, tokens, credentials");
    expect(prompt).toContain("project rules, architecture decisions, recurring commands");
  });

  it("builds a project-scan prompt with an explicit range and conversation list", () => {
    const prompt = buildProjectScanMemoryPrompt({
      repoRoot: "D:/VSP/agentswap-gui",
      sourceAgent: "codex",
      rangeLabel: "Recent 20 conversations",
      conversations: [
        {
          id: "conv-a",
          title: "Build portable release",
          updatedAt: "2026-04-20T10:00:00Z",
          storagePath: "C:/Users/demo/.codex/sessions/conv-a.jsonl",
        },
        {
          id: "conv-b",
          title: "Fix memory review",
          updatedAt: "2026-04-20T12:00:00Z",
          storagePath: null,
        },
      ],
    });

    expect(prompt).toContain("scan range: Recent 20 conversations");
    expect(prompt).toContain("codex:conv-a");
    expect(prompt).toContain("Build portable release");
    expect(prompt).toContain("C:/Users/demo/.codex/sessions/conv-a.jsonl");
    expect(prompt).toContain("codex:conv-b");
    expect(prompt).toContain("No storage path captured");
    expect(prompt).toContain("Create one candidate per durable fact");
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/memorySeedingPrompts.test.ts
```

Expected: FAIL because `src/memory-seeding/prompt.ts` does not exist.

- [ ] **Step 3: Add shared types**

Create `src/memory-seeding/types.ts`:

```ts
export type MemorySeedConversation = {
  id: string;
  title: string;
  updatedAt: string;
  storagePath: string | null;
};

export type CurrentConversationPromptInput = {
  repoRoot: string;
  sourceAgent: string;
  conversationId: string;
  conversationTitle: string;
  storagePath: string | null | undefined;
  resumeCommand: string | null | undefined;
};

export type ProjectScanPromptInput = {
  repoRoot: string;
  sourceAgent: string;
  rangeLabel: string;
  conversations: MemorySeedConversation[];
};

export type MemorySeedMode = "current-conversation" | "project-scan";

export type MemorySeedModalState =
  | {
      mode: "current-conversation";
      prompt: string;
      title: string;
      subtitle: string;
    }
  | {
      mode: "project-scan";
      prompt: string;
      title: string;
      subtitle: string;
      conversationCount: number;
    };
```

- [ ] **Step 4: Add prompt builders**

Create `src/memory-seeding/prompt.ts`:

```ts
import type {
  CurrentConversationPromptInput,
  MemorySeedConversation,
  ProjectScanPromptInput,
} from "./types";

function formatStoragePath(storagePath: string | null | undefined) {
  return storagePath?.trim() ? storagePath : "No storage path captured";
}

function formatResumeCommand(resumeCommand: string | null | undefined) {
  return resumeCommand?.trim() ? resumeCommand : "No resume command captured";
}

function durableMemoryRules() {
  return [
    "Only create durable, repo-scoped, future-useful memory candidates.",
    "Prefer project rules, architecture decisions, recurring commands, gotchas, user preferences, and verification requirements.",
    "Do not record secrets, tokens, credentials, private account details, temporary task lists, one-off debug noise, or unverified guesses.",
    "Create pending candidates only. Do not approve candidates.",
    "Use concise summaries and include evidence excerpts when available.",
  ].join("\n- ");
}

function formatConversationLine(sourceAgent: string, conversation: MemorySeedConversation) {
  return [
    `- conversation id: ${sourceAgent}:${conversation.id}`,
    `  title: ${conversation.title}`,
    `  updated: ${conversation.updatedAt}`,
    `  storage path: ${formatStoragePath(conversation.storagePath)}`,
  ].join("\n");
}

export function buildCurrentConversationMemoryPrompt(input: CurrentConversationPromptInput) {
  return [
    "Use ChatMem to extract project memory candidates from the selected conversation.",
    "",
    `repo: ${input.repoRoot}`,
    `source agent: ${input.sourceAgent}`,
    `conversation id: ${input.sourceAgent}:${input.conversationId}`,
    `conversation title: ${input.conversationTitle}`,
    `storage path: ${formatStoragePath(input.storagePath)}`,
    `resume command: ${formatResumeCommand(input.resumeCommand)}`,
    "",
    "Rules:",
    `- ${durableMemoryRules()}`,
    "",
    "Action:",
    "- Read the selected conversation from local history or the storage path above.",
    "- Use ChatMem MCP `create_memory_candidate` for every durable memory candidate.",
    "- Set `repo_root` to the repo above.",
    "- Use evidence excerpts from the conversation when possible.",
    "- Do not approve candidates; leave them pending for human review.",
  ].join("\n");
}

export function buildProjectScanMemoryPrompt(input: ProjectScanPromptInput) {
  const conversationLines =
    input.conversations.length > 0
      ? input.conversations
          .map((conversation) => formatConversationLine(input.sourceAgent, conversation))
          .join("\n")
      : "- No conversations selected";

  return [
    "Use ChatMem to scan prior project conversations and extract project memory candidates.",
    "",
    `repo: ${input.repoRoot}`,
    `source agent: ${input.sourceAgent}`,
    `scan range: ${input.rangeLabel}`,
    "",
    "Conversations:",
    conversationLines,
    "",
    "Rules:",
    `- ${durableMemoryRules()}`,
    "- Create one candidate per durable fact.",
    "- Avoid duplicate candidates; if a fact overlaps existing memory, make the value merge-aware.",
    "",
    "Action:",
    "- Read the listed conversations from local history or storage paths where available.",
    "- Use ChatMem MCP `create_memory_candidate` for durable candidates.",
    "- Include conversation-specific evidence excerpts when possible.",
    "- Do not approve candidates; leave them pending for human review.",
  ].join("\n");
}
```

- [ ] **Step 5: Run prompt tests**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/memorySeedingPrompts.test.ts
```

Expected: PASS.

- [ ] **Step 6: Commit**

Run:

```powershell
git add src/memory-seeding/types.ts src/memory-seeding/prompt.ts src/__tests__/memorySeedingPrompts.test.ts
git commit -m "feat: add memory seeding prompt builders"
```

## Task 2: Project Home Surface

**Files:**
- Create: `src/components/ProjectHome.tsx`
- Modify: `src/App.tsx`
- Modify: `src/styles.css`
- Test: `src/__tests__/MemoryWorkspace.test.tsx`

- [ ] **Step 1: Replace the old side-panel expectation with a project-home test**

In `src/__tests__/MemoryWorkspace.test.tsx`, replace the test named `"surfaces project memory and memory candidates beside the selected conversation"` with:

```tsx
it("opens a project home that owns project memory and pending candidates", async () => {
  renderApp();

  fireEvent.click(await screen.findByRole("button", { name: /agentswap-gui/i }));

  await waitFor(() => {
    expect(screen.getByRole("heading", { name: "Project Context" })).toBeTruthy();
    expect(screen.getByText("D:/VSP/agentswap-gui")).toBeTruthy();
    expect(screen.getByText("Primary verification")).toBeTruthy();
    expect(screen.getByText("npm run test:run")).toBeTruthy();
    expect(screen.getByText("Review pending memory")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Scan old conversations" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Review pending memory" })).toBeTruthy();
  });

  expect(screen.queryByRole("heading", { name: "Memory Candidates" })).toBeNull();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/MemoryWorkspace.test.tsx
```

Expected: FAIL because clicking a project header only expands/collapses and no project home exists.

- [ ] **Step 3: Create ProjectHome component**

Create `src/components/ProjectHome.tsx`:

```tsx
import type { ApprovedMemory, MemoryCandidate } from "../chatmem-memory/types";
import type { MemorySeedConversation } from "../memory-seeding/types";

type ProjectHomeProps = {
  locale: "zh-CN" | "en";
  projectLabel: string;
  projectPath: string;
  conversations: MemorySeedConversation[];
  memories: ApprovedMemory[];
  candidates: MemoryCandidate[];
  loading: boolean;
  onOpenConversation: (conversationId: string) => void;
  onSeedProject: () => void;
  onOpenReview: () => void;
};

function copy(locale: ProjectHomeProps["locale"]) {
  return locale === "en"
    ? {
        eyebrow: "Project",
        title: "Project Context",
        summary: "This is what future agents can use when they resume this project.",
        memory: "Approved project memory",
        pending: "Pending memory",
        recent: "Recent conversations",
        scan: "Scan old conversations",
        review: "Review pending memory",
        emptyMemory: "No approved project memory yet.",
        emptyPending: "No pending memory candidates.",
      }
    : {
        eyebrow: "项目",
        title: "项目上下文",
        summary: "这里的内容会影响未来 agent 恢复这个项目时看到的背景。",
        memory: "已确认项目记忆",
        pending: "待确认记忆",
        recent: "最近对话",
        scan: "扫描旧对话",
        review: "审核待确认记忆",
        emptyMemory: "还没有已确认的项目记忆。",
        emptyPending: "没有待确认的记忆候选。",
      };
}

export default function ProjectHome({
  locale,
  projectLabel,
  projectPath,
  conversations,
  memories,
  candidates,
  loading,
  onOpenConversation,
  onSeedProject,
  onOpenReview,
}: ProjectHomeProps) {
  const text = copy(locale);

  return (
    <div className="project-home-page">
      <header className="project-home-hero">
        <p className="page-eyebrow">{text.eyebrow}</p>
        <h1>{text.title}</h1>
        <p>{text.summary}</p>
        <div className="project-home-path">{projectPath}</div>
        <div className="project-home-stats">
          <span>{conversations.length} conversations</span>
          <span>{memories.length} memories</span>
          <span>{candidates.length} pending</span>
        </div>
      </header>

      <div className="project-home-actions">
        <button type="button" className="btn btn-primary" onClick={onSeedProject}>
          {text.scan}
        </button>
        <button
          type="button"
          className="btn btn-secondary"
          onClick={onOpenReview}
          disabled={candidates.length === 0}
        >
          {text.review}
        </button>
      </div>

      <div className="project-home-grid">
        <section className="task-panel">
          <div className="task-panel-header">
            <div>
              <span className="task-panel-label">{projectLabel}</span>
              <h2>{text.memory}</h2>
            </div>
          </div>
          {loading ? (
            <div className="loading-inline"><div className="spinner"></div></div>
          ) : memories.length === 0 ? (
            <div className="inline-empty-body">{text.emptyMemory}</div>
          ) : (
            <div className="memory-card-list">
              {memories.map((memory) => (
                <article key={memory.memory_id} className="memory-card">
                  <strong>{memory.title}</strong>
                  <p>{memory.value}</p>
                  {memory.usage_hint ? <span>{memory.usage_hint}</span> : null}
                </article>
              ))}
            </div>
          )}
        </section>

        <section className="task-panel">
          <div className="task-panel-header">
            <div>
              <span className="task-panel-label">{projectLabel}</span>
              <h2>{text.pending}</h2>
            </div>
          </div>
          {candidates.length === 0 ? (
            <div className="inline-empty-body">{text.emptyPending}</div>
          ) : (
            <div className="memory-card-list">
              {candidates.map((candidate) => (
                <article key={candidate.candidate_id} className="memory-card">
                  <strong>{candidate.summary}</strong>
                  <p>{candidate.why_it_matters}</p>
                </article>
              ))}
            </div>
          )}
        </section>

        <section className="task-panel project-home-recent">
          <div className="task-panel-header">
            <div>
              <span className="task-panel-label">{projectLabel}</span>
              <h2>{text.recent}</h2>
            </div>
          </div>
          <div className="task-list">
            {conversations.slice(0, 8).map((conversation) => (
              <button
                key={conversation.id}
                type="button"
                className="task-list-item"
                onClick={() => onOpenConversation(conversation.id)}
              >
                <div>
                  <strong>{conversation.title}</strong>
                  <span>{conversation.updatedAt}</span>
                </div>
              </button>
            ))}
          </div>
        </section>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Wire selected project state in App**

In `src/App.tsx`, add the import:

```tsx
import ProjectHome from "./components/ProjectHome";
```

Add state near `selectedConversation`:

```tsx
const [selectedProjectRoot, setSelectedProjectRoot] = useState<string | null>(null);
```

Update `activeRepoRoot`:

```tsx
const activeRepoRoot = selectedConversation?.project_dir ?? selectedProjectRoot;
```

Add helper near `projectGroups`:

```tsx
const selectedProjectGroup = useMemo(
  () =>
    selectedProjectRoot
      ? projectGroups.find((group) => projectPathKey(group.fullPath) === projectPathKey(selectedProjectRoot)) ?? null
      : null,
  [projectGroups, selectedProjectRoot],
);

const selectedProjectSeedConversations = useMemo(
  () =>
    selectedProjectGroup
      ? selectedProjectGroup.conversations.map((conversation) => ({
          id: conversation.id,
          title: normalizeConversationTitle(conversation.summary) || conversation.id,
          updatedAt: conversation.updated_at,
          storagePath: null,
        }))
      : [],
  [selectedProjectGroup],
);
```

Add open-project handler:

```tsx
const handleOpenProjectHome = (projectRoot: string) => {
  setSelectedConversation(null);
  setSelectedProjectRoot(projectRoot);
  setCopyState({ target: null, status: "idle" });
};
```

Update `loadConversationDetail` success path:

```tsx
setSelectedProjectRoot(null);
setSelectedConversation(normalizeConversationProject(result));
```

Update the project group header so the chevron toggles and the project copy opens the project home:

```tsx
<button
  type="button"
  className="project-group-chevron-button"
  aria-label={isExpanded ? shell.collapseProjects : shell.restoreProjects}
  onClick={() =>
    setExpandedProjects((current) => ({
      ...current,
      [group.id]: !isExpanded,
    }))
  }
>
  <span className={`project-group-chevron ${isExpanded ? "expanded" : ""}`}>
    <WindowButtonIcon type="chevron" />
  </span>
</button>
<button
  type="button"
  className="project-group-open"
  onClick={() => handleOpenProjectHome(group.fullPath)}
>
  <span className="project-group-title">{group.label}</span>
  <span className="project-group-path" title={group.fullPath}>
    {group.fullPath}
  </span>
</button>
```

- [ ] **Step 5: Render project home from workspace**

At the start of `renderWorkspace`, before the `!selectedConversation` branch, add:

```tsx
if (selectedProjectRoot && selectedProjectGroup) {
  return (
    <ProjectHome
      locale={locale}
      projectLabel={selectedProjectGroup.label}
      projectPath={selectedProjectGroup.fullPath}
      conversations={selectedProjectSeedConversations}
      memories={repoMemories}
      candidates={memoryCandidates}
      loading={memoryLoading}
      onOpenConversation={(conversationId) => void loadConversationDetail(conversationId)}
      onSeedProject={() => undefined}
      onOpenReview={() => undefined}
    />
  );
}
```

- [ ] **Step 6: Add project home styles**

Append to `src/styles.css`:

```css
.project-home-page {
  min-height: 100%;
  padding: 24px 28px 28px;
  display: grid;
  gap: 18px;
}

.project-home-hero {
  display: grid;
  gap: 10px;
  padding: 22px;
  border-radius: var(--radius-md);
  background: var(--bg-soft);
  border: 1px solid rgba(22, 32, 24, 0.06);
}

.project-home-hero h1 {
  margin: 0;
  font-size: 28px;
  line-height: 1.1;
}

.project-home-hero p {
  margin: 0;
  color: var(--text-secondary);
  max-width: 720px;
}

.project-home-path {
  font-family: var(--font-mono);
  color: var(--text-secondary);
  word-break: break-word;
}

.project-home-stats,
.project-home-actions {
  display: flex;
  gap: 10px;
  flex-wrap: wrap;
}

.project-home-stats span {
  border-radius: 999px;
  padding: 6px 10px;
  background: rgba(255, 255, 255, 0.76);
  color: var(--text-secondary);
  font-size: 12px;
  font-weight: 700;
}

.project-home-grid {
  display: grid;
  grid-template-columns: repeat(2, minmax(0, 1fr));
  gap: 14px;
}

.project-home-recent {
  grid-column: 1 / -1;
}

.project-group-title-wrap {
  display: grid;
  grid-template-columns: auto minmax(0, 1fr);
  align-items: start;
  gap: 8px;
}

.project-group-chevron-button,
.project-group-open {
  appearance: none;
  border: 0;
  background: transparent;
  padding: 0;
  text-align: left;
  cursor: pointer;
}

.project-group-open {
  min-width: 0;
  display: grid;
  gap: 1px;
}
```

- [ ] **Step 7: Run project-home test**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/MemoryWorkspace.test.tsx
```

Expected: PASS for project-home test.

- [ ] **Step 8: Commit**

Run:

```powershell
git add src/components/ProjectHome.tsx src/App.tsx src/styles.css src/__tests__/MemoryWorkspace.test.tsx
git commit -m "feat: add project memory home"
```

## Task 3: Full-Width Conversation Page And Current Conversation Seeding Entry

**Files:**
- Modify: `src/App.tsx`
- Modify: `src/styles.css`
- Test: `src/__tests__/App.test.tsx`
- Test: `src/__tests__/MemoryWorkspace.test.tsx`

- [ ] **Step 1: Update tests for full-width conversation**

In `src/__tests__/App.test.tsx`, replace the test named `"shows conversation details, migration, copy actions, and memory in one workspace"` with:

```tsx
it("shows conversation details full-width with memory seeding actions", async () => {
  localStorage.setItem(
    "chatmem.settings",
    JSON.stringify({ locale: "en", autoCheckUpdates: false }),
  );

  renderApp();

  fireEvent.click((await screen.findAllByText("Debug session"))[0]);

  await waitFor(() => {
    expect(screen.getByRole("heading", { name: "Debug session" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Copy location" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Copy resume command" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Migrate" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Extract memory" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Pending memory 0" })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Project Memory" })).toBeNull();
    expect(screen.queryByText("Use ChatMem for cross-agent continuation")).toBeNull();
  });

  expect(document.querySelector(".conversation-content-grid")).toBeNull();
});
```

In `src/__tests__/MemoryWorkspace.test.tsx`, replace `"reviews a pending memory candidate from the side panel"` with:

```tsx
it("keeps pending memory out of the conversation page and exposes a review button", async () => {
  renderApp();

  fireEvent.click((await screen.findAllByText("Memory workflow"))[0]);

  await waitFor(() => {
    expect(screen.getByRole("heading", { name: "Memory workflow" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Pending memory 1" })).toBeTruthy();
    expect(screen.queryByText("Review pending memory")).toBeNull();
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/App.test.tsx src/__tests__/MemoryWorkspace.test.tsx
```

Expected: FAIL because conversation view still renders memory side panel and no seeding buttons exist.

- [ ] **Step 3: Add shell copy labels**

In `ShellCopy`, add:

```ts
extractMemory: string;
pendingMemory: string;
```

In English copy:

```ts
extractMemory: "Extract memory",
pendingMemory: "Pending memory",
```

In Chinese copy:

```ts
extractMemory: "提炼成记忆",
pendingMemory: "待确认记忆",
```

- [ ] **Step 4: Add seeding buttons to conversation toolbar**

In `renderWorkspace`, inside `.conversation-toolbar-actions`, after Migrate, add:

```tsx
<button
  type="button"
  className="btn btn-secondary"
  onClick={() => undefined}
>
  {shell.extractMemory}
</button>
<button
  type="button"
  className={`btn ${memoryCandidates.length > 0 ? "btn-primary" : "btn-secondary"}`}
  onClick={() => undefined}
>
  {shell.pendingMemory} {memoryCandidates.length}
</button>
```

- [ ] **Step 5: Remove the persistent memory side panel from conversation render**

Replace:

```tsx
<div className="conversation-content-grid">
  <ConversationDetail conversation={selectedConversation} />
  {renderMemoryPanel()}
</div>
```

with:

```tsx
<div className="conversation-content">
  <ConversationDetail conversation={selectedConversation} />
</div>
```

Leave `renderMemoryPanel` unused during this task; Task 5 removes it after the review modal takes over candidate display.

- [ ] **Step 6: Add full-width styles**

In `src/styles.css`, replace the active conversation grid behavior with:

```css
.conversation-content {
  min-height: 0;
}

.conversation-content > .conversation-detail {
  min-width: 0;
  padding: 22px 28px 28px;
}

.conversation-content-grid {
  display: contents;
}
```

Task 5 removes `conversation-content-grid` after the review modal takes over candidate display.

- [ ] **Step 7: Run full-width tests**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/App.test.tsx src/__tests__/MemoryWorkspace.test.tsx
```

Expected: PASS for full-width and pending button assertions.

- [ ] **Step 8: Commit**

Run:

```powershell
git add src/App.tsx src/styles.css src/__tests__/App.test.tsx src/__tests__/MemoryWorkspace.test.tsx
git commit -m "feat: keep conversation memory controls off canvas"
```

## Task 4: Memory Seed Modal For Current Conversation And Project Scan

**Files:**
- Create: `src/components/MemorySeedModal.tsx`
- Modify: `src/App.tsx`
- Modify: `src/styles.css`
- Test: `src/__tests__/MemoryWorkspace.test.tsx`

- [ ] **Step 1: Add tests for prompt modal**

Append to `src/__tests__/MemoryWorkspace.test.tsx`:

```tsx
it("copies an agent-assisted prompt for the current conversation", async () => {
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.assign(navigator, { clipboard: { writeText } });

  renderApp();

  fireEvent.click((await screen.findAllByText("Memory workflow"))[0]);
  fireEvent.click(await screen.findByRole("button", { name: "Extract memory" }));

  expect(await screen.findByRole("heading", { name: "Extract memory from this conversation" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Copy prompt" }));

  await waitFor(() => {
    expect(writeText).toHaveBeenCalledWith(expect.stringContaining("conversation id: claude:conv-001"));
    expect(writeText).toHaveBeenCalledWith(expect.stringContaining("Use ChatMem MCP `create_memory_candidate`"));
  });
});

it("opens a project scan prompt from project home", async () => {
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.assign(navigator, { clipboard: { writeText } });

  renderApp();

  fireEvent.click(await screen.findByRole("button", { name: /agentswap-gui/i }));
  fireEvent.click(await screen.findByRole("button", { name: "Scan old conversations" }));

  expect(await screen.findByRole("heading", { name: "Scan old conversations" })).toBeTruthy();
  expect(screen.getByText("Recent 20 conversations")).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "Copy prompt" }));

  await waitFor(() => {
    expect(writeText).toHaveBeenCalledWith(expect.stringContaining("scan range: Recent 20 conversations"));
    expect(writeText).toHaveBeenCalledWith(expect.stringContaining("claude:conv-001"));
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/MemoryWorkspace.test.tsx
```

Expected: FAIL because `MemorySeedModal` does not exist and toolbar/project buttons do not open a prompt.

- [ ] **Step 3: Create MemorySeedModal component**

Create `src/components/MemorySeedModal.tsx`:

```tsx
import type { MemorySeedModalState } from "../memory-seeding/types";

type MemorySeedModalProps = {
  state: MemorySeedModalState;
  copied: boolean;
  onCopy: (prompt: string) => void;
  onClose: () => void;
};

export default function MemorySeedModal({
  state,
  copied,
  onCopy,
  onClose,
}: MemorySeedModalProps) {
  return (
    <div className="modal-backdrop">
      <div className="modal memory-seed-modal" role="dialog" aria-modal="true">
        <div className="modal-header">
          <h3>{state.title}</h3>
          <button type="button" className="icon-button" aria-label="Close" onClick={onClose}>
            ×
          </button>
        </div>
        <p className="modal-helper-text">{state.subtitle}</p>
        {state.mode === "project-scan" ? (
          <div className="seed-scope-summary">
            Recent 20 conversations · {state.conversationCount} selected
          </div>
        ) : null}
        <pre className="seed-prompt-preview">{state.prompt}</pre>
        <div className="modal-actions">
          <button type="button" className="btn btn-primary" onClick={() => onCopy(state.prompt)}>
            {copied ? "Prompt copied" : "Copy prompt"}
          </button>
          <button type="button" className="btn btn-secondary" onClick={onClose}>
            Close
          </button>
        </div>
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Wire seed modal state and builders**

In `src/App.tsx`, add imports:

```tsx
import MemorySeedModal from "./components/MemorySeedModal";
import {
  buildCurrentConversationMemoryPrompt,
  buildProjectScanMemoryPrompt,
} from "./memory-seeding/prompt";
import type { MemorySeedModalState } from "./memory-seeding/types";
```

Add state:

```tsx
const [memorySeedModal, setMemorySeedModal] = useState<MemorySeedModalState | null>(null);
const [seedPromptCopied, setSeedPromptCopied] = useState(false);
```

Add helper functions:

```tsx
const openCurrentConversationSeedModal = () => {
  if (!selectedConversation) {
    return;
  }

  const title = normalizeConversationTitle(selectedConversation.summary) || selectedConversation.id;
  setSeedPromptCopied(false);
  setMemorySeedModal({
    mode: "current-conversation",
    title: locale === "en" ? "Extract memory from this conversation" : "从这段对话提炼记忆",
    subtitle:
      locale === "en"
        ? "Copy this prompt into your current agent. It will create pending memory candidates through ChatMem MCP."
        : "把这段提示交给当前 agent。它会通过 ChatMem MCP 创建待确认记忆。",
    prompt: buildCurrentConversationMemoryPrompt({
      repoRoot: selectedConversation.project_dir,
      sourceAgent: selectedAgent,
      conversationId: selectedConversation.id,
      conversationTitle: title,
      storagePath: selectedConversation.storage_path,
      resumeCommand: selectedConversation.resume_command,
    }),
  });
};

const openProjectScanSeedModal = () => {
  if (!selectedProjectGroup) {
    return;
  }

  const conversations = selectedProjectGroup.conversations.slice(0, 20).map((conversation) => ({
    id: conversation.id,
    title: normalizeConversationTitle(conversation.summary) || conversation.id,
    updatedAt: conversation.updated_at,
    storagePath: null,
  }));

  setSeedPromptCopied(false);
  setMemorySeedModal({
    mode: "project-scan",
    title: locale === "en" ? "Scan old conversations" : "扫描旧对话",
    subtitle:
      locale === "en"
        ? "Copy this prompt into your current agent to generate pending candidates from prior project conversations."
        : "把这段提示交给当前 agent，从旧对话生成待确认记忆。",
    conversationCount: conversations.length,
    prompt: buildProjectScanMemoryPrompt({
      repoRoot: selectedProjectGroup.fullPath,
      sourceAgent: selectedAgent,
      rangeLabel: "Recent 20 conversations",
      conversations,
    }),
  });
};

const handleCopySeedPrompt = async (prompt: string) => {
  await navigator.clipboard.writeText(prompt);
  setSeedPromptCopied(true);
};
```

Wire callbacks:

```tsx
onSeedProject={openProjectScanSeedModal}
```

and:

```tsx
onClick={openCurrentConversationSeedModal}
```

Render modal near the other modals:

```tsx
{memorySeedModal ? (
  <MemorySeedModal
    state={memorySeedModal}
    copied={seedPromptCopied}
    onCopy={(prompt) => void handleCopySeedPrompt(prompt)}
    onClose={() => setMemorySeedModal(null)}
  />
) : null}
```

- [ ] **Step 5: Add seed modal styles**

Append to `src/styles.css`:

```css
.memory-seed-modal {
  width: min(860px, calc(100vw - 48px));
  max-height: min(760px, calc(100vh - 64px));
}

.seed-prompt-preview {
  max-height: 420px;
  overflow: auto;
  white-space: pre-wrap;
  word-break: break-word;
  margin: 0;
  padding: 14px;
  border-radius: var(--radius-md);
  background: rgba(22, 32, 24, 0.06);
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: 12px;
  line-height: 1.55;
}

.seed-scope-summary {
  padding: 10px 12px;
  border-radius: var(--radius-md);
  background: var(--accent-soft);
  color: var(--accent-strong);
  font-size: 12px;
  font-weight: 700;
}
```

- [ ] **Step 6: Run modal tests**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/MemoryWorkspace.test.tsx src/__tests__/memorySeedingPrompts.test.ts
```

Expected: PASS.

- [ ] **Step 7: Commit**

Run:

```powershell
git add src/components/MemorySeedModal.tsx src/App.tsx src/styles.css src/__tests__/MemoryWorkspace.test.tsx src/memory-seeding
git commit -m "feat: add agent-assisted memory seeding prompts"
```

## Task 5: Memory Review Modal

**Files:**
- Create: `src/components/MemoryReviewModal.tsx`
- Modify: `src/App.tsx`
- Modify: `src/styles.css`
- Test: `src/__tests__/MemoryWorkspace.test.tsx`

- [ ] **Step 1: Add review modal tests**

Append to `src/__tests__/MemoryWorkspace.test.tsx`:

```tsx
it("reviews pending memory candidates from a modal", async () => {
  renderApp();

  fireEvent.click((await screen.findAllByText("Memory workflow"))[0]);
  fireEvent.click(await screen.findByRole("button", { name: "Pending memory 1" }));

  expect(await screen.findByRole("heading", { name: "Review pending memory" })).toBeTruthy();
  expect(screen.getByText("Review pending memory")).toBeTruthy();

  fireEvent.click(screen.getByRole("button", { name: "Keep as project memory" }));

  await waitFor(() => {
    expect(mockInvoke).toHaveBeenCalledWith("review_memory_candidate", {
      candidateId: "cand-001",
      action: "approve",
      editedTitle: "Review pending memory",
      editedUsageHint: "Human review is required",
    });
  });
});

it("opens the same review modal from project home", async () => {
  renderApp();

  fireEvent.click(await screen.findByRole("button", { name: /agentswap-gui/i }));
  fireEvent.click(await screen.findByRole("button", { name: "Review pending memory" }));

  expect(await screen.findByRole("heading", { name: "Review pending memory" })).toBeTruthy();
  expect(screen.getByText("Do not auto-approve candidate writes")).toBeTruthy();
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/MemoryWorkspace.test.tsx
```

Expected: FAIL because `MemoryReviewModal` does not exist and pending buttons do not open it.

- [ ] **Step 3: Create MemoryReviewModal component**

Create `src/components/MemoryReviewModal.tsx`:

```tsx
import type { MemoryCandidate } from "../chatmem-memory/types";

type MemoryReviewModalProps = {
  candidates: MemoryCandidate[];
  loading: boolean;
  onApprove: (candidate: MemoryCandidate) => void;
  onSnooze: (candidateId: string) => void;
  onReject: (candidateId: string) => void;
  onClose: () => void;
};

export default function MemoryReviewModal({
  candidates,
  loading,
  onApprove,
  onSnooze,
  onReject,
  onClose,
}: MemoryReviewModalProps) {
  return (
    <div className="modal-backdrop">
      <div className="modal memory-review-modal" role="dialog" aria-modal="true">
        <div className="modal-header">
          <h3>Review pending memory</h3>
          <button type="button" className="icon-button" aria-label="Close" onClick={onClose}>
            ×
          </button>
        </div>
        <p className="modal-helper-text">
          These suggestions are not project memory until you approve them.
        </p>
        {loading ? (
          <div className="loading-inline"><div className="spinner"></div></div>
        ) : candidates.length === 0 ? (
          <div className="inline-empty-state">
            <div className="inline-empty-title">No pending memory</div>
            <div className="inline-empty-body">New candidates will appear here after extraction.</div>
          </div>
        ) : (
          <div className="memory-review-list">
            {candidates.map((candidate) => (
              <article key={candidate.candidate_id} className="memory-review-card">
                <div>
                  <strong>{candidate.summary}</strong>
                  <p>{candidate.value}</p>
                  <span>{candidate.why_it_matters}</span>
                </div>
                {candidate.merge_suggestion ? (
                  <div className="memory-review-note">
                    Potential merge with <strong>{candidate.merge_suggestion.memory_title}</strong>.{" "}
                    {candidate.merge_suggestion.reason}
                  </div>
                ) : null}
                {candidate.evidence_refs.length > 0 ? (
                  <div className="memory-evidence-list">
                    {candidate.evidence_refs.slice(0, 2).map((evidence, index) => (
                      <div key={`${candidate.candidate_id}-${index}`} className="memory-evidence-item">
                        {evidence.excerpt}
                      </div>
                    ))}
                  </div>
                ) : null}
                <div className="modal-actions">
                  <button type="button" className="btn btn-primary" onClick={() => onApprove(candidate)}>
                    Keep as project memory
                  </button>
                  <button type="button" className="btn btn-secondary" onClick={() => onSnooze(candidate.candidate_id)}>
                    Review later
                  </button>
                  <button type="button" className="btn btn-secondary" onClick={() => onReject(candidate.candidate_id)}>
                    Do not keep
                  </button>
                </div>
              </article>
            ))}
          </div>
        )}
      </div>
    </div>
  );
}
```

- [ ] **Step 4: Wire modal state**

In `src/App.tsx`, import:

```tsx
import MemoryReviewModal from "./components/MemoryReviewModal";
```

Add state:

```tsx
const [showMemoryReview, setShowMemoryReview] = useState(false);
```

Wire pending buttons:

```tsx
onClick={() => setShowMemoryReview(true)}
```

Wire `ProjectHome`:

```tsx
onOpenReview={() => setShowMemoryReview(true)}
```

Render modal:

```tsx
{showMemoryReview ? (
  <MemoryReviewModal
    candidates={memoryCandidates}
    loading={memoryLoading}
    onApprove={handleApproveCandidate}
    onSnooze={handleSnoozeCandidate}
    onReject={handleRejectCandidate}
    onClose={() => setShowMemoryReview(false)}
  />
) : null}
```

After `handleApproveCandidate`, `handleRejectCandidate`, and `handleSnoozeCandidate` refresh the candidates, keep the modal open. The empty state confirms the queue is clear.

- [ ] **Step 5: Remove unused renderMemoryPanel**

Delete the entire `renderMemoryPanel` function from `src/App.tsx`.

Delete these CSS selectors when no longer referenced:

```css
.memory-side-panel { ... }
.memory-panel-section { ... }
```

Keep `.memory-card`, `.memory-card-list`, `.memory-review-note`, and `.memory-evidence-list` because the project home and review modal use them.

- [ ] **Step 6: Add review modal styles**

Append to `src/styles.css`:

```css
.memory-review-modal {
  width: min(920px, calc(100vw - 48px));
  max-height: min(760px, calc(100vh - 64px));
}

.memory-review-list {
  display: grid;
  gap: 12px;
  overflow: auto;
  padding-right: 4px;
}

.memory-review-card {
  display: grid;
  gap: 10px;
  padding: 14px;
  border-radius: var(--radius-md);
  background: rgba(255, 255, 255, 0.82);
  border: 1px solid rgba(22, 32, 24, 0.06);
}

.memory-review-card p,
.memory-review-card span {
  margin: 0;
  color: var(--text-secondary);
  line-height: 1.6;
}
```

- [ ] **Step 7: Run review tests**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/MemoryWorkspace.test.tsx src/__tests__/App.test.tsx
```

Expected: PASS.

- [ ] **Step 8: Commit**

Run:

```powershell
git add src/components/MemoryReviewModal.tsx src/App.tsx src/styles.css src/__tests__/MemoryWorkspace.test.tsx src/__tests__/App.test.tsx
git commit -m "feat: review memory candidates in modal"
```

## Task 6: Project Batch Scope Controls

**Files:**
- Modify: `src/components/MemorySeedModal.tsx`
- Modify: `src/App.tsx`
- Modify: `src/memory-seeding/types.ts`
- Modify: `src/memory-seeding/prompt.ts`
- Test: `src/__tests__/MemoryWorkspace.test.tsx`
- Test: `src/__tests__/memorySeedingPrompts.test.ts`

- [ ] **Step 1: Add tests for recent-20 and all-project scan choices**

Add to `src/__tests__/MemoryWorkspace.test.tsx`:

```tsx
it("lets project scan prompts switch from recent conversations to all project conversations", async () => {
  const writeText = vi.fn().mockResolvedValue(undefined);
  Object.assign(navigator, { clipboard: { writeText } });

  renderApp();

  fireEvent.click(await screen.findByRole("button", { name: /agentswap-gui/i }));
  fireEvent.click(await screen.findByRole("button", { name: "Scan old conversations" }));

  expect(await screen.findByText("Recent 20 conversations")).toBeTruthy();
  fireEvent.click(screen.getByRole("radio", { name: "All project conversations" }));
  fireEvent.click(screen.getByRole("button", { name: "Copy prompt" }));

  await waitFor(() => {
    expect(writeText).toHaveBeenCalledWith(expect.stringContaining("scan range: All project conversations"));
  });
});
```

- [ ] **Step 2: Run tests to verify they fail**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/MemoryWorkspace.test.tsx
```

Expected: FAIL because `MemorySeedModal` has no range controls.

- [ ] **Step 3: Extend modal state**

In `src/memory-seeding/types.ts`, add:

```ts
export type ProjectScanRange = "recent-20" | "all-project";
```

Update the project-scan modal branch:

```ts
  | {
      mode: "project-scan";
      prompt: string;
      title: string;
      subtitle: string;
      conversationCount: number;
      range: ProjectScanRange;
    };
```

- [ ] **Step 4: Add range controls to MemorySeedModal**

Update `MemorySeedModalProps`:

```ts
onProjectRangeChange?: (range: "recent-20" | "all-project") => void;
```

Inside the `state.mode === "project-scan"` block, render:

```tsx
<fieldset className="seed-range-fieldset">
  <legend>Scan range</legend>
  <label className="seed-range-option">
    <input
      type="radio"
      name="seed-range"
      checked={state.range === "recent-20"}
      onChange={() => onProjectRangeChange?.("recent-20")}
    />
    <span>Recent 20 conversations</span>
  </label>
  <label className="seed-range-option">
    <input
      type="radio"
      name="seed-range"
      checked={state.range === "all-project"}
      onChange={() => onProjectRangeChange?.("all-project")}
    />
    <span>All project conversations</span>
  </label>
</fieldset>
```

- [ ] **Step 5: Rebuild project prompt when range changes**

In `src/App.tsx`, add helper:

```tsx
const buildProjectSeedModalState = (
  group: ProjectGroup,
  range: "recent-20" | "all-project",
): MemorySeedModalState => {
  const conversationsForRange =
    range === "recent-20" ? group.conversations.slice(0, 20) : group.conversations;
  const rangeLabel = range === "recent-20" ? "Recent 20 conversations" : "All project conversations";
  const conversations = conversationsForRange.map((conversation) => ({
    id: conversation.id,
    title: normalizeConversationTitle(conversation.summary) || conversation.id,
    updatedAt: conversation.updated_at,
    storagePath: null,
  }));

  return {
    mode: "project-scan",
    title: locale === "en" ? "Scan old conversations" : "扫描旧对话",
    subtitle:
      locale === "en"
        ? "Copy this prompt into your current agent to generate pending candidates from prior project conversations."
        : "把这段提示交给当前 agent，从旧对话生成待确认记忆。",
    conversationCount: conversations.length,
    range,
    prompt: buildProjectScanMemoryPrompt({
      repoRoot: group.fullPath,
      sourceAgent: selectedAgent,
      rangeLabel,
      conversations,
    }),
  };
};
```

Use it from `openProjectScanSeedModal`:

```tsx
setMemorySeedModal(buildProjectSeedModalState(selectedProjectGroup, "recent-20"));
```

Add range handler:

```tsx
const handleProjectSeedRangeChange = (range: "recent-20" | "all-project") => {
  if (!selectedProjectGroup) {
    return;
  }
  setSeedPromptCopied(false);
  setMemorySeedModal(buildProjectSeedModalState(selectedProjectGroup, range));
};
```

Pass to modal:

```tsx
onProjectRangeChange={handleProjectSeedRangeChange}
```

- [ ] **Step 6: Add range styles**

Append to `src/styles.css`:

```css
.seed-range-fieldset {
  border: 1px solid rgba(22, 32, 24, 0.08);
  border-radius: var(--radius-md);
  padding: 12px;
  display: grid;
  gap: 8px;
}

.seed-range-fieldset legend {
  color: var(--text-muted);
  font-size: 12px;
  font-weight: 700;
}

.seed-range-option {
  display: flex;
  align-items: center;
  gap: 8px;
  color: var(--text-primary);
  font-size: 13px;
}
```

- [ ] **Step 7: Run range tests**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/MemoryWorkspace.test.tsx src/__tests__/memorySeedingPrompts.test.ts
```

Expected: PASS.

- [ ] **Step 8: Commit**

Run:

```powershell
git add src/components/MemorySeedModal.tsx src/App.tsx src/styles.css src/memory-seeding src/__tests__/MemoryWorkspace.test.tsx src/__tests__/memorySeedingPrompts.test.ts
git commit -m "feat: support project memory scan ranges"
```

## Task 7: Full Verification And UI Polish

**Files:**
- Modify: `src/styles.css`
- Modify: tests touched by earlier tasks

- [ ] **Step 1: Run targeted frontend tests**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/App.test.tsx src/__tests__/MemoryWorkspace.test.tsx src/__tests__/MemoryFreshness.test.tsx src/__tests__/memorySeedingPrompts.test.ts
```

Expected: PASS.

- [ ] **Step 2: Run full frontend suite**

Run:

```powershell
npm.cmd run test:run
```

Expected: PASS.

- [ ] **Step 3: Run TypeScript build**

Run:

```powershell
npm.cmd run build
```

Expected: PASS with Vite build output.

- [ ] **Step 4: Run Rust tests**

Run:

```powershell
cargo test --manifest-path .\src-tauri\Cargo.toml
```

Expected: PASS.

- [ ] **Step 5: Start dev server for browser verification**

Run:

```powershell
npm.cmd run dev -- --host 127.0.0.1
```

Expected: Vite prints a local URL.

- [ ] **Step 6: Browser verification checklist**

Open the Vite URL and verify:

- The empty state still says choose a conversation.
- Clicking a project opens Project Context.
- Project Context shows approved memory, pending memory, and Scan old conversations.
- Clicking a conversation opens full-width conversation detail.
- Conversation detail does not show a right memory side panel.
- Extract memory opens a prompt modal.
- Pending memory opens the review modal.
- Project scan modal can switch Recent 20 and All project conversations.
- Text does not overlap at desktop width and narrow width.

- [ ] **Step 7: Commit final polish**

If any polish changes were needed, run:

```powershell
git add src/styles.css src/App.tsx src/components src/__tests__
git commit -m "polish: refine memory seeding interface"
```

If no polish changes were needed, do not create an empty commit.

## Spec Coverage Self-Review

- Project memory belongs to project home: Task 2.
- Conversation page remains full-width: Task 3.
- Current conversation to candidate prompt: Tasks 1 and 4.
- Project old-conversation scan prompt: Tasks 1, 4, and 6.
- Review modal for candidates: Task 5.
- No desktop model/API key requirement: Tasks 1 and 4 keep agent-assisted prompt-only flow.
- Batch scan default recent 20 plus all-project explicit option: Task 6.
- Existing approved memory and candidates remain backed by existing Tauri list/review calls: Tasks 2 and 5.

## Execution Notes

- Do not add direct AI API calls in this implementation.
- Do not auto-approve any candidate.
- Do not keep `Project Memory` or `Memory Candidates` as a persistent side panel on conversation pages.
- Keep generated prompt text explicit that agent must create pending candidates through ChatMem MCP.
