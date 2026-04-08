# ChatMem Full-Text Search Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Fix ChatMem search so terms that appear in conversation bodies can be found, then rebuild the portable package.

**Architecture:** Move search out of the frontend-only summary filter and into a dedicated Tauri command. The frontend will call `search_conversations` when a query is present, while the backend will scan summaries plus full conversation content so the result set matches what users actually read in the app.

**Tech Stack:** React 18, TypeScript, Vitest, Rust, Tauri

---

### Task 1: Lock the user-visible search bug with a failing UI test

**Files:**
- Modify: `D:\VSP\agentswap-gui\src\__tests__\App.test.tsx`
- Test: `D:\VSP\agentswap-gui\src\__tests__\App.test.tsx`

- [ ] **Step 1: Write the failing test**

```tsx
it("searches conversations by message body content", async () => {
  render(<App />);

  const input = screen.getByPlaceholderText("搜索对话...");
  fireEvent.change(input, { target: { value: "内存泄漏" } });

  await waitFor(() => {
    expect(mockInvoke).toHaveBeenCalledWith("search_conversations", {
      agent: "claude",
      query: "内存泄漏",
    });
    expect(screen.getByText("Memory investigation")).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run test to verify it fails**

Run: `npm run test:run -- src/__tests__/App.test.tsx`

Expected: FAIL because `App.tsx` still only performs local summary filtering and never calls `search_conversations`.

- [ ] **Step 3: Write minimal implementation**

Implement the frontend command switch and backend `search_conversations` command, but only enough to satisfy this user-visible behavior.

- [ ] **Step 4: Run test to verify it passes**

Run: `npm run test:run -- src/__tests__/App.test.tsx`

Expected: PASS

### Task 2: Implement backend full-text search and connect the app shell

**Files:**
- Modify: `D:\VSP\agentswap-gui\src\App.tsx`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\main.rs`
- Test: `D:\VSP\agentswap-gui\src\__tests__\App.test.tsx`

- [ ] **Step 1: Switch the frontend to backend-backed search**

```tsx
const loadConversations = async (query = searchQuery) => {
  const trimmed = query.trim();
  const command = trimmed ? "search_conversations" : "list_conversations";
  const payload = trimmed ? { agent: selectedAgent, query: trimmed } : { agent: selectedAgent };
  const result = await invoke<ConversationSummary[]>(command, payload);
  setConversations(result);
};
```

- [ ] **Step 2: Add backend full-text helpers and command**

```rust
fn conversation_matches_query(conversation: &Conversation, query: &str) -> bool {
    conversation.id.to_lowercase().contains(query)
        || conversation.project_dir.to_lowercase().contains(query)
        || conversation.summary.as_deref().unwrap_or_default().to_lowercase().contains(query)
        || conversation.messages.iter().any(|message| message.content.to_lowercase().contains(query))
}
```

```rust
#[command]
async fn search_conversations(agent: String, query: String) -> Result<Vec<ConversationSummaryResponse>, String> {
    ...
}
```

- [ ] **Step 3: Register the new Tauri command**

```rust
.invoke_handler(tauri::generate_handler![
    list_conversations,
    search_conversations,
    read_conversation,
    migrate_conversation,
    delete_conversation,
    check_agent_available,
])
```

- [ ] **Step 4: Run the app tests**

Run: `npm run test:run`

Expected: PASS

### Task 3: Rebuild the desktop app and refresh the portable zip

**Files:**
- Output: `D:\VSP\agentswap-gui\src-tauri\target\release\ChatMem.exe`
- Output: `D:\VSP\agentswap-gui\dist-portable\ChatMem\ChatMem.exe`
- Output: `D:\VSP\agentswap-gui\dist-portable\ChatMem-v0.1.0-portable.zip`

- [ ] **Step 1: Build the desktop app**

Run: `npm run tauri build`

Expected: PASS and a fresh `src-tauri\target\release\ChatMem.exe`.

- [ ] **Step 2: Refresh the portable directory and zip**

Run:

```powershell
$exe='D:\VSP\agentswap-gui\src-tauri\target\release\ChatMem.exe'
$portableDir='D:\VSP\agentswap-gui\dist-portable\ChatMem'
$portableExe=Join-Path $portableDir 'ChatMem.exe'
Copy-Item -LiteralPath $exe -Destination $portableExe -Force
$zip='D:\VSP\agentswap-gui\dist-portable\ChatMem-v0.1.0-portable.zip'
if (Test-Path $zip) { Remove-Item -LiteralPath $zip -Force }
Compress-Archive -Path (Join-Path $portableDir '*') -DestinationPath $zip -Force
```

- [ ] **Step 3: Verify the package artifact**

Run:

```powershell
Get-Item 'D:\VSP\agentswap-gui\dist-portable\ChatMem-v0.1.0-portable.zip' |
  Select-Object FullName, Length, LastWriteTime
```

Expected: the zip exists with a fresh timestamp after the search fix.
