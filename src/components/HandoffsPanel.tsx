import type { HandoffPacket } from "../chatmem-memory/types";

type HandoffsPanelProps = {
  handoffs: HandoffPacket[];
  loading: boolean;
  availableTargets: string[];
  onCreate: (targetAgent: string) => void;
};

export default function HandoffsPanel({
  handoffs,
  loading,
  availableTargets,
  onCreate,
}: HandoffsPanelProps) {
  if (loading) {
    return (
      <section className="memory-panel">
        <div className="loading">
          <div className="spinner"></div>
        </div>
      </section>
    );
  }

  return (
    <section className="memory-panel">
      <div className="memory-panel-header">
        <h3>Handoffs</h3>
        <p>Generate and review cross-agent working packets for the current repository.</p>
      </div>

      <div className="memory-card-actions">
        {availableTargets.map((target) => (
          <button
            key={target}
            type="button"
            className="btn btn-secondary"
            onClick={() => onCreate(target)}
          >
            Create handoff to {target}
          </button>
        ))}
      </div>

      {handoffs.length === 0 ? (
        <div className="empty-state">
          <div className="empty-state-icon">H</div>
          <div className="empty-state-text">No handoff packets created for this repository yet.</div>
        </div>
      ) : (
        <div className="memory-card-list">
          {handoffs.map((handoff) => (
            <article key={handoff.handoff_id} className="memory-card">
              <div className="memory-card-header">
                <div>
                  <strong>{handoff.current_goal}</strong>
                  <div className="memory-card-kind">
                    {handoff.from_agent}
                    {" -> "}
                    {handoff.to_agent}
                  </div>
                </div>
              </div>
              <div className="memory-card-split">
                <div>
                  <h4>Done</h4>
                  <ul>
                    {handoff.done_items.map((item) => (
                      <li key={`${handoff.handoff_id}-done-${item}`}>{item}</li>
                    ))}
                  </ul>
                </div>
                <div>
                  <h4>Next</h4>
                  <ul>
                    {handoff.next_items.map((item) => (
                      <li key={`${handoff.handoff_id}-next-${item}`}>{item}</li>
                    ))}
                  </ul>
                </div>
              </div>
              {handoff.key_files.length > 0 && (
                <div className="memory-list-inline">
                  <strong>Key files:</strong> {handoff.key_files.join(", ")}
                </div>
              )}
              {handoff.useful_commands.length > 0 && (
                <div className="memory-list-inline">
                  <strong>Commands:</strong> {handoff.useful_commands.join(", ")}
                </div>
              )}
            </article>
          ))}
        </div>
      )}
    </section>
  );
}
