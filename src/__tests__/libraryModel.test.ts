import { describe, expect, it } from "vitest";
import { buildRepoLibraryRecords } from "../library/model";

describe("repo library records", () => {
  it("keeps the library focused on stable knowledge and continuation records", () => {
    const records = buildRepoLibraryRecords({
      conversations: [
        {
          id: "conv-001",
          source_agent: "codex",
          project_dir: "D:/VSP/agentswap-gui",
          created_at: "2026-04-20T10:00:00Z",
          updated_at: "2026-04-20T10:30:00Z",
          summary: "Indexed local history",
          message_count: 4,
          file_count: 2,
        },
      ],
      memories: [
        {
          memory_id: "mem-001",
          kind: "command",
          title: "Run frontend tests",
          value: "npm run test:run",
          usage_hint: "Use before packaging",
          status: "active",
          last_verified_at: "2026-04-20T10:00:00Z",
          freshness_status: "fresh",
          freshness_score: 1,
          verified_at: "2026-04-20T10:00:00Z",
          verified_by: "codex",
          selected_because: null,
          evidence_refs: [],
        },
      ],
      checkpoints: [
        {
          checkpoint_id: "cp-001",
          repo_root: "D:/VSP/agentswap-gui",
          conversation_id: "conv-001",
          source_agent: "codex",
          status: "active",
          summary: "Resume packaging work",
          resume_command: "codex resume conv-001",
          metadata_json: "{}",
          handoff_id: null,
          created_at: "2026-04-20T11:00:00Z",
        },
      ],
      handoffs: [
        {
          handoff_id: "handoff-001",
          repo_root: "D:/VSP/agentswap-gui",
          from_agent: "claude",
          to_agent: "codex",
          status: "published",
          checkpoint_id: "cp-001",
          target_profile: "codex_compact",
          compression_strategy: null,
          current_goal: "Continue release work",
          done_items: [],
          next_items: ["Run release build"],
          key_files: [],
          useful_commands: [],
          related_memories: [],
          related_episodes: [],
          consumed_at: null,
          consumed_by: null,
          created_at: "2026-04-20T12:00:00Z",
        },
      ],
      runs: [
        {
          run_id: "run-001",
          repo_root: "D:/VSP/agentswap-gui",
          source_agent: "codex",
          task_hint: "Generated internal run",
          status: "waiting_for_review",
          summary: "This is not a mature user-facing layer yet",
          started_at: "2026-04-20T12:30:00Z",
          ended_at: null,
          artifact_count: 1,
        },
      ],
      artifacts: [
        {
          artifact_id: "artifact-001",
          run_id: "run-001",
          artifact_type: "summary",
          title: "Internal artifact",
          summary: "Hide until artifacts are real reusable objects",
          trust_state: "pending_review",
          created_at: "2026-04-20T13:00:00Z",
        },
      ],
      episodes: [
        {
          episode_id: "episode-001",
          title: "Internal episode",
          summary: "Feed wiki, not the visible library",
          outcome: "captured",
          created_at: "2026-04-20T09:30:00Z",
          source_conversation_id: "conv-001",
          evidence_refs: [],
        },
      ],
    });

    const kinds = records.map((record) => record.kind as string);

    expect(kinds).toEqual([
      "handoff",
      "checkpoint",
      "conversation",
      "memory",
    ]);
    expect(kinds.includes("run")).toBe(false);
    expect(kinds.includes("artifact")).toBe(false);
    expect(kinds.includes("episode")).toBe(false);
  });
});
