# ChatMem MCP Memory Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a repository-scoped ChatMem memory system with a local MCP server, approval-based memory management in the ChatMem desktop app, and thin Codex app / Claude Code integration shells that share the same memory core.

**Architecture:** Keep ChatMem as the human control surface, but move agent-facing memory access into a Rust MCP sidecar that reads from a ChatMem-owned SQLite asset store. Reuse the existing AgentSwap adapters to import canonical conversation evidence, then expose compact startup memory, repository history search, candidate creation, and handoff generation through five MCP tools and matching Tauri UI commands.

**Tech Stack:** React 18, TypeScript, Vitest, Tauri 1.x, Rust, rusqlite, rmcp, local plugin manifests, PowerShell

---

## Planned File Structure

**Backend core**

- Modify: `D:\VSP\agentswap-gui\src-tauri\Cargo.toml`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\lib.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\main.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\mod.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\db.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\models.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\repo_identity.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\sync.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\store.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\search.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\handoff.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\mcp.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\bin\chatmem-mcp.rs`

**Frontend app**

- Modify: `D:\VSP\agentswap-gui\src\App.tsx`
- Modify: `D:\VSP\agentswap-gui\src\styles.css`
- Modify: `D:\VSP\agentswap-gui\src\i18n\strings.ts`
- Modify: `D:\VSP\agentswap-gui\src\i18n\types.ts`
- Create: `D:\VSP\agentswap-gui\src\chatmem-memory\api.ts`
- Create: `D:\VSP\agentswap-gui\src\chatmem-memory\types.ts`
- Create: `D:\VSP\agentswap-gui\src\components\RepoMemoryPanel.tsx`
- Create: `D:\VSP\agentswap-gui\src\components\MemoryInboxPanel.tsx`
- Create: `D:\VSP\agentswap-gui\src\components\EpisodesPanel.tsx`
- Create: `D:\VSP\agentswap-gui\src\components\HandoffsPanel.tsx`

**Tests**

- Modify: `D:\VSP\agentswap-gui\src\__tests__\App.test.tsx`
- Create: `D:\VSP\agentswap-gui\src\__tests__\MemoryWorkspace.test.tsx`
- Create: `D:\VSP\agentswap-gui\src\__tests__\integrationManifests.test.ts`

**Integration shells and docs**

- Create: `D:\VSP\agentswap-gui\plugins\chatmem\.codex-plugin\plugin.json`
- Create: `D:\VSP\agentswap-gui\plugins\chatmem\.claude-plugin\plugin.json`
- Create: `D:\VSP\agentswap-gui\plugins\chatmem\skills\chatmem\SKILL.md`
- Create: `D:\VSP\agentswap-gui\plugins\chatmem\.mcp.json`
- Create: `D:\VSP\agentswap-gui\.agents\plugins\marketplace.json`
- Create: `D:\VSP\agentswap-gui\scripts\sync-chatmem-plugin.ps1`
- Create: `D:\VSP\agentswap-gui\docs\CHATMEM_MCP_SETUP.md`

### Task 1: Scaffold the Rust memory core and lock the schema with failing tests

**Files:**
- Modify: `D:\VSP\agentswap-gui\src-tauri\Cargo.toml`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\lib.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\main.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\mod.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\db.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\models.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\repo_identity.rs`
- Test: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\db.rs`
- Test: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\repo_identity.rs`

- [ ] **Step 1: Write the failing backend tests for repository identity and schema creation**

```rust
// src-tauri/src/chatmem_memory/repo_identity.rs
#[cfg(test)]
mod tests {
    use super::{fingerprint_repo, normalize_repo_root};

    #[test]
    fn normalizes_windows_repo_root() {
        assert_eq!(
            normalize_repo_root(r"D:\VSP\agentswap-gui\"),
            "d:/vsp/agentswap-gui"
        );
    }

    #[test]
    fn fingerprint_is_stable_for_equivalent_repo_inputs() {
        let left = fingerprint_repo(
            r"D:\VSP\agentswap-gui\",
            Some("git@github.com:Rimagination/ChatMem.git"),
            Some("main"),
        );
        let right = fingerprint_repo(
            "d:/vsp/agentswap-gui",
            Some("git@github.com:Rimagination/ChatMem.git"),
            Some("main"),
        );

        assert_eq!(left, right);
    }
}
```

```rust
// src-tauri/src/chatmem_memory/db.rs
#[cfg(test)]
mod tests {
    use super::{migrate, open_in_memory};

    #[test]
    fn migrations_create_memory_tables() {
        let conn = open_in_memory().unwrap();
        migrate(&conn).unwrap();

        let mut stmt = conn
            .prepare(
                "SELECT name FROM sqlite_master
                 WHERE type='table' AND name IN (
                   'repos',
                   'conversations',
                   'messages',
                   'tool_calls',
                   'file_changes',
                   'episodes',
                   'memory_candidates',
                   'approved_memories',
                   'handoff_packets',
                   'evidence_refs',
                   'search_documents'
                 )
                 ORDER BY name",
            )
            .unwrap();

        let names = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(
            names,
            vec![
                "approved_memories",
                "conversations",
                "episodes",
                "evidence_refs",
                "file_changes",
                "handoff_packets",
                "memory_candidates",
                "messages",
                "repos",
                "search_documents",
                "tool_calls",
            ]
        );
    }
}
```

