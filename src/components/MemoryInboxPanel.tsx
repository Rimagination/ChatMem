import type { MemoryCandidate } from "../chatmem-memory/types";
import type { Locale } from "../i18n/types";

export type MemoryCandidateDisplayDraft = {
  title: string;
  value: string;
  usageHint: string;
};

type MemoryInboxPanelProps = {
  candidates: MemoryCandidate[];
  loading: boolean;
  locale: Locale;
  onDelete: (candidateId: string) => void;
};

const knownChineseCandidateText = new Map<string, string>([
  [
    "Do not touch any files outside your ownership.",
    "\u4e0d\u8981\u4fee\u6539\u81ea\u5df1\u8d1f\u8d23\u8303\u56f4\u4e4b\u5916\u7684\u6587\u4ef6\u3002",
  ],
  [
    "Do not start Runs/Artifacts/Checkpoints.",
    "\u4e0d\u8981\u542f\u52a8 Runs\u3001Artifacts \u6216 Checkpoints\u3002",
  ],
  [
    "Do not revert others' edits.",
    "\u4e0d\u8981\u56de\u9000\u5176\u4ed6\u4eba\u7684\u6539\u52a8\u3002",
  ],
  [
    "You are not alone in the codebase; do not revert others' edits.",
    "\u4f60\u4e0d\u662f\u4ee3\u7801\u5e93\u91cc\u552f\u4e00\u7684\u534f\u4f5c\u8005\uff1b\u4e0d\u8981\u56de\u9000\u5176\u4ed6\u4eba\u7684\u6539\u52a8\u3002",
  ],
  [
    "Do not auto-approve candidate writes",
    "\u4e0d\u8981\u81ea\u52a8\u5199\u5165\u5019\u9009\u8bb0\u5fc6\u3002",
  ],
]);

function normalizeText(text: string) {
  return text.trim().replace(/\s+/g, " ");
}

function hasCjk(text: string) {
  return /[\u3400-\u9fff]/.test(text);
}

function isAutoExtracted(candidate: MemoryCandidate) {
  return candidate.proposed_by === "auto_extractor";
}

function isEnglishOriginal(candidate: MemoryCandidate) {
  return !hasCjk(candidate.summary) && !hasCjk(candidate.value) && /[a-z]/i.test(`${candidate.summary} ${candidate.value}`);
}

function localizeCandidateText(text: string, locale: Locale) {
  if (locale === "en" || !text.trim() || hasCjk(text)) {
    return text;
  }

  return knownChineseCandidateText.get(normalizeText(text)) ?? text;
}

function localizeUsageHint(candidate: MemoryCandidate, locale: Locale) {
  if (locale === "en") {
    return candidate.why_it_matters;
  }

  if (
    /automatically extracted/i.test(candidate.why_it_matters) ||
    (isAutoExtracted(candidate) && /批准|审批|确认/.test(candidate.why_it_matters))
  ) {
    return "\u4ece\u660e\u786e\u7684\u957f\u671f\u8bb0\u5fc6\u63aa\u8f9e\u81ea\u52a8\u62bd\u53d6\u3002\u662f\u5426\u5199\u5165\u542f\u52a8\u89c4\u5219\uff0c\u8bf7\u4ea4\u7ed9 agent \u7ed3\u5408\u4e0a\u4e0b\u6587\u5904\u7406\u3002";
  }

  if (/human review/i.test(candidate.why_it_matters) || /人工/.test(candidate.why_it_matters)) {
    return "\u9700\u8981 agent \u901a\u8fc7 MCP \u6216 skill \u5224\u65ad\u662f\u5426\u5199\u5165\u89c4\u5219\u3002";
  }

  if (hasCjk(candidate.why_it_matters)) {
    return candidate.why_it_matters;
  }

  return "\u8fd9\u662f\u4ece\u5386\u53f2\u5bf9\u8bdd\u4e2d\u62bd\u53d6\u7684\u8bb0\u5fc6\u5efa\u8bae\u3002\u8f6f\u4ef6\u91cc\u53ea\u5c55\u793a\u548c\u5220\u9664\uff1b\u5199\u5165\u89c4\u5219\u8bf7\u4ea4\u7ed9 agent \u5904\u7406\u3002";
}

