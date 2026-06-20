import { describe, expect, it } from "vitest";
import { continuationBriefEvalCases } from "../utils/continuationBriefEvalCases";
import { evaluateContinuationBriefSuite } from "../utils/continuationBriefEval";

describe("continuationBriefEvalCases", () => {
  it("covers a mixed suite without known regressions", () => {
    const result = evaluateContinuationBriefSuite(continuationBriefEvalCases);
    const splits = new Set(continuationBriefEvalCases.map((testCase) => testCase.split));

    expect(continuationBriefEvalCases.length).toBeGreaterThanOrEqual(5);
    expect(splits.has("dev")).toBe(true);
    expect(splits.has("holdout")).toBe(true);
    expect(result.p0Count).toBe(0);
    expect(result.p1Count).toBe(0);
    expect(result.passRate).toBe(1);
  });
});
