export type EvidenceRef = {
  evidence_id?: string | null;
  conversation_id?: string | null;
  message_id?: string | null;
  tool_call_id?: string | null;
  file_change_id?: string | null;
  excerpt: string;
};

export type ApprovedMemory = {
  memory_id: string;
  kind: string;
  title: string;
  value: string;
  usage_hint: string;
  status: string;
  last_verified_at: string | null;
  freshness_status: string;
  freshness_score: number;
  verified_at: string | null;
  verified_by: string | null;
  selected_because?: string | null;
  evidence_refs: EvidenceRef[];
};

export type MemoryFreshnessStatus = "fresh" | "stale" | "unknown";

export type MemoryMergeSuggestion = {
  candidate_id: string;
  memory_id: string;
  memory_title: string;
  reason: string;
};

export type MemoryCandidate = {
  candidate_id: string;
  kind: string;
  summary: string;
  value: string;
  why_it_matters: string;
  confidence: number;
  proposed_by: string;
  status: string;
  created_at: string;
  evidence_refs: EvidenceRef[];
  merge_suggestion?: MemoryMergeSuggestion | null;
};

export type EpisodeRecord = {
  episode_id: string;
  title: string;
  summary: string;
  outcome: string;
  created_at: string;
  source_conversation_id: string;
  evidence_refs: EvidenceRef[];
};

export type RunRecord = {
  run_id: string;
  repo_root: string;
  source_agent: string;
  task_hint: string | null;
  status: string;
  summary: string;
  started_at: string;
  ended_at: string | null;
  artifact_count: number;
};

export type ArtifactRecord = {
  artifact_id: string;
  run_id: string;
  artifact_type: string;
  title: string;
  summary: string;
  trust_state: string;
  created_at: string;
};

export type HandoffCreateInput = {
  repoRoot: string;
  fromAgent: string;
  toAgent: string;
  goalHint?: string;
  targetProfile?: string;
};

export type HandoffConsumeInput = {
  handoffId: string;
  consumedBy: string;
};

export type HandoffTargetProfileOption = {
  value: string;
  label: string;
  description: string;
};

export type HandoffPacket = {
  handoff_id: string;
  repo_root: string;
  from_agent: string;
  to_agent: string;
  status: string;
  checkpoint_id: string | null;
  target_profile: string | null;
  compression_strategy: string | null;
  current_goal: string;
  done_items: string[];
  next_items: string[];
  key_files: string[];
  useful_commands: string[];
  related_memories: ApprovedMemory[];
  related_episodes: EpisodeRecord[];
  consumed_at: string | null;
  consumed_by: string | null;
  created_at: string;
};
