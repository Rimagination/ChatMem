import { act, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "../App";
import { I18nProvider } from "../i18n/I18nProvider";
import { truncateSidebarTitle, truncateWorkspaceTitle } from "../utils/titleUtils";

const mockInvoke = vi.fn();
const mockCheckUpdate = vi.fn();
const mockInstallUpdate = vi.fn();
const mockRelaunch = vi.fn();
const mockMinimize = vi.fn();
const mockToggleMaximize = vi.fn();
const mockClose = vi.fn();
const mockStartDragging = vi.fn();
const mockIsMaximized = vi.fn();
const mockIsFullscreen = vi.fn();
const mockOnResized = vi.fn();
const appVersionPattern = /ChatMem v\d+\.\d+\.\d+/;
const longConversationTitle =
  "Review the latest changes in D:\\VSP\\agentswap-gui\\.worktrees\\chatmem-control-plane-v2 and focus on concrete risks instead of generic advice.";

vi.mock("@tauri-apps/api/tauri", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/updater", () => ({
  checkUpdate: () => mockCheckUpdate(),
  installUpdate: () => mockInstallUpdate(),
}));

vi.mock("@tauri-apps/api/process", () => ({
  relaunch: () => mockRelaunch(),
}));

vi.mock("@tauri-apps/api/window", () => ({
  appWindow: {
    minimize: () => mockMinimize(),
    toggleMaximize: () => mockToggleMaximize(),
    close: () => mockClose(),
    startDragging: () => mockStartDragging(),
    isMaximized: () => mockIsMaximized(),
    isFullscreen: () => mockIsFullscreen(),
    onResized: (handler: unknown) => mockOnResized(handler),
  },
}));

function renderApp() {
  return render(
    <I18nProvider>
      <App />
    </I18nProvider>,
  );
}

function getMemoryButton(label = "Manage Rules") {
  return screen.getByRole("button", { name: label });
}

