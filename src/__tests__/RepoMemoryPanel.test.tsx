import React from "react";
import { render } from "@testing-library/react";
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

describe("RepoMemoryPanel autofocus", () => {
  it("focus executes once for a single autofocus request in StrictMode", () => {
    const scrollIntoView = vi.fn();
    const scrollSpy = vi
      .spyOn(HTMLElement.prototype, "scrollIntoView" as keyof HTMLElement)
      .mockImplementation(scrollIntoView as () => void);
    const onAutoFocusHandled = vi.fn();

    render(
      <React.StrictMode>
        <RepoMemoryPanel
          memories={[buildMemory()]}
          loading={false}
          locale="en"
          onReverify={vi.fn()}
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
    const scrollIntoView = vi.fn();
    const scrollSpy = vi
      .spyOn(HTMLElement.prototype, "scrollIntoView" as keyof HTMLElement)
      .mockImplementation(scrollIntoView as () => void);
    const onAutoFocusHandled = vi.fn();
    const onReverify = vi.fn();

    const { rerender } = render(
      <React.StrictMode>
        <RepoMemoryPanel
          memories={[buildMemory()]}
          loading={false}
          locale="en"
          onReverify={onReverify}
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
    const scrollIntoView = vi.fn();
    const scrollSpy = vi
      .spyOn(HTMLElement.prototype, "scrollIntoView" as keyof HTMLElement)
      .mockImplementation(scrollIntoView as () => void);
    const onAutoFocusHandled = vi.fn();

    const { rerender } = render(
      <RepoMemoryPanel
        memories={[buildMemory()]}
        loading
        locale="en"
        onReverify={vi.fn()}
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
    const scrollIntoView = vi.fn();
    const scrollSpy = vi
      .spyOn(HTMLElement.prototype, "scrollIntoView" as keyof HTMLElement)
      .mockImplementation(scrollIntoView as () => void);
    const onAutoFocusHandled = vi.fn();

    render(
      <RepoMemoryPanel
        memories={[]}
        loading={false}
        locale="en"
        onReverify={vi.fn()}
        autoFocusFirstMemory
        onAutoFocusHandled={onAutoFocusHandled}
      />,
    );

    expect(document.querySelector(".memory-card")).toBeNull();
    expect(scrollIntoView).not.toHaveBeenCalled();
    expect(onAutoFocusHandled).toHaveBeenCalledTimes(1);

    scrollSpy.mockRestore();
  });
});