- [ ] **Step 2: Run the targeted Rust tests to verify they fail**

Run:

```powershell
cargo test --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml chatmem_memory
```

Expected: FAIL because the `chatmem_memory` module, migration helpers, and repository identity helpers do not exist yet.

- [ ] **Step 3: Add the dependency and module skeleton**

Update `Cargo.toml` to add MCP and repository fingerprint support:

```toml
[dependencies]
serde_json = "1"
serde = { version = "1", features = ["derive"] }
tauri = { version = "1", features = ["shell-open", "updater"] }
anyhow = "1"
chrono = { version = "0.4", features = ["serde"] }
uuid = { version = "1", features = ["v4", "v5", "serde"] }
walkdir = "2"
dirs = "5"
rusqlite = { version = "0.31", features = ["bundled"] }
rmcp = { version = "0.7", features = ["server", "transport-io"] }
```

Create the module tree:

```rust
// src-tauri/src/lib.rs
pub mod chatmem_memory;
```

```rust
// src-tauri/src/chatmem_memory/mod.rs
pub mod db;
pub mod models;
pub mod repo_identity;
pub mod sync;
pub mod store;
pub mod search;
pub mod handoff;
pub mod mcp;
```

Implement the minimal DB helpers:

```rust
// src-tauri/src/chatmem_memory/db.rs
use anyhow::Result;
use rusqlite::Connection;

pub fn open_in_memory() -> Result<Connection> {
    Ok(Connection::open_in_memory()?)
}

pub fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS repos (
            repo_id TEXT PRIMARY KEY,
            repo_root TEXT NOT NULL UNIQUE,
            repo_fingerprint TEXT NOT NULL,
            git_remote TEXT,
            default_branch TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS conversations (
            conversation_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            source_agent TEXT NOT NULL,
            source_conversation_id TEXT NOT NULL,
            summary TEXT,
            started_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            storage_path TEXT
        );

        CREATE TABLE IF NOT EXISTS messages (
            message_id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            role TEXT NOT NULL,
            content TEXT NOT NULL,
            timestamp TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS tool_calls (
            tool_call_id TEXT PRIMARY KEY,
            message_id TEXT NOT NULL,
            name TEXT NOT NULL,
            input_json TEXT NOT NULL,
            output_text TEXT,
            status TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS file_changes (
            file_change_id TEXT PRIMARY KEY,
            conversation_id TEXT NOT NULL,
            message_id TEXT NOT NULL,
            path TEXT NOT NULL,
            change_type TEXT NOT NULL,
            timestamp TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS episodes (
            episode_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            title TEXT NOT NULL,
            summary TEXT NOT NULL,
            outcome TEXT NOT NULL,
            created_at TEXT NOT NULL,
            source_conversation_id TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS memory_candidates (
            candidate_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            summary TEXT NOT NULL,
            value TEXT NOT NULL,
            why_it_matters TEXT NOT NULL,
            confidence REAL NOT NULL,
            proposed_by TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL,
            reviewed_at TEXT
        );

        CREATE TABLE IF NOT EXISTS approved_memories (
            memory_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            kind TEXT NOT NULL,
            title TEXT NOT NULL,
            value TEXT NOT NULL,
            usage_hint TEXT NOT NULL,
            status TEXT NOT NULL,
            last_verified_at TEXT,
            created_from_candidate_id TEXT,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS handoff_packets (
            handoff_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            from_agent TEXT NOT NULL,
            to_agent TEXT NOT NULL,
            current_goal TEXT NOT NULL,
            done_json TEXT NOT NULL,
            next_json TEXT NOT NULL,
            key_files_json TEXT NOT NULL,
            commands_json TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS evidence_refs (
            evidence_id TEXT PRIMARY KEY,
            owner_type TEXT NOT NULL,
            owner_id TEXT NOT NULL,
            conversation_id TEXT,
            message_id TEXT,
            tool_call_id TEXT,
            file_change_id TEXT,
            excerpt TEXT NOT NULL,
            created_at TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS search_documents (
            doc_id TEXT PRIMARY KEY,
            repo_id TEXT NOT NULL,
            doc_type TEXT NOT NULL,
            doc_ref_id TEXT NOT NULL,
            title TEXT NOT NULL,
            body TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS search_documents_fts USING fts5(
            doc_id UNINDEXED,
            title,
            body
        );
        ",
    )?;

    Ok(())
}
```

- [ ] **Step 4: Wire the module into the Tauri backend**

Use the shared library crate from `main.rs`:

```rust
use chatmem::chatmem_memory;
```

Add the repository identity helpers:

