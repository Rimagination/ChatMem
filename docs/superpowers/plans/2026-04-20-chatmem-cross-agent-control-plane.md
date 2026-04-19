# ChatMem Cross-Agent Control Plane Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Incrementally evolve ChatMem from repository memory plus basic handoffs into a local-first cross-agent control plane with richer handoffs, freshness-aware memory, runs and artifacts, checkpoints, and additive MCP/A2A-lite interfaces.

**Architecture:** Build V2 as a sequence of independently shippable phases on top of the current ChatMem V1 code. Keep the existing SQLite + Tauri + MCP architecture, extend the Rust data model first, then layer desktop UI and additive protocol surfaces without breaking the current five MCP tools.

**Tech Stack:** React 18, TypeScript, Vitest, Tauri 1.x, Rust, rusqlite, schemars, rmcp, PowerShell

---

## Scope Check

The approved V2 spec spans five semi-independent subsystems:

1. Handoff 2.0
2. Memory freshness and approvals
3. Runs and artifacts timeline
4. Checkpoints and resume flow
5. Additive MCP V2 and A2A-lite bridge

This plan keeps them as sequential shippable phases in a single roadmap document so implementation can proceed one phase at a time without mixing responsibilities.

## Planned File Structure

**Backend Rust core**

- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\db.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\models.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\store.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\search.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\handoff.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\mcp.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\mod.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\main.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\runs.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\checkpoints.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\a2a.rs`

**Frontend React app**

- Modify: `D:\VSP\agentswap-gui\src\App.tsx`
- Modify: `D:\VSP\agentswap-gui\src\styles.css`
- Modify: `D:\VSP\agentswap-gui\src\chatmem-memory\types.ts`
- Modify: `D:\VSP\agentswap-gui\src\chatmem-memory\api.ts`
- Modify: `D:\VSP\agentswap-gui\src\components\RepoMemoryPanel.tsx`
- Modify: `D:\VSP\agentswap-gui\src\components\MemoryInboxPanel.tsx`
- Modify: `D:\VSP\agentswap-gui\src\components\HandoffsPanel.tsx`
- Create: `D:\VSP\agentswap-gui\src\components\HandoffComposerModal.tsx`
- Create: `D:\VSP\agentswap-gui\src\components\RunsPanel.tsx`
- Create: `D:\VSP\agentswap-gui\src\components\ArtifactsPanel.tsx`
- Create: `D:\VSP\agentswap-gui\src\components\CheckpointsPanel.tsx`
- Create: `D:\VSP\agentswap-gui\src\components\ApprovalsPanel.tsx`

**Tests**

- Modify: `D:\VSP\agentswap-gui\src\__tests__\App.test.tsx`
- Create: `D:\VSP\agentswap-gui\src\__tests__\HandoffLifecycle.test.tsx`
- Create: `D:\VSP\agentswap-gui\src\__tests__\RunsWorkspace.test.tsx`
- Create: `D:\VSP\agentswap-gui\src\__tests__\CheckpointsWorkspace.test.tsx`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\db.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\store.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\handoff.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\mcp.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\runs.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\checkpoints.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\a2a.rs`

**Docs**

- Modify: `D:\VSP\agentswap-gui\docs\CHATMEM_MCP_SETUP.md`
- Create: `D:\VSP\agentswap-gui\docs\CHATMEM_CONTROL_PLANE.md`

### Task 1: Extend the schema and contracts for Handoff 2.0 and freshness metadata

**Files:**
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\db.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\models.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\handoff.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\store.rs`
- Test: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\db.rs`
- Test: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\handoff.rs`
- Test: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\store.rs`

- [ ] **Step 1: Write failing Rust tests for new handoff and freshness fields**

```rust
// src-tauri/src/chatmem_memory/db.rs
#[cfg(test)]
mod tests {
    use super::{migrate, open_in_memory};

    fn column_names(conn: &rusqlite::Connection, table: &str) -> Vec<String> {
        let mut stmt = conn
            .prepare(&format!("PRAGMA table_info({})", table))
            .unwrap();

        stmt.query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    #[test]
    fn migrations_add_handoff_lifecycle_columns() {
        let conn = open_in_memory().unwrap();
        migrate(&conn).unwrap();

        let names = column_names(&conn, "handoff_packets");
        assert!(names.contains(&"status".to_string()));
        assert!(names.contains(&"target_profile".to_string()));
        assert!(names.contains(&"checkpoint_id".to_string()));
        assert!(names.contains(&"consumed_at".to_string()));
    }

    #[test]
    fn migrations_add_memory_freshness_columns() {
        let conn = open_in_memory().unwrap();
        migrate(&conn).unwrap();

        let names = column_names(&conn, "approved_memories");
        assert!(names.contains(&"freshness_status".to_string()));
        assert!(names.contains(&"freshness_score".to_string()));
        assert!(names.contains(&"verified_at".to_string()));
        assert!(names.contains(&"verified_by".to_string()));
    }
}
```

```rust
// src-tauri/src/chatmem_memory/handoff.rs
#[cfg(test)]
mod tests {
    use super::build_handoff_packet;

    #[test]
    fn handoff_builder_sets_default_lifecycle_fields() {
        let packet = build_handoff_packet(
            "d:/vsp/agentswap-gui",
            "codex",
            "claude",
            Some("Finish the run timeline"),
            Some("claude_contextual"),
            vec!["Timeline schema landed".into()],
            vec!["Wire the React panel".into()],
            vec!["src/App.tsx".into()],
            vec!["npm.cmd run test:run".into()],
        );

        assert_eq!(packet.status, "draft");
        assert_eq!(packet.target_profile, "claude_contextual");
        assert!(packet.checkpoint_id.is_none());
        assert!(packet.consumed_at.is_none());
    }
}
```

