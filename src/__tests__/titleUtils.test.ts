import { describe, expect, it } from "vitest";
import {
  truncateSidebarTitle,
  truncateTitleByWidth,
  truncateWorkspaceTitle,
} from "../utils/titleUtils";

describe("titleUtils", () => {
  it("normalizes whitespace before truncating", () => {
    expect(truncateTitleByWidth("  alpha\n\nbeta   gamma  ", 32)).toBe("alpha beta gamma");
  });

  it("truncates mixed Chinese and ASCII titles with a fixed visual width", () => {
    expect(truncateSidebarTitle("我本地的chatmem项目，现在点击对话迁移为啥没反应？？")).toBe(
      "我本地的chatmem项目，现在点击对话迁移为啥...",
    );
  });

  it("uses a looser limit for the workspace title", () => {
    expect(
      truncateWorkspaceTitle(
        "你是最终收口补丁的独立代码质量 reviewer，请在工作树 D:\\VSP\\agentswap-gui\\.worktrees\\chatmem-control-plane-v2 review 最新提交 16a39b2",
      ),
    ).toBe(
      "你是最终收口补丁的独立代码质量 reviewer，请在工作树 D:\\VSP\\agentswap-gui\\.worktrees\\chatmem-co...",
    );
  });
});
