# ChatMem macOS Release Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Publish ChatMem macOS assets to GitHub Releases alongside the existing Windows packages.

**Architecture:** Keep the existing tag-triggered release workflow and add a macOS runner job that uses the same Tauri action and updater signing secrets. Add a small workflow test so CI release structure is checked by the normal Vitest suite.

**Tech Stack:** GitHub Actions, Tauri v1, Rust, TypeScript, Vitest.

---

### Task 1: Guard the Release Workflow

**Files:**
- Create: `src/__tests__/releaseWorkflow.test.ts`
- Modify: none

- [ ] **Step 1: Write the failing test**

```ts
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const workflow = readFileSync(resolve(process.cwd(), ".github/workflows/release.yml"), "utf8");

describe("release workflow", () => {
  it("builds Windows and macOS release assets", () => {
    expect(workflow).toContain("release-windows:");
    expect(workflow).toContain("runs-on: windows-latest");
    expect(workflow).toContain("release-macos:");
    expect(workflow).toContain("runs-on: macos-latest");
    expect(workflow).toContain("args: --bundles dmg,app");
  });

  it("signs updater artifacts in both platform jobs", () => {
    const secretReferences = workflow.match(/TAURI_PRIVATE_KEY: \\$\\{\\{ secrets\\.TAURI_PRIVATE_KEY \\}\\}/g) ?? [];
    expect(secretReferences).toHaveLength(2);
    expect(workflow).toContain("includeUpdaterJson: true");
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm run test:run -- src/__tests__/releaseWorkflow.test.ts`

Expected: FAIL because `release-macos` is not in the workflow yet.

### Task 2: Add macOS Release Job

**Files:**
- Modify: `.github/workflows/release.yml`
- Test: `src/__tests__/releaseWorkflow.test.ts`

- [ ] **Step 1: Add a macOS job**

Add a `release-macos` job that checks out the tag, sets up Node.js and Rust, runs `npm ci`, and runs `tauri-apps/tauri-action@v0.6.2` with `args: --bundles dmg,app`.

- [ ] **Step 2: Run the workflow test**

Run: `npm run test:run -- src/__tests__/releaseWorkflow.test.ts`

Expected: PASS.

### Task 3: Verify and Publish

**Files:**
- Modify: none

- [ ] **Step 1: Run local verification**

Run:

```powershell
npm run test:run
cargo test --manifest-path .\src-tauri\Cargo.toml
```

Expected: all tests pass.

- [ ] **Step 2: Commit and push**

Run:

```powershell
git add .github\workflows\release.yml src\__tests__\releaseWorkflow.test.ts docs\superpowers\specs\2026-04-12-chatmem-macos-release-design.md docs\superpowers\plans\2026-04-12-chatmem-macos-release.md
git commit -m "ci: add macos release build"
git push origin codex/macos-release
git push origin HEAD:main
```

- [ ] **Step 3: Re-run the release tag**

Run:

```powershell
git tag -f v0.1.0
git push origin -f v0.1.0
```

Expected: GitHub Actions starts a new release run.

- [ ] **Step 4: Verify GitHub Release assets**

Confirm the Release contains Windows assets and macOS assets, including `.dmg`, `.app.tar.gz`, and `.app.tar.gz.sig`.

