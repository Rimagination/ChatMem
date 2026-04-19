import type { MemoryCandidate } from "../chatmem-memory/types";

type MemoryInboxPanelProps = {
  candidates: MemoryCandidate[];
  loading: boolean;
  onApprove: (candidate: MemoryCandidate) => void;
  onReject: (candidateId: string) => void;
};

export default function MemoryInboxPanel({
  candidates,
  loading,
  onApprove,
  onReject,
}: MemoryInboxPanelProps) {
  const renderEvidenceCue = (candidate: MemoryCandidate) => {
    if (candidate.evidence_refs.length === 0) {
      return "No linked evidence yet";
    }

    if (candidate.evidence_refs.length === 1) {
      return "1 linked evidence reference";
    }

    return `${candidate.evidence_refs.length} linked evidence references`;
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
          <div className="empty-state-text">No pending memory candidates for this repository.</div>
        </div>
      </section>
    );
  }

  return (
    <section className="memory-panel">
      <div className="memory-panel-header">
        <h3>Memory Inbox</h3>
        <p>Agent-proposed repository memory waiting for human review.</p>
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
              <span>Proposed by {candidate.proposed_by}</span>
              <span>{candidate.status}</span>
              <span>{renderEvidenceCue(candidate)}</span>
            </div>
            {candidate.merge_suggestion && (
              <div className="memory-review-note">
                Potential merge with <strong>{candidate.merge_suggestion.memory_title}</strong>.{" "}
                {candidate.merge_suggestion.reason}
              </div>
            )}
            <div className="memory-review-cues">
              <span
                className={`memory-review-pill ${
                  candidate.evidence_refs.length > 0 ? "memory-review-pill-ready" : "memory-review-pill-needs"
                }`}
              >
                {candidate.evidence_refs.length > 0 ? "Evidence ready" : "Needs evidence"}
              </span>
              <span
                className={`memory-review-pill ${
                  candidate.merge_suggestion ? "memory-review-pill-merge" : "memory-review-pill-neutral"
                }`}
              >
                {candidate.merge_suggestion ? "Merge-aware review" : "Net new candidate"}
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
                Approve
              </button>
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => onReject(candidate.candidate_id)}
              >
                Reject
              </button>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}
