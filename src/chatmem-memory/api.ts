import { invoke } from "@tauri-apps/api/tauri";
import type {
  ApprovedMemory,
  EpisodeRecord,
  HandoffConsumeInput,
  HandoffCreateInput,
  HandoffPacket,
  MemoryCandidate,
} from "./types";

export function listRepoMemories(repoRoot: string) {
  return invoke<ApprovedMemory[]>("list_repo_memories", { repoRoot });
}

export function listMemoryCandidates(repoRoot: string, status?: string) {
  return invoke<MemoryCandidate[]>("list_memory_candidates", { repoRoot, status });
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

export function listHandoffs(repoRoot: string) {
  return invoke<HandoffPacket[]>("list_handoffs", { repoRoot });
}

export function createHandoffPacket(payload: HandoffCreateInput) {
  return invoke<HandoffPacket>("create_handoff_packet", payload);
}

export function markHandoffConsumed(payload: HandoffConsumeInput) {
  return invoke("mark_handoff_consumed", payload);
}
