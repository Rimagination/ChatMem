import type { EpisodeRecord } from "../chatmem-memory/types";

type EpisodesPanelProps = {
  episodes: EpisodeRecord[];
  loading: boolean;
};

export default function EpisodesPanel({ episodes, loading }: EpisodesPanelProps) {
  if (loading) {
    return (
      <section className="memory-panel">
        <div className="loading">
          <div className="spinner"></div>
        </div>
      </section>
    );
  }

  if (episodes.length === 0) {
    return (
      <section className="memory-panel">
        <div className="empty-state">
          <div className="empty-state-icon">E</div>
          <div className="empty-state-text">No repository episodes have been captured yet.</div>
        </div>
      </section>
    );
  }

  return (
    <section className="memory-panel">
      <div className="memory-panel-header">
        <h3>Episodes</h3>
        <p>Condensed experience cards derived from repository conversations.</p>
      </div>
      <div className="memory-card-list">
        {episodes.map((episode) => (
          <article key={episode.episode_id} className="memory-card">
            <div className="memory-card-header">
              <div>
                <strong>{episode.title}</strong>
                <div className="memory-card-kind">{episode.outcome}</div>
              </div>
            </div>
            <p className="memory-card-copy">{episode.summary}</p>
          </article>
        ))}
      </div>
    </section>
  );
}
