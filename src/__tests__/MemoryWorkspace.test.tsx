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

describe("Memory workspace", () => {
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
            created_at: "2026-04-19T08:00:00Z",
            updated_at: "2026-04-19T09:00:00Z",
            summary: "Memory workflow",
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
          created_at: "2026-04-19T08:00:00Z",
          updated_at: "2026-04-19T09:00:00Z",
          summary: "Memory workflow",
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
            title: "Primary verification",
            value: "npm run test:run",
            usage_hint: "Use before handoff",
            status: "active",
            last_verified_at: "2026-04-19T09:00:00Z",
            selected_because: null,
            evidence_refs: [],
          },
        ];
      }

      if (command === "list_memory_candidates") {
        return [
          {
            candidate_id: "cand-001",
            kind: "gotcha",
            summary: "Review pending memory",
            value: "Do not auto-approve candidate writes",
            why_it_matters: "Human review is required",
            confidence: 0.91,
            proposed_by: "codex",
            status: "pending_review",
            created_at: "2026-04-19T09:00:00Z",
            evidence_refs: [],
          },
        ];
      }

      if (command === "list_wiki_pages" || command === "rebuild_repo_wiki") {
        return [
          {
            page_id: "wiki:commands",
            repo_root: "D:/VSP/agentswap-gui",
            slug: "commands",
            title: "Commands",
            body: "# Commands\n\n- npm run test:run",
            status: "fresh",
            source_memory_ids: ["mem-001"],
            source_episode_ids: [],
            last_built_at: "2026-04-19T09:00:00Z",
            last_verified_at: "2026-04-19T09:00:00Z",
            updated_at: "2026-04-19T09:00:00Z",
          },
        ];
      }

      if (command === "review_memory_candidate") {
        return null;
      }

      return [];
    });
  });

  it("surfaces project memory and memory candidates beside the selected conversation", async () => {
    renderApp();

    fireEvent.click((await screen.findAllByText("Memory workflow"))[0]);

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Memory workflow" })).toBeTruthy();
      expect(screen.getByRole("heading", { name: "Project Memory" })).toBeTruthy();
      expect(screen.getByText("Primary verification")).toBeTruthy();
      expect(screen.getByText("npm run test:run")).toBeTruthy();
      expect(screen.getByRole("heading", { name: "Memory Candidates" })).toBeTruthy();
      expect(screen.getByText("Review pending memory")).toBeTruthy();
      expect(screen.getByRole("heading", { name: "Project Wiki" })).toBeTruthy();
      expect(screen.getByText("Commands")).toBeTruthy();
      expect(screen.getByText("Source of truth")).toBeTruthy();
      expect(screen.getByText("Generated projection")).toBeTruthy();
      expect(screen.getByText("Needs review")).toBeTruthy();
      expect(screen.getByText("1 memory source")).toBeTruthy();
    });

    expect(screen.queryByRole("button", { name: "Memory Inbox" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Repo Memory" })).toBeNull();
  });

  it("reviews a pending memory candidate from the side panel", async () => {
    renderApp();

    fireEvent.click((await screen.findAllByText("Memory workflow"))[0]);
    expect(await screen.findByText("Review pending memory")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Confirm" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("review_memory_candidate", {
        candidateId: "cand-001",
        action: "approve",
        editedTitle: "Review pending memory",
        editedUsageHint: "Human review is required",
      });
    });
  });
});
