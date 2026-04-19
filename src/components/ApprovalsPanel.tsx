import type { ApprovedMemory, MemoryCandidate } from "../chatmem-memory/types";

type ApprovalsPanelProps = {
  candidates: MemoryCandidate[];
  memories: ApprovedMemory[];
  loading: boolean;
  onOpenInbox: () => void;
  onOpenRepoMemory: () => void;
  onReverify: (memoryId: string) => void;
};

function staleMemories(memories: ApprovedMemory[]) {
  return memories.filter(
    (memory) => memory.freshness_status === "needs_review" || memory.freshness_status === "stale",
  );
}

function evidenceSummary(candidate: MemoryCandidate) {
  if (candidate.evidence_refs.length === 0) {
    return "Needs linked evidence";
  }

  if (candidate.evidence_refs.length === 1) {
    return "1 evidence ref ready";
  }

  return `${candidate.evidence_refs.length} evidence refs ready`;
}

export default function ApprovalsPanel({
  candidates,
  memories,
  loading,
  onOpenInbox,
  onOpenRepoMemory,
  onReverify,
}: ApprovalsPanelProps) {
  const waitingToReverify = staleMemories(memories);

  if (loading) {
    return (
      <section className="memory-panel">
        <div className="loading">
          <div className="spinner"></div>
        </div>
      </section>
    );
  }

  if (candidates.length === 0 && waitingToReverify.length === 0) {
    return (
      <section className="memory-panel">
        <div className="empty-state">
          <div className="empty-state-icon">A</div>
          <div className="empty-state-text">No pending approvals or memories waiting to re-verify right now.</div>
        </div>
      </section>
    );
  }

  return (
    <section className="memory-panel">
      <div className="memory-panel-header">
        <h3>Approvals Workspace</h3>
        <p>Review the queue that still needs a human decision: pending memory proposals and memories waiting to be re-verified.</p>
      </div>

      <div className="approvals-summary-grid">
        <article className="approval-summary-card">
          <span className="approval-summary-label">Pending decisions</span>
          <strong>{candidates.length}</strong>
          <p>Inbox candidates waiting for review.</p>
          <button type="button" className="btn btn-secondary" onClick={onOpenInbox}>
            Open Inbox
          </button>
        </article>
        <article className="approval-summary-card">
          <span className="approval-summary-label">Waiting to Re-verify</span>
          <strong>{waitingToReverify.length}</strong>
          <p>Approved memories that may need a fresh validation pass.</p>
          <button type="button" className="btn btn-secondary" onClick={onOpenRepoMemory}>
            Open Repo Memory
          </button>
        </article>
      </div>

      {candidates.length > 0 && (
        <div className="approval-section">
          <div className="approval-section-header">
            <h4>Pending Review</h4>
            <button type="button" className="btn btn-secondary" onClick={onOpenInbox}>
              Review All
            </button>
          </div>
          <div className="approval-queue">
            {candidates.slice(0, 4).map((candidate) => (
              <article key={candidate.candidate_id} className="approval-item">
                <div className="approval-item-header">
                  <strong>{candidate.summary}</strong>
                  <span className="memory-card-confidence">{candidate.confidence.toFixed(2)}</span>
                </div>
                <div className="memory-card-meta">
                  <span>{evidenceSummary(candidate)}</span>
                  <span>{candidate.merge_suggestion ? "Merge-aware review recommended" : "New memory review"}</span>
                </div>
                {candidate.merge_suggestion && (
                  <div className="memory-review-note">
                    Potential merge with <strong>{candidate.merge_suggestion.memory_title}</strong>.{" "}
                    {candidate.merge_suggestion.reason}
                  </div>
                )}
              </article>
            ))}
          </div>
        </div>
      )}

      {waitingToReverify.length > 0 && (
        <div className="approval-section">
          <div className="approval-section-header">
            <h4>Waiting to Re-verify</h4>
            <button type="button" className="btn btn-secondary" onClick={onOpenRepoMemory}>
              See All
            </button>
          </div>
          <div className="approval-queue">
            {waitingToReverify.slice(0, 4).map((memory) => (
              <article key={memory.memory_id} className="approval-item">
                <div className="approval-item-header">
                  <strong>{memory.title}</strong>
                  <span
                    className={`memory-freshness ${
                      memory.freshness_status === "stale"
                        ? "memory-freshness-stale"
                        : "memory-freshness-review"
                    }`}
                  >
                    {memory.freshness_status}
                  </span>
                </div>
                <div className="memory-card-meta">
                  <span>{memory.last_verified_at ? `Last verified: ${memory.last_verified_at}` : "Never re-verified"}</span>
                  <span>Score: {Number.isFinite(memory.freshness_score) ? memory.freshness_score.toFixed(2) : "0.00"}</span>
                </div>
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => onReverify(memory.memory_id)}
                >
                  Re-verify
                </button>
              </article>
            ))}
          </div>
        </div>
      )}
    </section>
  );
}