function buildDisplayDraft(candidate: MemoryCandidate, locale: Locale): MemoryCandidateDisplayDraft {
  return {
    title: localizeCandidateText(candidate.summary, locale),
    value: localizeCandidateText(candidate.value, locale),
    usageHint: localizeUsageHint(candidate, locale),
  };
}

function draftDiffers(candidate: MemoryCandidate, draft: MemoryCandidateDisplayDraft) {
  return (
    draft.title !== candidate.summary ||
    draft.value !== candidate.value ||
    draft.usageHint !== candidate.why_it_matters
  );
}

function formatKind(kind: string, isEnglish: boolean) {
  if (isEnglish) {
    return kind;
  }

  const labels: Record<string, string> = {
    command: "\u547d\u4ee4",
    convention: "\u7ea6\u5b9a",
    decision: "\u51b3\u7b56",
    gotcha: "\u6ce8\u610f\u4e8b\u9879",
    preference: "\u504f\u597d",
  };
  return labels[kind] ?? kind;
}

function formatProposedBy(proposedBy: string, isEnglish: boolean) {
  if (isEnglish) {
    return proposedBy;
  }

  if (proposedBy === "auto_extractor") {
    return "\u81ea\u52a8\u62bd\u53d6";
  }

  return proposedBy;
}

function formatOrigin(candidate: MemoryCandidate, isEnglish: boolean) {
  if (isAutoExtracted(candidate)) {
    return isEnglish ? "Source: automatic suggestion - view only" : "来源：自动建议 · 仅供查看";
  }

  return isEnglish
    ? `Source: ${candidate.proposed_by} - view only`
    : `\u6765\u6e90\uff1a${formatProposedBy(candidate.proposed_by, isEnglish)} \u63d0\u8bae \u00b7 \u4ec5\u4f9b\u67e5\u770b`;
}

function formatStatus(status: string, isEnglish: boolean) {
  if (isEnglish) {
    return status;
  }

  const labels: Record<string, string> = {
    pending_review: "\u5efa\u8bae",
    approved: "\u5df2\u5199\u5165",
    rejected: "\u5df2\u62d2\u7edd",
    snoozed: "\u5df2\u6682\u7f13",
  };
  return labels[status] ?? status;
}

function detectTrigger(candidate: MemoryCandidate) {
  const source = [
    candidate.summary,
    candidate.value,
    ...candidate.evidence_refs.map((evidence) => evidence.excerpt),
  ]
    .map(normalizeText)
    .join("\n")
    .toLowerCase();

  const triggers: Array<[string, string]> = [
    ["remember:", "Remember:"],
    ["remember that ", "Remember that"],
    ["rule:", "Rule:"],
    ["gotcha:", "Gotcha:"],
    ["note:", "Note:"],
    ["always ", "Always"],
    ["must ", "Must"],
    ["do not ", "Do not"],
    ["never ", "Never"],
    ["\u8bb0\u4f4f:", "\u8bb0\u4f4f"],
    ["\u8bb0\u4f4f\uff1a", "\u8bb0\u4f4f"],
    ["\u89c4\u5219:", "\u89c4\u5219"],
    ["\u89c4\u5219\uff1a", "\u89c4\u5219"],
    ["\u6ce8\u610f:", "\u6ce8\u610f"],
    ["\u6ce8\u610f\uff1a", "\u6ce8\u610f"],
  ];

  return triggers.find(([needle]) => source.includes(needle))?.[1] ?? null;
}

