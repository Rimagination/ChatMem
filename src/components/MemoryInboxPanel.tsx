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
