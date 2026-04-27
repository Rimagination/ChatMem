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

async function openLocalHistoryView() {
  fireEvent.click(await screen.findByRole("tab", { name: "Local history" }));
}

describe("Checkpoints workspace", () => {
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
            summary: "Checkpoint flow",
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
          created_at: "2026-04-20T10:00:00Z",
          updated_at: "2026-04-20T10:30:00Z",
          summary: "Checkpoint flow",
          storage_path: "C:/Users/demo/.claude/projects/conv-001.jsonl",
          resume_command: "claude --resume conv-001",
          messages: [],
          file_changes: [],
        };
      }

      if (command === "list_checkpoints") {
        return [
          {
            checkpoint_id: "cp-001",
            repo_root: "D:/VSP/agentswap-gui",
            conversation_id: "conv-001",
            source_agent: "claude",
            status: "active",
            summary: "Checkpoint keeps the current debugging state",
            resume_command: "claude --resume conv-001",
            metadata_json: "{}",
            handoff_id: null,
            created_at: "2026-04-20T10:35:00Z",
          },
        ];
      }

      if (command === "list_repo_memories" || command === "list_memory_candidates") {
        return [];
      }

      return [];
    });
  });

  it("loads checkpoints into the project context continuation tab", async () => {
    renderApp();

    fireEvent.click((await screen.findAllByText("Checkpoint flow"))[0]);

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Checkpoint flow" })).toBeTruthy();
      expect(screen.getByRole("button", { name: "Migrate" })).toBeTruthy();
    });

    expect(screen.queryByRole("button", { name: "Checkpoints" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Promote to Handoff" })).toBeNull();
    expect(mockInvoke.mock.calls.some(([command]) => command === "list_checkpoints")).toBe(true);

    await openLocalHistoryView();
    fireEvent.click(screen.getByRole("button", { name: "Manage Rules" }));
    const drawer = await screen.findByRole("complementary", { name: "Startup Rules" });
    expect(drawer).toBeTruthy();

    fireEvent.click(screen.getByRole("tab", { name: "Continue 1" }));
    expect(within(drawer).getByText("Checkpoint keeps the current debugging state")).toBeTruthy();
    expect(within(drawer).getByText("claude --resume conv-001")).toBeTruthy();
  });
});
