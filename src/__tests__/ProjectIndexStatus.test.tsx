import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ProjectIndexStatus from "../components/ProjectIndexStatus";
import type { LocalHistoryImportReport, RepoMemoryHealth } from "../chatmem-memory/types";

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

const importReportFixture: LocalHistoryImportReport = {
  scanned_conversation_count: 139,
  imported_conversation_count: 134,
  skipped_conversation_count: 5,
  indexed_repo_count: 4,
  source_agents: [{ source_agent: "codex", conversation_count: 134 }],
  imported_project_roots: [
    {
      source_agent: "codex",
      project_root: "D:/VSP",
      conversation_count: 134,
    },
  ],
  warnings: [],
  imported_at: "2026-04-25T12:00:00Z",
};

describe("ProjectIndexStatus", () => {
  it("shows local history metrics and triggers a rescan", () => {
    const onScan = vi.fn();
    const onOpenRules = vi.fn();

    render(
      <ProjectIndexStatus
        health={healthFixture}
        loading={false}
        scanning={false}
        locale="en"
        onScan={onScan}
        onOpenRules={onOpenRules}
      />,
    );

    expect(screen.getByText("Local history")).toBeTruthy();
    expect(screen.getByText("9")).toBeTruthy();
    expect(screen.getByText("31")).toBeTruthy();
    expect(screen.getByText("Needs review")).toBeTruthy();
    expect(screen.getByText("Startup rules")).toBeTruthy();
    expect(screen.getAllByText("3").length).toBeGreaterThanOrEqual(1);
    expect(screen.getByText(/ancestor repo/i)).toBeTruthy();
    expect(
      screen.queryByText(
        "Local history has not been indexed for this project yet, so older conversations may not be fully searchable. After indexing, you can ask what was discussed before.",
      ),
    ).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Rescan local history" }));
    expect(onScan).toHaveBeenCalledTimes(1);

    const rulesButton = screen.getByRole("button", { name: "Manage Rules" });
    fireEvent.click(rulesButton);
    expect(onOpenRules).toHaveBeenCalledTimes(1);
    expect(rulesButton.textContent).not.toContain("3");
    expect(screen.getByText("12")).toBeTruthy();
    expect(document.querySelectorAll(".project-index-metric")).toHaveLength(4);
    expect(document.querySelector(".project-index-grid")).toBeNull();
  });

  it("summarizes the latest full local-history import without adding metric cards", () => {
    render(
      <ProjectIndexStatus
        health={healthFixture}
        importReport={importReportFixture}
        loading={false}
        scanning={false}
        locale="zh-CN"
        onScan={vi.fn()}
      />,
    );

    expect(
      screen.getByText(
        "\u5168\u91cf\u5bfc\u5165\uff1a\u626b\u63cf 139 \u6761 / \u5bfc\u5165 134 \u6761 / \u8986\u76d6 4 \u4e2a\u9879\u76ee / \u8df3\u8fc7 5 \u6761",
      ),
    ).toBeTruthy();
    expect(document.querySelectorAll(".project-index-metric")).toHaveLength(4);
  });

  it("explains pending candidates as non-blocking startup rule review in English", () => {
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
            "10 pending memory candidate(s) need review before they become startup rules.",
          ],
        }}
        loading={false}
        scanning={false}
        locale="en"
        onScan={vi.fn()}
      />,
    );

    expect(screen.getByText("Needs review")).toBeTruthy();
    expect(screen.getByText("Startup rules")).toBeTruthy();
    expect(screen.queryByText("Note")).toBeNull();
    expect(screen.queryByText("10 candidate startup rules are waiting for review.")).toBeNull();
    expect(
      screen.queryByText((content) =>
        content.includes("indexed conversations remain searchable"),
      ),
    ).toBeNull();
    expect(screen.queryByText(/pending memory candidate/i)).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "Why check local history first?" }));
    expect(
      screen.getByText((content) =>
        content.includes("Local history is the full conversation index") &&
        content.includes("Startup rules only keep stable rules"),
      ),
    ).toBeTruthy();
  });

  it("explains pending candidates as non-blocking startup rule review in Chinese", () => {
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
            "10 pending memory candidate(s) need review before they become startup rules.",
          ],
        }}
        loading={false}
        scanning={false}
        locale="zh-CN"
        onScan={vi.fn()}
      />,
    );

    expect(screen.getByText("\u5f85\u786e\u8ba4")).toBeTruthy();
    expect(screen.getByText("\u542f\u52a8\u89c4\u5219")).toBeTruthy();
    expect(screen.queryByText("\u63d0\u793a")).toBeNull();
    expect(screen.queryByText("\u6709 10 \u6761\u542f\u52a8\u89c4\u5219\u5019\u9009\u7b49\u5f85\u786e\u8ba4\u3002")).toBeNull();
    expect(
      screen.queryByText((content) =>
        content.includes("78 \u6bb5\u5bf9\u8bdd\u548c 647 \u4e2a\u5206\u5757\u5df2\u7ecf\u53ef\u7528\u4e8e\u56de\u5fc6"),
      ),
    ).toBeNull();
    expect(screen.queryByText(/pending memory candidate/i)).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "\u4e3a\u4ec0\u4e48\u5148\u67e5\u672c\u5730\u5386\u53f2\uff1f" }));
    expect(
      screen.getByText((content) =>
        content.includes("\u672c\u5730\u5386\u53f2\u662f\u5b8c\u6574\u5bf9\u8bdd\u7d22\u5f15") &&
        content.includes("\u542f\u52a8\u89c4\u5219\u53ea\u4fdd\u7559\u65b0\u4efb\u52a1\u5fc5\u987b\u5e26\u4e0a\u7684\u7a33\u5b9a\u89c4\u5219"),
      ),
    ).toBeTruthy();
  });

  it("keeps the Chinese local-history explanation behind a question button", () => {
    render(
      <ProjectIndexStatus
        health={{
          ...healthFixture,
          approved_memory_count: 4,
          pending_candidate_count: 38,
          search_document_count: 668,
          indexed_chunk_count: 668,
          conversation_counts_by_agent: [
            {
              source_agent: "codex",
              conversation_count: 139,
            },
          ],
          warnings: [
            "38 pending memory candidate(s) need review before they become startup rules.",
          ],
        }}
        loading={false}
        scanning={false}
        locale="zh-CN"
        onScan={vi.fn()}
        onOpenRules={vi.fn()}
      />,
    );

    expect(screen.getByText("\u5df2\u7d22\u5f15\u5bf9\u8bdd\u53ef\u76f4\u63a5\u7528\u4e8e\u56de\u5fc6\u3002")).toBeTruthy();
    expect(screen.getByText("\u5f85\u786e\u8ba4")).toBeTruthy();
    expect(screen.getByText("\u542f\u52a8\u89c4\u5219")).toBeTruthy();
    expect(screen.queryByText("\u6709 38 \u6761\u542f\u52a8\u89c4\u5219\u5019\u9009\u7b49\u5f85\u786e\u8ba4\u3002")).toBeNull();
    expect(screen.queryByText("\u63d0\u793a")).toBeNull();
    const rulesButton = screen.getByRole("button", { name: "\u7ba1\u7406\u89c4\u5219" });
    expect(rulesButton.textContent).not.toContain("38");
    expect(
      screen.queryByText((content) =>
        content.includes("\u5b83\u4eec\u4e0d\u4f1a\u963b\u6b62\u672c\u5730\u5386\u53f2\u68c0\u7d22") ||
        content.includes("\u542f\u52a8\u89c4\u5219\u53ea\u662f\u65b0\u4efb\u52a1\u8981\u5e26\u4e0a\u7684\u7b80\u77ed\u6307\u5f15"),
      ),
    ).toBeNull();

    const helpButton = screen.getByRole("button", { name: "\u4e3a\u4ec0\u4e48\u5148\u67e5\u672c\u5730\u5386\u53f2\uff1f" });
    expect(helpButton.getAttribute("aria-expanded")).toBe("false");

    fireEvent.click(helpButton);

    expect(helpButton.getAttribute("aria-expanded")).toBe("true");
    expect(
      screen.getByText((content) =>
        content.includes("\u672c\u5730\u5386\u53f2\u662f\u5b8c\u6574\u5bf9\u8bdd\u7d22\u5f15") &&
        content.includes("\u542f\u52a8\u89c4\u5219\u53ea\u4fdd\u7559\u65b0\u4efb\u52a1\u5fc5\u987b\u5e26\u4e0a\u7684\u7a33\u5b9a\u89c4\u5219"),
      ),
    ).toBeTruthy();
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

  it("explains a scanned-but-unmatched local history state in Chinese", () => {
    const onMergeAlias = vi.fn();

    render(
      <ProjectIndexStatus
        bootstrapReady={false}
        health={{
          ...emptyHealthFixture,
          latest_scan: {
            repo_root: "D:/VSP/agentswap-gui",
            canonical_repo_root: "D:/VSP/agentswap-gui",
            scanned_conversation_count: 7,
            linked_conversation_count: 0,
            skipped_conversation_count: 7,
            source_agents: [{ source_agent: "codex", conversation_count: 7 }],
            unmatched_project_roots: [
              {
                source_agent: "codex",
                project_root: "d:/vsp/bm.md",
                conversation_count: 7,
              },
            ],
            warnings: [],
            scanned_at: "2026-04-25T12:00:00Z",
          },
        }}
        loading={false}
        scanning={false}
        locale="zh-CN"
        onScan={vi.fn()}
        onMergeAlias={onMergeAlias}
      />,
    );

    expect(
      screen.getByText(
        "\u6700\u8fd1\u626b\u63cf\u4e86 7 \u6761\u672c\u5730\u5bf9\u8bdd\uff0c\u4f46\u6ca1\u6709\u7eb3\u5165\u5f53\u524d\u9879\u76ee\u3002\u8bf7\u68c0\u67e5\u9879\u76ee\u8def\u5f84\u6216\u522b\u540d\u3002",
      ),
    ).toBeTruthy();
    expect(
      screen.getByText(
        "\u53ef\u80fd\u5339\u914d\u7684\u8def\u5f84\uff1ad:/vsp/bm.md\uff08codex 7 \u6761\uff09",
      ),
    ).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "\u5e76\u5165\u5f53\u524d\u9879\u76ee d:/vsp/bm.md" }));
    expect(onMergeAlias).toHaveBeenCalledWith("d:/vsp/bm.md");
  });

  it("keeps unmatched alias repair visible after some local history was imported", () => {
    const onMergeAlias = vi.fn();

    render(
      <ProjectIndexStatus
        bootstrapReady={false}
        health={{
          ...healthFixture,
          conversation_counts_by_agent: [{ source_agent: "codex", conversation_count: 134 }],
          indexed_chunk_count: 668,
          search_document_count: 668,
          latest_scan: {
            repo_root: "D:/VSP/agentswap-gui",
            canonical_repo_root: "D:/VSP/agentswap-gui",
            scanned_conversation_count: 139,
            linked_conversation_count: 134,
            skipped_conversation_count: 5,
            source_agents: [{ source_agent: "codex", conversation_count: 134 }],
            unmatched_project_roots: [
              {
                source_agent: "codex",
                project_root: "d:/vsp/easymd",
                conversation_count: 5,
              },
            ],
            warnings: [],
            scanned_at: "2026-04-25T12:00:00Z",
          },
        }}
        loading={false}
        scanning={false}
        locale="zh-CN"
        onScan={vi.fn()}
        onMergeAlias={onMergeAlias}
      />,
    );

    expect(
      screen.getByText(
        "\u5df2\u7eb3\u5165 134 \u6761\u672c\u5730\u5bf9\u8bdd\uff0c\u53e6\u6709 5 \u6761\u53ef\u80fd\u5c5e\u4e8e\u5f53\u524d\u9879\u76ee\uff0c\u4f46\u8def\u5f84\u8fd8\u6ca1\u5e76\u5165\u3002",
      ),
    ).toBeTruthy();
    expect(
      screen.getByText(
        "\u53ef\u80fd\u5339\u914d\u7684\u8def\u5f84\uff1ad:/vsp/easymd\uff08codex 5 \u6761\uff09",
      ),
    ).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "\u5e76\u5165\u5f53\u524d\u9879\u76ee d:/vsp/easymd" }));
    expect(onMergeAlias).toHaveBeenCalledWith("d:/vsp/easymd");
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
