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
            merge_suggestion: {
              candidate_id: "cand-001",
              memory_id: "mem-001",
              memory_title: "Primary verification",
              reason: "This candidate overlaps an approved memory and likely needs a merge-aware review.",
              proposed_title: "Primary verification",
              proposed_value: "npm run test:run\n\nUpdate: Do not auto-approve candidate writes",
              proposed_usage_hint: "Use before handoff\n\nUpdate: Human review is required",
              risk_note: "Review before approval: this proposal rewrites an existing approved memory.",
              proposed_by: "codex",
            },
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

      if (command === "get_repo_memory_health") {
        return {
          repo_root: "D:/VSP/agentswap-gui",
          canonical_repo_root: "D:/VSP/agentswap-gui",
          approved_memory_count: 1,
          pending_candidate_count: 1,
          search_document_count: 4,
          indexed_chunk_count: 8,
          inherited_repo_roots: [],
          conversation_counts_by_agent: [
            { source_agent: "claude", conversation_count: 1 },
          ],
          repo_aliases: [],
          warnings: [],
        };
      }

      if (command === "scan_repo_conversations") {
        return {
          repo_root: "D:/VSP/agentswap-gui",
          scanned_conversation_count: 1,
          updated_embedding_count: 1,
          repo_diagnostics: {
            repo_root: "D:/VSP/agentswap-gui",
            canonical_repo_root: "D:/VSP/agentswap-gui",
            approved_memory_count: 1,
            pending_candidate_count: 1,
            search_document_count: 4,
            indexed_chunk_count: 8,
            inherited_repo_roots: [],
            conversation_counts_by_agent: [
              { source_agent: "claude", conversation_count: 1 },
            ],
            repo_aliases: [],
            warnings: [],
          },
        };
      }

      if (command === "review_memory_candidate") {
        return null;
      }

      return [];
    });
  });

  it("keeps memory in a drawer with an inbox-style notification", async () => {
    renderApp();

    fireEvent.click((await screen.findAllByText("Memory workflow"))[0]);

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Memory workflow" })).toBeTruthy();
      expect(screen.getByRole("button", { name: "Memory 1" })).toBeTruthy();
    });

    expect(screen.queryByRole("complementary", { name: "Project Memory" })).toBeNull();
    expect(screen.queryByText("Primary verification")).toBeNull();
    expect(screen.queryByText("Review pending memory")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Memory 1" }));

    expect(await screen.findByRole("complementary", { name: "Project Memory" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Inbox 1" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Approved 1" })).toBeTruthy();
    expect(screen.getByRole("tab", { name: "Wiki 1" })).toBeTruthy();
    expect(screen.getByText("Review pending memory")).toBeTruthy();

    fireEvent.click(screen.getByRole("tab", { name: "Approved 1" }));
    expect(screen.getByText("Primary verification")).toBeTruthy();

    fireEvent.click(screen.getByRole("tab", { name: "Wiki 1" }));
    expect(screen.getByText("Commands")).toBeTruthy();
  });

  it("reviews a pending memory candidate from the drawer", async () => {
    renderApp();

    fireEvent.click((await screen.findAllByText("Memory workflow"))[0]);
    fireEvent.click(await screen.findByRole("button", { name: "Memory 1" }));
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

  it("approves a merge proposal from the memory drawer", async () => {
    renderApp();

    fireEvent.click((await screen.findAllByText("Memory workflow"))[0]);
    fireEvent.click(await screen.findByRole("button", { name: "Memory 1" }));
    expect(await screen.findByText("Suggested rewrite")).toBeTruthy();
    expect(screen.getByText("Merge proposed by codex")).toBeTruthy();
    expect(screen.getByText(/npm run test:run/)).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Approve merge" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("review_memory_candidate", {
        candidateId: "cand-001",
        action: "approve_merge",
        mergeMemoryId: "mem-001",
        editedTitle: "Primary verification",
        editedValue: "npm run test:run\n\nUpdate: Do not auto-approve candidate writes",
        editedUsageHint: "Use before handoff\n\nUpdate: Human review is required",
      });
    });
  });

  it("shows local history status and rescans the active repo", async () => {
    renderApp();

    fireEvent.click((await screen.findAllByText("Memory workflow"))[0]);

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("get_repo_memory_health", {
        repoRoot: "D:/VSP/agentswap-gui",
      });
    });

    expect(await screen.findByText("Local history")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Rescan local history" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("scan_repo_conversations", {
        repoRoot: "D:/VSP/agentswap-gui",
      });
    });
  });
});
