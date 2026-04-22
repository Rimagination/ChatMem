import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ProjectIndexStatus from "../components/ProjectIndexStatus";
import type { RepoMemoryHealth } from "../chatmem-memory/types";

const healthFixture: RepoMemoryHealth = {
  repo_root: "D:/VSP/agentswap-gui",
  canonical_repo_root: "D:/VSP/agentswap-gui",
  approved_memory_count: 12,
  pending_candidate_count: 3,
  search_document_count: 31,
  indexed_chunk_count: 31,
  inherited_repo_roots: ["D:/VSP"],
  conversation_counts_by_agent: [
    {
      source_agent: "codex",
      conversation_count: 7,
    },
    {
      source_agent: "claude",
      conversation_count: 2,
    },
  ],
  repo_aliases: [
    {
      alias_root: "D:/VSP",
      alias_kind: "ancestor",
      confidence: 0.94,
    },
  ],
  warnings: ["Ancestor repo alias detected and merged into current index."],
};

describe("ProjectIndexStatus", () => {
  it("shows local history metrics and triggers a rescan", () => {
    const onScan = vi.fn();

    render(
      <ProjectIndexStatus
        health={healthFixture}
        loading={false}
        scanning={false}
        locale="en"
        onScan={onScan}
      />,
    );

    expect(screen.getByText("Local history")).toBeTruthy();
    expect(screen.getByText("9")).toBeTruthy();
    expect(screen.getByText("31")).toBeTruthy();
    expect(screen.getByText("3")).toBeTruthy();
    expect(screen.getByText(/ancestor repo/i)).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Rescan local history" }));
    expect(onScan).toHaveBeenCalledTimes(1);
  });
});
