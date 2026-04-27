import type {
  ApprovedMemory,
  ArtifactRecord,
  CheckpointRecord,
  EpisodeRecord,
  HandoffPacket,
  RunRecord,
} from "../chatmem-memory/types";
import { normalizeConversationTitle } from "../utils/titleUtils";

export type LibraryRecordKind =
  | "conversation"
  | "memory"
  | "checkpoint"
  | "handoff";

export type LibraryDestination =
  | "history-conversations"
  | "history-recovery"
  | "history-transfers"
  | "history-outputs"
  | "review";

export type LibraryRecord = {
  id: string;
  kind: LibraryRecordKind;
  title: string;
  subtitle: string;
  status: string | null;
  timestamp: string;
  destination: LibraryDestination;
  conversationId: string | null;
};

export type LibraryConversationSummary = {
  id: string;
  source_agent: string;
  project_dir: string;
  created_at: string;
  updated_at: string;
  summary: string | null;
  message_count: number;
  file_count: number;
};

type BuildRepoLibraryRecordsInput = {
  conversations: LibraryConversationSummary[];
  memories: ApprovedMemory[];
  checkpoints: CheckpointRecord[];
  handoffs: HandoffPacket[];
  runs: RunRecord[];
  artifacts: ArtifactRecord[];
  episodes: EpisodeRecord[];
};

function statusTimestamp(...timestamps: Array<string | null | undefined>) {
  return timestamps.find((timestamp): timestamp is string => Boolean(timestamp)) ?? "1970-01-01T00:00:00Z";
}

export function buildRepoLibraryRecords({
  conversations,
  memories,
  checkpoints,
  handoffs,
  runs,
  artifacts,
  episodes,
}: BuildRepoLibraryRecordsInput): LibraryRecord[] {
  const records: LibraryRecord[] = [];

  conversations.forEach((conversation) => {
    const title = normalizeConversationTitle(conversation.summary) || conversation.id;
    records.push({
      id: conversation.id,
      kind: "conversation",
      title,
      subtitle: `${conversation.source_agent} / ${conversation.message_count} messages / ${conversation.file_count} files`,
      status: null,
      timestamp: conversation.updated_at,
      destination: "history-conversations",
      conversationId: conversation.id,
    });
  });

  memories.forEach((memory) => {
    records.push({
      id: memory.memory_id,
      kind: "memory",
      title: memory.title,
      subtitle: memory.usage_hint || memory.value,
      status: memory.freshness_status || memory.status,
      timestamp: statusTimestamp(memory.verified_at, memory.last_verified_at),
      destination: "review",
      conversationId: null,
    });
  });

  checkpoints.forEach((checkpoint) => {
    records.push({
      id: checkpoint.checkpoint_id,
      kind: "checkpoint",
      title: checkpoint.summary,
      subtitle: checkpoint.resume_command || checkpoint.source_agent,
      status: checkpoint.status,
      timestamp: checkpoint.created_at,
      destination: "history-recovery",
      conversationId: checkpoint.conversation_id,
    });
  });

  handoffs.forEach((handoff) => {
    records.push({
      id: handoff.handoff_id,
      kind: "handoff",
      title: handoff.current_goal,
      subtitle: `${handoff.from_agent} -> ${handoff.to_agent}`,
      status: handoff.status,
      timestamp: handoff.created_at,
      destination: "history-transfers",
      conversationId: null,
    });
  });

  void runs;
  void artifacts;
  void episodes;

  return records.sort((left, right) => right.timestamp.localeCompare(left.timestamp));
}
