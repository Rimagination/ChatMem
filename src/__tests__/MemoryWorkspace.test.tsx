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

describe("Memory workspace", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    localStorage.clear();

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

      if (command === "review_memory_candidate") {
        return null;
      }

      return [];
    });
  });

  it("loads repository memory when switching to the repo memory tab", async () => {
    renderApp();

    fireEvent.click(await screen.findByText("Memory workflow"));
    fireEvent.click(await screen.findByRole("button", { name: "Repo Memory" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("list_repo_memories", {
        repoRoot: "D:/VSP/agentswap-gui",
      });
      expect(screen.getByText("Primary verification")).toBeTruthy();
      expect(screen.getByText("npm run test:run")).toBeTruthy();
    });
  });

  it("reviews a pending memory candidate from the inbox", async () => {
    renderApp();

    fireEvent.click(await screen.findByText("Memory workflow"));
    fireEvent.click(await screen.findByRole("button", { name: "Memory Inbox" }));

    expect(await screen.findByText("Review pending memory")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Approve" }));

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
