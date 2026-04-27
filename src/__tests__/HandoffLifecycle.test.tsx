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

async function openLocalHistoryView() {
  fireEvent.click(await screen.findByRole("tab", { name: "Local history" }));
}

describe("Handoff lifecycle", () => {
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

      if (command === "list_handoffs") {
        return [
          {
            handoff_id: "handoff-001",
            repo_root: "D:/VSP/demo",
            from_agent: "claude",
            to_agent: "codex",
            status: "published",
            checkpoint_id: null,
            target_profile: "codex_compact",
            compression_strategy: null,
            current_goal: "Continue the debug session",
            done_items: ["Captured the failing path"],
            next_items: ["Run targeted tests"],
            key_files: ["src/App.tsx"],
            useful_commands: ["npm run test:run"],
            related_memories: [],
            related_episodes: [],
            consumed_at: null,
            consumed_by: null,
            created_at: "2026-04-08T09:05:00Z",
          },
        ];
      }

      if (command === "list_repo_memories" || command === "list_memory_candidates") {
        return [];
      }

      return [];
    });
  });

  it("loads handoffs into the project context continuation tab without restoring old pages", async () => {
    renderApp();

    fireEvent.click((await screen.findAllByText("Debug session"))[0]);

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Debug session" })).toBeTruthy();
      expect(screen.getByRole("button", { name: "Migrate" })).toBeTruthy();
    });

    expect(screen.queryByRole("button", { name: "Handoffs" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Create handoff to codex" })).toBeNull();
    expect(mockInvoke.mock.calls.some(([command]) => command === "list_handoffs")).toBe(true);

    await openLocalHistoryView();
    fireEvent.click(screen.getByRole("button", { name: "Manage Rules" }));
    expect(await screen.findByRole("complementary", { name: "Startup Rules" })).toBeTruthy();

    fireEvent.click(screen.getByRole("tab", { name: "Continue 1" }));
    expect(screen.getByText("Continue the debug session")).toBeTruthy();
    expect(screen.getByText("claude -> codex")).toBeTruthy();
    expect(screen.getByText("Run targeted tests")).toBeTruthy();
  });
});