```rust
// src-tauri/src/chatmem_memory/repo_identity.rs
pub fn normalize_repo_root(input: &str) -> String {
    input.trim_end_matches(['\\', '/'])
        .replace('\\', "/")
        .to_lowercase()
}

pub fn fingerprint_repo(
    repo_root: &str,
    git_remote: Option<&str>,
    branch: Option<&str>,
) -> String {
    let key = format!(
        "{}|{}|{}",
        normalize_repo_root(repo_root),
        git_remote.unwrap_or_default(),
        branch.unwrap_or_default()
    );

    uuid::Uuid::new_v5(&uuid::Uuid::NAMESPACE_URL, key.as_bytes()).to_string()
}
```

- [ ] **Step 5: Run the Rust tests again**

Run:

```powershell
cargo test --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml chatmem_memory
```

Expected: PASS for the new schema and repository identity tests.

- [ ] **Step 6: Commit**

Run:

```powershell
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui add `
  src-tauri/Cargo.toml `
  src-tauri/src/lib.rs `
  src-tauri/src/main.rs `
  src-tauri/src/chatmem_memory
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui commit -m "feat: scaffold ChatMem memory core"
```

Expected: a commit containing the new Rust module tree and schema foundation.

### Task 2: Import canonical conversation evidence and expose Tauri memory commands

**Files:**
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\main.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\sync.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\store.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\search.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\handoff.rs`
- Test: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\store.rs`
- Test: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\handoff.rs`

- [ ] **Step 1: Write failing tests for candidate review and handoff assembly**

```rust
// src-tauri/src/chatmem_memory/store.rs
#[cfg(test)]
mod tests {
    use super::{MemoryStore, ReviewAction};
    use crate::chatmem_memory::db::{migrate, open_in_memory};

    #[test]
    fn approving_a_candidate_promotes_it_to_approved_memory() {
        let conn = open_in_memory().unwrap();
        migrate(&conn).unwrap();
        let store = MemoryStore::new(conn);

        let candidate_id = store
            .create_candidate("repo-1", "command", "Run tests", "npm run test:run", "Needed before merge", 0.92, "codex")
            .unwrap();

        store
            .review_candidate(&candidate_id, ReviewAction::Approve {
                title: "Primary test command".into(),
                usage_hint: "Inject on startup when the task hint mentions tests".into(),
            })
            .unwrap();

        let memories = store.list_repo_memories("repo-1").unwrap();
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].value, "npm run test:run");
    }
}
```

```rust
// src-tauri/src/chatmem_memory/handoff.rs
#[cfg(test)]
mod tests {
    use super::build_handoff_packet;

    #[test]
    fn handoff_packet_prefers_recent_commands_and_next_steps() {
        let packet = build_handoff_packet(
            "repo-1",
            "codex",
            "claude",
            Some("Finish the MCP search tool"),
            vec!["Schema migrated".into()],
            vec!["Implement search ranking".into()],
            vec!["src-tauri/src/chatmem_memory/search.rs".into()],
            vec!["cargo test --manifest-path src-tauri/Cargo.toml chatmem_memory".into()],
        );

        assert_eq!(packet.current_goal, "Finish the MCP search tool");
        assert_eq!(packet.to_agent, "claude");
        assert_eq!(packet.next_items.len(), 1);
    }
}
```

- [ ] **Step 2: Run the targeted Rust tests to verify they fail**

Run:

```powershell
cargo test --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml approving_a_candidate
cargo test --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml handoff_packet_prefers_recent_commands
```

Expected: FAIL because the store layer, review flow, and handoff builder do not exist yet.

- [ ] **Step 3: Implement the synchronization and asset store**

Create a store with canonical read/write operations:

```rust
// src-tauri/src/chatmem_memory/store.rs
pub struct MemoryStore {
    conn: rusqlite::Connection,
}

pub enum ReviewAction {
    Approve { title: String, usage_hint: String },
    ApproveWithEdit { title: String, value: String, usage_hint: String },
    Reject,
    Snooze,
}

impl MemoryStore {
    pub fn new(conn: rusqlite::Connection) -> Self {
        Self { conn }
    }

