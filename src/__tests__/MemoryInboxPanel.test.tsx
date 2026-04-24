import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import MemoryInboxPanel from "../components/MemoryInboxPanel";
import type { MemoryCandidate } from "../chatmem-memory/types";

const englishCandidate: MemoryCandidate = {
  candidate_id: "cand-ownership",
  kind: "gotcha",
  summary: "Do not touch any files outside your ownership.",
  value: "Do not touch any files outside your ownership.",
  why_it_matters:
    "\u4ece\u660e\u786e\u7684 durable-memory wording \u81ea\u52a8\u63d0\u53d6\uff1b\u8bf7\u5728\u6279\u51c6\u524d\u590d\u6838\u4e2d\u6587\u8868\u8ff0\u548c\u6280\u672f token \u662f\u5426\u51c6\u786e\u3002",
  confidence: 0.62,
  proposed_by: "auto_extractor",
  status: "pending_review",
  created_at: "2026-04-24T08:00:00Z",
  evidence_refs: [
    {
      excerpt:
        "Implement Task 2. You own only these files: src/App.tsx and src/__tests__/MemoryWorkspace.test.tsx.",
    },
  ],
};

describe("MemoryInboxPanel", () => {
  it("shows a Chinese review draft for English auto-extracted candidates and approves the draft", () => {
    const onApprove = vi.fn();

    render(
      <MemoryInboxPanel
        candidates={[englishCandidate]}
        loading={false}
        locale="zh-CN"
        onApprove={onApprove}
        onApproveMerge={vi.fn()}
        onReject={vi.fn()}
      />,
    );

    expect(screen.getByText("\u542f\u52a8\u8bb0\u5fc6\u5019\u9009")).toBeTruthy();
    expect(
      screen.getByText(
        "\u672c\u5730\u5386\u53f2\u5df2\u7ecf\u8fdb\u5165\u68c0\u7d22\u7d22\u5f15\uff1b\u8fd9\u91cc\u53ea\u786e\u8ba4\u4ee5\u540e\u542f\u52a8\u65f6\u8981\u5e26\u4e0a\u7684\u7a33\u5b9a\u89c4\u5219\u3002",
      ),
    ).toBeTruthy();
    expect(
      screen.getAllByText(
        "\u4e0d\u8981\u4fee\u6539\u81ea\u5df1\u8d1f\u8d23\u8303\u56f4\u4e4b\u5916\u7684\u6587\u4ef6\u3002",
      ).length,
    ).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("\u539f\u59cb\u5019\u9009")).toBeTruthy();
    expect(screen.getAllByText("Do not touch any files outside your ownership.").length).toBeGreaterThan(0);
    expect(
      screen.getByText((content) => content.includes("\u81ea\u52a8\u62bd\u53d6")),
    ).toBeTruthy();
    expect(screen.getByText("\u5f85\u786e\u8ba4")).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "\u6279\u51c6\u4e3a\u542f\u52a8\u8bb0\u5fc6" }));

    expect(onApprove).toHaveBeenCalledWith(
      englishCandidate,
      expect.objectContaining({
        title: "\u4e0d\u8981\u4fee\u6539\u81ea\u5df1\u8d1f\u8d23\u8303\u56f4\u4e4b\u5916\u7684\u6587\u4ef6\u3002",
        value: "\u4e0d\u8981\u4fee\u6539\u81ea\u5df1\u8d1f\u8d23\u8303\u56f4\u4e4b\u5916\u7684\u6587\u4ef6\u3002",
      }),
    );
  });
});
