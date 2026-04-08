# ChatMem Compact Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the sidebar denser and add default folding for long assistant content so conversation browsing becomes faster and less vertically noisy.

**Architecture:** Keep the existing two-pane structure and change only the presentation layer. The sidebar remains a simple mapped list in `ConversationList.tsx`, while message folding state is kept locally inside `ConversationDetail.tsx` using message ids and tool-call keys so no backend or app-level state changes are required.

**Tech Stack:** React 18, TypeScript, Vitest, Testing Library, CSS

---

### Task 1: Tighten Sidebar Rows

**Files:**
- Modify: `D:\VSP\agentswap-gui\src\components\ConversationList.tsx`
- Modify: `D:\VSP\agentswap-gui\src\styles.css`
- Test: `D:\VSP\agentswap-gui\src\__tests__\ConversationList.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
it("renders a compact list row without the old metadata pills", () => {
  const conversations = [
    {
      id: "test-id-1",
      source_agent: "claude",
      project_dir: "/test/project",
      created_at: "2026-03-29T10:00:00Z",
      updated_at: "2026-03-29T10:00:00Z",
      summary: "Test conversation title that should truncate",
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
    />
  );

  expect(container.querySelector(".conversation-item-row")).toBeTruthy();
  expect(container.querySelector(".conversation-item-time")).toBeTruthy();
  expect(container.querySelector(".conversation-item-meta")).toBeFalsy();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm run test:run -- src/__tests__/ConversationList.test.tsx`
Expected: FAIL because `.conversation-item-row` does not exist and `.conversation-item-meta` still exists.

- [ ] **Step 3: Write minimal implementation**

```tsx
<div className={`conversation-item ${selected ? "selected" : ""}`} onClick={...}>
  <div className="conversation-item-row">
    <div className="conversation-item-main">
      <div className="conversation-item-title">{title}</div>
      <div className="conversation-item-path" title={conversation.project_dir}>
        {conversation.project_dir}
      </div>
    </div>
    <div className="conversation-item-time">{formatDistanceToNow(conversation.updated_at)}</div>
  </div>
</div>
```

```css
.conversation-item {
  padding: 10px 12px;
  border-radius: 14px;
}

.conversation-item-row {
  display: grid;
  grid-template-columns: minmax(0, 1fr) auto;
  gap: 10px;
  align-items: start;
}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npm run test:run -- src/__tests__/ConversationList.test.tsx`
Expected: PASS

- [ ] **Step 5: Verify visually relevant selectors exist**

Run: `Select-String -Path D:\VSP\agentswap-gui\src\styles.css -Pattern "conversation-item-row|conversation-item-time"`
Expected: both selectors present

### Task 2: Add Failing Tests For Assistant Message Folding

**Files:**
- Modify: `D:\VSP\agentswap-gui\src\__tests__\ConversationDetail.test.tsx`
- Modify: `D:\VSP\agentswap-gui\src\components\ConversationDetail.tsx`

- [ ] **Step 1: Write the failing tests**

```tsx
it("renders long assistant messages collapsed by default", () => {
  const longText = "Long assistant reply. ".repeat(80);
  const conversation = buildConversation({
    messages: [
      { id: "a1", role: "assistant", content: longText, tool_calls: [], metadata: {}, timestamp: "2026-04-08T08:31:00Z" },
    ],
  });

  const { container } = render(<ConversationDetail conversation={conversation} />);

  expect(screen.getByRole("button", { name: "展开全文" })).toBeTruthy();
  expect(container.querySelector(".message-content.is-collapsed")).toBeTruthy();
});

it("does not collapse user messages by default", () => {
  const longText = "Long user reply. ".repeat(80);
  const conversation = buildConversation({
    messages: [
      { id: "u1", role: "user", content: longText, tool_calls: [], metadata: {}, timestamp: "2026-04-08T08:30:00Z" },
    ],
  });

  const { container } = render(<ConversationDetail conversation={conversation} />);

  expect(screen.queryByRole("button", { name: "展开全文" })).toBeNull();
  expect(container.querySelector(".message-user .message-content.is-collapsed")).toBeFalsy();
});

it("expands and collapses assistant content on demand", async () => {
  const longText = "Long assistant reply. ".repeat(80);
  const conversation = buildConversation({
    messages: [
      { id: "a1", role: "assistant", content: longText, tool_calls: [], metadata: {}, timestamp: "2026-04-08T08:31:00Z" },
    ],
  });

  const { container } = render(<ConversationDetail conversation={conversation} />);

  await userEvent.click(screen.getByRole("button", { name: "展开全文" }));
  expect(screen.getByRole("button", { name: "收起" })).toBeTruthy();
  expect(container.querySelector(".message-content.is-expanded")).toBeTruthy();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm run test:run -- src/__tests__/ConversationDetail.test.tsx`
