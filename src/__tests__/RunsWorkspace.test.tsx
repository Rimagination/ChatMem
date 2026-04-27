import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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

describe("Runs workspace", () => {
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
            created_at: "2026-04-20T10:00:00Z",
            updated_at: "2026-04-20T10:30:00Z",
            summary: "Runs timeline",
            message_count: 4,
            file_count: 3,
          },
        ];
      }

      if (command === "read_conversation") {
        return {
          id: "conv-001",
          source_agent: payload?.agent ?? "claude",
          project_dir: "D:/VSP/agentswap-gui",
          created_at: "2026-04-20T10:00:00Z",
          updated_at: "2026-04-20T10:30:00Z",
          summary: "Runs timeline",
          storage_path: "C:/Users/demo/.claude/projects/conv-001.jsonl",
          resume_command: "claude --resume conv-001",
          messages: [],
          file_changes: [],
        };
      }

      if (command === "list_repo_memories" || command === "list_memory_candidates") {
        return [];
      }

      if (command === "list_runs") {
        return [
          {
            run_id: "run-001",
            repo_root: "D:/VSP/agentswap-gui",
            source_agent: "codex",
            task_hint: "Build the runs panel",
            status: "waiting_for_review",
            summary: "Waiting for approval",
            started_at: "2026-04-20T10:00:00Z",
            ended_at: null,
            artifact_count: 2,
          },
        ];
      }

      return [];
    });
  });

  it("keeps run history outside the simplified conversation workspace", async () => {
    renderApp();

    fireEvent.click((await screen.findAllByText("Runs timeline"))[0]);

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Runs timeline" })).toBeTruthy();
      expect(screen.getByRole("button", { name: "Migrate" })).toBeTruthy();
      expect(screen.getByText("Conversation file location")).toBeTruthy();
    });

    expect(screen.queryByRole("button", { name: "History" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Outputs" })).toBeNull();
    expect(mockInvoke).not.toHaveBeenCalledWith("list_runs", expect.anything());
  });
});