    pub fn create_candidate(
        &self,
        repo_id: &str,
        kind: &str,
        summary: &str,
        value: &str,
        why_it_matters: &str,
        confidence: f64,
        proposed_by: &str,
    ) -> anyhow::Result<String> {
        let candidate_id = uuid::Uuid::new_v4().to_string();
        self.conn.execute(
            "INSERT INTO memory_candidates (
               candidate_id, repo_id, kind, summary, value, why_it_matters,
               confidence, proposed_by, status, created_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'pending_review', ?9)",
            rusqlite::params![
                candidate_id,
                repo_id,
                kind,
                summary,
                value,
                why_it_matters,
                confidence,
                proposed_by,
                chrono::Utc::now().to_rfc3339(),
            ],
        )?;
        Ok(candidate_id)
    }
}
```

Create repository sync helpers that reuse the existing adapters:

```rust
// src-tauri/src/chatmem_memory/sync.rs
pub fn sync_conversation_into_repo(
    store: &MemoryStore,
    agent: &str,
    conversation: &agentswap_core::types::Conversation,
) -> anyhow::Result<String> {
    let repo_root = crate::chatmem_memory::repo_identity::normalize_repo_root(&conversation.project_dir);
    let repo_id = store.upsert_repo(&repo_root, None, None)?;
    store.upsert_conversation_snapshot(&repo_id, agent, conversation)?;
    Ok(repo_id)
}
```

Create a handoff builder:

```rust
// src-tauri/src/chatmem_memory/handoff.rs
pub fn build_handoff_packet(
    repo_id: &str,
    from_agent: &str,
    to_agent: &str,
    goal_hint: Option<&str>,
    done_items: Vec<String>,
    next_items: Vec<String>,
    key_files: Vec<String>,
    useful_commands: Vec<String>,
) -> HandoffPacket {
    HandoffPacket {
        handoff_id: uuid::Uuid::new_v4().to_string(),
        repo_id: repo_id.to_string(),
        from_agent: from_agent.to_string(),
        to_agent: to_agent.to_string(),
        current_goal: goal_hint.unwrap_or("Continue repository work").to_string(),
        done_items,
        next_items,
        key_files,
        useful_commands,
        created_at: chrono::Utc::now().to_rfc3339(),
    }
}
```

- [ ] **Step 4: Expose the Tauri commands for the desktop UI**

Add command functions in `main.rs`:

```rust
#[command]
async fn list_repo_memories(repo_root: String) -> Result<Vec<ApprovedMemoryResponse>, String> {
    let store = open_memory_store().map_err(|e| e.to_string())?;
    let repo_id = store.ensure_repo(&repo_root).map_err(|e| e.to_string())?;
    store.list_repo_memories(&repo_id).map_err(|e| e.to_string())
}

#[command]
async fn list_memory_candidates(repo_root: String) -> Result<Vec<MemoryCandidateResponse>, String> {
    let store = open_memory_store().map_err(|e| e.to_string())?;
    let repo_id = store.ensure_repo(&repo_root).map_err(|e| e.to_string())?;
    store.list_candidates(&repo_id).map_err(|e| e.to_string())
}

#[command]
async fn review_memory_candidate(
    candidate_id: String,
    action: String,
    edited_title: Option<String>,
    edited_value: Option<String>,
    edited_usage_hint: Option<String>,
) -> Result<(), String> {
    let store = open_memory_store().map_err(|e| e.to_string())?;
    let review = match action.as_str() {
        "approve" => ReviewAction::Approve {
            title: edited_title.unwrap_or_else(|| "Approved memory".into()),
            usage_hint: edited_usage_hint.unwrap_or_else(|| "Used for startup injection".into()),
        },
        "approve_with_edit" => ReviewAction::ApproveWithEdit {
            title: edited_title.unwrap_or_else(|| "Approved memory".into()),
            value: edited_value.unwrap_or_default(),
            usage_hint: edited_usage_hint.unwrap_or_else(|| "Used for startup injection".into()),
        },
        "reject" => ReviewAction::Reject,
        _ => ReviewAction::Snooze,
    };

    store.review_candidate(&candidate_id, review).map_err(|e| e.to_string())
}
```

Register them:

```rust
.invoke_handler(tauri::generate_handler![
    list_conversations,
    search_conversations,
    read_conversation,
    migrate_conversation,
    delete_conversation,
    check_agent_available,
    list_repo_memories,
    list_memory_candidates,
    review_memory_candidate,
    list_episodes,
    list_handoffs,
])
```

- [ ] **Step 5: Run Rust tests to verify the store and handoff flows pass**

Run:

```powershell
cargo test --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml chatmem_memory
```

Expected: PASS for the new asset-store and handoff tests, plus the earlier schema tests.

- [ ] **Step 6: Commit**

Run:

```powershell
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui add `
  src-tauri/src/main.rs `
  src-tauri/src/chatmem_memory
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui commit -m "feat: add ChatMem memory store and Tauri commands"
```

Expected: a commit that introduces repository sync, candidate review, and desktop command endpoints.

### Task 3: Build the ChatMem memory management workspace in the desktop app

**Files:**
- Modify: `D:\VSP\agentswap-gui\src\App.tsx`
- Modify: `D:\VSP\agentswap-gui\src\styles.css`
- Modify: `D:\VSP\agentswap-gui\src\i18n\strings.ts`
- Modify: `D:\VSP\agentswap-gui\src\i18n\types.ts`
- Create: `D:\VSP\agentswap-gui\src\chatmem-memory\api.ts`
- Create: `D:\VSP\agentswap-gui\src\chatmem-memory\types.ts`
- Create: `D:\VSP\agentswap-gui\src\components\RepoMemoryPanel.tsx`
- Create: `D:\VSP\agentswap-gui\src\components\MemoryInboxPanel.tsx`
- Create: `D:\VSP\agentswap-gui\src\components\EpisodesPanel.tsx`
- Create: `D:\VSP\agentswap-gui\src\components\HandoffsPanel.tsx`
- Modify: `D:\VSP\agentswap-gui\src\__tests__\App.test.tsx`
- Create: `D:\VSP\agentswap-gui\src\__tests__\MemoryWorkspace.test.tsx`

- [ ] **Step 1: Write failing UI tests for the memory workspace**

```tsx
// src/__tests__/MemoryWorkspace.test.tsx
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import App from "../App";

