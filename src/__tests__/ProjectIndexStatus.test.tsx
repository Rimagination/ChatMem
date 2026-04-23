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

const emptyHealthFixture: RepoMemoryHealth = {
  repo_root: "D:/VSP/agentswap-gui",
  canonical_repo_root: "D:/VSP/agentswap-gui",
  approved_memory_count: 0,
  pending_candidate_count: 0,
  search_document_count: 0,
  indexed_chunk_count: 0,
  inherited_repo_roots: [],
  conversation_counts_by_agent: [],
  repo_aliases: [],
  warnings: [],
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
    expect(
      screen.queryByText(
        "Local history has not been indexed for this project yet, so older conversations may not be fully searchable. After indexing, you can ask what was discussed before.",
      ),
    ).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Rescan local history" }));
    expect(onScan).toHaveBeenCalledTimes(1);
  });

  it("shows the idle bootstrap note when indexed history is empty", () => {
    const onScan = vi.fn();

    render(
      <ProjectIndexStatus
        bootstrapReady={false}
        health={emptyHealthFixture}
        loading={false}
        scanning={false}
        locale="en"
        onScan={onScan}
      />,
    );

    expect(
      screen.getByText(
        "Local history has not been indexed for this project yet, so older conversations may not be fully searchable. After indexing, you can ask what was discussed before.",
      ),
    ).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Rescan local history" }));
    expect(onScan).toHaveBeenCalledTimes(1);
  });

  it("shows the scanning bootstrap note when indexed history is empty", () => {
    const onScan = vi.fn();

    render(
      <ProjectIndexStatus
        bootstrapReady={false}
        health={emptyHealthFixture}
        loading={false}
        scanning
        locale="en"
        onScan={onScan}
      />,
    );

    expect(
      screen.getByText(
        "Importing local history for this project. Older conversations may not be fully searchable yet. When indexing finishes, you can ask what was discussed before.",
      ),
    ).toBeTruthy();

    expect(screen.getByRole("button", { name: "Scanning..." }).hasAttribute("disabled")).toBe(true);
  });

  it("renders legacy repo health payloads without optional arrays", () => {
    const onScan = vi.fn();
    const legacyHealth = {
      repo_root: "D:/VSP/agentswap-gui",
      canonical_repo_root: "D:/VSP/agentswap-gui",
      approved_memory_count: 0,
      pending_candidate_count: 0,
      search_document_count: 0,
      indexed_chunk_count: 0,
      inherited_repo_roots: [],
      repo_aliases: [],
    } as Partial<RepoMemoryHealth>;

    render(
      <ProjectIndexStatus
        bootstrapReady={false}
        health={legacyHealth as RepoMemoryHealth}
        loading={false}
        scanning={false}
        locale="en"
        onScan={onScan}
      />,
    );

    const conversationsLabel = screen.getByText("Conversations");
    expect(conversationsLabel.parentElement?.querySelector(".meta-value")?.textContent).toBe("0");
    expect(screen.queryByText("Warnings")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Rescan local history" }));
    expect(onScan).toHaveBeenCalledTimes(1);
  });

  it("shows the ready notice when automatic bootstrap just completed", () => {
    render(
      <ProjectIndexStatus
        bootstrapReady
        health={healthFixture}
        loading={false}
        scanning={false}
        locale="en"
        onScan={vi.fn()}
      />,
    );

    expect(
      screen.getByText(
        "Local history is ready for this project. You can now ask what was discussed before.",
      ),
    ).toBeTruthy();
    expect(
      screen.queryByText(
        "Local history has not been indexed for this project yet, so older conversations may not be fully searchable. After indexing, you can ask what was discussed before.",
      ),
    ).toBeNull();
  });

  it("hides the ready notice while chunks are still zero even if bootstrapReady is true", () => {
    render(
      <ProjectIndexStatus
        bootstrapReady
        health={emptyHealthFixture}
        loading={false}
        scanning={false}
        locale="en"
        onScan={vi.fn()}
      />,
    );

    expect(
      screen.queryByText(
        "Local history is ready for this project. You can now ask what was discussed before.",
      ),
    ).toBeNull();
    expect(
      screen.getByText(
        "Local history has not been indexed for this project yet, so older conversations may not be fully searchable. After indexing, you can ask what was discussed before.",
      ),
    ).toBeTruthy();
  });

  it("keeps nonzero-chunk states free of idle and ready notes until bootstrapReady becomes true", () => {
    const { rerender } = render(
      <ProjectIndexStatus
        bootstrapReady={false}
        health={healthFixture}
        loading={false}
        scanning={false}
        locale="en"
        onScan={vi.fn()}
      />,
    );

    expect(
      screen.queryByText(
        "Local history has not been indexed for this project yet, so older conversations may not be fully searchable. After indexing, you can ask what was discussed before.",
      ),
    ).toBeNull();
    expect(
      screen.queryByText(
        "Local history is ready for this project. You can now ask what was discussed before.",
      ),
    ).toBeNull();

    rerender(
      <ProjectIndexStatus
        bootstrapReady
        health={healthFixture}
        loading={false}
        scanning={false}
        locale="en"
        onScan={vi.fn()}
      />,
    );

    expect(
      screen.getByText(
        "Local history is ready for this project. You can now ask what was discussed before.",
      ),
    ).toBeTruthy();
  });
});
