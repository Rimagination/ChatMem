import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
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
const longConversationTitle =
  "You are Task 6 的独立代码质量 reviewer。请在工作树 D:\\VSP\\agentswap-gui\\.worktrees\\chatmem-control-plane-v2 review 最新提交，重点寻找真实风险，而不是泛泛建议。";

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

    expect(await screen.findByText("ChatMem v0.1.9")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Continue Work" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Needs Review" })).toBeNull();
    expect(screen.queryByRole("button", { name: "History" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Help" })).toBeNull();
    expect(screen.getByText("Projects")).toBeTruthy();
    expect(screen.queryByText("Chats")).toBeNull();
    expect(screen.getByRole("heading", { name: "Choose a conversation" })).toBeTruthy();
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

  it("starts native window dragging from the top bar without hijacking controls", async () => {
    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    renderApp();

    const title = await screen.findByText("ChatMem v0.1.9");
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

    await screen.findByText("ChatMem v0.1.9");

    await waitFor(() => {
      expect(container.querySelector(".app-shell")?.classList.contains("is-window-filled")).toBe(
        true,
      );
    });
  });

  it("shows conversation details, migration, copy actions, and memory drawer in one workspace", async () => {
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
      expect(screen.getByRole("button", { name: "Memory" })).toBeTruthy();
      expect(screen.getByRole("button", { name: "Migrate" })).toBeTruthy();
      expect(screen.queryByRole("heading", { name: "Suggested Next Step" })).toBeNull();
      expect(screen.queryByRole("heading", { name: "Recent Transfers" })).toBeNull();
    });

    expect(screen.queryByRole("complementary", { name: "Project Memory" })).toBeNull();
    expect(screen.queryByText("Use ChatMem for cross-agent continuation")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Memory" }));

    expect(await screen.findByRole("complementary", { name: "Project Memory" })).toBeTruthy();
    expect(screen.getByText("Use ChatMem for cross-agent continuation")).toBeTruthy();
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
        version: "0.1.9",
        date: "2026-04-08T12:00:00Z",
        body: "Bug fixes",
      },
    });

    renderApp();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(3600);
    });

    expect(mockCheckUpdate).toHaveBeenCalledTimes(1);
    expect(screen.getAllByText(/0\.1\.9/).length).toBeGreaterThan(0);
  });
});
