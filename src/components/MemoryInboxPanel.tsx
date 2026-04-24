import type { MemoryCandidate } from "../chatmem-memory/types";
import type { Locale } from "../i18n/types";

export type MemoryCandidateApprovalDraft = {
  title: string;
  value: string;
  usageHint: string;
};

type MemoryInboxPanelProps = {
  candidates: MemoryCandidate[];
  loading: boolean;
  locale: Locale;
  onApprove: (candidate: MemoryCandidate, reviewDraft?: MemoryCandidateApprovalDraft) => void;
  onApproveMerge: (candidate: MemoryCandidate) => void;
  onReject: (candidateId: string) => void;
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
    "\u4e0d\u8981\u81ea\u52a8\u6279\u51c6\u5019\u9009\u8bb0\u5fc6\u5199\u5165\u3002",
  ],
]);

function normalizeText(text: string) {
  return text.trim().replace(/\s+/g, " ");
}

function hasCjk(text: string) {
  return /[\u3400-\u9fff]/.test(text);
}

function localizeCandidateText(text: string, locale: Locale) {
  if (locale === "en" || !text.trim() || hasCjk(text)) {
    return text;
  }

  return knownChineseCandidateText.get(normalizeText(text)) ?? text;
}

function localizeUsageHint(candidate: MemoryCandidate, locale: Locale) {
  if (locale === "en" || hasCjk(candidate.why_it_matters)) {
    return candidate.why_it_matters;
  }

  if (/automatically extracted/i.test(candidate.why_it_matters)) {
    return "\u4ece\u660e\u786e\u7684\u957f\u671f\u8bb0\u5fc6\u63aa\u8f9e\u81ea\u52a8\u62bd\u53d6\u3002\u6279\u51c6\u524d\u8bf7\u786e\u8ba4\u5b83\u786e\u5b9e\u9002\u5408\u6bcf\u6b21\u542f\u52a8\u90fd\u5e26\u4e0a\u3002";
  }

  if (/human review/i.test(candidate.why_it_matters)) {
    return "\u9700\u8981\u4eba\u5de5\u786e\u8ba4\u540e\u624d\u4f1a\u6210\u4e3a\u542f\u52a8\u8bb0\u5fc6\u3002";
  }

  return "\u8fd9\u662f\u4ece\u5386\u53f2\u5bf9\u8bdd\u4e2d\u62bd\u53d6\u7684\u542f\u52a8\u8bb0\u5fc6\u5019\u9009\u3002\u6279\u51c6\u540e\u4f1a\u5728\u540e\u7eed\u4efb\u52a1\u5f00\u59cb\u65f6\u63d0\u4f9b\u7ed9 agent\uff0c\u8bf7\u5148\u786e\u8ba4\u5b83\u662f\u957f\u671f\u7a33\u5b9a\u7684\u89c4\u5219\u3002";
}

function buildApprovalDraft(candidate: MemoryCandidate, locale: Locale): MemoryCandidateApprovalDraft {
  return {
    title: localizeCandidateText(candidate.summary, locale),
    value: localizeCandidateText(candidate.value, locale),
    usageHint: localizeUsageHint(candidate, locale),
  };
}