- [ ] **Step 2: Run the targeted Rust tests to verify they fail**

Run:

```powershell
cargo test --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml migrations_add_handoff_lifecycle_columns
cargo test --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml handoff_builder_sets_default_lifecycle_fields
```

Expected:

- FAIL because the new columns and handoff fields do not exist yet

- [ ] **Step 3: Add the schema columns and default values**

```rust
// src-tauri/src/chatmem_memory/db.rs
conn.execute_batch(
    "
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
        updated_at TEXT NOT NULL,
        freshness_status TEXT NOT NULL DEFAULT 'fresh',
        freshness_score REAL NOT NULL DEFAULT 1.0,
        verified_at TEXT,
        verified_by TEXT,
        usage_count INTEGER NOT NULL DEFAULT 0,
        last_used_at TEXT,
        target_host_scope TEXT
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
        created_at TEXT NOT NULL,
        status TEXT NOT NULL DEFAULT 'draft',
        checkpoint_id TEXT,
        target_profile TEXT NOT NULL DEFAULT 'generic_json',
        compression_strategy TEXT NOT NULL DEFAULT 'balanced',
        consumed_at TEXT,
        consumed_by TEXT
    );
    ",
)?;
```

- [ ] **Step 4: Extend the Rust response models and store layer**

```rust
// src-tauri/src/chatmem_memory/models.rs
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct ApprovedMemoryResponse {
    pub memory_id: String,
    pub kind: String,
    pub title: String,
    pub value: String,
    pub usage_hint: String,
    pub status: String,
    pub last_verified_at: Option<String>,
    pub freshness_status: String,
    pub freshness_score: f64,
    pub verified_at: Option<String>,
    pub verified_by: Option<String>,
    pub selected_because: Option<String>,
    pub evidence_refs: Vec<EvidenceRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct HandoffPacketResponse {
    pub handoff_id: String,
    pub repo_root: String,
    pub from_agent: String,
    pub to_agent: String,
    pub current_goal: String,
    pub done_items: Vec<String>,
    pub next_items: Vec<String>,
    pub key_files: Vec<String>,
    pub useful_commands: Vec<String>,
    pub related_memories: Vec<ApprovedMemoryResponse>,
    pub related_episodes: Vec<EpisodeResponse>,
    pub created_at: String,
    pub status: String,
    pub checkpoint_id: Option<String>,
    pub target_profile: String,
    pub compression_strategy: String,
    pub consumed_at: Option<String>,
    pub consumed_by: Option<String>,
}
```

```rust
// src-tauri/src/chatmem_memory/store.rs
pub fn mark_handoff_consumed(
    &self,
    handoff_id: &str,
    consumed_by: &str,
) -> anyhow::Result<()> {
    self.conn.execute(
        "UPDATE handoff_packets
         SET status = 'consumed',
             consumed_at = ?2,
             consumed_by = ?3
         WHERE handoff_id = ?1",
        rusqlite::params![handoff_id, chrono::Utc::now().to_rfc3339(), consumed_by],
    )?;

    Ok(())
}
```

- [ ] **Step 5: Run the targeted Rust tests to verify they pass**

Run:

```powershell
cargo test --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml migrations_add_handoff_lifecycle_columns
cargo test --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml migrations_add_memory_freshness_columns
cargo test --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml handoff_builder_sets_default_lifecycle_fields
```

Expected:

- PASS for all three targeted tests

- [ ] **Step 6: Commit the schema foundation**

Run:

```powershell
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui add `
  src-tauri/src/chatmem_memory/db.rs `
  src-tauri/src/chatmem_memory/models.rs `
  src-tauri/src/chatmem_memory/handoff.rs `
  src-tauri/src/chatmem_memory/store.rs
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui commit -m "feat: extend chatmem schema for handoff lifecycle"
```

Expected:

- a commit containing the V2-compatible schema and model changes

### Task 2: Ship Handoff 2.0 in the desktop app and Tauri commands

**Files:**
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\main.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\store.rs`
- Modify: `D:\VSP\agentswap-gui\src\chatmem-memory\types.ts`
- Modify: `D:\VSP\agentswap-gui\src\chatmem-memory\api.ts`
- Modify: `D:\VSP\agentswap-gui\src\App.tsx`
- Modify: `D:\VSP\agentswap-gui\src\components\HandoffsPanel.tsx`
- Create: `D:\VSP\agentswap-gui\src\components\HandoffComposerModal.tsx`
- Modify: `D:\VSP\agentswap-gui\src\styles.css`
- Modify: `D:\VSP\agentswap-gui\src\__tests__\App.test.tsx`
- Create: `D:\VSP\agentswap-gui\src\__tests__\HandoffLifecycle.test.tsx`

- [ ] **Step 1: Write a failing frontend test for target profiles and consume status**

```tsx
// src/__tests__/HandoffLifecycle.test.tsx
import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { vi } from "vitest";
import App from "../App";

const mockInvoke = vi.fn();

vi.mock("@tauri-apps/api/tauri", () => ({
  invoke: (command: string, payload?: unknown) => mockInvoke(command, payload),
}));

