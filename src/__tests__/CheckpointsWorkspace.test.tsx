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

function renderApp() {
  return render(
    <I18nProvider>
      <App />
    </I18nProvider>,
  );
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
          summary: "Currently selected conversation summary",
          storage_path: "C:/Users/demo/.claude/projects/conv-001.jsonl",
          resume_command: "claude --resume conv-001",
          messages: [],
          file_changes: [],
        };
      }

      if (command === "list_checkpoints") {
        return [
          {
            checkpoint_id: "checkpoint-001",
            repo_root: "D:/VSP/agentswap-gui",
            conversation_id: "codex:conv-777",
            source_agent: "codex",
            status: "active",
            summary: "Checkpoint-owned goal",
            resume_command: "codex resume conv-777",
            metadata_json: "{}",
            handoff_id: null,
            created_at: "2026-04-20T10:31:00Z",
          },
        ];
      }

      if (command === "create_handoff_packet") {
        return {
          handoff_id: "handoff-001",
          repo_root: "D:/VSP/agentswap-gui",
          from_agent: payload?.fromAgent ?? "codex",
          to_agent: payload?.toAgent ?? "claude",
          status: "draft",
          checkpoint_id: payload?.checkpointId ?? "checkpoint-001",
          target_profile: payload?.targetProfile ?? "claude_contextual",
          compression_strategy: null,
          current_goal: payload?.goalHint ?? "Checkpoint-owned goal",
          done_items: ["Checkpoint frozen from codex: Checkpoint-owned goal"],
          next_items: ["Checkpoint-owned goal"],
          key_files: [],
          useful_commands: ["codex resume conv-777"],
          related_memories: [],
          related_episodes: [],
          consumed_at: null,
          consumed_by: null,
          created_at: "2026-04-20T10:35:00Z",
        };
      }

      return [];
    });
  });

  it("renders resume and promote-to-handoff actions for an active checkpoint", async () => {
    renderApp();

    fireEvent.click(await screen.findByText("Checkpoint flow"));
    fireEvent.click(await screen.findByRole("button", { name: "Checkpoints" }));

    expect(await screen.findByText("Checkpoint-owned goal")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Resume" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Promote to Handoff" })).toBeTruthy();
  });

  it("promotes using the checkpoint's own provenance, goal, and valid targets", async () => {
    renderApp();

    fireEvent.click(await screen.findByText("Checkpoint flow"));
    fireEvent.click(await screen.findByRole("button", { name: "Checkpoints" }));

    const targetAgent = await screen.findByLabelText("Target agent");
    const options = Array.from(targetAgent.querySelectorAll("option")).map((option) => option.value);

    expect(options).toEqual(["claude", "gemini"]);

    fireEvent.change(targetAgent, { target: { value: "claude" } });
    fireEvent.click(screen.getByRole("button", { name: "Promote to Handoff" }));
    fireEvent.click(await screen.findByRole("button", { name: "Create handoff" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("create_handoff_packet", {
        repoRoot: "D:/VSP/agentswap-gui",
        fromAgent: "codex",
        toAgent: "claude",
        goalHint: "Checkpoint-owned goal",
        targetProfile: "claude_contextual",
        checkpointId: "checkpoint-001",
      });
    });
  });
});
