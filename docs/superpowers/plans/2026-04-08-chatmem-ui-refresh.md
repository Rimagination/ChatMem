# ChatMem UI Refresh Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Refresh ChatMem into a cleaner Codex-style workspace, expose the backing conversation file location, support one-click copy for location and resume command, add a dedicated app icon, and strengthen user-side message bubbles.

**Architecture:** Extend the Tauri response shape with conversation metadata (`storage_path`, `resume_command`), then render that metadata in a new header strip above the message stream. Keep the existing two-pane shell, but restyle the list and detail areas around a calmer workspace hierarchy and Codex-style user bubbles.

**Tech Stack:** React 18, TypeScript, Vitest, Tauri 1.x, Rust, local icon assets

---

### Task 1: Add failing UI tests for metadata strip and message bubble treatment

**Files:**
- Modify: `D:\VSP\agentswap-gui\src\__tests__\App.test.tsx`
- Create: `D:\VSP\agentswap-gui\src\__tests__\ConversationDetail.test.tsx`

- [ ] **Step 1: Write the failing App metadata test**

```tsx
import { render, screen, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import App from "../App";

vi.mock("@tauri-apps/api/tauri", () => ({
  invoke: vi.fn(async (command: string) => {
    if (command === "list_conversations") {
      return [{
        id: "conv-001",
        source_agent: "codex",
        project_dir: "D:/VSP/demo",
        created_at: "2026-04-08T08:00:00Z",
        updated_at: "2026-04-08T09:00:00Z",
        summary: "Debug session",
        message_count: 2,
        file_count: 1,
      }];
    }

    if (command === "read_conversation") {
      return {
        id: "conv-001",
        source_agent: "codex",
        project_dir: "D:/VSP/demo",
        created_at: "2026-04-08T08:00:00Z",
        updated_at: "2026-04-08T09:00:00Z",
        summary: "Debug session",
        storage_path: "C:/Users/demo/.codex/sessions/2026/04/08/rollout-conv-001.jsonl",
        resume_command: "codex resume conv-001",
        messages: [],
        file_changes: [],
      };
    }

    return [];
  }),
}));

test("renders file location and copy actions for selected conversation", async () => {
  render(<App />);

  await screen.findByText("Debug session");
  screen.getByText("Debug session").click();

  await waitFor(() => {
    expect(screen.getByText("对话文件位置")).toBeTruthy();
    expect(screen.getByText("复制位置")).toBeTruthy();
    expect(screen.getByText("复制恢复命令")).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run the App metadata test to verify it fails**

Run: `npm run test:run -- src/__tests__/App.test.tsx`
Expected: FAIL because the metadata strip and copy actions do not exist yet.

- [ ] **Step 3: Write the failing message bubble test**

```tsx
import { render } from "@testing-library/react";
import ConversationDetail from "../components/ConversationDetail";

test("renders user messages with dedicated user bubble structure", () => {
  const conversation = {
    id: "conv-001",
    source_agent: "codex",
    project_dir: "D:/VSP/demo",
    created_at: "2026-04-08T08:00:00Z",
    updated_at: "2026-04-08T09:00:00Z",
    summary: "Debug session",
    storage_path: "C:/tmp/rollout.jsonl",
    resume_command: "codex resume conv-001",
    messages: [{
      id: "m1",
      timestamp: "2026-04-08T08:30:00Z",
      role: "user",
      content: "Open the config file",
      tool_calls: [],
      metadata: {},
    }],
    file_changes: [],
  };

  const { container } = render(<ConversationDetail conversation={conversation} />);
  expect(container.querySelector(".message-user .message-bubble")).toBeTruthy();
});
```

- [ ] **Step 4: Run the message bubble test to verify it fails**

Run: `npm run test:run -- src/__tests__/ConversationDetail.test.tsx`
Expected: FAIL if the component typing or structure does not yet support the new conversation shape and bubble contract.

- [ ] **Step 5: Commit**

Skip commit because `D:\VSP\agentswap-gui` is not a Git repository.

### Task 2: Add metadata helpers and backend response fields

**Files:**
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\main.rs`

- [ ] **Step 1: Write the failing Rust helper tests**

```rust
#[test]
fn builds_resume_command_for_codex() {
    assert_eq!(build_resume_command("codex", "conv-001"), Some("codex resume conv-001".to_string()));
}

#[test]
fn returns_none_for_unknown_agent_resume_command() {
    assert_eq!(build_resume_command("unknown", "conv-001"), None);
}
```