test("creates a handoff with a target profile and then marks it consumed", async () => {
  mockInvoke.mockImplementation(async (command: string) => {
    if (command === "list_conversations") {
      return [{
        id: "conv-001",
        source_agent: "codex",
        project_dir: "D:/VSP/agentswap-gui",
        created_at: "2026-04-20T08:00:00Z",
        updated_at: "2026-04-20T09:00:00Z",
        summary: "Control plane work",
        message_count: 4,
        file_count: 2,
      }];
    }

    if (command === "read_conversation") {
      return {
        id: "conv-001",
        source_agent: "codex",
        project_dir: "D:/VSP/agentswap-gui",
        created_at: "2026-04-20T08:00:00Z",
        updated_at: "2026-04-20T09:00:00Z",
        summary: "Control plane work",
        storage_path: "C:/Users/Liang/.codex/rollout.jsonl",
        resume_command: "codex resume conv-001",
        messages: [],
        file_changes: [],
      };
    }

    if (command === "list_handoffs") {
      return [{
        handoff_id: "handoff-001",
        repo_root: "D:/VSP/agentswap-gui",
        from_agent: "codex",
        to_agent: "claude",
        current_goal: "Finish Handoff 2.0",
        done_items: ["Schema updated"],
        next_items: ["Polish panel copy"],
        key_files: ["src/components/HandoffsPanel.tsx"],
        useful_commands: ["npm.cmd run test:run"],
        related_memories: [],
        related_episodes: [],
        created_at: "2026-04-20T09:00:00Z",
        status: "draft",
        checkpoint_id: null,
        target_profile: "claude_contextual",
        compression_strategy: "balanced",
        consumed_at: null,
        consumed_by: null,
      }];
    }

    if (command === "create_handoff_packet") {
      return {
        handoff_id: "handoff-002",
        repo_root: "D:/VSP/agentswap-gui",
        from_agent: "codex",
        to_agent: "claude",
        current_goal: "Finish Handoff 2.0",
        done_items: ["Schema updated"],
        next_items: ["Polish panel copy"],
        key_files: ["src/components/HandoffsPanel.tsx"],
        useful_commands: ["npm.cmd run test:run"],
        related_memories: [],
        related_episodes: [],
        created_at: "2026-04-20T09:10:00Z",
        status: "draft",
        checkpoint_id: null,
        target_profile: "claude_contextual",
        compression_strategy: "balanced",
        consumed_at: null,
        consumed_by: null,
      };
    }

    if (command === "mark_handoff_consumed") {
      return null;
    }

    return [];
  });

  render(<App />);
  fireEvent.click(await screen.findByText("Control plane work"));
  fireEvent.click(await screen.findByRole("button", { name: "Handoffs" }));

  await screen.findByText("claude_contextual");
  fireEvent.click(screen.getByRole("button", { name: /Mark as Consumed/i }));

  await waitFor(() => {
    expect(mockInvoke).toHaveBeenCalledWith("mark_handoff_consumed", {
      handoffId: "handoff-001",
      consumedBy: "codex",
    });
  });
});
```

- [ ] **Step 2: Run the frontend test to verify it fails**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/HandoffLifecycle.test.tsx
```

Expected:

- FAIL because the panel, modal, and `mark_handoff_consumed` command do not exist yet

- [ ] **Step 3: Add the Handoff 2.0 fields to the frontend API and modal workflow**

```ts
// src/chatmem-memory/types.ts
export type HandoffPacket = {
  handoff_id: string;
  repo_root: string;
  from_agent: string;
  to_agent: string;
  current_goal: string;
  done_items: string[];
  next_items: string[];
  key_files: string[];
  useful_commands: string[];
  related_memories: ApprovedMemory[];
  related_episodes: EpisodeRecord[];
  created_at: string;
  status: string;
  checkpoint_id: string | null;
  target_profile: string;
  compression_strategy: string;
  consumed_at: string | null;
  consumed_by: string | null;
};
```

```ts
// src/chatmem-memory/api.ts
export function createHandoffPacket(payload: {
  repoRoot: string;
  fromAgent: string;
  toAgent: string;
  goalHint?: string;
  targetProfile: "codex_compact" | "claude_contextual" | "gemini_brief" | "generic_json";
}) {
  return invoke<HandoffPacket>("create_handoff_packet", payload);
}

export function markHandoffConsumed(payload: {
  handoffId: string;
  consumedBy: string;
}) {
  return invoke("mark_handoff_consumed", payload);
}
```

- [ ] **Step 4: Expose the Tauri command and render lifecycle state in the panel**

```rust
// src-tauri/src/main.rs
#[tauri::command]
async fn mark_handoff_consumed(handoff_id: String, consumed_by: String) -> Result<(), String> {
    let store = open_memory_store().map_err(|e| e.to_string())?;
    store
        .mark_handoff_consumed(&handoff_id, &consumed_by)
        .map_err(|e| e.to_string())
}
```

```tsx
// src/components/HandoffsPanel.tsx
<div className="memory-card-meta">
  <span className={`status-pill status-pill-${handoff.status}`}>{handoff.status}</span>
  <span>{handoff.target_profile}</span>
  {handoff.consumed_at ? <span>Consumed</span> : <span>Ready to transfer</span>}
</div>

{!handoff.consumed_at && (
  <button
    type="button"
    className="btn btn-secondary"
    onClick={() => onConsume(handoff.handoff_id)}
  >
    Mark as Consumed
  </button>
)}
```

