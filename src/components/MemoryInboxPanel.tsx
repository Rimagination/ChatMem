import type { MemoryCandidate } from "../chatmem-memory/types";
import type { Locale } from "../i18n/types";

type MemoryInboxPanelProps = {
  candidates: MemoryCandidate[];
  loading: boolean;
  locale: Locale;
  onApprove: (candidate: MemoryCandidate) => void;
  onReject: (candidateId: string) => void;
};

export default function MemoryInboxPanel({
  candidates,
  loading,
  locale,
  onApprove,
  onReject,
}: MemoryInboxPanelProps) {
  const isEnglish = locale === "en";
  const copy = {
    empty: isEnglish
      ? "No pending memory candidates for this repository."
      : "\u8fd9\u4e2a\u4ed3\u5e93\u6682\u65e0\u5f85\u5ba1\u6838\u8bb0\u5fc6\u5019\u9009\u3002",
    heading: isEnglish ? "Memory Inbox" : "\u8bb0\u5fc6\u6536\u4ef6\u7bb1",
    subtitle: isEnglish
      ? "Agent-proposed repository memory waiting for human review."
      : "Agent \u63d0\u8bae\u7684\u4ed3\u5e93\u8bb0\u5fc6\uff0c\u7b49\u5f85\u4eba\u5de5\u5ba1\u6838\u3002",
    proposedBy: isEnglish ? "Proposed by" : "\u63d0\u8bae\u8005",
    noEvidence: isEnglish ? "No linked evidence yet" : "\u6682\u65e0\u5173\u8054\u8bc1\u636e",
    oneEvidence: isEnglish ? "1 linked evidence reference" : "1 \u6761\u5173\u8054\u8bc1\u636e",
    evidenceReady: isEnglish ? "Evidence ready" : "\u8bc1\u636e\u5c31\u7eea",
    needsEvidence: isEnglish ? "Needs evidence" : "\u9700\u8981\u8bc1\u636e",
    conflictReview: isEnglish ? "Conflict review" : "\u51b2\u7a81\u5ba1\u6838",
    mergeReview: isEnglish ? "Merge-aware review" : "\u5408\u5e76\u5ba1\u6838",
    netNew: isEnglish ? "Net new candidate" : "\u65b0\u589e\u5019\u9009",
    possibleConflict: isEnglish ? "Possible conflict with" : "\u53ef\u80fd\u4e0e",
    conflictSuffix: isEnglish ? "." : "\u51b2\u7a81\u3002",
    possibleMerge: isEnglish ? "Potential merge with" : "\u53ef\u4e0e",
    mergeSuffix: isEnglish ? "." : "\u5408\u5e76\u3002",
    approve: isEnglish ? "Approve" : "\u6279\u51c6",
    reject: isEnglish ? "Reject" : "\u62d2\u7edd",
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
        {candidates.map((candidate) => (
          <article key={candidate.candidate_id} className="memory-card">
            <div className="memory-card-header">
              <div>
                <strong>{candidate.summary}</strong>
                <div className="memory-card-kind">{candidate.kind}</div>
              </div>
              <span className="memory-card-confidence">{candidate.confidence.toFixed(2)}</span>
            </div>
            <div className="memory-card-value">{candidate.value}</div>
            <p className="memory-card-copy">{candidate.why_it_matters}</p>
            <div className="memory-card-meta">
              <span>
                {copy.proposedBy} {candidate.proposed_by}
              </span>
              <span>{candidate.status}</span>
              <span>{renderEvidenceCue(candidate)}</span>
            </div>
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
                {candidate.evidence_refs.slice(0, 2).map((evidence, index) => (
                  <div key={`${candidate.candidate_id}-evidence-${index}`} className="memory-evidence-item">
                    {evidence.excerpt}
                  </div>
                ))}
              </div>
            )}
            <div className="memory-card-actions">
              <button type="button" className="btn btn-primary" onClick={() => onApprove(candidate)}>
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
        ))}
      </div>
    </section>
  );
}
