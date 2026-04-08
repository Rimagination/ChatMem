import { fireEvent, render, screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
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

function buildConversation(overrides?: { messages?: Message[] }) {
  return {
    id: "conv-001",
    source_agent: "codex",
    project_dir: "D:/VSP/demo",
    created_at: "2026-04-08T08:00:00Z",
    updated_at: "2026-04-08T09:00:00Z",
    summary: "Debug session",
    messages: overrides?.messages ?? [],
    file_changes: [],
  };
}

describe("ConversationDetail", () => {
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
});
