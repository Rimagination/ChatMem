import type { ApprovedMemory } from "../chatmem-memory/types";

type RepoMemoryPanelProps = {
  memories: ApprovedMemory[];
  loading: boolean;
};

export default function RepoMemoryPanel({ memories, loading }: RepoMemoryPanelProps) {
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
        {memories.map((memory) => (
          <article key={memory.memory_id} className="memory-card">
            <div className="memory-card-header">
              <div>
                <strong>{memory.title}</strong>
                <div className="memory-card-kind">{memory.kind}</div>
              </div>
              <span className={`memory-status memory-status-${memory.status}`}>{memory.status}</span>
            </div>
            <div className="memory-card-value">{memory.value}</div>
            <p className="memory-card-copy">{memory.usage_hint}</p>
            {memory.evidence_refs.length > 0 && (
              <div className="memory-evidence-list">
                {memory.evidence_refs.slice(0, 2).map((evidence, index) => (
                  <div key={`${memory.memory_id}-evidence-${index}`} className="memory-evidence-item">
                    {evidence.excerpt}
                  </div>
                ))}
              </div>
            )}
          </article>
        ))}
      </div>
    </section>
  );
}
