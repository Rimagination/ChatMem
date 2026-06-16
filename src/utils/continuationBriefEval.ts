import {
  buildContinuationBriefPrompt,
  type ContinuationBriefInput,
} from "./continuationBrief";

export type ContinuationBriefEvalSeverity = "P0" | "P1";

export type ContinuationBriefEvalExpectations = {
  currentGoalIncludes?: string[];
  resumeIncludes?: string[];
  canonicalIncludes?: string[];
  canonicalBefore?: Array<[string, string]>;
  evidenceIncludes?: string[];
  forbidden?: string[];
  scopeForbidden?: string[];
  resumeForbidden?: string[];
  canonicalForbidden?: string[];
  evidenceForbidden?: string[];
  maxEvidenceArchiveLines?: number;
};

export type ContinuationBriefEvalCase = {
  id: string;
  name: string;
  split: "dev" | "holdout";
  input: ContinuationBriefInput;
  expectations: ContinuationBriefEvalExpectations;
};

export type ContinuationBriefEvalIssue = {
  caseId: string;
  caseName: string;
  severity: ContinuationBriefEvalSeverity;
  message: string;
};

export type ContinuationBriefEvalCaseResult = {
  caseId: string;
  caseName: string;
  split: "dev" | "holdout";
  prompt: string;
  issues: ContinuationBriefEvalIssue[];
};

export type ContinuationBriefEvalSuiteResult = {
  caseCount: number;
  passedCaseCount: number;
  passRate: number;
  p0Count: number;
  p1Count: number;
  cases: ContinuationBriefEvalCaseResult[];
  issues: ContinuationBriefEvalIssue[];
};

function promptSection(prompt: string, heading: string, nextHeading?: string) {
  const start = prompt.indexOf(heading);
  if (start < 0) {
    return "";
  }
  if (!nextHeading) {
    return prompt.slice(start);
  }
  const end = prompt.indexOf(nextHeading, start + heading.length);
  return end < 0 ? prompt.slice(start) : prompt.slice(start, end);
}

function addMissingIncludes(
  issues: ContinuationBriefEvalIssue[],
  testCase: ContinuationBriefEvalCase,
  severity: ContinuationBriefEvalSeverity,
  sectionName: string,
  sectionContent: string,
  expectedValues: string[] = [],
) {
  expectedValues.forEach((expected) => {
    if (!sectionContent.includes(expected)) {
      issues.push({
        caseId: testCase.id,
        caseName: testCase.name,
        severity,
        message: `${sectionName} should include "${expected}".`,
      });
    }
  });
}

function countArchiveLines(sectionContent: string) {
  return sectionContent
    .split("\n")
    .filter((line) => line.startsWith("- "))
    .filter((line) => /旧版|归档|删除|删掉|archive/i.test(line)).length;
}

function addForbiddenMatches(
  issues: ContinuationBriefEvalIssue[],
  testCase: ContinuationBriefEvalCase,
  severity: ContinuationBriefEvalSeverity,
  sectionName: string,
  sectionContent: string,
  forbiddenValues: string[] = [],
) {
  forbiddenValues.forEach((forbidden) => {
    if (sectionContent.includes(forbidden)) {
      issues.push({
        caseId: testCase.id,
        caseName: testCase.name,
        severity,
        message: `${sectionName} should not include "${forbidden}".`,
      });
    }
  });
}

