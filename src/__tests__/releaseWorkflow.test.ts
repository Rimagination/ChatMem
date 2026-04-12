import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const workflow = readFileSync(resolve(process.cwd(), ".github/workflows/release.yml"), "utf8");

describe("release workflow", () => {
  it("builds Windows and macOS release assets", () => {
    expect(workflow).toContain("release-windows:");
    expect(workflow).toContain("runs-on: windows-latest");
    expect(workflow).toContain("release-macos:");
    expect(workflow).toContain("runs-on: ${{ matrix.platform }}");
    expect(workflow).toContain("macos-15-intel");
    expect(workflow).toContain("macos-latest");
    expect(workflow).toContain("x86_64-apple-darwin");
    expect(workflow).toContain("aarch64-apple-darwin");
    expect(workflow).toContain("args: --bundles dmg,app,updater");
    expect(workflow).toContain("Prepare user-facing macOS DMG");
    expect(workflow).toContain("ChatMem-${{ github.ref_name }}-macOS-Apple-Silicon.dmg");
    expect(workflow).toContain("ChatMem-v${version}-macOS-${arch_label}.dmg");
  });

  it("signs updater artifacts in both platform jobs", () => {
    const secretReferences = workflow.match(
      /TAURI_PRIVATE_KEY: \$\{\{ secrets\.TAURI_PRIVATE_KEY \}\}/g,
    ) ?? [];

    expect(secretReferences).toHaveLength(2);
    expect(workflow).toContain("targets: ${{ matrix.target }}");
    expect(workflow).toContain("includeUpdaterJson: true");
  });
});
