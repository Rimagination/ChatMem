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

  it("explains pending candidates as non-blocking startup memory review in English", () => {
    render(
      <ProjectIndexStatus
        health={{
          ...healthFixture,
          approved_memory_count: 3,
          pending_candidate_count: 10,
          search_document_count: 647,
          indexed_chunk_count: 647,
          conversation_counts_by_agent: [
            {
              source_agent: "claude",
              conversation_count: 78,
            },
          ],
          warnings: [
            "10 pending memory candidate(s) need review before they become startup memory.",
          ],
        }}
        loading={false}
        scanning={false}
        locale="en"
        onScan={vi.fn()}
      />,
    );

    expect(screen.getByText("Review queue")).toBeTruthy();
    expect(screen.getByText("Startup memory")).toBeTruthy();
    expect(screen.getByText("Note")).toBeTruthy();
    expect(
      screen.getByText((content) =>
        content.includes("10 candidate memories are waiting for review") &&
        content.includes("indexed conversations remain searchable"),
      ),
    ).toBeTruthy();
    expect(screen.queryByText(/pending memory candidate/i)).toBeNull();
  });

  it("explains pending candidates as non-blocking startup memory review in Chinese", () => {
    render(
      <ProjectIndexStatus
        health={{
          ...healthFixture,
          approved_memory_count: 3,
          pending_candidate_count: 10,
          search_document_count: 647,
          indexed_chunk_count: 647,
          conversation_counts_by_agent: [
            {
              source_agent: "claude",
              conversation_count: 78,
            },
          ],
          warnings: [
            "10 pending memory candidate(s) need review before they become startup memory.",
          ],
        }}
        loading={false}
        scanning={false}
        locale="zh-CN"
        onScan={vi.fn()}
      />,
    );

    expect(screen.getByText("\u5f85\u786e\u8ba4\u5019\u9009")).toBeTruthy();
    expect(screen.getByText("\u542f\u52a8\u8bb0\u5fc6")).toBeTruthy();
    expect(screen.getByText("\u63d0\u793a")).toBeTruthy();
    expect(
      screen.getByText((content) =>
        content.includes("\u6709 10 \u6761\u5019\u9009\u8bb0\u5fc6\u7b49\u5f85\u786e\u8ba4") &&
        content.includes("78 \u6bb5\u5bf9\u8bdd\u548c 647 \u4e2a\u5206\u5757\u5df2\u7ecf\u53ef\u7528\u4e8e\u56de\u5fc6"),
      ),
    ).toBeTruthy();
    expect(screen.queryByText(/pending memory candidate/i)).toBeNull();
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
