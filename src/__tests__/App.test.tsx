import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "../App";
import { I18nProvider } from "../i18n/I18nProvider";

const mockInvoke = vi.fn();
const mockCheckUpdate = vi.fn();
const mockInstallUpdate = vi.fn();
const mockRelaunch = vi.fn();

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
    localStorage.clear();
    vi.useRealTimers();
    vi.stubGlobal("alert", vi.fn());

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
        ];
      }

      if (command === "read_conversation") {
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
          messages: [],
          file_changes: [],
        };
      }

      if (command === "search_conversations" && payload?.query === "memory leak") {
        return [
          {
            id: "conv-002",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/VSP/service",
            created_at: "2026-04-08T10:00:00Z",
            updated_at: "2026-04-08T11:00:00Z",
            summary: "Memory investigation",
            message_count: 4,
            file_count: 0,
          },
        ];
      }

      if (command === "migrate_conversation") {
        return "migrated-001";
      }

      return [];
    });

    mockCheckUpdate.mockResolvedValue({ shouldUpdate: false });
    mockInstallUpdate.mockResolvedValue(undefined);
    mockRelaunch.mockResolvedValue(undefined);
  });

  it("restores the saved language and renders the English shell copy", async () => {
    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: true }),
    );

    renderApp();

    expect(
      await screen.findByText("Your local AI conversations, ready to resume"),
    ).toBeTruthy();
    expect(screen.getByPlaceholderText("Search conversations...")).toBeTruthy();
  });

  it("opens settings and switches the interface language to English", async () => {
    renderApp();

    const settingsButton = document.querySelector(".toolbar-button") as HTMLButtonElement | null;
    expect(settingsButton).toBeTruthy();
    fireEvent.click(settingsButton!);

    const localeSelect = document.querySelector("select") as HTMLSelectElement | null;
    expect(localeSelect).toBeTruthy();
    fireEvent.change(localeSelect!, { target: { value: "en" } });

    expect(await screen.findByRole("heading", { name: "Settings" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Check for updates" })).toBeTruthy();
  });

  it("runs a manual update check from settings", async () => {
    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    renderApp();

    const settingsButton = document.querySelector(".toolbar-button") as HTMLButtonElement | null;
    expect(settingsButton).toBeTruthy();
    fireEvent.click(settingsButton!);
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
        version: "0.1.4",
        date: "2026-04-08T12:00:00Z",
        body: "Bug fixes",
      },
    });

    renderApp();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(3600);
    });

    expect(mockCheckUpdate).toHaveBeenCalledTimes(1);
    expect(screen.getByText(/0\.1\.4/)).toBeTruthy();
  });

  it("renders file location and copy actions for the selected conversation", async () => {
    renderApp();

    expect(document.querySelectorAll(".toolbar-button").length).toBeGreaterThanOrEqual(2);

    const conversation = await screen.findByText("Debug session");
    fireEvent.click(conversation);

    await waitFor(() => {
      expect(
        screen.getByText("C:/Users/demo/.codex/sessions/2026/04/08/rollout-conv-001.jsonl"),
      ).toBeTruthy();
      expect(screen.getByRole("button", { name: "复制位置" })).toBeTruthy();
      expect(screen.getByRole("button", { name: "复制恢复命令" })).toBeTruthy();
    });
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
      expect(screen.getByText("Memory investigation")).toBeTruthy();
    });
  });

  it("switches to the target agent and opens the migrated conversation", async () => {
    renderApp();

    fireEvent.click(await screen.findByText("Debug session"));

    await waitFor(() => {
      expect(
        screen.getByText("C:/Users/demo/.codex/sessions/2026/04/08/rollout-conv-001.jsonl"),
      ).toBeTruthy();
    });

    const migrateButton = document.querySelector(
      ".content-header-actions .btn.btn-secondary",
    ) as HTMLButtonElement | null;
    expect(migrateButton).toBeTruthy();
    fireEvent.click(migrateButton!);

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
      expect(screen.getByText("Migrated session")).toBeTruthy();
    });
  });

  it("truncates the workspace heading like Codex app while preserving the full title", async () => {
    const longTitle =
      "你是最终收口补丁的独立代码质量 reviewer，请在工作树 D:\\VSP\\agentswap-gui\\.worktrees\\chatmem-control-plane-v2 review 最新提交 16a39b2";

    mockInvoke.mockImplementation(async (command: string, payload?: Record<string, unknown>) => {
      if (command === "list_conversations") {
        return [
          {
            id: "conv-long",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/VSP/demo",
            created_at: "2026-04-08T08:00:00Z",
            updated_at: "2026-04-08T09:00:00Z",
            summary: longTitle,
            message_count: 2,
            file_count: 1,
          },
        ];
      }

      if (command === "read_conversation") {
        return {
          id: "conv-long",
          source_agent: payload?.agent ?? "claude",
          project_dir: "D:/VSP/demo",
          created_at: "2026-04-08T08:00:00Z",
          updated_at: "2026-04-08T09:00:00Z",
          summary: longTitle,
          storage_path: "C:/Users/demo/.codex/sessions/2026/04/08/rollout-conv-long.jsonl",
          resume_command: "codex resume conv-long",
          messages: [],
          file_changes: [],
        };
      }

      return [];
    });

    renderApp();

    fireEvent.click(await screen.findByTitle(longTitle));

    await waitFor(() => {
      const heading = screen.getByRole("heading", { level: 2 });
      expect(heading.textContent).not.toBe(longTitle);
      expect(heading.textContent?.endsWith("...")).toBe(true);
      expect(heading.getAttribute("title")).toBe(longTitle);
    });
  });
});
