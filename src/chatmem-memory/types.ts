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
