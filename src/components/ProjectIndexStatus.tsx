import { useEffect, useState, type FormEvent } from "react";
import type {
  LocalHistoryImportReport,
  ProjectContextPayload,
  RepoMemoryHealth,
} from "../chatmem-memory/types";
import type { Locale } from "../i18n/types";

type HistoryRecallMatch = ProjectContextPayload["relevant_history"][number];

type ProjectIndexStatusProps = {
  bootstrapReady?: boolean;
  health: RepoMemoryHealth | null;
  loading: boolean;
  scanning: boolean;
  importReport?: LocalHistoryImportReport | null;
  locale: Locale;
  onScan: () => void;
  onOpenRules?: () => void;
  onRecallHistory?: (query: string) => Promise<HistoryRecallMatch[]>;
  onMergeAlias?: (aliasRoot: string) => void;
  mergingAliasRoot?: string | null;
};

const pendingCandidateWarningPattern =
  /(\d+)\s+pending memory candidate\(s\) need review before they become startup (?:memory|rules)\./i;

function isPendingCandidateWarning(warning: string) {
  return pendingCandidateWarningPattern.test(warning);
}

function localizeWarning({
  warning,
  isEnglish,
}: {
  warning: string;
  isEnglish: boolean;
}) {
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
  importReport,
  locale,
  onScan,
  onOpenRules,
  onRecallHistory,
  onMergeAlias,
  mergingAliasRoot,
}: ProjectIndexStatusProps) {
  const [isHelpOpen, setIsHelpOpen] = useState(false);
  const [recallQuery, setRecallQuery] = useState("");
  const [recallLoading, setRecallLoading] = useState(false);
  const [recallError, setRecallError] = useState<string | null>(null);
  const [recallMatches, setRecallMatches] = useState<HistoryRecallMatch[]>([]);
  const [hasRecalled, setHasRecalled] = useState(false);
  const isEnglish = locale === "en";
  const conversationCounts = health?.conversation_counts_by_agent ?? [];
  const warnings = health?.warnings ?? [];
  const totalConversations =
    conversationCounts.reduce(
      (count, source) => count + source.conversation_count,
      0,
    ) ?? 0;
  const latestScan = health?.latest_scan ?? null;
  const unmatchedProjectRoots = latestScan?.unmatched_project_roots ?? [];
  const effectiveIndexedChunkCount =
    health?.indexed_chunk_count ?? health?.search_document_count ?? 0;
  const pendingRuleCount = health?.pending_candidate_count ?? 0;
  const approvedRuleCount = health?.approved_memory_count ?? 0;
  const visibleWarnings = warnings.filter((warning) => !isPendingCandidateWarning(warning));
  const localizedWarnings = visibleWarnings.map((warning) =>
    localizeWarning({
      warning,
      isEnglish,
    }),
  );
  const showBootstrapNote = effectiveIndexedChunkCount === 0;
  const hasUnmatchedProjectRoots = unmatchedProjectRoots.length > 0;
  const skippedScanConversationCount = latestScan?.skipped_conversation_count ?? 0;
  const linkedScanConversationCount = latestScan?.linked_conversation_count ?? 0;
  const showScannedButUnmatchedNote =
    showBootstrapNote &&
    !scanning &&
    (latestScan?.scanned_conversation_count ?? 0) > 0 &&
    linkedScanConversationCount === 0;
  const showAliasRepairNote =
    !scanning &&
    hasUnmatchedProjectRoots &&
    skippedScanConversationCount > 0 &&
    linkedScanConversationCount > 0;
  const showBootstrapReadyNotice =
    bootstrapReady && effectiveIndexedChunkCount > 0 && !showBootstrapNote;

  const copy = isEnglish
    ? {
        title: "Local history",
        subtitle: "Indexed conversations are ready for recall.",
        helpLabel: "Why check local history first?",
        helpTitle: "Why local history comes first",
        helpBody:
          "Local history is the full conversation index for questions like what we discussed before. It does not need approval. Startup rules only keep stable rules that new tasks must carry forward.",
        loading: "Loading local history...",
        rescan: "Rescan local history",
        scanning: "Scanning...",
        conversations: "Conversations",
        chunks: "Chunks",
        pending: "Needs review",
        approved: "Startup rules",
        rulesAction: "Manage Rules",
        note: "Note",
        warnings: "Warnings",
        importSummaryLabel: "Latest full local-history import",
        importSummary: (report: LocalHistoryImportReport) => {
          const projectWord = report.indexed_repo_count === 1 ? "project" : "projects";
          const skippedText =
            report.skipped_conversation_count === 0
              ? "none skipped"
              : `${report.skipped_conversation_count} skipped`;
          return `Full import: scanned ${report.scanned_conversation_count} / imported ${report.imported_conversation_count} / ${report.indexed_repo_count} ${projectWord} / ${skippedText}`;
        },
        bootstrapIdle:
          "Local history has not been indexed for this project yet, so older conversations may not be fully searchable. After indexing, you can ask what was discussed before.",
        bootstrapScanning:
          "Importing local history for this project. Older conversations may not be fully searchable yet. When indexing finishes, you can ask what was discussed before.",
        bootstrapReady:
          "Local history is ready for this project. You can now ask what was discussed before.",
        scanMismatch: (count: number) =>
          `Recently scanned ${count} local conversation${count === 1 ? "" : "s"}, but none matched this project. Check the project path or aliases.`,
        scanNeedsAliasRepair: (linked: number, skipped: number) =>
          `Imported ${linked} local conversation${linked === 1 ? "" : "s"}; ${skipped} more may belong to this project but still need a path alias.`,
        possiblePaths: (paths: string) => `Possible matching paths: ${paths}`,
        mergeAlias: "Merge into this project",
        mergingAlias: "Merging...",
        recallPlaceholder: "Ask local history...",
        recallButton: "Recall",
        recalling: "Searching...",
        recallHeading: "History evidence",
        recallEmpty: "No matching local-history evidence found.",
        recallError: "Could not search local history.",
        evidencePrefix: "Evidence",
      }
    : {
        title: "\u672c\u5730\u5386\u53f2",
        subtitle: "\u5df2\u7d22\u5f15\u5bf9\u8bdd\u53ef\u76f4\u63a5\u7528\u4e8e\u56de\u5fc6\u3002",
        helpLabel: "\u4e3a\u4ec0\u4e48\u5148\u67e5\u672c\u5730\u5386\u53f2\uff1f",
        helpTitle: "\u4e3a\u4ec0\u4e48\u672c\u5730\u5386\u53f2\u653e\u5728\u524d\u9762",
        helpBody:
          "\u672c\u5730\u5386\u53f2\u662f\u5b8c\u6574\u5bf9\u8bdd\u7d22\u5f15\uff0c\u7528\u6765\u56de\u7b54\u201c\u4e4b\u524d\u804a\u8fc7\u4ec0\u4e48\u201d\u8fd9\u7c7b\u95ee\u9898\u3002\u5b83\u4e0d\u9700\u8981\u9010\u6761\u6279\u51c6\uff1b\u542f\u52a8\u89c4\u5219\u53ea\u4fdd\u7559\u65b0\u4efb\u52a1\u5fc5\u987b\u5e26\u4e0a\u7684\u7a33\u5b9a\u89c4\u5219\u3002",
        loading: "\u6b63\u5728\u52a0\u8f7d\u672c\u5730\u5386\u53f2...",
        rescan: "\u91cd\u65b0\u626b\u63cf\u672c\u5730\u5386\u53f2",
        scanning: "\u626b\u63cf\u4e2d...",
        conversations: "\u4f1a\u8bdd\u6570",
        chunks: "\u5206\u5757\u6570",
        pending: "\u5f85\u786e\u8ba4",
        approved: "\u542f\u52a8\u89c4\u5219",
        rulesAction: "\u7ba1\u7406\u89c4\u5219",
        note: "\u63d0\u793a",
        warnings: "\u8b66\u544a",
        importSummaryLabel: "\u6700\u8fd1\u5168\u91cf\u672c\u5730\u5386\u53f2\u5bfc\u5165",
        importSummary: (report: LocalHistoryImportReport) => {
          const skippedText =
            report.skipped_conversation_count === 0
              ? "\u65e0\u8df3\u8fc7"
              : `\u8df3\u8fc7 ${report.skipped_conversation_count} \u6761`;
          return `\u5168\u91cf\u5bfc\u5165\uff1a\u626b\u63cf ${report.scanned_conversation_count} \u6761 / \u5bfc\u5165 ${report.imported_conversation_count} \u6761 / \u8986\u76d6 ${report.indexed_repo_count} \u4e2a\u9879\u76ee / ${skippedText}`;
        },
        bootstrapIdle:
          "\u8fd9\u4e2a\u9879\u76ee\u7684\u672c\u5730\u5386\u53f2\u8fd8\u6ca1\u6709\u5efa\u7acb\u7d22\u5f15\uff0c\u6240\u4ee5\u65e7\u5bf9\u8bdd\u6682\u65f6\u53ef\u80fd\u627e\u4e0d\u5168\u3002\u5b8c\u6210\u5bfc\u5165\u540e\uff0c\u4f60\u53ef\u4ee5\u76f4\u63a5\u95ee\u4ee5\u524d\u8ba8\u8bba\u8fc7\u4ec0\u4e48\u3002",
        bootstrapScanning:
          "\u6b63\u5728\u5bfc\u5165\u8fd9\u4e2a\u9879\u76ee\u7684\u672c\u5730\u5386\u53f2\u3002\u7d22\u5f15\u5b8c\u6210\u524d\uff0c\u65e7\u5bf9\u8bdd\u53ef\u80fd\u8fd8\u627e\u4e0d\u5168\u3002\u5b8c\u6210\u540e\uff0c\u4f60\u53ef\u4ee5\u76f4\u63a5\u95ee\u4ee5\u524d\u8ba8\u8bba\u8fc7\u4ec0\u4e48\u3002",
        bootstrapReady:
          "\u8fd9\u4e2a\u9879\u76ee\u7684\u672c\u5730\u5386\u53f2\u5df2\u7ecf\u5c31\u7eea\uff0c\u73b0\u5728\u53ef\u4ee5\u76f4\u63a5\u95ee\u4ee5\u524d\u8ba8\u8bba\u8fc7\u4ec0\u4e48\u3002",
        scanMismatch: (count: number) =>
          `\u6700\u8fd1\u626b\u63cf\u4e86 ${count} \u6761\u672c\u5730\u5bf9\u8bdd\uff0c\u4f46\u6ca1\u6709\u7eb3\u5165\u5f53\u524d\u9879\u76ee\u3002\u8bf7\u68c0\u67e5\u9879\u76ee\u8def\u5f84\u6216\u522b\u540d\u3002`,
        scanNeedsAliasRepair: (linked: number, skipped: number) =>
          `\u5df2\u7eb3\u5165 ${linked} \u6761\u672c\u5730\u5bf9\u8bdd\uff0c\u53e6\u6709 ${skipped} \u6761\u53ef\u80fd\u5c5e\u4e8e\u5f53\u524d\u9879\u76ee\uff0c\u4f46\u8def\u5f84\u8fd8\u6ca1\u5e76\u5165\u3002`,
        possiblePaths: (paths: string) =>
          `\u53ef\u80fd\u5339\u914d\u7684\u8def\u5f84\uff1a${paths}`,
        mergeAlias: "\u5e76\u5165\u5f53\u524d\u9879\u76ee",
        mergingAlias: "\u6b63\u5728\u5e76\u5165...",
        recallPlaceholder: "\u95ee\u672c\u5730\u5386\u53f2...",
        recallButton: "\u56de\u5fc6",
        recalling: "\u68c0\u7d22\u4e2d...",
        recallHeading: "\u5386\u53f2\u8bc1\u636e",
        recallEmpty: "\u6ca1\u6709\u627e\u5230\u5339\u914d\u7684\u672c\u5730\u5386\u53f2\u8bc1\u636e\u3002",
        recallError: "\u65e0\u6cd5\u68c0\u7d22\u672c\u5730\u5386\u53f2\u3002",
        evidencePrefix: "\u8bc1\u636e",
      };
  const noteBody = showScannedButUnmatchedNote
    ? copy.scanMismatch(latestScan?.scanned_conversation_count ?? 0)
    : showAliasRepairNote
    ? copy.scanNeedsAliasRepair(linkedScanConversationCount, skippedScanConversationCount)
    : showBootstrapNote
    ? scanning
      ? copy.bootstrapScanning
      : copy.bootstrapIdle
    : showBootstrapReadyNotice
      ? copy.bootstrapReady
      : null;
  const noteClassName = showBootstrapReadyNotice
    ? "project-index-note is-ready"
    : showScannedButUnmatchedNote || showAliasRepairNote
      ? "project-index-note is-warning"
    : "project-index-note";
  const unmatchedProjectRootSummary = unmatchedProjectRoots.length
    ? copy.possiblePaths(
        unmatchedProjectRoots
          .slice(0, 3)
          .map((root) =>
            isEnglish
              ? `${root.project_root} (${root.source_agent} ${root.conversation_count})`
              : `${root.project_root}\uff08${root.source_agent} ${root.conversation_count} \u6761\uff09`,
          )
          .join(isEnglish ? "; " : "\uff1b"),
      )
    : null;
  const actionableUnmatchedProjectRoots = unmatchedProjectRoots.slice(0, 3);
  const metrics = [
    { label: copy.conversations, value: totalConversations, primary: true },
    { label: copy.chunks, value: effectiveIndexedChunkCount, primary: true },
    { label: copy.pending, value: pendingRuleCount, primary: false },
    { label: copy.approved, value: approvedRuleCount, primary: false },
  ];
  const importSummaryText = importReport ? copy.importSummary(importReport) : null;
  const rulesButtonClassName = [
    "btn",
    "btn-secondary",
    "memory-drawer-trigger",
  ]
    .filter(Boolean)
    .join(" ");
  const activeRepoKey = health?.canonical_repo_root ?? health?.repo_root ?? "";
  const canRecallHistory = Boolean(onRecallHistory && activeRepoKey && recallQuery.trim());

  useEffect(() => {
    setRecallQuery("");
    setRecallMatches([]);
    setRecallError(null);
    setRecallLoading(false);
    setHasRecalled(false);
  }, [activeRepoKey]);

  const handleRecallSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const query = recallQuery.trim();
    if (!onRecallHistory || !query) {
      return;
    }

    setRecallLoading(true);
    setRecallError(null);
    setHasRecalled(true);
    try {
      const matches = await onRecallHistory(query);
      setRecallMatches(matches);
    } catch (error) {
      console.error("Failed to recall local history:", error);
      setRecallMatches([]);
      setRecallError(copy.recallError);
    } finally {
      setRecallLoading(false);
    }
  };

  if (loading && !health) {
    return (
      <section className="project-index-status task-panel" aria-live="polite">
        <div className="project-index-hero">
          <div>
            <span className="task-panel-label">{copy.title}</span>
            <h2>{copy.title}</h2>
            <p className="project-index-subtitle">{copy.loading}</p>
          </div>
        </div>
      </section>
    );
  }

  return (
    <section className="project-index-status task-panel" aria-live="polite">
      <div className="project-index-hero">
        <div>
          <div className="project-index-title-row">
            <span className="task-panel-label">{copy.title}</span>
            <button
              type="button"
              className="project-index-help-button"
              aria-label={copy.helpLabel}
              aria-expanded={isHelpOpen}
              aria-controls="project-index-help"
              onClick={() => setIsHelpOpen((current) => !current)}
            >
              ?
            </button>
          </div>
          <h2>{health?.canonical_repo_root ?? "--"}</h2>
          <p className="project-index-subtitle">{copy.subtitle}</p>
          {isHelpOpen ? (
            <div id="project-index-help" className="project-index-help-popover" role="note">
              <strong>{copy.helpTitle}</strong>
              <p>{copy.helpBody}</p>
            </div>
          ) : null}
        </div>
        <div className="project-index-actions">
          {onOpenRules ? (
            <button
              type="button"
              aria-label={copy.rulesAction}
              className={rulesButtonClassName}
              onClick={onOpenRules}
            >
              <span>{copy.rulesAction}</span>
            </button>
          ) : null}
          <button
            type="button"
            className="btn btn-secondary"
            onClick={onScan}
            disabled={scanning}
          >
            {scanning ? copy.scanning : copy.rescan}
          </button>
        </div>
      </div>

      {noteBody ? (
        <div className={noteClassName}>
          <p>{noteBody}</p>
          {(showScannedButUnmatchedNote || showAliasRepairNote) && unmatchedProjectRootSummary ? (
            <p className="project-index-note-detail">{unmatchedProjectRootSummary}</p>
          ) : null}
          {(showScannedButUnmatchedNote || showAliasRepairNote) &&
          onMergeAlias &&
          actionableUnmatchedProjectRoots.length > 0 ? (
            <div className="project-index-alias-actions">
              {actionableUnmatchedProjectRoots.map((root) => {
                const isMerging = mergingAliasRoot === root.project_root;
                return (
                  <button
                    key={`${root.source_agent}:${root.project_root}`}
                    type="button"
                    className="project-index-alias-button"
                    aria-label={`${copy.mergeAlias} ${root.project_root}`}
                    onClick={() => onMergeAlias(root.project_root)}
                    disabled={Boolean(mergingAliasRoot)}
                  >
                    <span>{isMerging ? copy.mergingAlias : copy.mergeAlias}</span>
                    <span className="project-index-alias-path">{root.project_root}</span>
                  </button>
                );
              })}
            </div>
          ) : null}
        </div>
      ) : null}

      {importSummaryText ? (
        <div className="project-index-import-summary" aria-label={copy.importSummaryLabel}>
          {importSummaryText}
        </div>
      ) : null}

      <div className="project-index-metrics">
        {metrics.map((metric) => (
          <div
            key={metric.label}
            className={`project-index-metric ${metric.primary ? "is-primary" : ""}`}
          >
            <span className="project-index-metric-value meta-value">{metric.value}</span>
            <span className="project-index-metric-label meta-label">{metric.label}</span>
          </div>
        ))}
      </div>

      {onRecallHistory ? (
        <div className="project-history-recall">
          <form className="project-history-recall-form" onSubmit={(event) => void handleRecallSubmit(event)}>
            <input
              type="text"
              className="settings-input project-history-recall-input"
              value={recallQuery}
              placeholder={copy.recallPlaceholder}
              onChange={(event) => setRecallQuery(event.target.value)}
            />
            <button
              type="submit"
              className="btn btn-primary project-history-recall-button"
              disabled={!canRecallHistory || recallLoading}
            >
              {recallLoading ? copy.recalling : copy.recallButton}
            </button>
          </form>
          {recallError ? (
            <p className="project-history-recall-message">{recallError}</p>
          ) : null}
          {hasRecalled && !recallLoading && recallMatches.length === 0 && !recallError ? (
            <p className="project-history-recall-message">{copy.recallEmpty}</p>
          ) : null}
          {recallMatches.length > 0 ? (
            <div className="project-history-results" aria-label={copy.recallHeading}>
              <span className="meta-label">{copy.recallHeading}</span>
              {recallMatches.map((match, index) => {
                const evidence = match.evidence_refs[0]?.excerpt;
                return (
                  <article
                    key={`${match.type}-${match.title}-${index}`}
                    className="project-history-result"
                  >
                    <div className="project-history-result-header">
                      <strong>{match.title}</strong>
                      <span>{match.why_matched}</span>
                    </div>
                    <p>{match.summary}</p>
                    {evidence ? (
                      <div className="project-history-evidence">
                        {copy.evidencePrefix}: {evidence}
                      </div>
                    ) : null}
                  </article>
                );
              })}
            </div>
          ) : null}
        </div>
      ) : null}

      {localizedWarnings.length ? (
        <div className="project-index-warnings" role="status">
          <span className="meta-label">{copy.warnings}</span>
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
