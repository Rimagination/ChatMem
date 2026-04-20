import { fireEvent, render, screen, waitFor } from "@testing-library/react";
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

function buildConversation(agent = "claude") {
  return {
    id: "conv-001",
    source_agent: agent,
    project_dir: "D:/VSP/demo",
    created_at: "2026-04-08T08:00:00Z",
    updated_at: "2026-04-08T09:00:00Z",
    summary: "Debug session",
    message_count: 2,
    file_count: 1,
  };
}

function buildConversationDetail(agent = "claude") {
  return {
    ...buildConversation(agent),
    storage_path: "C:/Users/demo/.codex/sessions/2026/04/08/rollout-conv-001.jsonl",
    resume_command: "codex resume conv-001",
    messages: [],
    file_changes: [],
  };
}

describe("Handoff lifecycle", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    mockCheckUpdate.mockReset();
    mockInstallUpdate.mockReset();
    mockRelaunch.mockReset();
    localStorage.clear();
    localStorage.setItem(
      "chatmem.settings",
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    mockCheckUpdate.mockResolvedValue({ shouldUpdate: false });
    mockInstallUpdate.mockResolvedValue(undefined);
    mockRelaunch.mockResolvedValue(undefined);
  });

  it("renders target profiles and lets the current agent mark a handoff as consumed", async () => {
    mockInvoke.mockImplementation(async (command: string, payload?: Record<string, unknown>) => {
      if (command === "list_conversations") {
        return [buildConversation(payload?.agent as string | undefined)];
      }

      if (command === "read_conversation") {
        return buildConversationDetail(payload?.agent as string | undefined);
      }

      if (command === "list_handoffs") {
        return [
          {
            handoff_id: "handoff-001",
            repo_root: "D:/VSP/demo",
            from_agent: "codex",
            to_agent: "claude",
            status: "ready",
            checkpoint_id: null,
            target_profile: "claude_contextual",
            compression_strategy: "balanced",
            current_goal: "Finish lifecycle wiring",
            done_items: ["Added lifecycle schema"],
            next_items: ["Hook up the consume action"],
            key_files: ["src/App.tsx"],
            useful_commands: ["npm.cmd run test:run -- src/__tests__/HandoffLifecycle.test.tsx"],
            related_memories: [],
            related_episodes: [],
            consumed_at: null,
            consumed_by: null,
            created_at: "2026-04-20T10:00:00Z",
          },
          {
            handoff_id: "handoff-002",
            repo_root: "D:/VSP/demo",
            from_agent: "claude",
            to_agent: "codex",
            status: "ready",
            checkpoint_id: null,
            target_profile: "codex_execution",
            compression_strategy: "balanced",
            current_goal: "Wrong agent should not consume this",
            done_items: ["Prepared packet"],
            next_items: ["Hand off to codex"],
            key_files: [],
            useful_commands: [],
            related_memories: [],
            related_episodes: [],
            consumed_at: null,
            consumed_by: null,
            created_at: "2026-04-20T10:01:00Z",
          },
        ];
      }

      if (command === "mark_handoff_consumed") {
        return null;
      }

      return [];
    });

    renderApp();

    fireEvent.click(await screen.findByText("Debug session"));
    fireEvent.click(await screen.findByRole("button", { name: "Handoffs" }));

    expect(await screen.findByText("claude_contextual")).toBeTruthy();
    expect(screen.getAllByText("ready")).toHaveLength(2);
    expect(screen.getAllByRole("button", { name: "Mark as Consumed" })).toHaveLength(1);
    expect(screen.getByText("Wrong agent should not consume this")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Mark as Consumed" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("mark_handoff_consumed", {
        handoffId: "handoff-001",
        consumedBy: "claude",
      });
    });

    expect(await screen.findByText("consumed")).toBeTruthy();
    expect(screen.getByText("Consumed by claude")).toBeTruthy();
  });

  it("hides the consume action when the selected agent does not match the handoff target", async () => {
    mockInvoke.mockImplementation(async (command: string, payload?: Record<string, unknown>) => {
      if (command === "list_conversations") {
        return [buildConversation(payload?.agent as string | undefined)];
      }

      if (command === "read_conversation") {
        return buildConversationDetail(payload?.agent as string | undefined);
      }

      if (command === "list_handoffs") {
        return [
          {
            handoff_id: "handoff-003",
            repo_root: "D:/VSP/demo",
            from_agent: "claude",
            to_agent: "codex",
            status: "ready",
            checkpoint_id: null,
            target_profile: "codex_execution",
            compression_strategy: "balanced",
            current_goal: "Wait for codex",
            done_items: ["Prepared packet"],
            next_items: ["Hand off to codex"],
            key_files: [],
            useful_commands: [],
            related_memories: [],
            related_episodes: [],
            consumed_at: null,
            consumed_by: null,
            created_at: "2026-04-20T10:02:00Z",
          },
        ];
      }

      return [];
    });

    renderApp();

    fireEvent.click(await screen.findByText("Debug session"));
    fireEvent.click(await screen.findByRole("button", { name: "Handoffs" }));

    expect(await screen.findByText("codex_execution")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Mark as Consumed" })).toBeNull();
    expect(screen.getByText("Awaiting consumption")).toBeTruthy();
  });

  it("creates a handoff with the selected target profile", async () => {
    mockInvoke.mockImplementation(async (command: string, payload?: Record<string, unknown>) => {
      if (command === "list_conversations") {
        return [buildConversation(payload?.agent as string | undefined)];
      }

      if (command === "read_conversation") {
        return buildConversationDetail(payload?.agent as string | undefined);
      }

      if (command === "list_handoffs") {
        return [];
      }

      if (command === "create_handoff_packet") {
        return {
          handoff_id: "handoff-002",
          repo_root: "D:/VSP/demo",
          from_agent: "claude",
          to_agent: "codex",
          status: "ready",
          checkpoint_id: null,
          target_profile: payload?.targetProfile ?? null,
          compression_strategy: "balanced",
          current_goal: "Debug session",
          done_items: ["Summarized current state"],
          next_items: ["Continue from the handoff"],
          key_files: [],
          useful_commands: [],
          related_memories: [],
          related_episodes: [],
          consumed_at: null,
          consumed_by: null,
          created_at: "2026-04-20T10:05:00Z",
        };
      }

      return [];
    });

    renderApp();

    fireEvent.click(await screen.findByText("Debug session"));
    fireEvent.click(await screen.findByRole("button", { name: "Handoffs" }));
    fireEvent.click(await screen.findByRole("button", { name: "Create handoff to codex" }));

    fireEvent.change(await screen.findByLabelText("Target profile"), {
      target: { value: "codex_execution" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Create handoff" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("create_handoff_packet", {
        repoRoot: "D:/VSP/demo",
        fromAgent: "claude",
        toAgent: "codex",
        goalHint: "Debug session",
        targetProfile: "codex_execution",
      });
    });

    expect(await screen.findByText("codex_execution")).toBeTruthy();
  });
});