const mockInvoke = vi.fn(async (command: string) => {
  if (command === "list_conversations") {
    return [{
      id: "conv-001",
      source_agent: "codex",
      project_dir: "D:/VSP/agentswap-gui",
      created_at: "2026-04-19T08:00:00Z",
      updated_at: "2026-04-19T09:00:00Z",
      summary: "MCP memory work",
      message_count: 3,
      file_count: 2,
    }];
  }

  if (command === "read_conversation") {
    return {
      id: "conv-001",
      source_agent: "codex",
      project_dir: "D:/VSP/agentswap-gui",
      created_at: "2026-04-19T08:00:00Z",
      updated_at: "2026-04-19T09:00:00Z",
      summary: "MCP memory work",
      storage_path: "C:/Users/Liang/.codex/rollout.jsonl",
      resume_command: "codex resume conv-001",
      messages: [],
      file_changes: [],
    };
  }

  if (command === "list_memory_candidates") {
    return [{
      candidate_id: "cand-001",
      kind: "command",
      summary: "Use npm run test:run before merge",
      value: "npm run test:run",
      why_it_matters: "Repository test gate",
      confidence: 0.95,
      proposed_by: "codex",
      status: "pending_review",
    }];
  }

  if (command === "list_repo_memories") {
    return [];
  }

  return [];
});

vi.mock("@tauri-apps/api/tauri", () => ({
  invoke: (command: string, payload?: unknown) => mockInvoke(command, payload),
}));

