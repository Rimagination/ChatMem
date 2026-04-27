import type { RunRecord } from "../chatmem-memory/types";

type RunsPanelProps = {
  runs: RunRecord[];
  loading: boolean;
};

function formatArtifactCount(count: number) {
  return count === 1 ? "1 artifact" : `${count} artifacts`;
}

export default function RunsPanel({ runs, loading }: RunsPanelProps) {
  if (loading) {
    return (
      <section className="memory-panel">
        <div className="loading">
          <div className="spinner"></div>
        </div>
      </section>
    );
  }

  if (runs.length === 0) {
    return (
      <section className="memory-panel">
        <div className="empty-state">
          <div className="empty-state-icon">R</div>
          <div className="empty-state-text">No ChatMem runs have been captured for this repository yet.</div>
        </div>
      </section>
    );
  }

  return (
    <section className="memory-panel">
      <div className="memory-panel-header">
        <h3>Runs</h3>
        <p>Timeline of recent cross-agent runs and the artifacts produced during each task.</p>
      </div>
      <div className="memory-card-list">
        {runs.map((run) => (
          <article key={run.run_id} className="memory-card">
            <div className="memory-card-header">
              <div>
                <strong>{run.task_hint || run.summary}</strong>
                <div className="memory-card-kind">{run.source_agent}</div>
              </div>
              <span className={`timeline-pill run-status-pill run-status-${run.status}`}>
                {run.status}
              </span>
            </div>
            <p className="memory-card-copy">{run.summary}</p>
            <div className="timeline-meta-row">
              <span>Started: {run.started_at}</span>
              <span>{run.ended_at ? `Ended: ${run.ended_at}` : "Still active"}</span>
              <span>{formatArtifactCount(run.artifact_count)}</span>
            </div>
          </article>
        ))}
      </div>
    </section>
  );
}
