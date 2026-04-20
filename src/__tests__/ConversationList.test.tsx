import { render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import ConversationList from "../components/ConversationList";

describe("ConversationList", () => {
  it("renders empty state when no conversations", () => {
    render(
      <ConversationList
        conversations={[]}
        selectedId={null}
        onSelect={() => {}}
        loading={false}
      />,
    );

    expect(screen.getByText("未找到对话")).toBeTruthy();
  });

  it("renders loading state", () => {
    render(
      <ConversationList
        conversations={[]}
        selectedId={null}
        onSelect={() => {}}
        loading={true}
      />,
    );

    expect(document.querySelector(".spinner")).toBeTruthy();
  });

  it("renders a compact list row without the old metadata pills", () => {
    const longTitle = "我本地的chatmem项目，现在点击对话迁移为啥没反应？？";
    const conversations = [
      {
        id: "test-id-1",
        source_agent: "claude",
        project_dir: "/test/project",
        created_at: "2026-03-29T10:00:00Z",
        updated_at: "2026-03-29T10:00:00Z",
        summary: longTitle,
        message_count: 5,
        file_count: 2,
      },
    ];

    const { container } = render(
      <ConversationList
        conversations={conversations}
        selectedId={null}
        onSelect={() => {}}
        loading={false}
      />,
    );

    const title = container.querySelector(".conversation-item-title");
    expect(title?.textContent).toBe("我本地的chatmem项目，现在点击对话迁移为啥...");
    expect(title?.getAttribute("title")).toBe(longTitle);
    expect(screen.getByText("/test/project")).toBeTruthy();
    expect(container.querySelector(".conversation-item-row")).toBeTruthy();
    expect(container.querySelector(".conversation-item-time")).toBeTruthy();
    expect(container.querySelector(".conversation-item-meta")).toBeFalsy();
  });
});