test("shows memory inbox for the selected repository", async () => {
  render(<App />);
  await screen.findByText("MCP memory work");
  fireEvent.click(screen.getByText("MCP memory work"));
  fireEvent.click(screen.getByRole("button", { name: "Memory Inbox" }));

  await waitFor(() => {
    expect(screen.getByText("Use npm run test:run before merge")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Approve" })).toBeTruthy();
    expect(screen.getByRole("button", { name: "Reject" })).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run the frontend test to verify it fails**

Run:

```powershell
npm run test:run -- src/__tests__/MemoryWorkspace.test.tsx
```

Expected: FAIL because the workspace tabs, API client, and memory panel components do not exist yet.

- [ ] **Step 3: Add the API client and app-level workspace state**

Create typed API helpers:

```ts
// src/chatmem-memory/types.ts
export type MemoryCandidate = {
  candidate_id: string;
  kind: string;
  summary: string;
  value: string;
  why_it_matters: string;
  confidence: number;
  proposed_by: string;
  status: string;
};

export type ApprovedMemory = {
  memory_id: string;
  kind: string;
  title: string;
  value: string;
  usage_hint: string;
  status: string;
  last_verified_at: string | null;
};
```

```ts
// src/chatmem-memory/api.ts
import { invoke } from "@tauri-apps/api/tauri";
import type { ApprovedMemory, MemoryCandidate } from "./types";

export function listRepoMemories(repoRoot: string) {
  return invoke<ApprovedMemory[]>("list_repo_memories", { repoRoot });
}

export function listMemoryCandidates(repoRoot: string) {
  return invoke<MemoryCandidate[]>("list_memory_candidates", { repoRoot });
}

export function reviewMemoryCandidate(payload: {
  candidateId: string;
  action: "approve" | "approve_with_edit" | "reject" | "snooze";
  editedTitle?: string;
  editedValue?: string;
  editedUsageHint?: string;
}) {
  return invoke("review_memory_candidate", payload);
}
```

Add workspace mode and current repo derivation in `App.tsx`:

```tsx
type WorkspaceView = "conversation" | "repo-memory" | "memory-inbox" | "episodes" | "handoffs";

const [workspaceView, setWorkspaceView] = useState<WorkspaceView>("conversation");
const activeRepoRoot = selectedConversation?.project_dir ?? null;
```

- [ ] **Step 4: Render the new panels and navigation**

Add header tabs:

```tsx
<div className="workspace-mode-tabs">
  <button onClick={() => setWorkspaceView("conversation")}>Conversation</button>
  <button onClick={() => setWorkspaceView("repo-memory")} disabled={!activeRepoRoot}>Repo Memory</button>
  <button onClick={() => setWorkspaceView("memory-inbox")} disabled={!activeRepoRoot}>Memory Inbox</button>
  <button onClick={() => setWorkspaceView("episodes")} disabled={!activeRepoRoot}>Episodes</button>
  <button onClick={() => setWorkspaceView("handoffs")} disabled={!activeRepoRoot}>Handoffs</button>
</div>
```

Create the inbox panel:

```tsx
// src/components/MemoryInboxPanel.tsx
export default function MemoryInboxPanel({
  candidates,
  onApprove,
  onReject,
}: {
  candidates: MemoryCandidate[];
  onApprove: (candidate: MemoryCandidate) => void;
  onReject: (candidateId: string) => void;
}) {
  return (
    <section className="memory-panel">
      <h3>Memory Inbox</h3>
      <div className="memory-card-list">
        {candidates.map((candidate) => (
          <article key={candidate.candidate_id} className="memory-card">
            <div className="memory-card-header">
              <strong>{candidate.summary}</strong>
              <span>{candidate.kind}</span>
            </div>
            <p>{candidate.why_it_matters}</p>
            <div className="memory-card-meta">
              <span>{candidate.proposed_by}</span>
              <span>{candidate.confidence.toFixed(2)}</span>
            </div>
            <div className="memory-card-actions">
              <button onClick={() => onApprove(candidate)}>Approve</button>
              <button onClick={() => onReject(candidate.candidate_id)}>Reject</button>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}
```

Update `styles.css` with dedicated panel and tab styles:

```css
.workspace-mode-tabs {
  display: flex;
  gap: 8px;
  margin-top: 12px;
}

.memory-panel {
  display: flex;
  flex-direction: column;
  gap: 16px;
}

.memory-card {
  border: 1px solid rgba(15, 23, 42, 0.12);
  border-radius: 8px;
  padding: 16px;
  background: #fff;
}
```

Add matching translation keys in `strings.ts` and `types.ts`.

- [ ] **Step 5: Run the frontend tests and build**

Run:

```powershell
npm run test:run -- src/__tests__/App.test.tsx src/__tests__/MemoryWorkspace.test.tsx
npm run build
```

Expected: PASS for the focused UI tests and a successful production build.

- [ ] **Step 6: Commit**

Run:

```powershell
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui add `
  src/App.tsx `
  src/styles.css `
  src/i18n/strings.ts `
  src/i18n/types.ts `
  src/chatmem-memory `
  src/components/RepoMemoryPanel.tsx `
  src/components/MemoryInboxPanel.tsx `
  src/components/EpisodesPanel.tsx `
  src/components/HandoffsPanel.tsx `
  src/__tests__/App.test.tsx `
  src/__tests__/MemoryWorkspace.test.tsx
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui commit -m "feat: add ChatMem memory management workspace"
```

Expected: a commit containing the new app-facing memory views and review UI.

### Task 4: Implement the ChatMem MCP sidecar and the five tool handlers

**Files:**
- Modify: `D:\VSP\agentswap-gui\src-tauri\Cargo.toml`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\mcp.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\bin\chatmem-mcp.rs`
- Test: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\mcp.rs`

- [ ] **Step 1: Write failing MCP tool tests**

```rust
// src-tauri/src/chatmem_memory/mcp.rs
#[cfg(test)]
mod tests {
    use super::{build_repo_memory_tool, build_search_history_tool};

    #[test]
    fn repo_memory_tool_returns_compact_startup_payload() {
        let payload = build_repo_memory_tool(
            "d:/vsp/agentswap-gui",
            "codex",
            Some("run tests before merge"),
        );

        assert!(payload.approved_memories.len() <= 3);
        assert!(payload.priority_gotchas.len() <= 2);
    }

    #[test]
    fn search_history_tool_never_returns_raw_transcript_blobs() {
        let result = build_search_history_tool(
            "d:/vsp/agentswap-gui",
            "handoff",
            5,
        );

        assert!(result.matches.iter().all(|item| item.summary.len() < 600));
    }
}
```

- [ ] **Step 2: Run the targeted MCP tests to verify they fail**

Run:

```powershell
cargo test --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml mcp
```

Expected: FAIL because the MCP tool layer and sidecar binary do not exist yet.

- [ ] **Step 3: Implement the MCP tool mapping**

Create typed tool handlers backed by the store/search/handoff modules:

```rust
// src-tauri/src/chatmem_memory/mcp.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoMemoryPayload {
    pub repo_summary: String,
    pub approved_memories: Vec<ApprovedMemoryResponse>,
    pub priority_gotchas: Vec<ApprovedMemoryResponse>,
    pub recent_handoff: Option<HandoffPacketResponse>,
}

pub fn get_repo_memory(
    store: &MemoryStore,
    repo_root: &str,
    task_hint: Option<&str>,
) -> anyhow::Result<RepoMemoryPayload> {
    let repo_id = store.ensure_repo(repo_root)?;
    let approved = store.list_repo_memories(&repo_id)?;
    let gotchas = approved
        .iter()
        .filter(|memory| memory.kind == "gotcha")
        .take(2)
        .cloned()
        .collect::<Vec<_>>();

    Ok(RepoMemoryPayload {
        repo_summary: format!("Repository memory for {}", repo_root),
        approved_memories: approved.into_iter().take(3).collect(),
        priority_gotchas: gotchas,
        recent_handoff: store.latest_handoff(&repo_id)?,
    })
}
```

Expose the five public tools:

```rust
pub enum ChatMemTool {
    GetRepoMemory,
    SearchRepoHistory,
    CreateMemoryCandidate,
    ListMemoryCandidates,
    BuildHandoffPacket,
}
```

- [ ] **Step 4: Create the stdio MCP binary**

Create a dedicated binary so Codex and Claude can spawn ChatMem through `.mcp.json`:

```rust
// src-tauri/src/bin/chatmem-mcp.rs
use chatmem::chatmem_memory::{db, mcp, store::MemoryStore};
use rmcp::{ServiceExt, transport::stdio};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let conn = db::open_app_database()?;
    db::migrate(&conn)?;
    let store = MemoryStore::new(conn);
    let service = mcp::ChatMemMcpService::new(store);

    service.serve(stdio()).await?;
    Ok(())
}
```

The service should delegate each MCP request to the same store/search/handoff code used by the Tauri app.

- [ ] **Step 5: Run the Rust tests and a binary smoke test**

Run:

```powershell
cargo test --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml mcp
cargo run --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml --bin chatmem-mcp
```

Expected: the tests PASS, and the binary starts without panicking while waiting on stdio input.

- [ ] **Step 6: Commit**

Run:

```powershell
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui add `
  src-tauri/Cargo.toml `
  src-tauri/src/chatmem_memory/mcp.rs `
  src-tauri/src/bin/chatmem-mcp.rs
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui commit -m "feat: add ChatMem MCP sidecar"
```

Expected: a commit containing the MCP server binary and tool handlers.

### Task 5: Add thin Codex and Claude integration shells plus installation scripts

**Files:**
- Create: `D:\VSP\agentswap-gui\plugins\chatmem\.codex-plugin\plugin.json`
- Create: `D:\VSP\agentswap-gui\plugins\chatmem\.claude-plugin\plugin.json`
- Create: `D:\VSP\agentswap-gui\plugins\chatmem\skills\chatmem\SKILL.md`
- Create: `D:\VSP\agentswap-gui\plugins\chatmem\.mcp.json`
- Create: `D:\VSP\agentswap-gui\.agents\plugins\marketplace.json`
- Create: `D:\VSP\agentswap-gui\scripts\sync-chatmem-plugin.ps1`
- Create: `D:\VSP\agentswap-gui\docs\CHATMEM_MCP_SETUP.md`
- Create: `D:\VSP\agentswap-gui\src\__tests__\integrationManifests.test.ts`

- [ ] **Step 1: Write a failing manifest validation test**

```ts
// src/__tests__/integrationManifests.test.ts
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

describe("ChatMem integration manifests", () => {
  it("ships Codex and Claude plugin manifests plus MCP configs", () => {
    const codexPlugin = JSON.parse(
      readFileSync(resolve("plugins/chatmem/.codex-plugin/plugin.json"), "utf8"),
    );
    const claudePlugin = JSON.parse(
      readFileSync(resolve("plugins/chatmem/.claude-plugin/plugin.json"), "utf8"),
    );
    const codexMcp = JSON.parse(
      readFileSync(resolve("plugins/chatmem/.mcp.json"), "utf8"),
    );
    const claudeMcp = JSON.parse(
      readFileSync(resolve("plugins/chatmem/.mcp.json"), "utf8"),
    );

    expect(codexPlugin.name).toContain("chatmem");
    expect(claudePlugin.name).toContain("chatmem");
    expect(codexMcp.mcpServers.chatmem.command).toContain("chatmem-mcp");
    expect(claudeMcp.mcpServers.chatmem.command).toContain("chatmem-mcp");
  });
});
```

- [ ] **Step 2: Run the manifest test to verify it fails**

Run:

```powershell
npm run test:run -- src/__tests__/integrationManifests.test.ts
```

Expected: FAIL because the integration directories and manifest files do not exist yet.

- [ ] **Step 3: Create the plugin and skill shells**

Create the Codex manifest:

```json
{
  "name": "chatmem",
  "version": "0.1.0",
  "description": "Repository memory, history search, and handoff support powered by ChatMem MCP",
  "author": {
    "name": "ChatMem",
    "email": "support@example.com",
    "url": "https://github.com/Rimagination/ChatMem"
  },
  "homepage": "https://github.com/Rimagination/ChatMem",
  "repository": "https://github.com/Rimagination/ChatMem",
  "license": "MIT",
  "keywords": ["memory", "mcp", "handoff", "coding-agent"],
  "skills": "./skills/",
  "interface": {
    "displayName": "ChatMem",
    "shortDescription": "Repository memory, history search, and handoffs",
    "longDescription": "Use ChatMem to load repository memory, search prior repository work, propose evidence-backed memory candidates, and build handoff packets.",
    "developerName": "ChatMem",
    "category": "Coding",
    "capabilities": ["Interactive", "Read", "Write"],
    "websiteURL": "https://github.com/Rimagination/ChatMem",
    "privacyPolicyURL": "https://github.com/Rimagination/ChatMem",
    "termsOfServiceURL": "https://github.com/Rimagination/ChatMem",
    "defaultPrompt": [
      "Use ChatMem when entering a repository, searching prior work, proposing memory candidates, or preparing an agent handoff"
    ],
    "screenshots": [],
    "brandColor": "#0F172A"
  }
}
```

Create the Claude manifest with the same product meaning:

```json
{
  "name": "chatmem",
  "version": "0.1.0",
  "description": "Repository memory, history search, and handoff support powered by ChatMem MCP",
  "skills": "./skills/"
}
```

Use the same thin skill language in both `SKILL.md` files:

```md
---
name: chatmem
description: Use when entering a repository, searching prior repository work, creating repository memory candidates, or preparing an agent handoff.
---

When the user enters a repository or asks how work was done before:
1. Query ChatMem MCP instead of reading raw local transcripts directly.
2. Prefer `get_repo_memory` for startup context.
3. Prefer `search_repo_history` for prior work lookup.
4. Use `create_memory_candidate` only for evidence-backed repository facts.
5. Use `build_handoff_packet` before switching agents.
```

- [ ] **Step 4: Add `.mcp.json` configs and local install scripts**

Create the shared `.mcp.json` file:

```json
{
  "mcpServers": {
    "chatmem": {
      "command": "D:/VSP/agentswap-gui/src-tauri/target/debug/chatmem-mcp.exe",
      "args": []
    }
  }
}
```

Create the repo-local Codex marketplace entry:

```json
{
  "name": "ChatMem Local Plugins",
  "interface": {
    "displayName": "ChatMem Local Plugins"
  },
  "plugins": [
    {
      "name": "chatmem",
      "source": {
        "source": "local",
        "path": "./plugins/chatmem"
      },
      "policy": {
        "installation": "AVAILABLE",
        "authentication": "ON_INSTALL"
      },
      "category": "Coding"
    }
  ]
}
```

Create the sync script for optional home-local use:

```powershell
$repoRoot = "D:\VSP\agentswap-gui"
$sourcePlugin = Join-Path $repoRoot "plugins\chatmem"
$homePluginDir = Join-Path $env:USERPROFILE "plugins\chatmem"
$homeMarketplace = Join-Path $env:USERPROFILE ".agents\plugins\marketplace.json"

New-Item -ItemType Directory -Force -Path $homePluginDir | Out-Null
Copy-Item -LiteralPath (Join-Path $sourcePlugin "*") -Destination $homePluginDir -Recurse -Force

if (-not (Test-Path $homeMarketplace)) {
  New-Item -ItemType Directory -Force -Path (Split-Path $homeMarketplace) | Out-Null
  @'
{
  "name": "Home Local Plugins",
  "interface": {
    "displayName": "Home Local Plugins"
  },
  "plugins": []
}
'@ | Set-Content -Path $homeMarketplace
}
```

Write `docs/CHATMEM_MCP_SETUP.md` with:

- how to build `chatmem-mcp`
- how to load the repo-local plugin through `.agents/plugins/marketplace.json`
- how to copy the plugin for home-local use with `scripts/sync-chatmem-plugin.ps1`
- how to point `.mcp.json` at the built binary when the path changes
- how to smoke test `get_repo_memory`

- [ ] **Step 5: Run the manifest test again**

Run:

```powershell
npm run test:run -- src/__tests__/integrationManifests.test.ts
```

Expected: PASS because the plugin manifests, skill files, and `.mcp.json` files now exist and point at the MCP binary.

- [ ] **Step 6: Commit**

Run:

```powershell
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui add `
  plugins/chatmem `
  .agents/plugins/marketplace.json `
  scripts/sync-chatmem-plugin.ps1 `
  docs/CHATMEM_MCP_SETUP.md `
  src/__tests__/integrationManifests.test.ts
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui commit -m "feat: add Codex and Claude ChatMem integration shells"
```

Expected: a commit that adds the thin entry shells and setup docs for both hosts.

### Task 6: Verify startup memory, candidate review, and handoff flow end-to-end

**Files:**
- Output: `D:\VSP\agentswap-gui\src-tauri\target\debug\chatmem-mcp.exe`
- Output: `D:\VSP\agentswap-gui\dist\index.html`
- Output: `D:\VSP\agentswap-gui\src-tauri\target\release\ChatMem.exe`

- [ ] **Step 1: Run the full automated test suite**

Run:

```powershell
npm run test:run
cargo test --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml
```

Expected: PASS for all frontend and Rust tests, including memory workspace and MCP coverage.

- [ ] **Step 2: Run production builds for both the frontend and desktop app**

Run:

```powershell
npm run build
npm run tauri build
```

Expected: PASS and fresh desktop artifacts under `src-tauri\target\release`.

- [ ] **Step 3: Smoke test the desktop review flow**

Run:

```powershell
npm run tauri dev
```

Manual expectation:

- select a conversation with a valid `project_dir`
- switch to `Memory Inbox`
- approve a pending candidate
- confirm it appears under `Repo Memory`
- switch to `Handoffs` and confirm a generated packet renders

- [ ] **Step 4: Smoke test the MCP binary directly**

Run:

```powershell
Get-Content D:\VSP\agentswap-gui\plugins\chatmem\.mcp.json
cargo run --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml --bin chatmem-mcp
```

Manual expectation:

- the binary starts cleanly on stdio
- the configured command path matches the built artifact
- Codex and Claude shell configs point at the same binary

- [ ] **Step 5: Commit**

Run:

```powershell
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui add .
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui commit -m "test: verify ChatMem MCP memory workflow"
```

Expected: a final verification commit after the end-to-end checks pass.
