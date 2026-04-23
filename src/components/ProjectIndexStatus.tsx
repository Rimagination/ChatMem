import type { RepoMemoryHealth } from "../chatmem-memory/types";
import type { Locale } from "../i18n/types";

type ProjectIndexStatusProps = {
  health: RepoMemoryHealth | null;
  loading: boolean;
  scanning: boolean;
  locale: Locale;
  onScan: () => void;
};

export default function ProjectIndexStatus({
  health,
  loading,
  scanning,
  locale,
  onScan,
}: ProjectIndexStatusProps) {
  const isEnglish = locale === "en";
  const conversationCounts = health?.conversation_counts_by_agent ?? [];
  const warnings = health?.warnings ?? [];
  const totalConversations =
    conversationCounts.reduce(
      (count, source) => count + source.conversation_count,
      0,
    ) ?? 0;
  const effectiveIndexedChunkCount =
    health?.indexed_chunk_count ?? health?.search_document_count ?? 0;
  const showBootstrapNote = effectiveIndexedChunkCount === 0;

  const copy = isEnglish
    ? {
        title: "Local history",
        loading: "Loading local history...",
        rescan: "Rescan local history",
        scanning: "Scanning...",
        conversations: "Conversations",
        chunks: "Chunks",
        pending: "Pending memory",
        approved: "Approved memory",
        warnings: "Warnings",
        bootstrapIdle:
          "Local history has not been indexed for this project yet, so older conversations may not be fully searchable. After indexing, you can ask what was discussed before.",
        bootstrapScanning:
          "Importing local history for this project. Older conversations may not be fully searchable yet. When indexing finishes, you can ask what was discussed before.",
      }
    : {
        title: "本地历史",
        loading: "正在加载本地历史...",
        rescan: "重新扫描本地历史",
        scanning: "扫描中...",
        conversations: "会话数",
        chunks: "分块数",
        pending: "待审核记忆",
        approved: "已批准记忆",
        warnings: "警告",
        bootstrapIdle:
          "这个项目的本地历史还没有建立索引，所以旧对话暂时可能找不全。完成导入后，你可以直接问以前讨论过什么。",
        bootstrapScanning:
          "正在导入这个项目的本地历史。索引完成前，旧对话可能还找不全。完成后，你可以直接问以前讨论过什么。",
      };

  if (loading && !health) {
    return (
      <section className="project-index-status task-panel" aria-live="polite">
        <div className="task-panel-header">
          <div>
            <span className="task-panel-label">{copy.title}</span>
            <h2>{copy.title}</h2>
            <p>{copy.loading}</p>
          </div>
        </div>
      </section>
    );
  }

  return (
    <section className="project-index-status task-panel" aria-live="polite">
      <div className="task-panel-header">
        <div>
          <span className="task-panel-label">{copy.title}</span>
          <h2>{health?.canonical_repo_root ?? "--"}</h2>
        </div>
        <button
          type="button"
          className="btn btn-secondary"
          onClick={onScan}
          disabled={scanning}
        >
          {scanning ? copy.scanning : copy.rescan}
        </button>
      </div>

      {showBootstrapNote ? (
        <div className="project-index-note">
          <p>{scanning ? copy.bootstrapScanning : copy.bootstrapIdle}</p>
        </div>
      ) : null}

      <div className="project-index-grid">
        <div className="meta-block">
          <span className="meta-label">{copy.conversations}</span>
          <span className="meta-value">{totalConversations}</span>
        </div>
        <div className="meta-block">
          <span className="meta-label">{copy.chunks}</span>
          <span className="meta-value">{effectiveIndexedChunkCount}</span>
        </div>
        <div className="meta-block">
          <span className="meta-label">{copy.pending}</span>
          <span className="meta-value">{health?.pending_candidate_count ?? 0}</span>
        </div>
        <div className="meta-block">
          <span className="meta-label">{copy.approved}</span>
          <span className="meta-value">{health?.approved_memory_count ?? 0}</span>
        </div>
      </div>

      {warnings.length ? (
        <div className="project-index-warnings" role="status">
          <span className="meta-label">{copy.warnings}</span>
          <ul>
            {warnings.map((warning) => (
              <li key={warning}>{warning}</li>
            ))}
          </ul>
        </div>
      ) : null}
    </section>
  );
}