async function openLocalHistoryView() {
  const historyTab = await screen.findByRole("tab", { name: "Local history" });
  fireEvent.click(historyTab);
  return historyTab;
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

describe("App", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockCheckUpdate.mockReset();
    mockInstallUpdate.mockReset();
    mockRelaunch.mockReset();
    mockMinimize.mockReset();
    mockToggleMaximize.mockReset();
    mockClose.mockReset();
    mockStartDragging.mockReset();
    mockIsMaximized.mockReset();
    mockIsFullscreen.mockReset();
    mockOnResized.mockReset();
    localStorage.clear();
    vi.useRealTimers();
    vi.stubGlobal("alert", vi.fn());
    vi.stubGlobal("confirm", vi.fn(() => true));
    mockIsMaximized.mockResolvedValue(false);
    mockIsFullscreen.mockResolvedValue(false);
    mockOnResized.mockResolvedValue(vi.fn());

    mockInvoke.mockImplementation(async (command: string, payload?: Record<string, unknown>) => {
      if (command === "list_conversations") {
        if (payload?.agent === "codex") {
          return [
            {
              id: "migrated-001",
              source_agent: "codex",
              project_dir: "D:/VSP/demo",
              created_at: "2026-04-08T08:00:00Z",
              updated_at: "2026-04-08T09:30:00Z",
              summary: "Migrated session",
              message_count: 2,
              file_count: 1,
            },
          ];
        }

        return [
          {
            id: "conv-001",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/VSP/demo",
            created_at: "2026-04-08T08:00:00Z",
            updated_at: "2026-04-08T09:00:00Z",
            summary: "Debug session",
            message_count: 2,
            file_count: 1,
          },
          {
            id: "conv-002",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/PV/service",
            created_at: "2026-04-08T10:00:00Z",
            updated_at: "2026-04-08T11:00:00Z",
            summary: "Memory investigation",
            message_count: 4,
            file_count: 0,
          },
          {
            id: "conv-long",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/VSP/agentswap-gui/.worktrees/chatmem-control-plane-v2",
            created_at: "2026-04-08T12:00:00Z",
            updated_at: "2026-04-08T12:30:00Z",
            summary: longConversationTitle,
            message_count: 19,
            file_count: 0,
          },
        ];
      }

      if (command === "read_conversation") {
        if (payload?.id === "conv-long") {
          return {
            id: "conv-long",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/VSP/agentswap-gui/.worktrees/chatmem-control-plane-v2",
            created_at: "2026-04-08T12:00:00Z",
            updated_at: "2026-04-08T12:30:00Z",
            summary: longConversationTitle,
            storage_path: "C:/Users/demo/.codex/sessions/2026/04/08/rollout-conv-long.jsonl",
            resume_command: "codex resume conv-long",
            messages: [],
            file_changes: [],
          };
        }

        if (payload?.id === "migrated-001") {
          return {
            id: "migrated-001",
            source_agent: payload?.agent ?? "codex",
            project_dir: "D:/VSP/demo",
            created_at: "2026-04-08T08:00:00Z",
            updated_at: "2026-04-08T09:30:00Z",
            summary: "Migrated session",
            storage_path: "C:/Users/demo/.codex/sessions/2026/04/08/rollout-migrated-001.jsonl",
            resume_command: "codex resume migrated-001",
            messages: [],
            file_changes: [],
          };
        }

        return {
          id: "conv-001",
          source_agent: payload?.agent ?? "claude",
          project_dir: "D:/VSP/demo",
          created_at: "2026-04-08T08:00:00Z",
          updated_at: "2026-04-08T09:00:00Z",
          summary: "Debug session",
          storage_path: "C:/Users/demo/.codex/sessions/2026/04/08/rollout-conv-001.jsonl",
          resume_command: "codex resume conv-001",
          messages: [
            {
              id: "msg-001",
              timestamp: "2026-04-08T08:00:00Z",
              role: "user",
              content: "Fix the memory view",
              tool_calls: [],
              metadata: {},
            },
          ],
          file_changes: [],
        };
      }

      if (command === "search_conversations" && payload?.query === "memory leak") {
        return [
          {
            id: "conv-002",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/PV/service",
            created_at: "2026-04-08T10:00:00Z",
            updated_at: "2026-04-08T11:00:00Z",
            summary: "Memory investigation",
            message_count: 4,
            file_count: 0,
          },
        ];
      }

      if (command === "list_repo_memories") {
        return [
          {
            memory_id: "mem-001",
            kind: "project_rule",
            title: "Use ChatMem for cross-agent continuation",
            value: "Prefer memory handoff over pasting long transcripts.",
            usage_hint: "Load this before resuming the project in another agent.",
            status: "active",
            last_verified_at: null,
            freshness_status: "fresh",
            freshness_score: 1,
            verified_at: null,
            verified_by: null,
            evidence_refs: [],
          },
        ];
      }

      if (command === "migrate_conversation") {
        return "migrated-001";
      }

      if (
        command === "list_memory_candidates" ||
        command === "list_handoffs" ||
        command === "list_checkpoints" ||
        command === "list_runs" ||
        command === "list_artifacts" ||
        command === "list_episodes"
      ) {
        return [];
      }

      return [];
    });

    mockCheckUpdate.mockResolvedValue({ shouldUpdate: false });
    mockInstallUpdate.mockResolvedValue(undefined);
    mockRelaunch.mockResolvedValue(undefined);
  });

  it("renders a simple conversation manager shell without dashboard navigation", async () => {
    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    renderApp();

    expect(await screen.findByText(appVersionPattern)).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Continue Work" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Needs Review" })).toBeNull();
    expect(screen.queryByRole("button", { name: "History" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Help" })).toBeNull();
    expect(screen.getByText("Projects")).toBeTruthy();
    expect(screen.queryByText("Chats")).toBeNull();
    expect(screen.getByRole("heading", { name: "Choose a conversation" })).toBeTruthy();
  });

  it("keeps delete available on each sidebar conversation", async () => {
    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    renderApp();

    const deleteButton = await screen.findByRole("button", {
      name: "Delete Debug session",
    });
    fireEvent.click(deleteButton);

    await waitFor(() => {
      expect(window.confirm).toHaveBeenCalledWith(expect.stringContaining("Debug session"));
      expect(mockInvoke).toHaveBeenCalledWith("delete_conversation", {
        agent: "claude",
        id: "conv-001",
      });
    });
  });

  it("merges equivalent project paths and does not repeat project conversations as chats", async () => {
    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );
    mockInvoke.mockImplementation(async (command: string, payload?: Record<string, unknown>) => {
      if (command === "list_conversations") {
        return [
          {
            id: "windows-prefix",
            source_agent: payload?.agent ?? "claude",
            project_dir: "\\\\?\\D:\\VSP",
            created_at: "2026-04-21T08:00:00Z",
            updated_at: "2026-04-21T09:00:00Z",
            summary: "Prefixed project path",
            message_count: 1,
            file_count: 0,
          },
          {
            id: "plain-windows",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:\\VSP",
            created_at: "2026-04-21T08:30:00Z",
            updated_at: "2026-04-21T09:30:00Z",
            summary: "Plain project path",
            message_count: 1,
            file_count: 0,
          },
          {
            id: "file-cwd",
            source_agent: payload?.agent ?? "claude",
            project_dir: "\\\\?\\D:\\VSP\\bm.md",
            created_at: "2026-04-21T09:00:00Z",
            updated_at: "2026-04-21T10:00:00Z",
            summary: "File cwd path",
            message_count: 1,
            file_count: 0,
          },
        ];
      }

      if (
        command === "list_memory_candidates" ||
        command === "list_handoffs" ||
        command === "list_checkpoints" ||
        command === "list_runs" ||
        command === "list_artifacts" ||
        command === "list_episodes" ||
        command === "list_repo_memories"
      ) {
        return [];
      }

      return [];
    });

    renderApp();

    await screen.findByText("Prefixed project path");

    await waitFor(() => {
      const projectGroups = document.querySelectorAll(".project-group");
      expect(projectGroups).toHaveLength(1);
      expect(projectGroups[0].textContent).toContain("VSP");
      expect(projectGroups[0].textContent).toContain("Prefixed project path");
      expect(projectGroups[0].textContent).toContain("Plain project path");
      expect(projectGroups[0].textContent).toContain("File cwd path");
      expect(document.querySelector(".chats-section")).toBeNull();
    });
  });

  it("classifies Codex generated chat folders as chats instead of projects", async () => {
    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );
    mockInvoke.mockImplementation(async (command: string, payload?: Record<string, unknown>) => {
      if (command === "list_conversations") {
        return [
          {
            id: "codex-project-vsp",
            source_agent: payload?.agent ?? "codex",
            project_dir: "D:/VSP",
            created_at: "2026-04-25T01:00:00Z",
            updated_at: "2026-04-25T02:00:00Z",
            summary: "VSP project work",
            message_count: 8,
            file_count: 2,
          },
          {
            id: "codex-project-data",
            source_agent: payload?.agent ?? "codex",
            project_dir: "D:/VSP/data",
            created_at: "2026-04-25T01:10:00Z",
            updated_at: "2026-04-25T02:10:00Z",
            summary: "Data project work",
            message_count: 4,
            file_count: 1,
          },
          {
            id: "codex-chat-numbered",
            source_agent: payload?.agent ?? "codex",
            project_dir: "C:/Users/Liang/Documents/Codex/2026-04-25/new-chat-2",
            created_at: "2026-04-25T01:20:00Z",
            updated_at: "2026-04-25T02:20:00Z",
            summary: "Where are our conversation files?",
            message_count: 3,
            file_count: 0,
          },
          {
            id: "codex-chat-flat",
            source_agent: payload?.agent ?? "codex",
            project_dir: "C:/Users/Liang/Documents/Codex/2026-04-21-new-chat",
            created_at: "2026-04-21T01:20:00Z",
            updated_at: "2026-04-21T02:20:00Z",
            summary: "Which model is this chat using?",
            message_count: 5,
            file_count: 0,
          },
        ];
      }

      if (
        command === "list_memory_candidates" ||
        command === "list_handoffs" ||
        command === "list_checkpoints" ||
        command === "list_runs" ||
        command === "list_artifacts" ||
        command === "list_episodes" ||
        command === "list_repo_memories"
      ) {
        return [];
      }

      return [];
    });

    renderApp();

    fireEvent.click(screen.getByRole("button", { name: "Codex" }));
    await screen.findByText("Where are our conversation files?");

    await waitFor(() => {
      const projectGroups = Array.from(document.querySelectorAll(".project-group"));
      const projectText = projectGroups.map((group) => group.textContent ?? "").join("\n");
      const chatSection = document.querySelector(".chats-section");

      expect(projectGroups).toHaveLength(2);
      expect(projectText).toContain("VSP");
      expect(projectText).toContain("data");
      expect(projectText).toContain("VSP project work");
      expect(projectText).toContain("Data project work");
      expect(projectText).not.toContain("new-chat");
      expect(projectText).not.toContain("new-chat-2");
      expect(projectText).not.toContain("2026-04-21-new-chat");
      expect(chatSection).toBeTruthy();
      expect(chatSection?.textContent).toContain("Where are our conversation files?");
      expect(chatSection?.textContent).toContain("Which model is this chat using?");
      expect(chatSection?.textContent).not.toContain("VSP project work");
      expect(chatSection?.textContent).not.toContain("Data project work");
    });
  });

  it("switches local history into an independent workspace view", async () => {
    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    renderApp();

    fireEvent.click((await screen.findAllByText("Debug session"))[0]);

    const currentTab = await screen.findByRole("tab", { name: "Current conversation" });
    const historyTab = screen.getByRole("tab", { name: "Local history" });

    expect(currentTab.getAttribute("aria-selected")).toBe("true");
    expect(historyTab.getAttribute("aria-selected")).toBe("false");
    expect(screen.getByRole("heading", { name: "Debug session" })).toBeTruthy();
    expect(screen.queryByText("Indexed conversations are ready for recall.")).toBeNull();

    fireEvent.click(historyTab);

    await waitFor(() => {
      expect(historyTab.getAttribute("aria-selected")).toBe("true");
      expect(screen.queryByRole("heading", { name: "Debug session" })).toBeNull();
      expect(screen.getByText("Indexed conversations are ready for recall.")).toBeTruthy();
    });

    fireEvent.click(currentTab);

    await waitFor(() => {
      expect(currentTab.getAttribute("aria-selected")).toBe("true");
      expect(screen.getByRole("heading", { name: "Debug session" })).toBeTruthy();
      expect(screen.queryByText("Indexed conversations are ready for recall.")).toBeNull();
    });
  });

  it("uses readable hover labels for project section controls", async () => {
    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    renderApp();

    await screen.findByText("Projects");

    const collapseButton = screen.getByRole("button", { name: "Collapse all projects" });
    expect(collapseButton.classList.contains("sidebar-action-button")).toBe(true);
    expect(within(collapseButton).getByText("Collapse all projects")).toBeTruthy();

    const organizeButton = screen.getByRole("button", {
      name: "Filter, sort, and organize conversations",
    });
    expect(organizeButton.classList.contains("sidebar-action-button")).toBe(true);
    expect(within(organizeButton).getByText("Filter, sort, and organize conversations")).toBeTruthy();

    fireEvent.click(collapseButton);

    const restoreButton = screen.getByRole("button", { name: "Restore previous expansion" });
    expect(within(restoreButton).getByText("Restore previous expansion")).toBeTruthy();
    expect(
      Array.from(restoreButton.querySelectorAll("svg path")).map((path) => path.getAttribute("d")),
    ).toEqual(["M10.5 4H12v1.5", "M5.5 12H4v-1.5"]);
  });

  it("starts native window dragging from the top bar without hijacking controls", async () => {
    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    renderApp();

    const title = await screen.findByText(appVersionPattern);
    const topbar = title.closest(".app-topbar");
    expect(topbar).toBeTruthy();

    fireEvent.mouseDown(topbar!, { button: 0 });
    expect(mockStartDragging).toHaveBeenCalledTimes(1);

    fireEvent.mouseDown(screen.getByRole("button", { name: "Settings" }), { button: 0 });
    expect(mockStartDragging).toHaveBeenCalledTimes(1);
  });

  it("removes the floating shell when the native window is maximized", async () => {
    mockIsMaximized.mockResolvedValue(true);
    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    const { container } = renderApp();

    await screen.findByText(appVersionPattern);

    await waitFor(() => {
      expect(container.querySelector(".app-shell")?.classList.contains("is-window-filled")).toBe(
        true,
      );
    });
  });

  it("shows conversation details, migration, copy actions, and startup rules drawer in one workspace", async () => {
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
      expect(screen.queryByRole("button", { name: "Manage Rules" })).toBeNull();
      expect(screen.getByRole("button", { name: "Migrate" })).toBeTruthy();
      expect(screen.queryByRole("heading", { name: "Suggested Next Step" })).toBeNull();
      expect(screen.queryByRole("heading", { name: "Recent Transfers" })).toBeNull();
    });

    expect(screen.queryByRole("complementary", { name: "Startup Rules" })).toBeNull();
    expect(screen.queryByText("Use ChatMem for cross-agent continuation")).toBeNull();

    await openLocalHistoryView();
    fireEvent.click(getMemoryButton());

    expect(await screen.findByRole("complementary", { name: "Startup Rules" })).toBeTruthy();
    expect(screen.getByText("Use ChatMem for cross-agent continuation")).toBeTruthy();
  });

  it("does not let an auto scan from a stale repo overwrite the active repo history state", async () => {
    const deferredScan = createDeferred<{
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
            id: "conv-001",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/VSP/demo",
            created_at: "2026-04-08T08:00:00Z",
            updated_at: "2026-04-08T09:00:00Z",
            summary: "Debug session",
            message_count: 2,
            file_count: 1,
          },
          {
            id: "conv-002",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/PV/service",
            created_at: "2026-04-08T10:00:00Z",
            updated_at: "2026-04-08T11:00:00Z",
            summary: "Memory investigation",
            message_count: 4,
            file_count: 0,
          },
        ];
      }

      if (command === "read_conversation") {
        if (payload?.id === "conv-002") {
          return {
            id: "conv-002",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/PV/service",
            created_at: "2026-04-08T10:00:00Z",
            updated_at: "2026-04-08T11:00:00Z",
            summary: "Memory investigation",
            storage_path: "C:/Users/demo/.codex/sessions/2026/04/08/rollout-conv-002.jsonl",
            resume_command: "codex resume conv-002",
            messages: [],
            file_changes: [],
          };
        }

        return {
          id: "conv-001",
          source_agent: payload?.agent ?? "claude",
          project_dir: "D:/VSP/demo",
          created_at: "2026-04-08T08:00:00Z",
          updated_at: "2026-04-08T09:00:00Z",
          summary: "Debug session",
          storage_path: "C:/Users/demo/.codex/sessions/2026/04/08/rollout-conv-001.jsonl",
          resume_command: "codex resume conv-001",
          messages: [],
          file_changes: [],
        };
      }

      if (command === "list_repo_memories") {
        return [];
      }

      if (
        command === "list_memory_candidates" ||
        command === "list_handoffs" ||
        command === "list_checkpoints" ||
        command === "list_runs" ||
        command === "list_artifacts" ||
        command === "list_episodes" ||
        command === "rebuild_repo_wiki"
      ) {
        return [];
      }

      if (command === "get_repo_memory_health") {
        if (payload?.repoRoot === "D:/VSP/demo") {
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

        if (payload?.repoRoot === "D:/PV/service") {
          return {
            repo_root: "D:/PV/service",
            canonical_repo_root: "D:/PV/service",
            approved_memory_count: 0,
            pending_candidate_count: 0,
            search_document_count: 4,
            indexed_chunk_count: 8,
            inherited_repo_roots: [],
            conversation_counts_by_agent: [{ source_agent: "claude", conversation_count: 1 }],
            repo_aliases: [],
            warnings: [],
          };
        }
      }

      if (command === "scan_repo_conversations" && payload?.repoRoot === "D:/VSP/demo") {
        return deferredScan.promise;
      }

      return [];
    });

    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    renderApp();

    fireEvent.click((await screen.findAllByText("Debug session"))[0]);

    await screen.findByRole("heading", { name: "Debug session" });
    await openLocalHistoryView();
    await waitFor(() => {
      expect(screen.getByRole("heading", { level: 2, name: "D:/VSP/demo" })).toBeTruthy();
      expect(mockInvoke).toHaveBeenCalledWith("scan_repo_conversations", {
        repoRoot: "D:/VSP/demo",
      });
    });

    fireEvent.click((await screen.findAllByText("Memory investigation"))[0]);

    await waitFor(() => {
      expect(screen.getByRole("heading", { level: 2, name: "D:/PV/service" })).toBeTruthy();
    });

    await act(async () => {
      deferredScan.resolve({
        repo_root: "D:/VSP/demo",
        canonical_repo_root: "D:/VSP/demo",
        scanned_conversation_count: 1,
        linked_conversation_count: 1,
        skipped_conversation_count: 0,
        source_agents: [{ source_agent: "claude", conversation_count: 1 }],
        warnings: [],
      });
      await deferredScan.promise;
    });

    await waitFor(() => {
      expect(screen.getByRole("heading", { level: 2, name: "D:/PV/service" })).toBeTruthy();
    });
    expect(screen.queryByRole("heading", { level: 2, name: "D:/VSP/demo" })).toBeNull();
  });

  it("auto bootstraps another empty repo even while a different repo bootstrap is already in flight", async () => {
    const deferredRepoAScan = createDeferred<{
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
            id: "conv-001",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/VSP/demo",
            created_at: "2026-04-08T08:00:00Z",
            updated_at: "2026-04-08T09:00:00Z",
            summary: "Debug session",
            message_count: 2,
            file_count: 1,
          },
          {
            id: "conv-002",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/PV/service",
            created_at: "2026-04-08T10:00:00Z",
            updated_at: "2026-04-08T11:00:00Z",
            summary: "Memory investigation",
            message_count: 4,
            file_count: 0,
          },
        ];
      }

      if (command === "read_conversation") {
        if (payload?.id === "conv-002") {
          return {
            id: "conv-002",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/PV/service",
            created_at: "2026-04-08T10:00:00Z",
            updated_at: "2026-04-08T11:00:00Z",
            summary: "Memory investigation",
            storage_path: "C:/Users/demo/.codex/sessions/2026/04/08/rollout-conv-002.jsonl",
            resume_command: "codex resume conv-002",
            messages: [],
            file_changes: [],
          };
        }

        return {
          id: "conv-001",
          source_agent: payload?.agent ?? "claude",
          project_dir: "D:/VSP/demo",
          created_at: "2026-04-08T08:00:00Z",
          updated_at: "2026-04-08T09:00:00Z",
          summary: "Debug session",
          storage_path: "C:/Users/demo/.codex/sessions/2026/04/08/rollout-conv-001.jsonl",
          resume_command: "codex resume conv-001",
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
        command === "list_episodes" ||
        command === "rebuild_repo_wiki"
      ) {
        return [];
      }

      if (command === "get_repo_memory_health") {
        if (payload?.repoRoot === "D:/VSP/demo") {
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

        if (payload?.repoRoot === "D:/PV/service") {
          return {
            repo_root: "D:/PV/service",
            canonical_repo_root: "D:/PV/service",
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
      }

      if (command === "scan_repo_conversations" && payload?.repoRoot === "D:/VSP/demo") {
        return deferredRepoAScan.promise;
      }

      if (command === "scan_repo_conversations" && payload?.repoRoot === "D:/PV/service") {
        return {
          repo_root: "D:/PV/service",
          canonical_repo_root: "D:/PV/service",
          scanned_conversation_count: 1,
          linked_conversation_count: 1,
          skipped_conversation_count: 0,
          source_agents: [{ source_agent: "claude", conversation_count: 1 }],
          warnings: [],
        };
      }

      return [];
    });

    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    renderApp();

    fireEvent.click((await screen.findAllByText("Debug session"))[0]);

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Debug session" })).toBeTruthy();
      expect(
        mockInvoke.mock.calls.filter(
          ([command, callPayload]) =>
            command === "scan_repo_conversations" &&
            callPayload?.repoRoot === "D:/VSP/demo",
        ),
      ).toHaveLength(1);
    });

    fireEvent.click((await screen.findAllByText("Memory investigation"))[0]);

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Memory investigation" })).toBeTruthy();
      expect(mockInvoke).toHaveBeenCalledWith("get_repo_memory_health", {
        repoRoot: "D:/PV/service",
      });
    });

    await waitFor(() => {
      expect(
        mockInvoke.mock.calls.filter(
          ([command, callPayload]) =>
            command === "scan_repo_conversations" &&
            callPayload?.repoRoot === "D:/PV/service",
        ),
      ).toHaveLength(1);
    });
  });

  it("shows local history readiness after automatic bootstrap finishes", async () => {
    let hasIndexedChunks = false;

    mockInvoke.mockImplementation(async (command: string, payload?: Record<string, unknown>) => {
      if (command === "list_conversations") {
        return [
          {
            id: "conv-001",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/VSP/demo",
            created_at: "2026-04-08T08:00:00Z",
            updated_at: "2026-04-08T09:00:00Z",
            summary: "Debug session",
            message_count: 2,
            file_count: 1,
          },
        ];
      }

      if (command === "read_conversation") {
        return {
          id: "conv-001",
          source_agent: payload?.agent ?? "claude",
          project_dir: "D:/VSP/demo",
          created_at: "2026-04-08T08:00:00Z",
          updated_at: "2026-04-08T09:00:00Z",
          summary: "Debug session",
          storage_path: "C:/Users/demo/.codex/sessions/2026/04/08/rollout-conv-001.jsonl",
          resume_command: "codex resume conv-001",
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
        command === "list_episodes" ||
        command === "rebuild_repo_wiki"
      ) {
        return [];
      }

      if (command === "import_all_local_history") {
        return {
          scanned_conversation_count: 3,
          imported_conversation_count: 2,
          skipped_conversation_count: 1,
          indexed_repo_count: 2,
          source_agents: [{ source_agent: "claude", conversation_count: 2 }],
          imported_project_roots: [
            {
              source_agent: "claude",
              project_root: "D:/VSP/demo",
              conversation_count: 1,
            },
            {
              source_agent: "claude",
              project_root: "D:/PV/service",
              conversation_count: 1,
            },
          ],
          warnings: [],
          imported_at: "2026-04-25T12:00:00Z",
        };
      }

      if (command === "get_repo_memory_health") {
        if (hasIndexedChunks) {
          return {
            repo_root: "D:/VSP/demo",
            canonical_repo_root: "D:/VSP/demo",
            approved_memory_count: 0,
            pending_candidate_count: 0,
            search_document_count: 4,
            indexed_chunk_count: 8,
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
        hasIndexedChunks = true;
        return {
          repo_root: "D:/VSP/demo",
          canonical_repo_root: "D:/VSP/demo",
          scanned_conversation_count: 1,
          linked_conversation_count: 1,
          skipped_conversation_count: 0,
          source_agents: [{ source_agent: "claude", conversation_count: 1 }],
          warnings: [],
        };
      }

      return [];
    });

    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    renderApp();

    fireEvent.click((await screen.findAllByText("Debug session"))[0]);
    await openLocalHistoryView();

    await waitFor(() => {
      expect(getMemoryButton()).toBeTruthy();
    });

    expect(getMemoryButton().getAttribute("aria-label")).toBe("Manage Rules");

    await waitFor(() => {
      const memoryButton = getMemoryButton();
      expect(memoryButton.getAttribute("aria-label")).toBe("Manage Rules");
      expect(memoryButton.classList.contains("is-ready")).toBe(false);
      expect(within(memoryButton).queryByText("Ready")).toBeNull();
      expect(
        screen.getByText("Local history is ready for this project. You can now ask what was discussed before."),
      ).toBeTruthy();
      expect(
        screen.getByText("Full import: scanned 3 / imported 2 / 2 projects / 1 skipped"),
      ).toBeTruthy();
    });
    expect(mockInvoke).toHaveBeenCalledWith("import_all_local_history");
  });

  it("merges a local-history alias without reimporting all local history", async () => {
    mockInvoke.mockImplementation(async (command: string, payload?: Record<string, unknown>) => {
      if (command === "list_conversations") {
        return [
          {
            id: "conv-001",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/VSP/demo",
            created_at: "2026-04-08T08:00:00Z",
            updated_at: "2026-04-08T09:00:00Z",
            summary: "Debug session",
            message_count: 2,
            file_count: 1,
          },
        ];
      }

      if (command === "read_conversation") {
        return {
          id: "conv-001",
          source_agent: payload?.agent ?? "claude",
          project_dir: "D:/VSP/demo",
          created_at: "2026-04-08T08:00:00Z",
          updated_at: "2026-04-08T09:00:00Z",
          summary: "Debug session",
          storage_path: "C:/Users/demo/.codex/sessions/2026/04/08/rollout-conv-001.jsonl",
          resume_command: "codex resume conv-001",
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
        command === "list_episodes" ||
        command === "rebuild_repo_wiki"
      ) {
        return [];
      }

      if (command === "get_repo_memory_health") {
        return {
          repo_root: "D:/VSP/demo",
          canonical_repo_root: "D:/VSP/demo",
          approved_memory_count: 0,
          pending_candidate_count: 0,
          search_document_count: 668,
          indexed_chunk_count: 668,
          inherited_repo_roots: [],
          conversation_counts_by_agent: [{ source_agent: "codex", conversation_count: 134 }],
          repo_aliases: [],
          latest_scan: {
            repo_root: "D:/VSP/demo",
            canonical_repo_root: "D:/VSP/demo",
            scanned_conversation_count: 139,
            linked_conversation_count: 134,
            skipped_conversation_count: 5,
            source_agents: [{ source_agent: "codex", conversation_count: 134 }],
            unmatched_project_roots: [
              {
                source_agent: "codex",
                project_root: "d:/vsp/easymd",
                conversation_count: 5,
              },
            ],
            warnings: [],
            scanned_at: "2026-04-25T12:00:00Z",
          },
          warnings: [],
        };
      }

      if (command === "merge_repo_alias") {
        return {
          alias_root: payload?.aliasRoot,
          alias_kind: "manual",
          confidence: 1,
        };
      }

      if (command === "scan_repo_conversations") {
        return {
          repo_root: "D:/VSP/demo",
          canonical_repo_root: "D:/VSP/demo",
          scanned_conversation_count: 139,
          linked_conversation_count: 139,
          skipped_conversation_count: 0,
          source_agents: [{ source_agent: "codex", conversation_count: 139 }],
          unmatched_project_roots: [],
          warnings: [],
        };
      }

      return [];
    });

    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    renderApp();

    fireEvent.click((await screen.findAllByText("Debug session"))[0]);
    await openLocalHistoryView();
    const mergeButton = await screen.findByRole("button", {
      name: "Merge into this project d:/vsp/easymd",
    });

    mockInvoke.mockClear();
    fireEvent.click(mergeButton);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("merge_repo_alias", {
        repoRoot: "D:/VSP/demo",
        aliasRoot: "d:/vsp/easymd",
      });
      expect(mockInvoke).toHaveBeenCalledWith("scan_repo_conversations", {
        repoRoot: "D:/VSP/demo",
      });
    });
    expect(mockInvoke).not.toHaveBeenCalledWith("import_all_local_history");
  });

  it("runs the full local-history import once across automatic bootstraps", async () => {
    const indexedRepos = new Set<string>();

    mockInvoke.mockImplementation(async (command: string, payload?: Record<string, unknown>) => {
      if (command === "list_conversations") {
        return [
          {
            id: "conv-001",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/VSP/demo",
            created_at: "2026-04-08T08:00:00Z",
            updated_at: "2026-04-08T09:00:00Z",
            summary: "Debug session",
            message_count: 2,
            file_count: 1,
          },
          {
            id: "conv-002",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/PV/service",
            created_at: "2026-04-08T10:00:00Z",
            updated_at: "2026-04-08T11:00:00Z",
            summary: "Memory investigation",
            message_count: 4,
            file_count: 0,
          },
        ];
      }

      if (command === "read_conversation") {
        const isSecond = payload?.id === "conv-002";
        return {
          id: isSecond ? "conv-002" : "conv-001",
          source_agent: payload?.agent ?? "claude",
          project_dir: isSecond ? "D:/PV/service" : "D:/VSP/demo",
          created_at: isSecond ? "2026-04-08T10:00:00Z" : "2026-04-08T08:00:00Z",
          updated_at: isSecond ? "2026-04-08T11:00:00Z" : "2026-04-08T09:00:00Z",
          summary: isSecond ? "Memory investigation" : "Debug session",
          storage_path: "C:/Users/demo/.codex/sessions/2026/04/08/rollout-conv.jsonl",
          resume_command: "codex resume conv",
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
        command === "list_episodes" ||
        command === "rebuild_repo_wiki"
      ) {
        return [];
      }

      if (command === "get_repo_memory_health") {
        const repoRoot = String(payload?.repoRoot ?? "");
        const indexed = indexedRepos.has(repoRoot);
        return {
          repo_root: repoRoot,
          canonical_repo_root: repoRoot,
          approved_memory_count: 0,
          pending_candidate_count: 0,
          search_document_count: indexed ? 4 : 0,
          indexed_chunk_count: indexed ? 8 : 0,
          inherited_repo_roots: [],
          conversation_counts_by_agent: indexed
            ? [{ source_agent: "claude", conversation_count: 1 }]
            : [],
          repo_aliases: [],
          warnings: [],
        };
      }

      if (command === "scan_repo_conversations") {
        const repoRoot = String(payload?.repoRoot ?? "");
        indexedRepos.add(repoRoot);
        return {
          repo_root: repoRoot,
          canonical_repo_root: repoRoot,
          scanned_conversation_count: 1,
          linked_conversation_count: 1,
          skipped_conversation_count: 0,
          source_agents: [{ source_agent: "claude", conversation_count: 1 }],
          warnings: [],
        };
      }

      return [];
    });

    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    renderApp();

    fireEvent.click((await screen.findAllByText("Debug session"))[0]);
    await openLocalHistoryView();
    await screen.findByText("Local history is ready for this project. You can now ask what was discussed before.");

    fireEvent.click((await screen.findAllByText("Memory investigation"))[0]);
    await screen.findByRole("heading", { level: 2, name: "D:/PV/service" });
    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("scan_repo_conversations", {
        repoRoot: "D:/PV/service",
      });
    });

    expect(
      mockInvoke.mock.calls.filter(([command]) => command === "import_all_local_history"),
    ).toHaveLength(1);
  });

  it("keeps pending-rule counts out of the manage rules button while local history readiness stays in the status band", async () => {
    let hasIndexedChunks = false;

    mockInvoke.mockImplementation(async (command: string, payload?: Record<string, unknown>) => {
      if (command === "list_conversations") {
        return [
          {
            id: "conv-001",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/VSP/demo",
            created_at: "2026-04-08T08:00:00Z",
            updated_at: "2026-04-08T09:00:00Z",
            summary: "Debug session",
            message_count: 2,
            file_count: 1,
          },
        ];
      }

      if (command === "read_conversation") {
        return {
          id: "conv-001",
          source_agent: payload?.agent ?? "claude",
          project_dir: "D:/VSP/demo",
          created_at: "2026-04-08T08:00:00Z",
          updated_at: "2026-04-08T09:00:00Z",
          summary: "Debug session",
          storage_path: "C:/Users/demo/.codex/sessions/2026/04/08/rollout-conv-001.jsonl",
          resume_command: "codex resume conv-001",
          messages: [],
          file_changes: [],
        };
      }

      if (command === "list_repo_memories" || command === "rebuild_repo_wiki") {
        return [];
      }

      if (command === "list_memory_candidates") {
        return [
          {
            id: "cand-001",
            title: "Pending memory 1",
          },
          {
            id: "cand-002",
            title: "Pending memory 2",
          },
        ];
      }

      if (
        command === "list_handoffs" ||
        command === "list_checkpoints" ||
        command === "list_runs" ||
        command === "list_artifacts" ||
        command === "list_episodes"
      ) {
        return [];
      }

      if (command === "get_repo_memory_health") {
        if (hasIndexedChunks) {
          return {
            repo_root: "D:/VSP/demo",
            canonical_repo_root: "D:/VSP/demo",
            approved_memory_count: 0,
            pending_candidate_count: 2,
            search_document_count: 4,
            indexed_chunk_count: 8,
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
          pending_candidate_count: 2,
          search_document_count: 0,
          indexed_chunk_count: 0,
          inherited_repo_roots: [],
          conversation_counts_by_agent: [],
          repo_aliases: [],
          warnings: [],
        };
      }

      if (command === "scan_repo_conversations") {
        hasIndexedChunks = true;
        return {
          repo_root: "D:/VSP/demo",
          canonical_repo_root: "D:/VSP/demo",
          scanned_conversation_count: 1,
          linked_conversation_count: 1,
          skipped_conversation_count: 0,
          source_agents: [{ source_agent: "claude", conversation_count: 1 }],
          warnings: [],
        };
      }

      return [];
    });

    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    renderApp();

    fireEvent.click((await screen.findAllByText("Debug session"))[0]);
    await openLocalHistoryView();

    await waitFor(() => {
      expect(getMemoryButton()).toBeTruthy();
    });

    await waitFor(() => {
      const memoryButton = getMemoryButton();
      expect(memoryButton.getAttribute("aria-label")).toBe("Manage Rules");
      expect(memoryButton.classList.contains("is-ready")).toBe(false);
      expect(within(memoryButton).queryByText("2")).toBeNull();
      expect(screen.getByText("Needs review")).toBeTruthy();
      expect(memoryButton.querySelector(".memory-drawer-trigger-ready.is-visible")).toBeNull();
      expect(
        screen.getByText("Local history is ready for this project. You can now ask what was discussed before."),
      ).toBeTruthy();
    });
  });

  it("keeps startup rules action separate from local-history readiness during an async conversation switch", async () => {
    let hasDemoIndexedChunks = false;
    const deferredSecondConversation = createDeferred<{
      id: string;
      source_agent: string;
      project_dir: string;
      created_at: string;
      updated_at: string;
      summary: string;
      storage_path: string;
      resume_command: string;
      messages: never[];
      file_changes: never[];
    }>();

    mockInvoke.mockImplementation(async (command: string, payload?: Record<string, unknown>) => {
      if (command === "list_conversations") {
        return [
          {
            id: "conv-001",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/VSP/demo",
            created_at: "2026-04-08T08:00:00Z",
            updated_at: "2026-04-08T09:00:00Z",
            summary: "Debug session",
            message_count: 2,
            file_count: 1,
          },
          {
            id: "conv-002",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/PV/service",
            created_at: "2026-04-08T10:00:00Z",
            updated_at: "2026-04-08T11:00:00Z",
            summary: "Memory investigation",
            message_count: 4,
            file_count: 0,
          },
        ];
      }

      if (command === "read_conversation") {
        if (payload?.id === "conv-002") {
          return deferredSecondConversation.promise;
        }

        return {
          id: "conv-001",
          source_agent: payload?.agent ?? "claude",
          project_dir: "D:/VSP/demo",
          created_at: "2026-04-08T08:00:00Z",
          updated_at: "2026-04-08T09:00:00Z",
          summary: "Debug session",
          storage_path: "C:/Users/demo/.codex/sessions/2026/04/08/rollout-conv-001.jsonl",
          resume_command: "codex resume conv-001",
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
        command === "list_episodes" ||
        command === "rebuild_repo_wiki"
      ) {
        return [];
      }

      if (command === "get_repo_memory_health") {
        if (payload?.repoRoot === "D:/VSP/demo") {
          if (hasDemoIndexedChunks) {
            return {
              repo_root: "D:/VSP/demo",
              canonical_repo_root: "D:/VSP/demo",
              approved_memory_count: 0,
              pending_candidate_count: 0,
              search_document_count: 4,
              indexed_chunk_count: 8,
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

        return {
          repo_root: "D:/PV/service",
          canonical_repo_root: "D:/PV/service",
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
        hasDemoIndexedChunks = true;
        return {
          repo_root: "D:/VSP/demo",
          canonical_repo_root: "D:/VSP/demo",
          scanned_conversation_count: 1,
          linked_conversation_count: 1,
          skipped_conversation_count: 0,
          source_agents: [{ source_agent: "claude", conversation_count: 1 }],
          warnings: [],
        };
      }

      return [];
    });

    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    renderApp();

    fireEvent.click((await screen.findAllByText("Debug session"))[0]);
    await openLocalHistoryView();

    await waitFor(() => {
      expect(getMemoryButton()).toBeTruthy();
    });

    await waitFor(() => {
      const memoryButton = getMemoryButton();
      expect(memoryButton.getAttribute("aria-label")).toBe("Manage Rules");
      expect(memoryButton.classList.contains("is-ready")).toBe(false);
      expect(within(memoryButton).queryByText("Ready")).toBeNull();
    });

    const nextConversationRow = (await screen.findAllByText("Memory investigation"))[0]
      .closest("button") as HTMLButtonElement | null;
    expect(nextConversationRow).toBeTruthy();
    fireEvent.click(nextConversationRow!);

    await waitFor(() => {
      const memoryButton = getMemoryButton();
      expect(screen.getByRole("heading", { level: 2, name: "D:/VSP/demo" })).toBeTruthy();
      expect(memoryButton.getAttribute("aria-label")).toBe("Manage Rules");
      expect(memoryButton.classList.contains("is-ready")).toBe(false);
      expect(memoryButton.querySelector(".memory-drawer-trigger-ready.is-visible")).toBeNull();
    });
  });

  it("truncates very long workspace titles while keeping the full title available", async () => {
    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    renderApp();

    fireEvent.click((await screen.findAllByText(truncateSidebarTitle(longConversationTitle)))[0]);

    const heading = await screen.findByRole("heading", {
      name: truncateWorkspaceTitle(longConversationTitle),
    });

    expect(heading.getAttribute("title")).toBe(longConversationTitle);
    const workspacePath = document.querySelector(".conversation-title-block span");
    expect(workspacePath?.getAttribute("title")).toBe(
      "D:/VSP/agentswap-gui/.worktrees/chatmem-control-plane-v2",
    );
  });

  it("searches conversations by message body content", async () => {
    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    renderApp();

    const input = await screen.findByPlaceholderText("Search conversations...");
    fireEvent.change(input, { target: { value: "memory leak" } });

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("search_conversations", {
        agent: "claude",
        query: "memory leak",
      });
      expect(screen.getAllByText("Memory investigation").length).toBeGreaterThan(0);
    });
  });

  it("keeps migration working from the selected conversation detail", async () => {
    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    renderApp();

    fireEvent.click((await screen.findAllByText("Debug session"))[0]);
    fireEvent.click(await screen.findByRole("button", { name: "Migrate" }));

    const confirmButton = document.querySelector(
      ".modal-actions .btn.btn-primary",
    ) as HTMLButtonElement | null;
    expect(confirmButton).toBeTruthy();
    fireEvent.click(confirmButton!);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("migrate_conversation", {
        source: "claude",
        target: "codex",
        id: "conv-001",
        mode: "copy",
      });
      expect(mockInvoke).toHaveBeenCalledWith("list_conversations", {
        agent: "codex",
      });
      expect(mockInvoke).toHaveBeenCalledWith("read_conversation", {
        agent: "codex",
        id: "migrated-001",
      });
      expect(screen.getAllByText("Migrated session").length).toBeGreaterThan(0);
    });
  });

  it("runs a manual update check from settings", async () => {
    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    renderApp();

    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    fireEvent.click(await screen.findByRole("button", { name: "Check for updates" }));

    await waitFor(() => {
      expect(mockCheckUpdate).toHaveBeenCalledTimes(1);
    });
  });

  it("auto-checks for updates on launch when enabled", async () => {
    vi.useFakeTimers();
    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: true }),
    );
    mockCheckUpdate.mockResolvedValue({
      shouldUpdate: true,
      manifest: {
        version: "1.0.0",
        date: "2026-04-08T12:00:00Z",
        body: "Bug fixes",
      },
    });

    renderApp();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(3600);
    });

    expect(mockCheckUpdate).toHaveBeenCalledTimes(1);
    expect(screen.getAllByText(/1\.0\.0/).length).toBeGreaterThan(0);
  });
});
