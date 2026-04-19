import type { ApprovedMemory } from "../chatmem-memory/types";

type RepoMemoryPanelProps = {
  memories: ApprovedMemory[];
  loading: boolean;
  onReverify: (memoryId: string) => void;
};

function formatFreshnessLabel(status: string) {
  if (status === "fresh") {
    return "fresh";
  }

  if (status === "stale") {
    return "stale";
  }

  return "unknown";
}

function formatVerifiedLabel(memory: ApprovedMemory) {
  if (!memory.last_verified_at) {
    return "Last verified: not yet verified";
  }

  const byline = memory.verified_by ? ` by ${memory.verified_by}` : "";
  return `Last verified: ${memory.last_verified_at}${byline}`;
}

export default function RepoMemoryPanel({ memories, loading, onReverify }: RepoMemoryPanelProps) {
  if (loading) {
    return (
      <section className="memory-panel">
        <div className="loading">
          <div className="spinner"></div>
        </div>
      </section>
    );
  }

  if (memories.length === 0) {
    return (
      <section className="memory-panel">
        <div className="empty-state">
          <div className="empty-state-icon">M</div>
          <div className="empty-state-text">No approved repository memory yet.</div>
        </div>
      </section>
    );
  }

  return (
    <section className="memory-panel">
      <div className="memory-panel-header">
        <h3>Repo Memory</h3>
        <p>Approved repository memory that can be used for startup context and handoffs.</p>
      </div>
      <div className="memory-card-list">
        {memories.map((memory) => {
          const freshnessState = memory.freshness_status || "unknown";
          const freshnessScore = Number.isFinite(memory.freshness_score) ? memory.freshness_score : 0;

          return (
            <article key={memory.memory_id} className="memory-card">
            <div className="memory-card-header">
              <div>
                <strong>{memory.title}</strong>
                <div className="memory-card-kind">{memory.kind}</div>
              </div>
              <div className="memory-card-badges">
                <span className={`memory-freshness memory-freshness-${freshnessState}`}>
                  {formatFreshnessLabel(freshnessState)}
                </span>
                <span className={`memory-status memory-status-${memory.status}`}>{memory.status}</span>
              </div>
            </div>
            <div className="memory-card-value">{memory.value}</div>
            <p className="memory-card-copy">{memory.usage_hint}</p>
            <div className="memory-card-meta">
              <span>{formatVerifiedLabel(memory)}</span>
              <span>Freshness score: {freshnessScore.toFixed(2)}</span>
            </div>
            {memory.evidence_refs.length > 0 && (
              <div className="memory-evidence-list">
                {memory.evidence_refs.slice(0, 2).map((evidence, index) => (
                  <div key={`${memory.memory_id}-evidence-${index}`} className="memory-evidence-item">
                    {evidence.excerpt}
                  </div>
                ))}
              </div>
            )}
            <div className="memory-card-actions">
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => onReverify(memory.memory_id)}
              >
                Re-verify
              </button>
            </div>
            </article>
          );
        })}
      </div>
    </section>
  );
}
