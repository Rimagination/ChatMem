import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { MemoryCandidate } from "../chatmem-memory/types";
import MemoryInboxPanel from "../components/MemoryInboxPanel";

const englishCandidate: MemoryCandidate = {
  candidate_id: "cand-ownership",
  kind: "gotcha",
  summary: "Do not touch any files outside your ownership.",
  value: "Do not touch any files outside your ownership.",
  why_it_matters: "从明确的长期记忆措辞自动抽取。是否写入启动规则，请交给 Agent 结合上下文处理。",
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
  it("shows suggestions as view-only items that can be deleted", () => {
    const onDelete = vi.fn();

    render(
      <MemoryInboxPanel
        candidates={[englishCandidate]}
        loading={false}
        locale="zh-CN"
        onDelete={onDelete}
      />,
    );

    expect(screen.getByText("记忆建议")).toBeTruthy();
    expect(screen.queryByText("启动规则候选")).toBeNull();
    expect(
      screen.getByText("你可以在这里查看或删除建议。有用的建议可由支持的 Agent 写入启动规则。"),
    ).toBeTruthy();
    expect(screen.getAllByText("不要修改自己负责范围之外的文件。").length).toBeGreaterThanOrEqual(2);
    expect(screen.getByText("原文")).toBeTruthy();
    expect(screen.getAllByText("Do not touch any files outside your ownership.").length).toBeGreaterThan(0);
    expect(
      screen.getByText("从明确的长期记忆措辞自动抽取。是否写入启动规则，请交给 Agent 结合上下文处理。"),
    ).toBeTruthy();
    expect(screen.queryByText(/鎵瑰噯/)).toBeNull();
    expect(screen.getByText("来源：自动建议 · 仅供查看")).toBeTruthy();
    expect(screen.getByText("触发词：Do not")).toBeTruthy();
    expect(
      screen.getByText("英文原文。如果它应该成为中文启动规则，请让 agent 改写或写入。"),
    ).toBeTruthy();
    expect(screen.getByText("建议")).toBeTruthy();

    expect(screen.queryByRole("button", { name: "批准为启动规则" })).toBeNull();
    expect(screen.queryByRole("button", { name: "批准合并" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "批量删除自动建议" }));
    expect(onDelete).toHaveBeenCalledWith("cand-ownership");

    fireEvent.click(screen.getByRole("button", { name: "删除建议" }));
    expect(onDelete).toHaveBeenCalledWith("cand-ownership");
  });
});
