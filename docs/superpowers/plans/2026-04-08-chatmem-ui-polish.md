# ChatMem UI Polish Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement the approved ChatMem UI polish, then rebuild the portable package at `dist-portable/ChatMem-v0.1.0-portable.zip`.

**Architecture:** Keep the existing React/Tauri structure and concentrate the work in the app shell plus shared CSS tokens. Update user-facing copy in `App.tsx`, rework the toolbar and meta action layout in markup, then tighten the visual system in `styles.css` so the whole interface shifts together.

**Tech Stack:** React 18, TypeScript, Vite, Vitest, Tauri packaging assets already in repo

---

### Task 1: Lock behavior with tests first

**Files:**
- Modify: `D:\VSP\agentswap-gui\src\__tests__\App.test.tsx`
- Test: `D:\VSP\agentswap-gui\src\__tests__\App.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
it("renders the Chinese brand copy, compact refresh control, and grouped copy actions", async () => {
  render(<App />);

  expect(screen.getByText("本地对话记录，一处查看，随时续接")).toBeTruthy();
  expect(screen.getByRole("button", { name: "刷新会话列表" })).toBeTruthy();

  const conversation = await screen.findByText("Debug session");
  fireEvent.click(conversation);

  await waitFor(() => {
    expect(screen.getByText("对话文件位置")).toBeTruthy();
    expect(screen.getByText("操作")).toBeTruthy();
    expect(screen.getByRole("button", { name: "复制位置" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "复制恢复命令" })).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm run test:run -- src/__tests__/App.test.tsx`

Expected: FAIL because the new Chinese brand copy, `刷新会话列表` accessible name, and `操作` label do not exist yet.

- [ ] **Step 3: Write minimal implementation**

Implement only the copy and markup required to satisfy the new assertions in `src/App.tsx`, without changing unrelated logic.

- [ ] **Step 4: Run test to verify it passes**

Run: `npm run test:run -- src/__tests__/App.test.tsx`

Expected: PASS

### Task 2: Apply the approved shell and action-bar polish

**Files:**
- Modify: `D:\VSP\agentswap-gui\src\App.tsx`
- Modify: `D:\VSP\agentswap-gui\src\styles.css`
- Test: `D:\VSP\agentswap-gui\src\__tests__\App.test.tsx`

- [ ] **Step 1: Update the shell markup**

```tsx
<div className="brand-copy">
  <h1>ChatMem</h1>
  <p>本地对话记录，一处查看，随时续接</p>
</div>

<button
  className="toolbar-button"
  type="button"
  onClick={loadConversations}
  aria-label="刷新会话列表"
>
  <span className="toolbar-button-icon" aria-hidden="true">↻</span>
  <span>刷新</span>
</button>
```

```tsx
<div className="conversation-meta-strip">
  <div className="conversation-meta-copy">
    <span className="conversation-meta-label">对话文件位置</span>
    <span className={`conversation-meta-value ${selectedConversation.storage_path ? "" : "is-muted"}`}>
      {selectedConversation.storage_path || "当前来源无法提供文件位置"}
    </span>
  </div>

  <div className="conversation-meta-actions-block">
    <span className="conversation-meta-label">操作</span>
    <div className="conversation-meta-actions">
      ...
    </div>
  </div>
</div>
```

- [ ] **Step 2: Update the visual system**

```css
:root {
  --accent: #2a9d62;
  --accent-strong: #1f7d4e;
  --accent-soft: rgba(42, 157, 98, 0.14);
  --radius-sm: 8px;
  --radius-md: 12px;
  --radius-lg: 16px;
  --radius-xl: 22px;
}
```

```css
.conversation-meta-actions {
  display: flex;
  flex-wrap: wrap;
  gap: 8px;
}

.conversation-meta-actions .btn {
  min-width: 112px;
  white-space: nowrap;
}
```

```css
.toolbar-button {
  ...
}
```

- [ ] **Step 3: Run focused tests**

Run: `npm run test:run -- src/__tests__/App.test.tsx`

Expected: PASS

- [ ] **Step 4: Run the broader UI test set**

Run: `npm run test:run`

Expected: PASS with no new failures in existing component tests.

### Task 3: Build and refresh the portable zip

**Files:**
- Output: `D:\VSP\agentswap-gui\dist`
- Output: `D:\VSP\agentswap-gui\dist-portable\ChatMem-v0.1.0-portable.zip`

- [ ] **Step 1: Run production build**

Run: `npm run build`

Expected: Vite build completes successfully and refreshes the frontend bundle under `dist`.

- [ ] **Step 2: Recreate the portable zip from the current portable directory contents**

Run:

```powershell
$zip = 'D:\VSP\agentswap-gui\dist-portable\ChatMem-v0.1.0-portable.zip'
$src = 'D:\VSP\agentswap-gui\dist-portable\ChatMem'
if (Test-Path $zip) { Remove-Item -LiteralPath $zip -Force }
Compress-Archive -Path (Join-Path $src '*') -DestinationPath $zip -Force
```

Expected: the zip timestamp updates and contains the latest portable app payload.

- [ ] **Step 3: Sanity-check the new package**

Run:

```powershell
Get-Item 'D:\VSP\agentswap-gui\dist-portable\ChatMem-v0.1.0-portable.zip' |
  Select-Object FullName, Length, LastWriteTime
```

Expected: file exists with a fresh `LastWriteTime`.