- [ ] **Step 5: Run the focused UI tests and production build**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/App.test.tsx src/__tests__/HandoffLifecycle.test.tsx
npm.cmd run build
```

Expected:

- PASS for the focused handoff tests
- PASS for the production build

- [ ] **Step 6: Commit the Handoff 2.0 UI phase**

Run:

```powershell
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui add `
  src-tauri/src/main.rs `
  src/chatmem-memory/types.ts `
  src/chatmem-memory/api.ts `
  src/App.tsx `
  src/components/HandoffsPanel.tsx `
  src/components/HandoffComposerModal.tsx `
  src/styles.css `
  src/__tests__/App.test.tsx `
  src/__tests__/HandoffLifecycle.test.tsx
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui commit -m "feat: ship handoff lifecycle and target profiles"
```

Expected:

- a commit containing the desktop handoff lifecycle upgrade

### Task 3: Add memory freshness, merge-aware review, and approvals workspace

**Files:**
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\store.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\search.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\main.rs`
- Modify: `D:\VSP\agentswap-gui\src\chatmem-memory\types.ts`
- Modify: `D:\VSP\agentswap-gui\src\chatmem-memory\api.ts`
- Modify: `D:\VSP\agentswap-gui\src\components\RepoMemoryPanel.tsx`
- Modify: `D:\VSP\agentswap-gui\src\components\MemoryInboxPanel.tsx`
- Create: `D:\VSP\agentswap-gui\src\components\ApprovalsPanel.tsx`
- Modify: `D:\VSP\agentswap-gui\src\App.tsx`
- Modify: `D:\VSP\agentswap-gui\src\styles.css`
- Create: `D:\VSP\agentswap-gui\src\__tests__\MemoryFreshness.test.tsx`

- [ ] **Step 1: Write failing tests for freshness badges and re-verify actions**

```tsx
// src/__tests__/MemoryFreshness.test.tsx
import { render, screen } from "@testing-library/react";
import RepoMemoryPanel from "../components/RepoMemoryPanel";

test("shows freshness metadata and re-verify action", () => {
  render(
    <RepoMemoryPanel
      memories={[{
        memory_id: "mem-001",
        kind: "command",
        title: "Primary test command",
        value: "npm.cmd run test:run",
        usage_hint: "Use before merge",
        status: "active",
        last_verified_at: "2026-04-20T09:00:00Z",
        freshness_status: "needs_review",
        freshness_score: 0.42,
        verified_at: "2026-04-10T09:00:00Z",
        verified_by: "codex",
        selected_because: null,
        evidence_refs: [],
      }]}
      loading={false}
      onReverify={() => {}}
    />,
  );

  screen.getByText("needs_review");
  screen.getByRole("button", { name: "Re-verify" });
});
```

- [ ] **Step 2: Run the test to verify it fails**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/MemoryFreshness.test.tsx
```

Expected:

- FAIL because the panel and types do not expose freshness metadata yet

- [ ] **Step 3: Extend store logic for re-verify and merge-aware review suggestions**

```rust
// src-tauri/src/chatmem_memory/store.rs
pub fn reverify_memory(&self, memory_id: &str, verified_by: &str) -> anyhow::Result<()> {
    self.conn.execute(
        "UPDATE approved_memories
         SET freshness_status = 'fresh',
             freshness_score = 1.0,
             verified_at = ?2,
             verified_by = ?3,
             last_verified_at = ?2,
             updated_at = ?2
         WHERE memory_id = ?1",
        rusqlite::params![memory_id, chrono::Utc::now().to_rfc3339(), verified_by],
    )?;

    Ok(())
}

pub fn suggest_memory_merges(&self, repo_root: &str) -> anyhow::Result<Vec<(String, String)>> {
    let candidates = self.list_candidates_with_status(repo_root, Some("pending_review"))?;
    let memories = self.list_repo_memories(repo_root)?;

    let suggestions = candidates
        .iter()
        .flat_map(|candidate| {
            memories.iter().filter_map(move |memory| {
                if memory.kind == candidate.kind && memory.value == candidate.value {
                    Some((candidate.candidate_id.clone(), memory.memory_id.clone()))
                } else {
                    None
                }
            })
        })
        .collect::<Vec<_>>();

    Ok(suggestions)
}
```

- [ ] **Step 4: Expose a re-verify command and render the Approvals view**

```rust
// src-tauri/src/main.rs
#[tauri::command]
async fn reverify_memory(memory_id: String, verified_by: String) -> Result<(), String> {
    let store = open_memory_store().map_err(|e| e.to_string())?;
    store
        .reverify_memory(&memory_id, &verified_by)
        .map_err(|e| e.to_string())
}
```

```tsx
// src/components/ApprovalsPanel.tsx
export default function ApprovalsPanel({
  pendingCount,
  staleMemoryCount,
}: {
  pendingCount: number;
  staleMemoryCount: number;
}) {
  return (
    <section className="memory-panel">
      <div className="memory-panel-header">
        <h3>Approvals</h3>
        <p>Centralize pending memory review and stale knowledge maintenance.</p>
      </div>
      <div className="memory-card-list">
        <article className="memory-card">
          <strong>{pendingCount} pending memory decisions</strong>
        </article>
        <article className="memory-card">
          <strong>{staleMemoryCount} memories need review</strong>
        </article>
      </div>
    </section>
  );
}
```

- [ ] **Step 5: Run the focused tests and verify search ranking still passes**

Run:

```powershell
npm.cmd run test:run -- src/__tests__/MemoryFreshness.test.tsx src/__tests__/MemoryWorkspace.test.tsx
cargo test --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml trim_search_matches
```

Expected:

- PASS for freshness UI tests
- PASS for the targeted Rust search test

- [ ] **Step 6: Commit the freshness and approvals phase**

Run:

