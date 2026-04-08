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

    mockInvoke.mockImplementation(async (command: string, payload?: Record<string, unknown>) => {
      if (command === "list_conversations") {
        return [
          {
            id: "conv-001",
            source_agent: payload?.agent ?? "codex",
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
          source_agent: payload?.agent ?? "codex",
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

      if (command === "search_conversations" && payload?.query === "内存泄漏") {
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

    fireEvent.click(screen.getByRole("button", { name: "打开设置" }));
    expect(screen.getByRole("heading", { name: "设置" })).toBeTruthy();

    fireEvent.change(screen.getByLabelText("语言 Language"), {
      target: { value: "en" },
    });

    expect(await screen.findByRole("heading", { name: "Settings" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Check for updates" })).toBeTruthy();
  });

  it("shows the up-to-date state after a manual check", async () => {
    mockCheckUpdate.mockResolvedValue({ shouldUpdate: false });

    renderApp();

    fireEvent.click(screen.getByRole("button", { name: "打开设置" }));
    fireEvent.click(screen.getByRole("button", { name: "检查更新" }));

    expect(await screen.findByText("当前已是最新版本")).toBeTruthy();
  });

  it("auto-checks for updates on launch when enabled", async () => {
    vi.useFakeTimers();
    mockCheckUpdate.mockResolvedValue({
      shouldUpdate: true,
      manifest: {
        version: "0.1.1",
        date: "2026-04-08T12:00:00Z",
        body: "Bug fixes",
      },
    });

    renderApp();

    await act(async () => {
      await vi.advanceTimersByTimeAsync(3600);
    });

    expect(mockCheckUpdate).toHaveBeenCalledTimes(1);
    expect(screen.getByText("发现新版本 0.1.1")).toBeTruthy();
  });

  it("renders file location and copy actions for the selected conversation", async () => {
    renderApp();

    expect(screen.getByText("本地对话记录，一处查看，随时续接")).toBeTruthy();
    expect(screen.getByRole("button", { name: "刷新会话列表" })).toBeTruthy();

    const conversation = await screen.findByText("Debug session");
    fireEvent.click(conversation);

    await waitFor(() => {
      expect(screen.getByText("对话文件位置")).toBeTruthy();
      expect(screen.getByText("操作")).toBeTruthy();
      expect(
        screen.getByText("C:/Users/demo/.codex/sessions/2026/04/08/rollout-conv-001.jsonl"),
      ).toBeTruthy();
      expect(screen.getByRole("button", { name: "复制位置" })).toBeTruthy();
      expect(screen.getByRole("button", { name: "复制恢复命令" })).toBeTruthy();
    });
  });

  it("searches conversations by message body content", async () => {
    renderApp();

    const input = await screen.findByPlaceholderText("搜索对话...");
    fireEvent.change(input, { target: { value: "内存泄漏" } });

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("search_conversations", {
        agent: "claude",
        query: "内存泄漏",
      });
      expect(screen.getByText("Memory investigation")).toBeTruthy();
    });
  });
});