export default function MemoryInboxPanel({
  candidates,
  loading,
  locale,
  onDelete,
}: MemoryInboxPanelProps) {
  const isEnglish = locale === "en";
  const copy = {
    empty: isEnglish
      ? "No memory suggestions for this repository."
      : "\u8fd9\u4e2a\u4ed3\u5e93\u6682\u65e0\u8bb0\u5fc6\u5efa\u8bae\u3002",
    heading: isEnglish ? "Memory Suggestions" : "\u8bb0\u5fc6\u5efa\u8bae",
    subtitle: isEnglish
      ? "Review or delete suggestions here. Supported agents can turn useful suggestions into startup rules."
      : "你可以在这里查看或删除建议。有用的建议可由支持的 Agent 写入启动规则。",
    noEvidence: isEnglish ? "No linked source yet" : "暂无关联来源",
    oneEvidence: isEnglish ? "1 linked source" : "1 条关联来源",
    evidenceReady: isEnglish ? "Source ready" : "来源就绪",
    evidenceExcerpt: isEnglish ? "Source excerpt" : "来源片段",
    needsEvidence: isEnglish ? "Needs source" : "需要来源",
    conflictReview: isEnglish ? "Possible conflict" : "可能冲突",
    mergeReview: isEnglish ? "Related suggestion" : "相关建议",
    netNew: isEnglish ? "New suggestion" : "\u65b0\u589e\u5efa\u8bae",
    possibleConflict: isEnglish ? "Possible conflict with" : "\u53ef\u80fd\u4e0e",
    conflictSuffix: isEnglish ? "." : "\u51b2\u7a81\u3002",
    possibleMerge: isEnglish ? "Potential merge with" : "\u53ef\u4e0e",
    mergeSuffix: isEnglish ? "." : "\u5408\u5e76\u3002",
    suggestedRewrite: isEnglish ? "Suggested rewrite" : "\u5efa\u8bae\u6539\u5199",
    mergeProposedBy: isEnglish ? "Merge proposed by" : "\u5408\u5e76\u5efa\u8bae\u6765\u81ea",
    mergedValue: isEnglish ? "Rule value" : "\u89c4\u5219\u5185\u5bb9",
    mergedUsage: isEnglish ? "Usage hint" : "\u4f7f\u7528\u63d0\u793a",
    originalCandidate: isEnglish ? "Original text" : "\u539f\u6587",
    englishOriginal: isEnglish
      ? "English original. Ask an agent to rewrite or promote it if it should become a Chinese startup rule."
      : "\u82f1\u6587\u539f\u6587\u3002\u5982\u679c\u5b83\u5e94\u8be5\u6210\u4e3a\u4e2d\u6587\u542f\u52a8\u89c4\u5219\uff0c\u8bf7\u8ba9 agent \u6539\u5199\u6216\u5199\u5165\u3002",
    trigger: isEnglish ? "Trigger" : "\u89e6\u53d1\u8bcd",
    batchRejectAuto: isEnglish ? "Delete auto suggestions" : "\u6279\u91cf\u5220\u9664\u81ea\u52a8\u5efa\u8bae",
    reject: isEnglish ? "Delete suggestion" : "\u5220\u9664\u5efa\u8bae",
  };
  const autoSuggestionIds = candidates
    .filter((candidate) => candidate.status === "pending_review" && isAutoExtracted(candidate))
    .map((candidate) => candidate.candidate_id);

  const renderEvidenceCue = (candidate: MemoryCandidate) => {
    if (candidate.evidence_refs.length === 0) {
      return copy.noEvidence;
    }

    if (candidate.evidence_refs.length === 1) {
      return copy.oneEvidence;
    }

    return isEnglish
      ? `${candidate.evidence_refs.length} linked sources`
      : `${candidate.evidence_refs.length} 条关联来源`;
  };

  if (loading) {
    return (
      <section className="memory-panel">
        <div className="loading">
          <div className="spinner"></div>
        </div>
      </section>
    );
  }

  if (candidates.length === 0) {
    return (
      <section className="memory-panel">
        <div className="empty-state">
          <div className="empty-state-icon">I</div>
          <div className="empty-state-text">{copy.empty}</div>
        </div>
      </section>
    );
  }

  return (
    <section className="memory-panel">
      <div className="memory-panel-header">
        <div className="memory-panel-title">
          <h3>{copy.heading}</h3>
          <p>{copy.subtitle}</p>
        </div>
        {autoSuggestionIds.length > 0 ? (
          <button
            type="button"
            className="btn btn-secondary memory-batch-action"
            onClick={() => autoSuggestionIds.forEach((candidateId) => onDelete(candidateId))}
          >
            {copy.batchRejectAuto}
          </button>
        ) : null}
      </div>
      <div className="memory-card-list">
        {candidates.map((candidate) => {
          const draft = buildDisplayDraft(candidate, locale);
          const hasReviewDraft = draftDiffers(candidate, draft);
          const trigger = detectTrigger(candidate);

          return (
            <article key={candidate.candidate_id} className="memory-card">
              <div className="memory-card-header">
                <div>
                  <strong>{draft.title}</strong>
                  <div className="memory-card-kind">{formatKind(candidate.kind, isEnglish)}</div>
                </div>
              </div>
              <div className="memory-card-value">{draft.value}</div>
              <p className="memory-card-copy">{draft.usageHint}</p>
              {!isEnglish && isAutoExtracted(candidate) && isEnglishOriginal(candidate) ? (
                <p className="memory-card-warning">{copy.englishOriginal}</p>
              ) : null}
              <div className="memory-card-meta">
                <span>{formatOrigin(candidate, isEnglish)}</span>
                <span>{formatStatus(candidate.status, isEnglish)}</span>
                {trigger ? <span>{`${copy.trigger}${isEnglish ? ": " : "\uff1a"}${trigger}`}</span> : null}
                <span>{renderEvidenceCue(candidate)}</span>
              </div>
              {hasReviewDraft ? (
                <div className="memory-card-original">
                  <span>{copy.originalCandidate}</span>
                  <p>{candidate.summary}</p>
                  {candidate.value !== candidate.summary ? <p>{candidate.value}</p> : null}
                </div>
              ) : null}
              {candidate.conflict_suggestion && (
                <div className="memory-review-note memory-review-note-conflict">
                  {copy.possibleConflict} <strong>{candidate.conflict_suggestion.memory_title}</strong>
                  {copy.conflictSuffix}{" "}
                  {candidate.conflict_suggestion.reason}
                </div>
              )}
              {candidate.merge_suggestion && (
                <div className="memory-review-note">
                  {copy.possibleMerge} <strong>{candidate.merge_suggestion.memory_title}</strong>
                  {copy.mergeSuffix}{" "}
                  {candidate.merge_suggestion.reason}
                </div>
              )}
              {candidate.merge_suggestion?.proposed_value && (
                <div className="memory-merge-proposal">
                  <div className="memory-merge-proposal-heading">
                    <strong>{copy.suggestedRewrite}</strong>
                    {candidate.merge_suggestion.proposed_by ? (
                      <span>
                        {copy.mergeProposedBy} {candidate.merge_suggestion.proposed_by}
                      </span>
                    ) : null}
                  </div>
                  <div className="memory-merge-proposal-block">
                    <span>{copy.mergedValue}</span>
                    <p>{candidate.merge_suggestion.proposed_value}</p>
                  </div>
                  {candidate.merge_suggestion.proposed_usage_hint ? (
                    <div className="memory-merge-proposal-block">
                      <span>{copy.mergedUsage}</span>
                      <p>{candidate.merge_suggestion.proposed_usage_hint}</p>
                    </div>
                  ) : null}
                  {candidate.merge_suggestion.risk_note ? (
                    <p className="memory-card-copy">{candidate.merge_suggestion.risk_note}</p>
                  ) : null}
                </div>
              )}
              <div className="memory-review-cues">
                <span
                  className={`memory-review-pill ${
                    candidate.evidence_refs.length > 0 ? "memory-review-pill-ready" : "memory-review-pill-needs"
                  }`}
                >
                  {candidate.evidence_refs.length > 0 ? copy.evidenceReady : copy.needsEvidence}
                </span>
                <span
                  className={`memory-review-pill ${
                    candidate.conflict_suggestion
                      ? "memory-review-pill-conflict"
                      : candidate.merge_suggestion
                        ? "memory-review-pill-merge"
                        : "memory-review-pill-neutral"
                  }`}
                >
                  {candidate.conflict_suggestion
                    ? copy.conflictReview
                    : candidate.merge_suggestion
                      ? copy.mergeReview
                      : copy.netNew}
                </span>
              </div>
              {candidate.evidence_refs.length > 0 && (
                <div className="memory-evidence-list">
                  <span className="memory-evidence-label">{copy.evidenceExcerpt}</span>
                  {candidate.evidence_refs.slice(0, 2).map((evidence, index) => (
                    <div key={`${candidate.candidate_id}-evidence-${index}`} className="memory-evidence-item">
                      {evidence.excerpt}
                    </div>
                  ))}
                </div>
              )}
              <div className="memory-card-actions">
                <button
                  type="button"
                  className="btn btn-danger"
                  onClick={() => onDelete(candidate.candidate_id)}
                >
                  {copy.reject}
                </button>
              </div>
            </article>
          );
        })}
      </div>
    </section>
  );
}
