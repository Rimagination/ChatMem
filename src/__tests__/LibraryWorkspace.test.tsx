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
    startDragging: vi.fn(),
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

describe("Library workspace", () => {
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
            summary: "Library workspace",
            message_count: 4,
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
          summary: "Library workspace",
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
            title: "Test command memory",
            value: "npm run test:run",
            usage_hint: "Use before packaging",
            status: "active",
            last_verified_at: "2026-04-20T10:00:00Z",
            freshness_status: "fresh",
            freshness_score: 1,
            verified_at: "2026-04-20T10:00:00Z",
            verified_by: "claude",
            selected_because: null,
            evidence_refs: [],
          },
        ];
      }

      if (command === "list_checkpoints") {
        return [
          {
            checkpoint_id: "cp-001",
            repo_root: "D:/VSP/agentswap-gui",
            conversation_id: "conv-001",
            source_agent: "codex",
            status: "active",
            summary: "Checkpoint for review",
            resume_command: "codex resume conv-001",
            metadata_json: "{}",
            handoff_id: null,
            created_at: "2026-04-20T11:00:00Z",
          },
        ];
      }

      if (command === "list_handoffs") {
        return [
          {
            handoff_id: "handoff-001",
            repo_root: "D:/VSP/agentswap-gui",
            from_agent: "claude",
            to_agent: "codex",
            status: "published",
            checkpoint_id: "cp-001",
            target_profile: "codex_compact",
            compression_strategy: null,
            current_goal: "Publish handoff",
            done_items: [],
            next_items: ["Verify release build"],
            key_files: [],
            useful_commands: [],
            related_memories: [],
            related_episodes: [],
            consumed_at: null,
            consumed_by: null,
            created_at: "2026-04-20T12:00:00Z",
          },
        ];
      }

      if (command === "list_runs") {
        return [
          {
            run_id: "run-001",
            repo_root: "D:/VSP/agentswap-gui",
            source_agent: "codex",
            task_hint: "Build library panel",
            status: "waiting_for_review",
            summary: "Waiting for approval",
            started_at: "2026-04-20T12:30:00Z",
            ended_at: null,
            artifact_count: 1,
          },
        ];
      }

      if (command === "list_artifacts") {
        return [
          {
            artifact_id: "artifact-001",
            run_id: "run-001",
            artifact_type: "summary",
            title: "Library digest",
            summary: "Unified repo library view",
            trust_state: "reviewed",
            created_at: "2026-04-20T13:00:00Z",
          },
        ];
      }

      if (command === "list_episodes") {
        return [
          {
            episode_id: "episode-001",
            title: "Earlier migration",
            summary: "Moved the handoff flow into ChatMem.",
            outcome: "completed",
            created_at: "2026-04-20T09:30:00Z",
            source_conversation_id: "conv-001",
            evidence_refs: [],
          },
        ];
      }

      if (command === "list_memory_candidates") {
        return [];
      }

      return [];
    });
  });

  it("surfaces project memory through the drawer", async () => {
    renderApp();

    fireEvent.click((await screen.findAllByText("Library workspace"))[0]);

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Library workspace" })).toBeTruthy();
      expect(screen.getByRole("button", { name: "Copy location" })).toBeTruthy();
      expect(screen.getByRole("button", { name: "Copy resume command" })).toBeTruthy();
      expect(screen.queryByRole("button", { name: "Manage Rules" })).toBeNull();
      expect(screen.getByRole("button", { name: "Migrate" })).toBeTruthy();
      expect(screen.queryByRole("button", { name: "History" })).toBeNull();
    });

    expect(screen.queryByRole("complementary", { name: "Startup Rules" })).toBeNull();
    expect(screen.queryByText("Test command memory")).toBeNull();

    await openLocalHistoryView();
    fireEvent.click(screen.getByRole("button", { name: "Manage Rules" }));

    expect(await screen.findByRole("complementary", { name: "Startup Rules" })).toBeTruthy();
    expect(screen.getByText("Test command memory")).toBeTruthy();
    expect(screen.getByText("Use before packaging")).toBeTruthy();
  });
});