function draftDiffers(candidate: MemoryCandidate, draft: MemoryCandidateApprovalDraft) {
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

function formatStatus(status: string, isEnglish: boolean) {
  if (isEnglish) {
    return status;
  }

  const labels: Record<string, string> = {
    pending_review: "\u5f85\u786e\u8ba4",
    approved: "\u5df2\u6279\u51c6",
    rejected: "\u5df2\u62d2\u7edd",
    snoozed: "\u5df2\u6682\u7f13",
  };
  return labels[status] ?? status;
}

export default function MemoryInboxPanel({
  candidates,
  loading,
  locale,
  onApprove,
  onApproveMerge,
  onReject,
}: MemoryInboxPanelProps) {
  const isEnglish = locale === "en";
  const copy = {
    empty: isEnglish
      ? "No pending startup memory candidates for this repository."
      : "\u8fd9\u4e2a\u4ed3\u5e93\u6682\u65e0\u5f85\u786e\u8ba4\u7684\u542f\u52a8\u8bb0\u5fc6\u5019\u9009\u3002",
    heading: isEnglish ? "Memory Inbox" : "\u542f\u52a8\u8bb0\u5fc6\u5019\u9009",
    subtitle: isEnglish
      ? "Only durable startup memories need review here; indexed local history remains searchable."
      : "\u672c\u5730\u5386\u53f2\u5df2\u7ecf\u8fdb\u5165\u68c0\u7d22\u7d22\u5f15\uff1b\u8fd9\u91cc\u53ea\u786e\u8ba4\u4ee5\u540e\u542f\u52a8\u65f6\u8981\u5e26\u4e0a\u7684\u7a33\u5b9a\u89c4\u5219\u3002",
    proposedBy: isEnglish ? "Proposed by" : "\u6765\u6e90",
    noEvidence: isEnglish ? "No linked evidence yet" : "\u6682\u65e0\u5173\u8054\u8bc1\u636e",
    oneEvidence: isEnglish ? "1 linked evidence reference" : "1 \u6761\u5173\u8054\u8bc1\u636e",
    evidenceReady: isEnglish ? "Evidence ready" : "\u8bc1\u636e\u5c31\u7eea",
    evidenceExcerpt: isEnglish ? "Evidence excerpt" : "\u8bc1\u636e\u539f\u6587",
    needsEvidence: isEnglish ? "Needs evidence" : "\u9700\u8981\u8bc1\u636e",
    conflictReview: isEnglish ? "Conflict review" : "\u51b2\u7a81\u5ba1\u6838",
    mergeReview: isEnglish ? "Merge-aware review" : "\u5408\u5e76\u5ba1\u6838",
    netNew: isEnglish ? "Net new candidate" : "\u65b0\u589e\u5019\u9009",
    possibleConflict: isEnglish ? "Possible conflict with" : "\u53ef\u80fd\u4e0e",
    conflictSuffix: isEnglish ? "." : "\u51b2\u7a81\u3002",
    possibleMerge: isEnglish ? "Potential merge with" : "\u53ef\u4e0e",
    mergeSuffix: isEnglish ? "." : "\u5408\u5e76\u3002",
    suggestedRewrite: isEnglish ? "Suggested rewrite" : "\u5efa\u8bae\u6539\u5199",
    mergeProposedBy: isEnglish ? "Merge proposed by" : "\u5408\u5e76\u5efa\u8bae\u6765\u81ea",
    mergedValue: isEnglish ? "Memory value" : "\u8bb0\u5fc6\u5185\u5bb9",
    mergedUsage: isEnglish ? "Usage hint" : "\u4f7f\u7528\u63d0\u793a",
    originalCandidate: isEnglish ? "Original candidate" : "\u539f\u59cb\u5019\u9009",
    approveMerge: isEnglish ? "Approve merge" : "\u6279\u51c6\u5408\u5e76",
    approve: isEnglish ? "Approve" : "\u6279\u51c6\u4e3a\u542f\u52a8\u8bb0\u5fc6",
    reject: isEnglish ? "Reject" : "\u5ffd\u7565\u5019\u9009",
  };

  const renderEvidenceCue = (candidate: MemoryCandidate) => {
    if (candidate.evidence_refs.length === 0) {
      return copy.noEvidence;
    }

    if (candidate.evidence_refs.length === 1) {
      return copy.oneEvidence;
    }

    return isEnglish
      ? `${candidate.evidence_refs.length} linked evidence references`
      : `${candidate.evidence_refs.length} \u6761\u5173\u8054\u8bc1\u636e`;
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
        <h3>{copy.heading}</h3>
        <p>{copy.subtitle}</p>
      </div>
      <div className="memory-card-list">
        {candidates.map((candidate) => {
          const draft = buildApprovalDraft(candidate, locale);
          const hasReviewDraft = draftDiffers(candidate, draft);

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
              <div className="memory-card-meta">
                <span>
                  {copy.proposedBy} {formatProposedBy(candidate.proposed_by, isEnglish)}
                </span>
                <span>{formatStatus(candidate.status, isEnglish)}</span>
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
                {candidate.merge_suggestion?.proposed_value ? (
                  <button type="button" className="btn btn-primary" onClick={() => onApproveMerge(candidate)}>
                    {copy.approveMerge}
                  </button>
                ) : null}
                <button
                  type="button"
                  className="btn btn-primary"
                  onClick={() => onApprove(candidate, locale === "en" ? undefined : draft)}
                >
                  {copy.approve}
                </button>
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => onReject(candidate.candidate_id)}
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