Expected: FAIL because folding controls and collapsed classes do not exist.

- [ ] **Step 3: Write minimal implementation**

```tsx
const [expandedMessages, setExpandedMessages] = useState<Record<string, boolean>>({});
const shouldCollapse = message.role === "assistant" && message.content.length > 280;
const isExpanded = expandedMessages[message.id] ?? false;

<div className={`message-content ${shouldCollapse && !isExpanded ? "is-collapsed" : "is-expanded"}`}>
  {message.content}
</div>
{shouldCollapse && (
  <button
    type="button"
    className="message-toggle"
    onClick={() => setExpandedMessages((current) => ({ ...current, [message.id]: !isExpanded }))}
  >
    {isExpanded ? "收起" : "展开全文"}
  </button>
)}
```

- [ ] **Step 4: Run test to verify it passes**

Run: `npm run test:run -- src/__tests__/ConversationDetail.test.tsx`
Expected: PASS

- [ ] **Step 5: Add collapse styling**

```css
.message-content.is-collapsed {
  display: -webkit-box;
  -webkit-line-clamp: 5;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
```

### Task 3: Fold Tool Call Details And Finish Styling

**Files:**
- Modify: `D:\VSP\agentswap-gui\src\__tests__\ConversationDetail.test.tsx`
- Modify: `D:\VSP\agentswap-gui\src\components\ConversationDetail.tsx`
- Modify: `D:\VSP\agentswap-gui\src\styles.css`

- [ ] **Step 1: Write the failing test**

```tsx
it("renders tool calls collapsed by default and expands details on demand", async () => {
  const conversation = buildConversation({
    messages: [
      {
        id: "a1",
        role: "assistant",
        content: "Done",
        timestamp: "2026-04-08T08:31:00Z",
        metadata: {},
        tool_calls: [
          { name: "read_file", input: { path: "demo.txt" }, output: "content", status: "success" },
        ],
      },
    ],
  });

  render(<ConversationDetail conversation={conversation} />);

  expect(screen.getByRole("button", { name: "展开工具详情" })).toBeTruthy();
  expect(screen.queryByText('"path": "demo.txt"')).toBeNull();

  await userEvent.click(screen.getByRole("button", { name: "展开工具详情" }));
  expect(screen.getByText('"path": "demo.txt"')).toBeTruthy();
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm run test:run -- src/__tests__/ConversationDetail.test.tsx`
Expected: FAIL because tool call details are always rendered.

- [ ] **Step 3: Write minimal implementation**

```tsx
const [expandedTools, setExpandedTools] = useState<Record<string, boolean>>({});
const toolKey = `${message.id}-${index}`;
const toolExpanded = expandedTools[toolKey] ?? false;

<button
  type="button"
  className="tool-call-toggle"
  onClick={() => setExpandedTools((current) => ({ ...current, [toolKey]: !toolExpanded }))}
>
  {toolExpanded ? "收起工具详情" : "展开工具详情"}
</button>

{toolExpanded && (
  <>
    <pre className="tool-call-input">{JSON.stringify(toolCall.input, null, 2)}</pre>
    {toolCall.output && <div className="tool-call-output">{toolCall.output}</div>}
  </>
)}
```

- [ ] **Step 4: Run focused tests to verify they pass**

Run: `npm run test:run -- src/__tests__/ConversationDetail.test.tsx src/__tests__/ConversationList.test.tsx`
Expected: PASS

- [ ] **Step 5: Run full verification**

Run: `npm run test:run`
Expected: PASS for the full frontend suite
