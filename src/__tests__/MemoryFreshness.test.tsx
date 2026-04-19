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

function renderApp() {
  return render(
    <I18nProvider>
      <App />
    </I18nProvider>,
  );
}

describe("Memory freshness", () => {
  beforeEach(() => {
    mockInvoke.mockReset();
    localStorage.clear();

    let repoMemoryReads = 0;

    mockInvoke.mockImplementation(async (command: string, payload?: Record<string, unknown>) => {
      if (command === "list_conversations") {
        return [
          {
            id: "conv-001",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/VSP/agentswap-gui",
            created_at: "2026-04-19T08:00:00Z",
            updated_at: "2026-04-19T09:00:00Z",
            summary: "Freshness workflow",
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
          summary: "Freshness workflow",
          storage_path: "C:/Users/demo/.claude/projects/conv-001.jsonl",
          resume_command: "claude --resume conv-001",
          messages: [],
          file_changes: [],
        };
      }

      if (command === "list_repo_memories") {
        repoMemoryReads += 1;
        return [
          {
            memory_id: "mem-stale",
            kind: "command",
            title: "Stale verification command",
            value: "npm run test:run",
            usage_hint: "Use before merge",
            status: "active",
            last_verified_at: repoMemoryReads > 1 ? "2026-04-20T08:30:00Z" : "2026-04-10T08:30:00Z",
            freshness_status: repoMemoryReads > 1 ? "fresh" : "stale",
            freshness_score: repoMemoryReads > 1 ? 0.98 : 0.26,
            verified_at: repoMemoryReads > 1 ? "2026-04-20T08:30:00Z" : "2026-04-10T08:30:00Z",
            verified_by: repoMemoryReads > 1 ? "claude" : "codex",
            selected_because: null,
            evidence_refs: [{ excerpt: "Verified against the current test workflow." }],
          },
        ];
      }

      if (command === "reverify_memory") {
        return null;
      }

      if (command === "list_memory_candidates") {
        return [];
      }

      return [];
    });
  });

  it("shows freshness state and lets the reviewer re-verify a stale memory", async () => {
    renderApp();

    fireEvent.click(await screen.findByText("Freshness workflow"));
    fireEvent.click(await screen.findByRole("button", { name: "Repo Memory" }));

    expect(await screen.findByText("Stale verification command")).toBeTruthy();
    expect(screen.getByText("stale")).toBeTruthy();
    expect(screen.getByText(/last verified/i)).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Re-verify" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("reverify_memory", {
        memoryId: "mem-stale",
        verifiedBy: "claude",
      });
    });

    await waitFor(() => {
      expect(screen.getByText("fresh")).toBeTruthy();
    });
  });

  it("treats unknown freshness separately from stale in approvals", async () => {
    mockInvoke.mockImplementation(async (command: string, payload?: Record<string, unknown>) => {
      if (command === "list_conversations") {
        return [
          {
            id: "conv-001",
            source_agent: payload?.agent ?? "claude",
            project_dir: "D:/VSP/agentswap-gui",
            created_at: "2026-04-19T08:00:00Z",
            updated_at: "2026-04-19T09:00:00Z",
            summary: "Freshness workflow",
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
          summary: "Freshness workflow",
          storage_path: "C:/Users/demo/.claude/projects/conv-001.jsonl",
          resume_command: "claude --resume conv-001",
          messages: [],
          file_changes: [],
        };
      }

      if (command === "list_repo_memories") {
        return [
          {
            memory_id: "mem-unknown",
            kind: "command",
            title: "Unknown verification command",
            value: "npm run docs",
            usage_hint: "Use when docs change",
            status: "active",
            last_verified_at: null,
            freshness_status: "unknown",
            freshness_score: 0,
            verified_at: null,
            verified_by: null,
            selected_because: null,
            evidence_refs: [],
          },
          {
            memory_id: "mem-stale",
            kind: "command",
            title: "Stale verification command",
            value: "npm run test:run",
            usage_hint: "Use before merge",
            status: "active",
            last_verified_at: "2026-03-01T08:30:00Z",
            freshness_status: "stale",
            freshness_score: 0.2,
            verified_at: "2026-03-01T08:30:00Z",
            verified_by: "codex",
            selected_because: null,
            evidence_refs: [],
          },
        ];
      }

      if (command === "list_memory_candidates") {
        return [];
      }

      return null;
    });

    renderApp();

    fireEvent.click(await screen.findByText("Freshness workflow"));
    fireEvent.click(await screen.findByRole("button", { name: "Approvals" }));

    const staleCardLabel = await screen.findByText("Stale memories");
    const staleCard = staleCardLabel.closest("article");

    expect(staleCard).toBeTruthy();
    expect(within(staleCard as HTMLElement).getByText("1")).toBeTruthy();
    expect(screen.getByText("Stale verification command")).toBeTruthy();
    expect(screen.queryByText("Unknown verification command")).toBeNull();
  });
});
