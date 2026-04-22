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

describe("Memory freshness", () => {
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
            memory_id: "mem-stale",
            kind: "command",
            title: "Stale verification command",
            value: "npm run test:run",
            usage_hint: "Use before merge",
            status: "active",
            last_verified_at: "2026-04-10T08:30:00Z",
            freshness_status: "stale",
            freshness_score: 0.26,
            verified_at: "2026-04-10T08:30:00Z",
            verified_by: "codex",
            selected_because: null,
            evidence_refs: [{ excerpt: "Verified against the current test workflow." }],
          },
        ];
      }

      if (command === "list_memory_candidates") {
        return [];
      }

      return [];
    });
  });

  it("shows repository memory in the drawer without a separate review page", async () => {
    renderApp();

    fireEvent.click((await screen.findAllByText("Freshness workflow"))[0]);

    await waitFor(() => {
      expect(screen.getByRole("heading", { name: "Freshness workflow" })).toBeTruthy();
      expect(screen.getByRole("button", { name: "Memory" })).toBeTruthy();
    });

    expect(screen.queryByRole("complementary", { name: "Project Memory" })).toBeNull();
    expect(screen.queryByText("Stale verification command")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Memory" }));

    expect(await screen.findByRole("complementary", { name: "Project Memory" })).toBeTruthy();
    expect(screen.getByText("Stale verification command")).toBeTruthy();
    expect(screen.getByText("Use before merge")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Needs Review" })).toBeNull();
    expect(screen.getByRole("button", { name: "Re-verify" })).toBeTruthy();
  });
});