```powershell
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui add `
  src-tauri/src/chatmem_memory/store.rs `
  src-tauri/src/chatmem_memory/search.rs `
  src-tauri/src/main.rs `
  src/chatmem-memory/types.ts `
  src/chatmem-memory/api.ts `
  src/components/RepoMemoryPanel.tsx `
  src/components/MemoryInboxPanel.tsx `
  src/components/ApprovalsPanel.tsx `
  src/App.tsx `
  src/styles.css `
  src/__tests__/MemoryFreshness.test.tsx
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui commit -m "feat: add memory freshness and approvals workspace"
```

Expected:

- a commit containing freshness-aware memory governance

### Task 4: Add runs and artifacts timeline support

**Files:**
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\runs.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\db.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\models.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\mod.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\main.rs`
- Modify: `D:\VSP\agentswap-gui\src\chatmem-memory\types.ts`
- Modify: `D:\VSP\agentswap-gui\src\chatmem-memory\api.ts`
- Modify: `D:\VSP\agentswap-gui\src\App.tsx`
- Create: `D:\VSP\agentswap-gui\src\components\RunsPanel.tsx`
- Create: `D:\VSP\agentswap-gui\src\components\ArtifactsPanel.tsx`
- Modify: `D:\VSP\agentswap-gui\src\styles.css`
- Create: `D:\VSP\agentswap-gui\src\__tests__\RunsWorkspace.test.tsx`
- Test: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\runs.rs`

- [ ] **Step 1: Write failing backend and frontend tests for runs**

```rust
// src-tauri/src/chatmem_memory/runs.rs
#[cfg(test)]
mod tests {
    use super::{ArtifactRecord, RunEventRecord, RunRecord};

    #[test]
    fn run_record_tracks_waiting_review_status() {
        let run = RunRecord {
            run_id: "run-001".into(),
            repo_root: "d:/vsp/agentswap-gui".into(),
            source_agent: "codex".into(),
            task_hint: Some("Build the runs panel".into()),
            status: "waiting_for_review".into(),
            summary: "Needs human validation".into(),
            started_at: "2026-04-20T10:00:00Z".into(),
            ended_at: None,
        };

        assert_eq!(run.status, "waiting_for_review");
    }

    #[test]
    fn artifact_record_stores_type_and_trust_state() {
        let artifact = ArtifactRecord {
            artifact_id: "artifact-001".into(),
            run_id: "run-001".into(),
            artifact_type: "patch_set".into(),
            title: "Timeline patch".into(),
            summary: "Adds the new panel".into(),
            trust_state: "reviewed".into(),
            created_at: "2026-04-20T10:05:00Z".into(),
        };

        assert_eq!(artifact.trust_state, "reviewed");
    }
}
```

```tsx
// src/__tests__/RunsWorkspace.test.tsx
import { render, screen } from "@testing-library/react";
import RunsPanel from "../components/RunsPanel";

