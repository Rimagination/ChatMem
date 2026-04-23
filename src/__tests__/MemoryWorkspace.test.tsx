import { fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "../App";
import { I18nProvider } from "../i18n/I18nProvider";

const mockInvoke = vi.fn();

vi.mock("@tauri-apps/api/tauri", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/updater", () => ({
  checkUpdate: vi.fn().mockResolvedValue({ shouldUpdate: false }),
  installUpdate: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@tauri-apps/api/process", () => ({
  relaunch: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@tauri-apps/api/window", () => ({
  appWindow: {
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    close: vi.fn(),
  },
}));

function renderApp() {
  return render(
    <I18nProvider>
      <App />
    </I18nProvider>,
  );
}

function createDeferred<T>() {
  let resolve!: (value: T | PromiseLike<T>) => void;
  let reject!: (reason?: unknown) => void;

  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });

  return { promise, resolve, reject };
}

describe("Memory workspace", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    localStorage.clear();
    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    mockInvoke.mockImplementation(async (command: string, payload?: Record<string, unknown>) => {
      if (command === "list_conversations") {
        return [
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
        ];
      }

      if (command === "read_conversation") {
        return {
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
        };
      }

      if (command === "list_repo_memories") {
        return [
          {
            memory_id: "mem-001",
            kind: "command",
            title: "Primary verification",
            value: "npm run test:run",
            usage_hint: "Use before handoff",
            status: "active",
            last_verified_at: "2026-04-19T09:00:00Z",
            selected_because: null,
            evidence_refs: [],
          },
        ];
      }

      if (command === "list_memory_candidates") {
        return [
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
            merge_suggestion: {
              candidate_id: "cand-001",
              memory_id: "mem-001",
              memory_title: "Primary verification",
              reason: "This candidate overlaps an approved memory and likely needs a merge-aware review.",
              proposed_title: "Primary verification",
              proposed_value: "npm run test:run\n\nUpdate: Do not auto-approve candidate writes",
              proposed_usage_hint: "Use before handoff\n\nUpdate: Human review is required",
              risk_note: "Review before approval: this proposal rewrites an existing approved memory.",
              proposed_by: "codex",
            },
          },
        ];
      }

      if (command === "list_wiki_pages" || command === "rebuild_repo_wiki") {
        return [
          {
            page_id: "wiki:commands",
            repo_root: "D:/VSP/agentswap-gui",
            slug: "commands",
            title: "Commands",
            body: "# Commands\n\n- npm run test:run",
            status: "fresh",
            source_memory_ids: ["mem-001"],
            source_episode_ids: [],
            last_built_at: "2026-04-19T09:00:00Z",
            last_verified_at: "2026-04-19T09:00:00Z",
            updated_at: "2026-04-19T09:00:00Z",
          },
        ];
      }

      if (command === "get_repo_memory_health") {
        return {
          repo_root: "D:/VSP/agentswap-gui",
          canonical_repo_root: "D:/VSP/agentswap-gui",
          approved_memory_count: 1,
          pending_candidate_count: 1,
          search_document_count: 4,
          indexed_chunk_count: 8,
          inherited_repo_roots: [],
          conversation_counts_by_agent: [
            { source_agent: "claude", conversation_count: 1 },
          ],
          repo_aliases: [],
          warnings: [],
        };
      }

      if (command === "scan_repo_conversations") {
        return {
          repo_root: "D:/VSP/agentswap-gui",
          canonical_repo_root: "D:/VSP/agentswap-gui",
          scanned_conversation_count: 1,
          linked_conversation_count: 1,
          skipped_conversation_count: 0,
          source_agents: [{ source_agent: "claude", conversation_count: 1 }],
          warnings: [],
        };
      }

      if (command === "review_memory_candidate") {
        return null;
      }

      return [];
    });
  });

  it("keeps memory in a drawer with an inbox-style notification", async () => {
    renderApp();

    fireEvent.click((await screen.findAllByText("Memory workflow"))[0]);

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Memory workflow" })).toBeTruthy();
      expect(screen.getByRole("button", { name: "Memory 1" })).toBeTruthy();
    });

    expect(screen.queryByRole("complementary", { name: "Project Memory" })).toBeNull();
    expect(screen.queryByText("Primary verification")).toBeNull();
    expect(screen.queryByText("Review pending memory")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Memory 1" }));

    expect(await screen.findByRole("complementary", { name: "Project Memory" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Inbox 1" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Approved 1" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Wiki 1" })).toBeTruthy();
    expect(screen.getByText("Review pending memory")).toBeTruthy();

    fireEvent.click(screen.getByRole("tab", { name: "Approved 1" }));
    expect(screen.getByText("Primary verification")).toBeTruthy();

    fireEvent.click(screen.getByRole("tab", { name: "Wiki 1" }));
    expect(screen.getByText("Commands")).toBeTruthy();
  });

  it("reviews a pending memory candidate from the drawer", async () => {
    renderApp();

    fireEvent.click((await screen.findAllByText("Memory workflow"))[0]);
    fireEvent.click(await screen.findByRole("button", { name: "Memory 1" }));
    expect(await screen.findByText("Review pending memory")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Approve" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("review_memory_candidate", {
        candidateId: "cand-001",
        action: "approve",
        editedTitle: "Review pending memory",
        editedUsageHint: "Human review is required",
      });
    });
  });

  it("approves a merge proposal from the memory drawer", async () => {
    renderApp();

    fireEvent.click((await screen.findAllByText("Memory workflow"))[0]);
    fireEvent.click(await screen.findByRole("button", { name: "Memory 1" }));
    expect(await screen.findByText("Suggested rewrite")).toBeTruthy();
    expect(screen.getByText("Merge proposed by codex")).toBeTruthy();
    expect(screen.getByText(/npm run test:run/)).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Approve merge" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("review_memory_candidate", {
        candidateId: "cand-001",
        action: "approve_merge",
        mergeMemoryId: "mem-001",
        editedTitle: "Primary verification",
        editedValue: "npm run test:run\n\nUpdate: Do not auto-approve candidate writes",
        editedUsageHint: "Use before handoff\n\nUpdate: Human review is required",
      });
    });
  });

  it("shows local history status and rescans the active repo", async () => {
    renderApp();

    fireEvent.click((await screen.findAllByText("Memory workflow"))[0]);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_repo_memory_health", {
        repoRoot: "D:/VSP/agentswap-gui",
      });
    });

    expect(await screen.findByText("Local history")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Rescan local history" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("scan_repo_conversations", {
        repoRoot: "D:/VSP/agentswap-gui",
      });
    });
  });

  it("shows bootstrap scan status copy while auto-import is still running", async () => {
    const scanDeferred = createDeferred<{
      repo_root: string;
      canonical_repo_root: string;
      scanned_conversation_count: number;
      linked_conversation_count: number;
      skipped_conversation_count: number;
      source_agents: { source_agent: string; conversation_count: number }[];
      warnings: string[];
    }>();
    const baseImplementation = mockInvoke.getMockImplementation();
    if (!baseImplementation) {
      throw new Error("Missing base invoke mock implementation");
    }

    mockInvoke.mockImplementation((command: string, payload?: Record<string, unknown>) => {
      if (
        command === "get_repo_memory_health" &&
        payload?.repoRoot === "D:/VSP/agentswap-gui"
      ) {
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

    const scanningButton = screen.getByRole("button", { name: "Scanning..." });
    expect((scanningButton as HTMLButtonElement).disabled).toBe(true);

    scanDeferred.resolve({
      repo_root: "D:/VSP/agentswap-gui",
      canonical_repo_root: "D:/VSP/agentswap-gui",
      scanned_conversation_count: 1,
      linked_conversation_count: 1,
      skipped_conversation_count: 0,
      source_agents: [{ source_agent: "claude", conversation_count: 1 }],
      warnings: [],
    });
  });

  it("auto bootstraps local history when the repo has no indexed chunks", async () => {
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
        if (healthCallCount === 1) {
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

        return Promise.resolve({
          repo_root: "D:/VSP/agentswap-gui",
          canonical_repo_root: "D:/VSP/agentswap-gui",
          approved_memory_count: 1,
          pending_candidate_count: 1,
          search_document_count: 4,
          indexed_chunk_count: 8,
          inherited_repo_roots: [],
          conversation_counts_by_agent: [{ source_agent: "claude", conversation_count: 1 }],
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
      expect(screen.getByRole("button", { name: "Rescan local history" })).toBeTruthy();
    });

    const localHistoryPanel = screen.getByText("Local history").closest("section");
    expect(localHistoryPanel).toBeTruthy();

    await waitFor(() => {
      expect(within(localHistoryPanel!).getByText("8")).toBeTruthy();
    });

    expect(
      mockInvoke.mock.calls.filter(
        ([command, payload]) =>
          command === "scan_repo_conversations" &&
          payload?.repoRoot === "D:/VSP/agentswap-gui",
      ),
    ).toHaveLength(1);
    expect(
      mockInvoke.mock.calls.filter(
        ([command, payload]) =>
          command === "get_repo_memory_health" &&
          payload?.repoRoot === "D:/VSP/agentswap-gui",
      ),
    ).toHaveLength(2);
  });

  it("auto bootstrap transitions from scanning note to ready notice after scan success and refreshed health", async () => {
    let healthCallCount = 0;
    const scanDeferred = createDeferred<{
      repo_root: string;
      canonical_repo_root: string;
      scanned_conversation_count: number;
      linked_conversation_count: number;
      skipped_conversation_count: number;
      source_agents: { source_agent: string; conversation_count: number }[];
      warnings: string[];
    }>();
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
        if (healthCallCount === 1) {
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

        return Promise.resolve({
          repo_root: "D:/VSP/agentswap-gui",
          canonical_repo_root: "D:/VSP/agentswap-gui",
          approved_memory_count: 1,
          pending_candidate_count: 1,
          search_document_count: 4,
          indexed_chunk_count: 8,
          inherited_repo_roots: [],
          conversation_counts_by_agent: [{ source_agent: "claude", conversation_count: 1 }],
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

    expect(
      await screen.findByText(
        "Local history is ready for this project. You can now ask what was discussed before.",
      ),
    ).toBeTruthy();
    expect(
      screen.queryByText(
        "Importing local history for this project. Older conversations may not be fully searchable yet. When indexing finishes, you can ask what was discussed before.",
      ),
    ).toBeNull();
  });

  it("switching to another conversation clears the ready notice before async detail load resolves", async () => {
    let repoAHealthCallCount = 0;
    const repoBDetailDeferred = createDeferred<{
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
            summary: "Repo A first conversation",
            message_count: 3,
            file_count: 2,
          },
          {
            id: "conv-002",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/VSP/another-repo",
            created_at: "2026-04-19T09:30:00Z",
            updated_at: "2026-04-19T09:45:00Z",
            summary: "Repo B conversation",
            message_count: 4,
            file_count: 1,
          },
        ]);
      }

      if (command === "read_conversation" && payload?.id === "conv-001") {
        return Promise.resolve({
          id: "conv-001",
          source_agent: payload?.agent ?? "claude",
          project_dir: "D:/VSP/agentswap-gui",
          created_at: "2026-04-19T08:00:00Z",
          updated_at: "2026-04-19T09:00:00Z",
          summary: "Repo A first conversation",
          storage_path: "C:/Users/demo/.claude/projects/conv-001.jsonl",
          resume_command: "claude --resume conv-001",
          messages: [],
          file_changes: [],
        });
      }

      if (command === "read_conversation" && payload?.id === "conv-002") {
        return repoBDetailDeferred.promise;
      }

      if (
        command === "get_repo_memory_health" &&
        payload?.repoRoot === "D:/VSP/agentswap-gui"
      ) {
        repoAHealthCallCount += 1;
        if (repoAHealthCallCount === 1) {
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

        return Promise.resolve({
          repo_root: "D:/VSP/agentswap-gui",
          canonical_repo_root: "D:/VSP/agentswap-gui",
          approved_memory_count: 1,
          pending_candidate_count: 1,
          search_document_count: 4,
          indexed_chunk_count: 8,
          inherited_repo_roots: [],
          conversation_counts_by_agent: [{ source_agent: "claude", conversation_count: 1 }],
          repo_aliases: [],
          warnings: [],
        });
      }

      if (
        command === "get_repo_memory_health" &&
        payload?.repoRoot === "D:/VSP/another-repo"
      ) {
        return Promise.resolve({
          repo_root: "D:/VSP/another-repo",
          canonical_repo_root: "D:/VSP/another-repo",
          approved_memory_count: 1,
          pending_candidate_count: 0,
          search_document_count: 2,
          indexed_chunk_count: 6,
          inherited_repo_roots: [],
          conversation_counts_by_agent: [{ source_agent: "claude", conversation_count: 1 }],
          repo_aliases: [],
          warnings: [],
        });
      }

      return baseImplementation(command, payload);
    });

    renderApp();

    fireEvent.click((await screen.findAllByText("Repo A first conversation"))[0]);

    expect(
      await screen.findByText(
        "Local history is ready for this project. You can now ask what was discussed before.",
      ),
    ).toBeTruthy();

    fireEvent.click((await screen.findAllByText("Repo B conversation"))[0]);

    expect(
      screen.queryByText(
        "Local history is ready for this project. You can now ask what was discussed before.",
      ),
    ).toBeNull();

    repoBDetailDeferred.resolve({
      id: "conv-002",
      source_agent: "claude",
      project_dir: "D:/VSP/another-repo",
      created_at: "2026-04-19T09:30:00Z",
      updated_at: "2026-04-19T09:45:00Z",
      summary: "Repo B conversation",
      storage_path: "C:/Users/demo/.claude/projects/conv-002.jsonl",
      resume_command: "claude --resume conv-002",
      messages: [],
      file_changes: [],
    });

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Repo B conversation" })).toBeTruthy();
    });
  });

  it("manual rescan does not show the ready notice", async () => {
    const baseImplementation = mockInvoke.getMockImplementation();
    if (!baseImplementation) {
      throw new Error("Missing base invoke mock implementation");
    }

    mockInvoke.mockImplementation((command: string, payload?: Record<string, unknown>) => {
      if (
        command === "get_repo_memory_health" &&
        payload?.repoRoot === "D:/VSP/agentswap-gui"
      ) {
        return Promise.resolve({
          repo_root: "D:/VSP/agentswap-gui",
          canonical_repo_root: "D:/VSP/agentswap-gui",
          approved_memory_count: 1,
          pending_candidate_count: 1,
          search_document_count: 4,
          indexed_chunk_count: 8,
          inherited_repo_roots: [],
          conversation_counts_by_agent: [{ source_agent: "claude", conversation_count: 1 }],
          repo_aliases: [],
          warnings: [],
        });
      }

      return baseImplementation(command, payload);
    });

    renderApp();

    fireEvent.click((await screen.findAllByText("Memory workflow"))[0]);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Rescan local history" })).toBeTruthy();
    });

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

  it("auto bootstraps local history only once per repo across repo revisits in a session", async () => {
    let repoAHealthCallCount = 0;
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
            summary: "Repo A first conversation",
            message_count: 3,
            file_count: 2,
          },
          {
            id: "conv-002",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/VSP/another-repo",
            created_at: "2026-04-19T09:30:00Z",
            updated_at: "2026-04-19T09:45:00Z",
            summary: "Repo B conversation",
            message_count: 4,
            file_count: 1,
          },
          {
            id: "conv-003",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/VSP/agentswap-gui",
            created_at: "2026-04-19T10:00:00Z",
            updated_at: "2026-04-19T11:00:00Z",
            summary: "Repo A second conversation",
            message_count: 5,
            file_count: 4,
          },
        ]);
      }

      if (
        command === "read_conversation" &&
        (payload?.id === "conv-001" || payload?.id === "conv-002" || payload?.id === "conv-003")
      ) {
        return Promise.resolve({
          id: payload.id,
          source_agent: payload?.agent ?? "claude",
          project_dir:
            payload.id === "conv-002" ? "D:/VSP/another-repo" : "D:/VSP/agentswap-gui",
          created_at:
            payload.id === "conv-001"
              ? "2026-04-19T08:00:00Z"
              : payload.id === "conv-002"
                ? "2026-04-19T09:30:00Z"
                : "2026-04-19T10:00:00Z",
          updated_at:
            payload.id === "conv-001"
              ? "2026-04-19T09:00:00Z"
              : payload.id === "conv-002"
                ? "2026-04-19T09:45:00Z"
                : "2026-04-19T11:00:00Z",
          summary:
            payload.id === "conv-001"
              ? "Repo A first conversation"
              : payload.id === "conv-002"
                ? "Repo B conversation"
                : "Repo A second conversation",
          storage_path: `C:/Users/demo/.claude/projects/${payload.id}.jsonl`,
          resume_command: `claude --resume ${payload.id}`,
          messages: [],
          file_changes: [],
        });
      }

      if (
        command === "get_repo_memory_health" &&
        payload?.repoRoot === "D:/VSP/agentswap-gui"
      ) {
        repoAHealthCallCount += 1;
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

      if (
        command === "get_repo_memory_health" &&
        payload?.repoRoot === "D:/VSP/another-repo"
      ) {
        return Promise.resolve({
          repo_root: "D:/VSP/another-repo",
          canonical_repo_root: "D:/VSP/another-repo",
          approved_memory_count: 2,
          pending_candidate_count: 0,
          search_document_count: 6,
          indexed_chunk_count: 12,
          inherited_repo_roots: [],
          conversation_counts_by_agent: [{ source_agent: "claude", conversation_count: 1 }],
          repo_aliases: [],
          warnings: [],
        });
      }

      if (
        command === "scan_repo_conversations" &&
        payload?.repoRoot === "D:/VSP/another-repo"
      ) {
        return Promise.resolve({
          repo_root: "D:/VSP/another-repo",
          canonical_repo_root: "D:/VSP/another-repo",
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

    fireEvent.click((await screen.findAllByText("Repo A first conversation"))[0]);

    await waitFor(() => {
      expect(
        mockInvoke.mock.calls.filter(
          ([command, callPayload]) =>
            command === "scan_repo_conversations" &&
            callPayload?.repoRoot === "D:/VSP/agentswap-gui",
        ),
      ).toHaveLength(1);
    });

    const localHistoryLabel = await screen.findByText("Local history");
    const localHistoryPanel = localHistoryLabel.closest("section");
    expect(localHistoryPanel).toBeTruthy();

    await waitFor(() => {
      expect(
        mockInvoke.mock.calls.filter(
          ([command, callPayload]) =>
            command === "get_repo_memory_health" &&
            callPayload?.repoRoot === "D:/VSP/agentswap-gui",
        ),
      ).toHaveLength(2);
    });

    await waitFor(() => {
      expect(within(localHistoryPanel!).getAllByText("0").length).toBeGreaterThan(0);
    });

    expect(repoAHealthCallCount).toBe(2);

    fireEvent.click((await screen.findAllByText("Repo B conversation"))[0]);

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Repo B conversation" })).toBeTruthy();
    });

    expect(
      mockInvoke.mock.calls.filter(
        ([command, callPayload]) =>
          command === "scan_repo_conversations" &&
          callPayload?.repoRoot === "D:/VSP/another-repo",
      ),
    ).toHaveLength(0);

    fireEvent.click((await screen.findAllByText("Repo A second conversation"))[0]);

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Repo A second conversation" })).toBeTruthy();
    });

    await waitFor(() => {
      expect(
        mockInvoke.mock.calls.filter(
          ([command, callPayload]) =>
            command === "get_repo_memory_health" &&
            callPayload?.repoRoot === "D:/VSP/agentswap-gui",
        ),
      ).toHaveLength(3);
    });

    expect(repoAHealthCallCount).toBe(3);

    expect(
      mockInvoke.mock.calls.filter(
        ([command, callPayload]) =>
          command === "scan_repo_conversations" &&
          callPayload?.repoRoot === "D:/VSP/agentswap-gui",
      ),
    ).toHaveLength(1);
  });

  it("still loads memory drawer data when repo health load fails", async () => {
    const baseImplementation = mockInvoke.getMockImplementation();
    if (!baseImplementation) {
      throw new Error("Missing base invoke mock implementation");
    }

    mockInvoke.mockImplementation((command: string, payload?: Record<string, unknown>) => {
      if (command === "get_repo_memory_health") {
        return Promise.reject(new Error("health unavailable"));
      }
      return baseImplementation(command, payload);
    });

    renderApp();

    fireEvent.click((await screen.findAllByText("Memory workflow"))[0]);

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Memory 1" })).toBeTruthy();
    });

    fireEvent.click(screen.getByRole("button", { name: "Memory 1" }));
    expect(await screen.findByRole("complementary", { name: "Project Memory" })).toBeTruthy();
    expect(screen.getByText("Review pending memory")).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Approved 1" })).toBeTruthy();
    expect(mockInvoke).not.toHaveBeenCalledWith("scan_repo_conversations", {
      repoRoot: "D:/VSP/agentswap-gui",
    });
  });
});
