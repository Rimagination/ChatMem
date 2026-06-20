import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const styles = readFileSync(resolve(process.cwd(), "src/styles.css"), "utf8");

function ruleFor(selector: string) {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const match = styles.match(new RegExp(`${escaped}\\s*\\{(?<body>[^}]+)\\}`, "u"));
  return match?.groups?.body ?? "";
}

describe("conversation layout CSS", () => {
  it("lets the right conversation workspace shrink inside the app frame", () => {
    [
      ".conversation-workspace",
      ".workspace-view-panel",
      ".workspace-view-panel-conversation",
      ".conversation-content-grid",
      ".conversation-content-grid > .conversation-detail",
      ".conversation-detail",
      ".stats",
      ".stat-item",
      ".message-list",
      ".message",
      ".message-shell",
    ].forEach((selector) => {
      expect(ruleFor(selector), selector).toContain("min-width: 0");
    });
  });

  it("prevents long conversation paths from defining the workspace min-content width", () => {
    expect(ruleFor(".conversation-meta-strip.compact")).toContain("min-width: 0");
    expect(ruleFor(".conversation-meta-strip.compact .meta-block")).toContain("min-width: 0");
    expect(ruleFor(".conversation-meta-strip.compact .meta-value")).toContain("overflow-wrap: anywhere");
  });

  it("lets conversation toolbar actions wrap instead of widening the workspace", () => {
    expect(ruleFor(".conversation-toolbar-actions")).toContain("flex-wrap: wrap");
    expect(ruleFor(".conversation-toolbar-actions")).toContain("max-width: min(");
  });

  it("keeps the collapse control inline with the topbar brand", () => {
    const collapseControl = ruleFor(".topbar-sidebar-toggle");

    expect(styles).not.toContain(".sidebar-collapse-float");
    expect(collapseControl).toContain("height: 32px");
    expect(collapseControl).toContain("align-items: center");
    expect(collapseControl).toContain("justify-content: center");
  });

  it("left-aligns the topbar brand group with the sidebar content", () => {
    expect(ruleFor(".app-topbar")).toContain("padding: 0 8px 0 14px");
    expect(ruleFor(".topbar-center")).toContain("justify-content: flex-start");
  });

  it("preserves the inset rounded workspace surface when the window is filled", () => {
    expect(ruleFor(".workspace")).toContain("padding: 12px 14px 14px");
    expect(ruleFor(".workspace-surface")).toContain("border-radius: var(--radius-xl)");
    expect(ruleFor(".app-shell.is-window-filled .workspace")).not.toContain("padding: 0");
    expect(ruleFor(".app-shell.is-window-filled .workspace-surface")).not.toContain("border-radius: 0");
  });

  it("uses browser render containment for repeated conversation rows", () => {
    expect(ruleFor(".message")).toContain("content-visibility: auto");
    expect(ruleFor(".conversation-item")).toContain("content-visibility: auto");
  });

  it("keeps Trash header actions reachable in narrow workspaces", () => {
    expect(ruleFor(".trash-workspace-page")).toContain("width: min(");
    expect(ruleFor(".trash-workspace-page")).toContain("margin: 0 auto");
    expect(ruleFor(".trash-page-header")).toContain("flex-wrap: wrap");
    expect(ruleFor(".trash-page-header > div:first-child")).toContain("min-width: 0");
    expect(ruleFor(".trash-page-actions")).toContain("flex: 1 1");
    expect(ruleFor(".trash-page-actions")).toContain("min-width: 0");
    expect(ruleFor(".trash-page-actions")).toContain("justify-content: flex-start");
    expect(ruleFor(".trash-page-actions")).toContain("flex-wrap: wrap");
  });
});