test("renders run status and artifact count", () => {
  render(
    <RunsPanel
      loading={false}
      runs={[{
        run_id: "run-001",
        repo_root: "D:/VSP/agentswap-gui",
        source_agent: "codex",
        task_hint: "Build the runs panel",
        status: "waiting_for_review",
        summary: "Waiting for approval",
        started_at: "2026-04-20T10:00:00Z",
        ended_at: null,
        artifact_count: 2,
      }]}
    />,
  );

  screen.getByText("waiting_for_review");
  screen.getByText("2 artifacts");
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:

```powershell
cargo test --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml run_record_tracks_waiting_review_status
npm.cmd run test:run -- src/__tests__/RunsWorkspace.test.tsx
```

Expected:

- FAIL because the run types, tables, and panel do not exist yet

- [ ] **Step 3: Add the run and artifact tables plus the Rust module**

```rust
// src-tauri/src/chatmem_memory/db.rs
conn.execute_batch(
    "
    CREATE TABLE IF NOT EXISTS agent_runs (
        run_id TEXT PRIMARY KEY,
        repo_id TEXT NOT NULL,
        source_agent TEXT NOT NULL,
        task_hint TEXT,
        status TEXT NOT NULL,
        summary TEXT NOT NULL,
        started_at TEXT NOT NULL,
        ended_at TEXT
    );

    CREATE TABLE IF NOT EXISTS run_events (
        event_id TEXT PRIMARY KEY,
        run_id TEXT NOT NULL,
        event_type TEXT NOT NULL,
        detail_json TEXT NOT NULL,
        created_at TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS artifacts (
        artifact_id TEXT PRIMARY KEY,
        run_id TEXT NOT NULL,
        artifact_type TEXT NOT NULL,
        title TEXT NOT NULL,
        summary TEXT NOT NULL,
        body TEXT,
        file_path TEXT,
        trust_state TEXT NOT NULL,
        created_at TEXT NOT NULL
    );
    ",
)?;
```

```rust
// src-tauri/src/chatmem_memory/mod.rs
pub mod runs;
```

```rust
// src-tauri/src/chatmem_memory/runs.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRecord {
    pub run_id: String,
    pub repo_root: String,
    pub source_agent: String,
    pub task_hint: Option<String>,
    pub status: String,
    pub summary: String,
    pub started_at: String,
    pub ended_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArtifactRecord {
    pub artifact_id: String,
    pub run_id: String,
    pub artifact_type: String,
    pub title: String,
    pub summary: String,
    pub trust_state: String,
    pub created_at: String,
}
```

- [ ] **Step 4: Expose the UI views and list commands**

```rust
// src-tauri/src/main.rs
#[tauri::command]
async fn list_runs(repo_root: String) -> Result<Vec<chatmem::chatmem_memory::runs::RunRecord>, String> {
    let store = open_memory_store().map_err(|e| e.to_string())?;
    store.list_runs(&repo_root).map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_artifacts(repo_root: String) -> Result<Vec<chatmem::chatmem_memory::runs::ArtifactRecord>, String> {
    let store = open_memory_store().map_err(|e| e.to_string())?;
    store.list_artifacts(&repo_root).map_err(|e| e.to_string())
}
```

```tsx
// src/components/RunsPanel.tsx
export default function RunsPanel({ runs, loading }: { runs: RunSummary[]; loading: boolean }) {
  if (loading) return <section className="memory-panel"><div className="loading"><div className="spinner"></div></div></section>;

  return (
    <section className="memory-panel">
      <div className="memory-panel-header">
        <h3>Runs</h3>
        <p>See active and recent agent attempts for this repository.</p>
      </div>
      <div className="memory-card-list">
        {runs.map((run) => (
          <article key={run.run_id} className="memory-card">
            <div className="memory-card-header">
              <strong>{run.task_hint ?? "Repository work"}</strong>
              <span className={`status-pill status-pill-${run.status}`}>{run.status}</span>
            </div>
            <p>{run.summary}</p>
            <div className="memory-card-meta">
              <span>{run.source_agent}</span>
              <span>{run.artifact_count} artifacts</span>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}
```

- [ ] **Step 5: Run the focused tests and build**

Run:

```powershell
cargo test --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml artifact_record_stores_type_and_trust_state
npm.cmd run test:run -- src/__tests__/RunsWorkspace.test.tsx
npm.cmd run build
```

Expected:

- PASS for the targeted Rust and React tests
- PASS for the production build

- [ ] **Step 6: Commit the runs and artifacts phase**

Run:

```powershell
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui add `
  src-tauri/src/chatmem_memory/db.rs `
  src-tauri/src/chatmem_memory/mod.rs `
  src-tauri/src/chatmem_memory/runs.rs `
  src-tauri/src/main.rs `
  src/chatmem-memory/types.ts `
  src/chatmem-memory/api.ts `
  src/App.tsx `
  src/components/RunsPanel.tsx `
  src/components/ArtifactsPanel.tsx `
  src/styles.css `
  src/__tests__/RunsWorkspace.test.tsx
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui commit -m "feat: add chatmem runs and artifacts timeline"
```

Expected:

- a commit containing the run timeline foundation

### Task 5: Add checkpoints and resume-oriented continuation flow

**Files:**
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\checkpoints.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\db.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\mod.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\store.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\mcp.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\main.rs`
- Modify: `D:\VSP\agentswap-gui\src\chatmem-memory\types.ts`
- Modify: `D:\VSP\agentswap-gui\src\chatmem-memory\api.ts`
- Modify: `D:\VSP\agentswap-gui\src\App.tsx`
- Create: `D:\VSP\agentswap-gui\src\components\CheckpointsPanel.tsx`
- Modify: `D:\VSP\agentswap-gui\src\components\HandoffsPanel.tsx`
- Modify: `D:\VSP\agentswap-gui\src\styles.css`
- Create: `D:\VSP\agentswap-gui\src\__tests__\CheckpointsWorkspace.test.tsx`
- Test: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\checkpoints.rs`

- [ ] **Step 1: Write failing tests for checkpoint freeze and resume**

```rust
// src-tauri/src/chatmem_memory/checkpoints.rs
#[cfg(test)]
mod tests {
    use super::CheckpointRecord;

    #[test]
    fn checkpoint_defaults_to_active_state() {
        let checkpoint = CheckpointRecord {
            checkpoint_id: "checkpoint-001".into(),
            repo_root: "d:/vsp/agentswap-gui".into(),
            current_goal: "Finish checkpoint support".into(),
            status: "active".into(),
            source_run_id: Some("run-001".into()),
            created_at: "2026-04-20T11:00:00Z".into(),
        };

        assert_eq!(checkpoint.status, "active");
    }
}
```

```tsx
// src/__tests__/CheckpointsWorkspace.test.tsx
import { render, screen } from "@testing-library/react";
import CheckpointsPanel from "../components/CheckpointsPanel";

test("renders checkpoint resume and promote actions", () => {
  render(
    <CheckpointsPanel
      loading={false}
      checkpoints={[{
        checkpoint_id: "checkpoint-001",
        repo_root: "D:/VSP/agentswap-gui",
        current_goal: "Finish checkpoint support",
        status: "active",
        source_run_id: "run-001",
        created_at: "2026-04-20T11:00:00Z",
      }]}
      onResume={() => {}}
      onPromoteToHandoff={() => {}}
    />,
  );

  screen.getByRole("button", { name: "Resume" });
  screen.getByRole("button", { name: "Promote to Handoff" });
});
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:

```powershell
cargo test --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml checkpoint_defaults_to_active_state
npm.cmd run test:run -- src/__tests__/CheckpointsWorkspace.test.tsx
```

Expected:

- FAIL because checkpoint tables, types, and views do not exist yet

- [ ] **Step 3: Add the checkpoint table and Rust types**

```rust
// src-tauri/src/chatmem_memory/db.rs
conn.execute_batch(
    "
    CREATE TABLE IF NOT EXISTS checkpoints (
        checkpoint_id TEXT PRIMARY KEY,
        repo_id TEXT NOT NULL,
        current_goal TEXT NOT NULL,
        status TEXT NOT NULL,
        source_run_id TEXT,
        done_json TEXT NOT NULL,
        next_json TEXT NOT NULL,
        questions_json TEXT NOT NULL,
        key_files_json TEXT NOT NULL,
        commands_json TEXT NOT NULL,
        created_at TEXT NOT NULL
    );
    ",
)?;
```

```rust
// src-tauri/src/chatmem_memory/checkpoints.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CheckpointRecord {
    pub checkpoint_id: String,
    pub repo_root: String,
    pub current_goal: String,
    pub status: String,
    pub source_run_id: Option<String>,
    pub created_at: String,
}
```

- [ ] **Step 4: Expose checkpoint commands, MCP tools, and the panel**

```rust
// src-tauri/src/main.rs
#[tauri::command]
async fn create_checkpoint(repo_root: String, current_goal: String) -> Result<(), String> {
    let store = open_memory_store().map_err(|e| e.to_string())?;
    store.create_checkpoint(&repo_root, &current_goal).map_err(|e| e.to_string())
}

#[tauri::command]
async fn list_checkpoints(repo_root: String) -> Result<Vec<chatmem::chatmem_memory::checkpoints::CheckpointRecord>, String> {
    let store = open_memory_store().map_err(|e| e.to_string())?;
    store.list_checkpoints(&repo_root).map_err(|e| e.to_string())
}
```

```rust
// src-tauri/src/chatmem_memory/mcp.rs
#[tool(name = "create_checkpoint", description = "Freeze a resumable repository checkpoint")]
async fn create_checkpoint_tool(
    &self,
    Parameters(input): Parameters<GetRepoMemoryInput>,
) -> Result<Json<serde_json::Value>, McpError> {
    self.store
        .create_checkpoint(&input.repo_root, input.task_hint.as_deref().unwrap_or("Continue repository work"))
        .map_err(|error| internal_error(error.to_string()))?;

    Ok(Json(serde_json::json!({ "status": "checkpoint_created" })))
}
```

```tsx
// src/components/CheckpointsPanel.tsx
export default function CheckpointsPanel({
  checkpoints,
  loading,
  onResume,
  onPromoteToHandoff,
}: {
  checkpoints: CheckpointRecord[];
  loading: boolean;
  onResume: (checkpointId: string) => void;
  onPromoteToHandoff: (checkpointId: string) => void;
}) {
  if (loading) return <section className="memory-panel"><div className="loading"><div className="spinner"></div></div></section>;

  return (
    <section className="memory-panel">
      <div className="memory-panel-header">
        <h3>Checkpoints</h3>
        <p>Freeze and resume repository work without replaying full transcripts.</p>
      </div>
      <div className="memory-card-list">
        {checkpoints.map((checkpoint) => (
          <article key={checkpoint.checkpoint_id} className="memory-card">
            <div className="memory-card-header">
              <strong>{checkpoint.current_goal}</strong>
              <span className={`status-pill status-pill-${checkpoint.status}`}>{checkpoint.status}</span>
            </div>
            <div className="memory-card-actions">
              <button onClick={() => onResume(checkpoint.checkpoint_id)}>Resume</button>
              <button onClick={() => onPromoteToHandoff(checkpoint.checkpoint_id)}>Promote to Handoff</button>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}
```

- [ ] **Step 5: Run the focused tests and targeted MCP schema check**

Run:

```powershell
cargo test --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml checkpoint_defaults_to_active_state
cargo test --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml list_memory_candidates_payload_schema_has_object_root
npm.cmd run test:run -- src/__tests__/CheckpointsWorkspace.test.tsx
```

Expected:

- PASS for the checkpoint Rust test
- PASS for the retained MCP schema regression test
- PASS for the checkpoint React test

- [ ] **Step 6: Commit the checkpoint phase**

Run:

```powershell
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui add `
  src-tauri/src/chatmem_memory/db.rs `
  src-tauri/src/chatmem_memory/mod.rs `
  src-tauri/src/chatmem_memory/checkpoints.rs `
  src-tauri/src/chatmem_memory/store.rs `
  src-tauri/src/chatmem_memory/mcp.rs `
  src-tauri/src/main.rs `
  src/chatmem-memory/types.ts `
  src/chatmem-memory/api.ts `
  src/App.tsx `
  src/components/CheckpointsPanel.tsx `
  src/components/HandoffsPanel.tsx `
  src/styles.css `
  src/__tests__/CheckpointsWorkspace.test.tsx
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui commit -m "feat: add checkpoint freeze and resume flow"
```

Expected:

- a commit containing the resumable-state phase

### Task 6: Additive MCP V2 tools, A2A-lite metadata bridge, docs, and full verification

**Files:**
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\mcp.rs`
- Create: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\a2a.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\mod.rs`
- Modify: `D:\VSP\agentswap-gui\src-tauri\src\main.rs`
- Modify: `D:\VSP\agentswap-gui\docs\CHATMEM_MCP_SETUP.md`
- Create: `D:\VSP\agentswap-gui\docs\CHATMEM_CONTROL_PLANE.md`
- Test: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\mcp.rs`
- Test: `D:\VSP\agentswap-gui\src-tauri\src\chatmem_memory\a2a.rs`

- [ ] **Step 1: Write failing Rust tests for the additive MCP tools and Agent Card**

```rust
// src-tauri/src/chatmem_memory/mcp.rs
#[cfg(test)]
mod tests {
    use super::ChatMemMcpService;
    use crate::chatmem_memory::store::MemoryStore;

    fn new_store() -> MemoryStore {
        let path = std::env::temp_dir()
            .join(format!("chatmem-mcp-v2-test-{}.sqlite", uuid::Uuid::new_v4()));
        MemoryStore::new(path).unwrap()
    }

    #[test]
    fn service_registers_v2_tools_without_removing_v1_tools() {
        let service = ChatMemMcpService::new(new_store());
        let names = service.debug_tool_names();

        assert!(names.contains(&"get_repo_memory".to_string()));
        assert!(names.contains(&"list_active_runs".to_string()));
        assert!(names.contains(&"create_checkpoint".to_string()));
        assert!(names.contains(&"resume_from_checkpoint".to_string()));
    }
}
```

```rust
// src-tauri/src/chatmem_memory/a2a.rs
#[cfg(test)]
mod tests {
    use super::AgentCard;

    #[test]
    fn agent_card_describes_chatmem_as_control_plane() {
        let card = AgentCard::chatmem_default();

        assert_eq!(card.name, "ChatMem");
        assert!(card.skills.contains(&"handoff".to_string()));
        assert!(card.skills.contains(&"checkpoint".to_string()));
    }
}
```

- [ ] **Step 2: Run the tests to verify they fail**

Run:

```powershell
cargo test --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml service_registers_v2_tools_without_removing_v1_tools
cargo test --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml agent_card_describes_chatmem_as_control_plane
```

Expected:

- FAIL because the new tool registrations and A2A module do not exist yet

- [ ] **Step 3: Implement additive MCP tools and the A2A-lite metadata module**

```rust
// src-tauri/src/chatmem_memory/a2a.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCard {
    pub name: String,
    pub description: String,
    pub skills: Vec<String>,
}

impl AgentCard {
    pub fn chatmem_default() -> Self {
        Self {
            name: "ChatMem".into(),
            description: "Local-first repository memory and continuation control plane".into(),
            skills: vec![
                "memory".into(),
                "handoff".into(),
                "checkpoint".into(),
                "artifacts".into(),
            ],
        }
    }
}
```

```rust
// src-tauri/src/chatmem_memory/mcp.rs
fn build_tool_router() -> ToolRouter<Self> {
    ToolRouter::new()
        .with_route((Self::get_repo_memory_tool_attr(), Self::get_repo_memory))
        .with_route((Self::search_repo_history_tool_attr(), Self::search_repo_history))
        .with_route((Self::create_memory_candidate_tool_attr(), Self::create_memory_candidate))
        .with_route((Self::list_memory_candidates_tool_attr(), Self::list_memory_candidates))
        .with_route((Self::build_handoff_packet_tool_attr(), Self::build_handoff_packet))
        .with_route((Self::list_active_runs_tool_attr(), Self::list_active_runs))
        .with_route((Self::list_run_artifacts_tool_attr(), Self::list_run_artifacts))
        .with_route((Self::create_checkpoint_tool_attr(), Self::create_checkpoint_tool))
        .with_route((Self::resume_from_checkpoint_tool_attr(), Self::resume_from_checkpoint))
}
```

- [ ] **Step 4: Update docs and add a simple Tauri view for the Agent Card**

```rust
// src-tauri/src/main.rs
#[tauri::command]
async fn get_agent_card() -> Result<chatmem::chatmem_memory::a2a::AgentCard, String> {
    Ok(chatmem::chatmem_memory::a2a::AgentCard::chatmem_default())
}
```

```markdown
<!-- docs/CHATMEM_CONTROL_PLANE.md -->
# ChatMem Control Plane

ChatMem V2 treats repository memory, handoffs, runs, checkpoints, and artifacts as first-class local assets.

## Surfaces

- Desktop app for human review and operational visibility
- MCP tools for host integrations
- A2A-lite metadata for future agent-to-agent task exchange

## Compatibility

The original five MCP tools remain supported:

- `get_repo_memory`
- `search_repo_history`
- `create_memory_candidate`
- `list_memory_candidates`
- `build_handoff_packet`
```

- [ ] **Step 5: Run the full test suite and smoke-test the desktop build**

Run:

```powershell
npm.cmd run test:run
cargo test --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml
npm.cmd run build
```

Expected:

- PASS for all Vitest suites
- PASS for all Rust tests
- PASS for the frontend build

- [ ] **Step 6: Commit the additive protocol and documentation phase**

Run:

```powershell
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui add `
  src-tauri/src/chatmem_memory/mcp.rs `
  src-tauri/src/chatmem_memory/a2a.rs `
  src-tauri/src/chatmem_memory/mod.rs `
  src-tauri/src/main.rs `
  docs/CHATMEM_MCP_SETUP.md `
  docs/CHATMEM_CONTROL_PLANE.md
git -c safe.directory=D:/VSP/agentswap-gui -C D:\VSP\agentswap-gui commit -m "feat: add chatmem control plane protocol surfaces"
```

Expected:

- a commit containing the additive MCP V2 and A2A-lite work

## Self-Review

### Spec coverage

Covered sections from the approved spec:

- Handoff 2.0: Task 1 and Task 2
- Memory freshness and governance: Task 1 and Task 3
- Runs and artifacts timeline: Task 4
- Checkpoints: Task 5
- Additive MCP V2 and A2A-lite: Task 6
- UI information architecture: Tasks 2 through 5
- Verification and compatibility: Tasks 1 through 6, especially Task 6

No spec sections were intentionally left without an implementation task.

### Placeholder scan

The plan contains no `TODO`, `TBD`, `FIXME`, or deferred “implement later” steps.

### Type consistency

The plan consistently uses these names across tasks:

- `HandoffPacket`
- `ApprovedMemoryResponse`
- `RunRecord`
- `ArtifactRecord`
- `CheckpointRecord`
- `mark_handoff_consumed`
- `create_checkpoint`
- `list_active_runs`
- `resume_from_checkpoint`