- [ ] **Step 2: Run the Rust helper tests to verify they fail**

Run: `cargo test --manifest-path src-tauri/Cargo.toml build_resume_command`
Expected: FAIL because the helper does not exist yet.

- [ ] **Step 3: Implement minimal backend metadata support**

```rust
fn build_resume_command(agent: &str, id: &str) -> Option<String> {
    match agent {
        "claude" => Some(format!("claude --resume {}", id)),
        "codex" => Some(format!("codex resume {}", id)),
        "gemini" => Some(format!("gemini --resume {}", id)),
        _ => None,
    }
}
```

Also extend `ConversationResponse` with:

```rust
storage_path: Option<String>,
resume_command: Option<String>,
```

and populate those fields inside `read_conversation`.

- [ ] **Step 4: Add minimal storage path resolution**

Implement per-agent path lookup directly in `main.rs`:

- Claude: search `~/.claude/projects/*/<id>.jsonl`
- Codex: query the local thread DB for `rollout_path`
- Gemini: search `~/.gemini/tmp/*/chats/session-<id>.json` and `~/.gemini/tmp/*/chats/<id>.json`

Return `None` when the path cannot be resolved.

- [ ] **Step 5: Run Rust tests to verify they pass**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: PASS for the new helper tests and existing Tauri Rust unit tests.

- [ ] **Step 6: Commit**

Skip commit because `D:\VSP\agentswap-gui` is not a Git repository.

### Task 3: Render metadata strip, copy actions, refined chat UI, and icon assets

**Files:**
- Modify: `D:\VSP\agentswap-gui\src\App.tsx`
- Modify: `D:\VSP\agentswap-gui\src\components\ConversationDetail.tsx`
- Modify: `D:\VSP\agentswap-gui\src\components\ConversationList.tsx`
- Modify: `D:\VSP\agentswap-gui\src\styles.css`
- Modify: `D:\VSP\agentswap-gui\src\__tests__\App.test.tsx`
- Create: `D:\VSP\agentswap-gui\src\__tests__\ConversationDetail.test.tsx`
- Modify: `D:\VSP\agentswap-gui\src-tauri\icons\32x32.png`
- Modify: `D:\VSP\agentswap-gui\src-tauri\icons\128x128.png`
- Modify: `D:\VSP\agentswap-gui\src-tauri\icons\128x128@2x.png`
- Modify: `D:\VSP\agentswap-gui\src-tauri\icons\icon.ico`

- [ ] **Step 1: Implement the App metadata strip and clipboard handlers**

Add `storage_path` and `resume_command` to the conversation type and render:

```tsx
<div className="conversation-meta-strip">
  <div className="conversation-meta-copy">
    <span className="conversation-meta-label">对话文件位置</span>
    <code className="conversation-meta-value">{selectedConversation.storage_path ?? "当前来源不可提供文件位置"}</code>
  </div>
  <div className="conversation-meta-actions">
    <button className="btn btn-secondary">复制位置</button>
    <button className="btn btn-secondary">复制恢复命令</button>
  </div>
</div>
```

Use `navigator.clipboard.writeText()` with temporary success/failure button text.

- [ ] **Step 2: Implement the refined message layout**

Keep message grouping in `ConversationDetail`, but make sure the message body uses:

```tsx
<div className={`message message-${message.role}`}>
  <div className="message-shell">
    <div className="message-header">...</div>
    <div className="message-bubble">...</div>
  </div>
</div>
```

This preserves a stable CSS target for stronger user bubbles.

- [ ] **Step 3: Restyle the interface**

Update `styles.css` to:

- reduce left-list card heaviness
- elevate the right-side reading area
- introduce a compact metadata strip under the title
- strengthen `.message-user .message-bubble`
- keep assistant and system surfaces quieter than user messages

- [ ] **Step 4: Replace icon assets**

Generate a simple ChatMem icon with:

- bright neutral base
- compact chat bubble silhouette
- memory cue accent mark

Write the generated image into the existing Tauri icon files.

- [ ] **Step 5: Run focused UI tests**

Run: `npm run test:run -- src/__tests__/App.test.tsx src/__tests__/ConversationDetail.test.tsx src/__tests__/ConversationList.test.tsx`
Expected: PASS

- [ ] **Step 6: Run the frontend build**

Run: `npm run build`
Expected: PASS

- [ ] **Step 7: Commit**

Skip commit because `D:\VSP\agentswap-gui` is not a Git repository.
