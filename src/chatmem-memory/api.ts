import { invoke } from "@tauri-apps/api/tauri";
import type {
  ApprovedMemory,
  ArtifactRecord,
  CheckpointCreateInput,
  CheckpointRecord,
  EntityGraph,
  EpisodeRecord,
  HandoffConsumeInput,
  HandoffCreateInput,
  HandoffPacket,
  MemoryConflict,
  MemoryCandidate,
  RunRecord,
  WikiPage,
} from "./types";

export function listRepoMemories(repoRoot: string) {
  return invoke<ApprovedMemory[]>("list_repo_memories", { repoRoot });
}

export function listMemoryCandidates(repoRoot: string, status?: string) {
  return invoke<MemoryCandidate[]>("list_memory_candidates", { repoRoot, status });
}

export function listMemoryConflicts(repoRoot: string, status?: string) {
  return invoke<MemoryConflict[]>("list_memory_conflicts", { repoRoot, status });
}

export function listEntityGraph(repoRoot: string, limit?: number) {
  return invoke<EntityGraph>("list_entity_graph", { repoRoot, limit });
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

export function reverifyMemory(payload: { memoryId: string; verifiedBy: string }) {
  return invoke("reverify_memory", payload);
}

export function listEpisodes(repoRoot: string) {
  return invoke<EpisodeRecord[]>("list_episodes", { repoRoot });
}

export function listWikiPages(repoRoot: string) {
  return invoke<WikiPage[]>("list_wiki_pages", { repoRoot });
}

export function rebuildRepoWiki(repoRoot: string) {
  return invoke<WikiPage[]>("rebuild_repo_wiki", { repoRoot });
}

export function listHandoffs(repoRoot: string) {
  return invoke<HandoffPacket[]>("list_handoffs", { repoRoot });
}

export function listCheckpoints(repoRoot: string) {
  return invoke<CheckpointRecord[]>("list_checkpoints", { repoRoot });
}

export function listRuns(repoRoot: string) {
  return invoke<RunRecord[]>("list_runs", { repoRoot });
}

export function listArtifacts(repoRoot: string) {
  return invoke<ArtifactRecord[]>("list_artifacts", { repoRoot });
}

export function createHandoffPacket(payload: HandoffCreateInput) {
  return invoke<HandoffPacket>("create_handoff_packet", payload);
}

export function createCheckpoint(payload: CheckpointCreateInput) {
  return invoke<CheckpointRecord>("create_checkpoint", payload);
}

export function markHandoffConsumed(payload: HandoffConsumeInput) {
  return invoke("mark_handoff_consumed", payload);
}
