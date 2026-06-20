import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import ConversationDetail from "../components/ConversationDetail";

type ToolCall = {
  name: string;
  input: unknown;
  output: string | null;
  status: string;
};

type Message = {
  id: string;
  timestamp: string;
  role: string;
  content: string;
  tool_calls: ToolCall[];
  metadata: Record<string, unknown>;
};

function buildConversation(overrides?: { messages?: Message[]; totalMessageCount?: number }) {
  return {
    id: "conv-001",
    source_agent: "codex",
    project_dir: "D:/VSP/demo",
    created_at: "2026-04-08T08:00:00Z",
    updated_at: "2026-04-08T09:00:00Z",
    summary: "Debug session",
    total_message_count: overrides?.totalMessageCount,
    messages: overrides?.messages ?? [],
    file_changes: [],
  };
}

describe("ConversationDetail", () => {
  it("renders message markdown as structured readable content", () => {
    const conversation = buildConversation({
      messages: [
        {
          id: "a1",
          timestamp: "2026-04-08T08:31:00Z",
          role: "assistant",
          content: [
            "## \u65b9\u6848",
            "",
            "- **\u5bfc\u5165** ZCode \u4f1a\u8bdd",
            "",
            "```ts",
            "const ok = true;",
            "```",
          ].join("\n"),
          tool_calls: [],
          metadata: {},
        },
      ],
    });

    const { container } = render(<ConversationDetail conversation={conversation} />);

    expect(screen.getByRole("heading", { name: "\u65b9\u6848" })).toBeTruthy();
    expect(container.querySelector(".message-content strong")?.textContent).toBe("\u5bfc\u5165");
    expect(container.querySelector(".message-content pre code")?.textContent).toContain("const ok = true;");
  });

  it("renders user messages with a dedicated bubble element", () => {
    const conversation = buildConversation({
      messages: [
        {
          id: "m1",
          timestamp: "2026-04-08T08:30:00Z",
          role: "user",
          content: "Open the config file",
          tool_calls: [],
          metadata: {},
        },
      ],
    });

    const { container } = render(<ConversationDetail conversation={conversation} />);

    expect(container.querySelector(".message-user .message-bubble")).toBeTruthy();
  });

  it("renders long assistant messages collapsed by default", () => {
    const conversation = buildConversation({
      messages: [
        {
          id: "a1",
          timestamp: "2026-04-08T08:31:00Z",
          role: "assistant",
          content: "Long assistant reply. ".repeat(80),
          tool_calls: [],
          metadata: {},
        },
      ],
    });

    const { container } = render(<ConversationDetail conversation={conversation} />);

    expect(screen.getByRole("button", { name: "展开全文" })).toBeTruthy();
    expect(container.querySelector(".message-content.is-collapsed")).toBeTruthy();
  });

  it("renders only a preview for collapsed assistant messages", () => {
    const longContent = `${"Long assistant reply. ".repeat(40)}UNIQUE_TAIL_TOKEN`;
    const conversation = buildConversation({
      messages: [
        {
          id: "a1",
          timestamp: "2026-04-08T08:31:00Z",
          role: "assistant",
          content: longContent,
          tool_calls: [],
          metadata: {},
        },
      ],
    });

    render(<ConversationDetail conversation={conversation} />);

    expect(screen.queryByText(/UNIQUE_TAIL_TOKEN/)).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: /展开全文/ }));
    expect(screen.getByText(/UNIQUE_TAIL_TOKEN/)).toBeTruthy();
  });

  it("renders large conversations progressively from the first messages", () => {
    const messages = Array.from({ length: 125 }, (_, index) => ({
      id: `m${index + 1}`,
      timestamp: `2026-04-08T08:${String(index % 60).padStart(2, "0")}:00Z`,
      role: index % 2 === 0 ? "user" : "assistant",
      content: `message-${index + 1}`,
      tool_calls: [],
      metadata: {},
    }));
    const conversation = buildConversation({ messages });

    render(<ConversationDetail conversation={conversation} />);

    expect(screen.getByText("message-1")).toBeTruthy();
    expect(screen.getByText("message-80")).toBeTruthy();
    expect(screen.queryByText("message-81")).toBeNull();
    expect(screen.queryByText("message-125")).toBeNull();
    expect(screen.getAllByRole("article")).toHaveLength(80);

    fireEvent.click(screen.getByRole("button", { name: /\u663e\u793a\u540e\u7eed\u6d88\u606f/ }));

    expect(screen.getByText("message-125")).toBeTruthy();
    expect(screen.getAllByRole("article")).toHaveLength(125);
  });

  it("requests a larger server-side message window when following messages are not loaded yet", () => {
    const messages = Array.from({ length: 80 }, (_, index) => ({
      id: `m${index + 1}`,
      timestamp: `2026-04-08T08:${String(index % 60).padStart(2, "0")}:00Z`,
      role: index % 2 === 0 ? "user" : "assistant",
      content: `message-${index + 1}`,
      tool_calls: [],
      metadata: {},
    }));
    const onLoadFollowingMessages = vi.fn();
    const conversation = buildConversation({ messages, totalMessageCount: 125 });

    render(
      <ConversationDetail
        conversation={conversation}
        onLoadFollowingMessages={onLoadFollowingMessages}
      />,
    );

    expect(screen.getByText("125")).toBeTruthy();
    expect(screen.getByText("message-1")).toBeTruthy();
    expect(screen.getByText("message-80")).toBeTruthy();
    expect(screen.queryByText("message-81")).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: /\u663e\u793a\u540e\u7eed\u6d88\u606f/ }));

    expect(onLoadFollowingMessages).toHaveBeenCalledWith(125);
  });

  it("does not show a load-following action when no messages were loaded", () => {
    const conversation = buildConversation({ messages: [], totalMessageCount: 2 });

    const { container } = render(<ConversationDetail conversation={conversation} />);

    expect(screen.getByText("2")).toBeTruthy();
    expect(container.querySelector(".message-load-following")).toBeNull();
    expect(screen.queryAllByRole("article")).toHaveLength(0);
  });

  it("does not collapse user messages by default", () => {
    const conversation = buildConversation({
      messages: [
        {
          id: "u1",
          timestamp: "2026-04-08T08:30:00Z",
          role: "user",
          content: "Long user reply. ".repeat(80),
          tool_calls: [],
          metadata: {},
        },
      ],
    });

    const { container } = render(<ConversationDetail conversation={conversation} />);

    expect(screen.queryByRole("button", { name: "展开全文" })).toBeNull();
    expect(container.querySelector(".message-user .message-content.is-collapsed")).toBeFalsy();
  });

  it("expands and collapses assistant content on demand", () => {
    const conversation = buildConversation({
      messages: [
        {
          id: "a1",
          timestamp: "2026-04-08T08:31:00Z",
          role: "assistant",
          content: "Long assistant reply. ".repeat(80),
          tool_calls: [],
          metadata: {},
        },
      ],
    });

    const { container } = render(<ConversationDetail conversation={conversation} />);

    fireEvent.click(screen.getByRole("button", { name: "展开全文" }));
    expect(screen.getByRole("button", { name: "收起" })).toBeTruthy();
    expect(container.querySelector(".message-content.is-expanded")).toBeTruthy();
  });

  it("renders tool calls collapsed by default and expands details on demand", () => {
    const conversation = buildConversation({
      messages: [
        {
          id: "a1",
          timestamp: "2026-04-08T08:31:00Z",
          role: "assistant",
          content: "Done",
          tool_calls: [
            {
              name: "read_file",
              input: { path: "demo.txt" },
              output: "content",
              status: "success",
            },
          ],
          metadata: {},
        },
      ],
    });

    render(<ConversationDetail conversation={conversation} />);

    expect(screen.getByRole("button", { name: "展开工具详情" })).toBeTruthy();
    expect(screen.queryByText(/demo\.txt/)).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "展开工具详情" }));
    expect(screen.getByText(/demo\.txt/)).toBeTruthy();
  });
  it("groups multiple tool calls behind one collapsed control", () => {
    const conversation = buildConversation({
      messages: [
        {
          id: "a1",
          timestamp: "2026-04-08T08:31:00Z",
          role: "assistant",
          content: "I checked it.",
          tool_calls: [
            {
              name: "read_file",
              input: { path: "a.txt" },
              output: "A",
              status: "success",
            },
            {
              name: "list_dir",
              input: { path: "D:/VSP" },
              output: "chatmem",
              status: "success",
            },
            {
              name: "shell",
              input: { command: "git status" },
              output: "clean",
              status: "success",
            },
          ],
          metadata: {},
        },
      ],
    });

    const { container } = render(<ConversationDetail conversation={conversation} />);

    const expandToolsLabel = "\u5c55\u5f00\u5de5\u5177\u8be6\u60c5";
    expect(container.querySelector(".tool-call-kicker")?.textContent).toBe("\u5de5\u5177\u8c03\u7528");
    expect(screen.getByText("3 \u4e2a\u8c03\u7528")).toBeTruthy();
    expect(screen.getAllByRole("button", { name: expandToolsLabel })).toHaveLength(1);
    expect(screen.queryByText(/git status/)).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: expandToolsLabel }));
    expect(screen.getByText(/git status/)).toBeTruthy();
    expect(screen.getByText(/chatmem/)).toBeTruthy();
  });
});