export function evaluateContinuationBriefCase(
  testCase: ContinuationBriefEvalCase,
): ContinuationBriefEvalCaseResult {
  const prompt = buildContinuationBriefPrompt(testCase.input);
  const scope = promptSection(prompt, "## Scope", "## Current workline");
  const resume = promptSection(prompt, "## Where to resume", "## Canonical files");
  const canonicalFiles = promptSection(
    prompt,
    "## Canonical files",
    "## Obsolete or archived context",
  );
  const evidence = promptSection(prompt, "## Evidence", "## Token posture:");
  const issues: ContinuationBriefEvalIssue[] = [];

  addMissingIncludes(
    issues,
    testCase,
    "P0",
    "Scope",
    scope,
    testCase.expectations.currentGoalIncludes,
  );
  addMissingIncludes(
    issues,
    testCase,
    "P0",
    "Where to resume",
    resume,
    testCase.expectations.resumeIncludes,
  );
  addMissingIncludes(
    issues,
    testCase,
    "P0",
    "Canonical files",
    canonicalFiles,
    testCase.expectations.canonicalIncludes,
  );
  addMissingIncludes(
    issues,
    testCase,
    "P1",
    "Evidence",
    evidence,
    testCase.expectations.evidenceIncludes,
  );
  addForbiddenMatches(
    issues,
    testCase,
    "P0",
    "Scope",
    scope,
    testCase.expectations.scopeForbidden,
  );
  addForbiddenMatches(
    issues,
    testCase,
    "P0",
    "Where to resume",
    resume,
    testCase.expectations.resumeForbidden,
  );
  addForbiddenMatches(
    issues,
    testCase,
    "P0",
    "Canonical files",
    canonicalFiles,
    testCase.expectations.canonicalForbidden,
  );
  addForbiddenMatches(
    issues,
    testCase,
    "P1",
    "Evidence",
    evidence,
    testCase.expectations.evidenceForbidden,
  );

  testCase.expectations.canonicalBefore?.forEach(([first, second]) => {
    const firstIndex = canonicalFiles.indexOf(first);
    const secondIndex = canonicalFiles.indexOf(second);
    if (firstIndex < 0 || secondIndex < 0 || firstIndex >= secondIndex) {
      issues.push({
        caseId: testCase.id,
        caseName: testCase.name,
        severity: "P0",
        message: `Canonical files should rank "${first}" before "${second}".`,
      });
    }
  });

  testCase.expectations.forbidden?.forEach((forbidden) => {
    if (prompt.includes(forbidden)) {
      issues.push({
        caseId: testCase.id,
        caseName: testCase.name,
        severity: "P0",
        message: `Prompt should not include "${forbidden}".`,
      });
    }
  });

  if (typeof testCase.expectations.maxEvidenceArchiveLines === "number") {
    const archiveLineCount = countArchiveLines(evidence);
    if (archiveLineCount > testCase.expectations.maxEvidenceArchiveLines) {
      issues.push({
        caseId: testCase.id,
        caseName: testCase.name,
        severity: "P1",
        message: `Evidence should have at most ${testCase.expectations.maxEvidenceArchiveLines} archive lines, found ${archiveLineCount}.`,
      });
    }
  }

  return {
    caseId: testCase.id,
    caseName: testCase.name,
    split: testCase.split,
    prompt,
    issues,
  };
}

export function evaluateContinuationBriefSuite(
  cases: ContinuationBriefEvalCase[],
): ContinuationBriefEvalSuiteResult {
  const caseResults = cases.map(evaluateContinuationBriefCase);
  const issues = caseResults.flatMap((result) => result.issues);
  const passedCaseCount = caseResults.filter((result) => result.issues.length === 0).length;

  return {
    caseCount: cases.length,
    passedCaseCount,
    passRate: cases.length === 0 ? 1 : passedCaseCount / cases.length,
    p0Count: issues.filter((issue) => issue.severity === "P0").length,
    p1Count: issues.filter((issue) => issue.severity === "P1").length,
    cases: caseResults,
    issues,
  };
}

export function formatContinuationBriefEvalReport(result: ContinuationBriefEvalSuiteResult) {
  const lines = [
    "Continuation brief eval",
    `Cases: ${result.caseCount}`,
    `Passed cases: ${result.passedCaseCount}`,
    `P0 fatal errors: ${result.p0Count}`,
    `P1 quality issues: ${result.p1Count}`,
    `Pass rate: ${(result.passRate * 100).toFixed(1)}%`,
  ];

  if (result.issues.length > 0) {
    lines.push("", "Issues:");
    result.issues.forEach((issue) => {
      lines.push(`- ${issue.caseId} [${issue.severity}] ${issue.message}`);
    });
  }

  return lines.join("\n");
}
