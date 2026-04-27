import React from "react";
import { fireEvent, render, screen } from "@testing-library/react";
import { beforeAll, describe, expect, it, vi } from "vitest";
import RepoMemoryPanel from "../components/RepoMemoryPanel";
import type { ApprovedMemory } from "../chatmem-memory/types";

function buildMemory(overrides: Partial<ApprovedMemory> = {}): ApprovedMemory {
  return {
    memory_id: "mem-001",
    kind: "command",
    title: "Primary verification",
    value: "npm.cmd run test:run",
    usage_hint: "Use before handoff",
    status: "active",
    last_verified_at: "2026-04-24T08:00:00Z",
    freshness_status: "fresh",
    freshness_score: 0.94,
    verified_at: "2026-04-24T08:00:00Z",
    verified_by: "codex",
    selected_because: null,
    evidence_refs: [],
    ...overrides,
  };
}

beforeAll(() => {
  if (!("scrollIntoView" in HTMLElement.prototype)) {
    Object.defineProperty(HTMLElement.prototype, "scrollIntoView", {
      configurable: true,
      value: () => {},
      writable: true,
    });
  }
});

function mockScrollIntoView() {
  const scrollIntoView = vi.fn<
    Parameters<HTMLElement["scrollIntoView"]>,
    ReturnType<HTMLElement["scrollIntoView"]>
  >();
  const scrollSpy = vi.spyOn(HTMLElement.prototype, "scrollIntoView");
  scrollSpy.mockImplementation((...args) => scrollIntoView(...args));

  return { scrollIntoView, scrollSpy };
}

