import { describe, expect, it } from "vitest";
import {
  evaluateContinuationBriefSuite,
  formatContinuationBriefEvalReport,
} from "../utils/continuationBriefEval";

describe("evaluateContinuationBriefSuite", () => {
  const baseInput = {
    repoRoot: "F:/OneDrive/19-郑吉林博士论文",
    conversation: {
      id: "eval-a4-figure",
      source_agent: "codex",
      summary: "A4 figure adjustment",
      resume_command: "codex resume eval-a4-figure",
      messages: [
        {
          id: "user-1",
          role: "user",
          content: "有压盖 字体大小也要调整 我们这个最后要放在A4版面里",
        },
        {
          id: "assistant-1",
          role: "assistant",
          content:
            "我又按 A4 可读性重排了一版。最新版：[图0-graphical abstract_JoH版.png](F:/paper/图0-graphical abstract_JoH版.png)，同步版：[图0-graphical abstract_JoH版.pdf](F:/paper/图0-graphical abstract_JoH版.pdf)。",
        },
      ],
      file_changes: [],
    },
  };

  it("separates fatal resume errors from evidence quality issues", () => {
    const result = evaluateContinuationBriefSuite([
      {
        id: "fatal-goal",
        name: "fatal missing current goal",
        split: "dev",
        input: baseInput,
        expectations: {
          currentGoalIncludes: ["完全不存在的目标"],
        },
      },
      {
        id: "quality-evidence",
        name: "quality missing evidence line",
        split: "dev",
        input: baseInput,
        expectations: {
          currentGoalIncludes: ["A4 版图件已重排完成"],
          evidenceIncludes: ["完全不存在的证据"],
        },
      },
    ]);

    expect(result.caseCount).toBe(2);
    expect(result.p0Count).toBe(1);
    expect(result.p1Count).toBe(1);
    expect(result.passRate).toBe(0);
    expect(result.issues.map((issue) => `${issue.caseId}:${issue.severity}`)).toEqual([
      "fatal-goal:P0",
      "quality-evidence:P1",
    ]);
  });

  it("allows original user wording in evidence while banning it from active resume fields", () => {
    const result = evaluateContinuationBriefSuite([
      {
        id: "section-forbidden",
        name: "section scoped forbidden checks",
        split: "dev",
        input: baseInput,
        expectations: {
          currentGoalIncludes: ["A4 版图件已重排完成"],
          scopeForbidden: ["有压盖 字体大小也要调整"],
          resumeForbidden: ["有压盖 字体大小也要调整"],
          evidenceIncludes: ["user: 有压盖 字体大小也要调整"],
        },
      },
    ]);

    expect(result.issues).toEqual([]);
  });

  it("reports section-scoped forbidden text when it appears in that section", () => {
    const result = evaluateContinuationBriefSuite([
      {
        id: "section-forbidden-fail",
        name: "section scoped forbidden failure",
        split: "dev",
        input: baseInput,
        expectations: {
          currentGoalIncludes: ["A4 版图件已重排完成"],
          scopeForbidden: ["A4 版图件已重排完成"],
        },
      },
    ]);

    expect(result.issues.map((issue) => `${issue.caseId}:${issue.severity}`)).toEqual([
      "section-forbidden-fail:P0",
    ]);
  });

  it("formats a compact command-line report", () => {
    const result = evaluateContinuationBriefSuite([
      {
        id: "fatal-goal",
        name: "fatal missing current goal",
        split: "dev",
        input: baseInput,
        expectations: {
          currentGoalIncludes: ["完全不存在的目标"],
        },
      },
    ]);

    expect(formatContinuationBriefEvalReport(result)).toContain("Cases: 1");
    expect(formatContinuationBriefEvalReport(result)).toContain("P0 fatal errors: 1");
    expect(formatContinuationBriefEvalReport(result)).toContain("fatal-goal [P0]");
  });
});
