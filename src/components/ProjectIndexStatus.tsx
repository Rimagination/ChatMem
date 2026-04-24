import type { RepoMemoryHealth } from "../chatmem-memory/types";
import type { Locale } from "../i18n/types";

type ProjectIndexStatusProps = {
  bootstrapReady?: boolean;
  health: RepoMemoryHealth | null;
  loading: boolean;
  scanning: boolean;
  locale: Locale;
  onScan: () => void;
};

const pendingCandidateWarningPattern =
  /(\d+)\s+pending memory candidate\(s\) need review before they become startup memory\./i;

function isPendingCandidateWarning(warning: string) {
  return pendingCandidateWarningPattern.test(warning);
}

function localizeWarning({
  warning,
  isEnglish,
  pendingCandidateCount,
  totalConversations,
  indexedChunkCount,
}: {
  warning: string;
  isEnglish: boolean;
  pendingCandidateCount: number;
  totalConversations: number;
  indexedChunkCount: number;
}) {
  const pendingMatch = warning.match(pendingCandidateWarningPattern);
  if (pendingMatch) {
    const count = Number(pendingMatch[1] ?? pendingCandidateCount);
    if (isEnglish) {
      const noun = count === 1 ? "candidate memory is" : "candidate memories are";
      return `${count} ${noun} waiting for review. This does not block local-history search; indexed conversations remain searchable. Approve only durable rules for startup memory.`;
    }

    return `\u6709 ${count} \u6761\u5019\u9009\u8bb0\u5fc6\u7b49\u5f85\u786e\u8ba4\u3002\u5b83\u4eec\u4e0d\u4f1a\u963b\u6b62\u672c\u5730\u5386\u53f2\u68c0\u7d22\uff1b${totalConversations} \u6bb5\u5bf9\u8bdd\u548c ${indexedChunkCount} \u4e2a\u5206\u5757\u5df2\u7ecf\u53ef\u7528\u4e8e\u56de\u5fc6\u3002\u53ea\u628a\u7a33\u5b9a\u89c4\u5219\u6279\u51c6\u4e3a\u542f\u52a8\u8bb0\u5fc6\u3002`;
  }

  if (!isEnglish && warning === "Ancestor repo alias detected and merged into current index.") {
    return "\u68c0\u6d4b\u5230\u4e0a\u7ea7\u4ed3\u5e93\u522b\u540d\uff0c\u5df2\u5408\u5e76\u5230\u5f53\u524d\u7d22\u5f15\u3002";
  }

  return warning;
}

export default function ProjectIndexStatus({
  bootstrapReady = false,
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
  const localizedWarnings = warnings.map((warning) =>
    localizeWarning({
      warning,
      isEnglish,
      pendingCandidateCount: health?.pending_candidate_count ?? 0,
      totalConversations,
      indexedChunkCount: effectiveIndexedChunkCount,
    }),
  );
  const reviewNoticeOnly =
    warnings.length > 0 && warnings.every((warning) => isPendingCandidateWarning(warning));
  const showBootstrapNote = effectiveIndexedChunkCount === 0;
  const showBootstrapReadyNotice =
    bootstrapReady && effectiveIndexedChunkCount > 0 && !showBootstrapNote;

  const copy = isEnglish
    ? {
        title: "Local history",
        loading: "Loading local history...",
        rescan: "Rescan local history",
        scanning: "Scanning...",
        conversations: "Conversations",
        chunks: "Chunks",
        pending: "Review queue",
        approved: "Startup memory",
        note: "Note",
        warnings: "Warnings",
        bootstrapIdle:
          "Local history has not been indexed for this project yet, so older conversations may not be fully searchable. After indexing, you can ask what was discussed before.",
        bootstrapScanning:
          "Importing local history for this project. Older conversations may not be fully searchable yet. When indexing finishes, you can ask what was discussed before.",
        bootstrapReady:
          "Local history is ready for this project. You can now ask what was discussed before.",
      }
    : {
        title: "\u672c\u5730\u5386\u53f2",
        loading: "\u6b63\u5728\u52a0\u8f7d\u672c\u5730\u5386\u53f2...",
        rescan: "\u91cd\u65b0\u626b\u63cf\u672c\u5730\u5386\u53f2",
        scanning: "\u626b\u63cf\u4e2d...",
        conversations: "\u4f1a\u8bdd\u6570",
        chunks: "\u5206\u5757\u6570",
        pending: "\u5f85\u786e\u8ba4\u5019\u9009",
        approved: "\u542f\u52a8\u8bb0\u5fc6",
        note: "\u63d0\u793a",
        warnings: "\u8b66\u544a",
        bootstrapIdle:
          "\u8fd9\u4e2a\u9879\u76ee\u7684\u672c\u5730\u5386\u53f2\u8fd8\u6ca1\u6709\u5efa\u7acb\u7d22\u5f15\uff0c\u6240\u4ee5\u65e7\u5bf9\u8bdd\u6682\u65f6\u53ef\u80fd\u627e\u4e0d\u5168\u3002\u5b8c\u6210\u5bfc\u5165\u540e\uff0c\u4f60\u53ef\u4ee5\u76f4\u63a5\u95ee\u4ee5\u524d\u8ba8\u8bba\u8fc7\u4ec0\u4e48\u3002",
        bootstrapScanning:
          "\u6b63\u5728\u5bfc\u5165\u8fd9\u4e2a\u9879\u76ee\u7684\u672c\u5730\u5386\u53f2\u3002\u7d22\u5f15\u5b8c\u6210\u524d\uff0c\u65e7\u5bf9\u8bdd\u53ef\u80fd\u8fd8\u627e\u4e0d\u5168\u3002\u5b8c\u6210\u540e\uff0c\u4f60\u53ef\u4ee5\u76f4\u63a5\u95ee\u4ee5\u524d\u8ba8\u8bba\u8fc7\u4ec0\u4e48\u3002",
        bootstrapReady:
          "\u8fd9\u4e2a\u9879\u76ee\u7684\u672c\u5730\u5386\u53f2\u5df2\u7ecf\u5c31\u7eea\uff0c\u73b0\u5728\u53ef\u4ee5\u76f4\u63a5\u95ee\u4ee5\u524d\u8ba8\u8bba\u8fc7\u4ec0\u4e48\u3002",
      };
  const noteBody = showBootstrapNote
    ? scanning
      ? copy.bootstrapScanning
      : copy.bootstrapIdle
    : showBootstrapReadyNotice
      ? copy.bootstrapReady
      : null;
  const noteClassName = showBootstrapReadyNotice
    ? "project-index-note is-ready"
    : "project-index-note";
  const warningClassName = reviewNoticeOnly
    ? "project-index-warnings is-info"
    : "project-index-warnings";
  const warningLabel = reviewNoticeOnly ? copy.note : copy.warnings;

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

      {noteBody ? (
        <div className={noteClassName}>
          <p>{noteBody}</p>
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

      {localizedWarnings.length ? (
        <div className={warningClassName} role="status">
          <span className="meta-label">{warningLabel}</span>
          <ul>
            {localizedWarnings.map((warning) => (
              <li key={warning}>{warning}</li>
            ))}
          </ul>
        </div>
      ) : null}
    </section>
  );
}