describe("RepoMemoryPanel autofocus", () => {
  it("labels approved memories as startup rules rather than all repository history", () => {
    const { getByText, queryByText } = render(
      <RepoMemoryPanel
        memories={[buildMemory()]}
        loading={false}
        locale="zh-CN"
        onReverify={vi.fn()}
        onRetire={vi.fn()}
      />,
    );

    expect(getByText("\u5df2\u6279\u51c6\u542f\u52a8\u89c4\u5219")).toBeTruthy();
    expect(
      getByText(
        "\u8fd9\u4e9b\u662f\u4efb\u52a1\u5f00\u59cb\u65f6\u8981\u5e26\u4e0a\u7684\u7a33\u5b9a\u89c4\u5219\u3002\u672c\u5730\u5386\u53f2\u4ecd\u7136\u901a\u8fc7\u68c0\u7d22\u5355\u72ec\u63d0\u4f9b\u8bc1\u636e\u3002",
      ),
    ).toBeTruthy();
    expect(queryByText("\u4ed3\u5e93\u8bb0\u5fc6")).toBeNull();
  });

  it("focus executes once for a single autofocus request in StrictMode", () => {
    const { scrollIntoView, scrollSpy } = mockScrollIntoView();
    const onAutoFocusHandled = vi.fn();

    render(
      <React.StrictMode>
        <RepoMemoryPanel
          memories={[buildMemory()]}
          loading={false}
          locale="en"
          onReverify={vi.fn()}
          onRetire={vi.fn()}
          autoFocusFirstMemory
          onAutoFocusHandled={onAutoFocusHandled}
        />
      </React.StrictMode>,
    );

    const firstCard = document.querySelector(".memory-card");
    expect(firstCard).toBeInstanceOf(HTMLElement);
    expect(firstCard?.getAttribute("tabindex")).toBe("-1");
    expect(document.activeElement).toBe(firstCard);
    expect(scrollIntoView).toHaveBeenCalledTimes(1);
    expect(scrollIntoView).toHaveBeenCalledWith({ block: "nearest" });
    expect(onAutoFocusHandled).toHaveBeenCalledTimes(1);

    scrollSpy.mockRestore();
  });

  it("keeps the same autofocus request idempotent across rerender in StrictMode", () => {
    const { scrollIntoView, scrollSpy } = mockScrollIntoView();
    const onAutoFocusHandled = vi.fn();
    const onReverify = vi.fn();

    const { rerender } = render(
      <React.StrictMode>
        <RepoMemoryPanel
          memories={[buildMemory()]}
          loading={false}
          locale="en"
          onReverify={onReverify}
          onRetire={vi.fn()}
          autoFocusFirstMemory
          onAutoFocusHandled={onAutoFocusHandled}
        />
      </React.StrictMode>,
    );

    rerender(
      <React.StrictMode>
        <RepoMemoryPanel
          memories={[buildMemory({ title: "Primary verification" })]}
          loading={false}
          locale="en"
          onReverify={onReverify}
          onRetire={vi.fn()}
          autoFocusFirstMemory
          onAutoFocusHandled={onAutoFocusHandled}
        />
      </React.StrictMode>,
    );

    const firstCard = document.querySelector(".memory-card");
    expect(document.activeElement).toBe(firstCard);
    expect(scrollIntoView).toHaveBeenCalledTimes(1);
    expect(onAutoFocusHandled).toHaveBeenCalledTimes(1);

    scrollSpy.mockRestore();
  });

  it("loading defers focus until rerender with loading false", () => {
    const { scrollIntoView, scrollSpy } = mockScrollIntoView();
    const onAutoFocusHandled = vi.fn();

    const { rerender } = render(
      <RepoMemoryPanel
        memories={[buildMemory()]}
        loading
        locale="en"
        onReverify={vi.fn()}
        onRetire={vi.fn()}
        autoFocusFirstMemory
        onAutoFocusHandled={onAutoFocusHandled}
      />,
    );

    expect(document.querySelector(".memory-card")).toBeNull();
    expect(scrollIntoView).not.toHaveBeenCalled();
    expect(onAutoFocusHandled).not.toHaveBeenCalled();

    rerender(
      <RepoMemoryPanel
        memories={[buildMemory()]}
        loading={false}
        locale="en"
        onReverify={vi.fn()}
        onRetire={vi.fn()}
        autoFocusFirstMemory
        onAutoFocusHandled={onAutoFocusHandled}
      />,
    );

    const firstCard = document.querySelector(".memory-card");
    expect(document.activeElement).toBe(firstCard);
    expect(scrollIntoView).toHaveBeenCalledTimes(1);
    expect(onAutoFocusHandled).toHaveBeenCalledTimes(1);

    scrollSpy.mockRestore();
  });

  it("empty list clears autofocus quietly without scrolling or focusing", () => {
    const { scrollIntoView, scrollSpy } = mockScrollIntoView();
    const onAutoFocusHandled = vi.fn();

    render(
      <RepoMemoryPanel
        memories={[]}
        loading={false}
        locale="en"
        onReverify={vi.fn()}
        onRetire={vi.fn()}
        autoFocusFirstMemory
        onAutoFocusHandled={onAutoFocusHandled}
      />,
    );

    expect(document.querySelector(".memory-card")).toBeNull();
    expect(scrollIntoView).not.toHaveBeenCalled();
    expect(onAutoFocusHandled).toHaveBeenCalledTimes(1);

    scrollSpy.mockRestore();
  });

  it("uses explicit validity and retire actions for startup rules", () => {
    const onReverify = vi.fn();
    const onRetire = vi.fn();

    render(
      <RepoMemoryPanel
        memories={[buildMemory()]}
        loading={false}
        locale="zh-CN"
        onReverify={onReverify}
        onRetire={onRetire}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "\u786e\u8ba4\u4ecd\u6709\u6548" }));
    expect(onReverify).toHaveBeenCalledWith("mem-001");

    fireEvent.click(screen.getByRole("button", { name: "\u505c\u7528\u89c4\u5219" }));
    expect(onRetire).toHaveBeenCalledWith("mem-001");
    expect(screen.queryByRole("button", { name: "\u91cd\u65b0\u9a8c\u8bc1" })).toBeNull();
  });

  it("explains quarantined legacy auto rules without leaking internal labels", () => {
    render(
      <RepoMemoryPanel
        memories={[
          buildMemory({
            freshness_status: "needs_review",
            freshness_score: 0.2,
            last_verified_at: null,
            verified_by: "auto_quarantine",
          }),
        ]}
        loading={false}
        locale="zh-CN"
        onReverify={vi.fn()}
        onRetire={vi.fn()}
      />,
    );

    expect(screen.getByText(/旧版自动抽取/)).toBeTruthy();
    expect(screen.getByText(/确认后才会作为启动规则使用/)).toBeTruthy();
    expect(screen.queryByText(/auto_quarantine/)).toBeNull();
    expect(screen.getByText("\u542f\u7528")).toBeTruthy();
    expect(screen.queryByText(/^active$/)).toBeNull();
    expect(screen.queryByText(/\u65b0\u9c9c\u5ea6\u5206\u6570: 0\.20/)).toBeNull();
  });

  it("offers one action to retire all quarantined legacy auto rules", () => {
    const onRetireMany = vi.fn();

    render(
      <RepoMemoryPanel
        memories={[
          buildMemory({
            memory_id: "mem-old-1",
            freshness_status: "needs_review",
            last_verified_at: null,
            verified_by: "auto_quarantine",
          }),
          buildMemory({
            memory_id: "mem-old-2",
            title: "Legacy instruction",
            freshness_status: "needs_review",
            last_verified_at: null,
            verified_by: "auto_quarantine",
          }),
          buildMemory({
            memory_id: "mem-fresh",
            title: "Keep this rule",
          }),
        ]}
        loading={false}
        locale="zh-CN"
        onReverify={vi.fn()}
        onRetire={vi.fn()}
        onRetireMany={onRetireMany}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: "\u5168\u90e8\u505c\u7528\u65e7\u7248\u81ea\u52a8\u89c4\u5219 2" }));

    expect(onRetireMany).toHaveBeenCalledWith(["mem-old-1", "mem-old-2"]);
  });
});
