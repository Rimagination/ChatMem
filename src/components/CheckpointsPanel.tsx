import { useState } from "react";
import type { CheckpointRecord } from "../chatmem-memory/types";

type CheckpointsPanelProps = {
  checkpoints: CheckpointRecord[];
  loading: boolean;
  allAgents: string[];
  onCreate: () => void;
  onPromote: (checkpoint: CheckpointRecord, targetAgent: string) => void;
};

export default function CheckpointsPanel({
  checkpoints,
  loading,
  allAgents,
  onCreate,
  onPromote,
}: CheckpointsPanelProps) {
  const [targetByCheckpoint, setTargetByCheckpoint] = useState<Record<string, string>>({});
  const [copiedCheckpointId, setCopiedCheckpointId] = useState<string | null>(null);

  if (loading) {
    return (
      <section className="memory-panel">
        <div className="loading">
          <div className="spinner"></div>
        </div>
      </section>
    );
  }

  const handleResume = async (checkpoint: CheckpointRecord) => {
    if (!checkpoint.resume_command) {
      return;
    }

    try {
      await navigator.clipboard.writeText(checkpoint.resume_command);
      setCopiedCheckpointId(checkpoint.checkpoint_id);
      window.setTimeout(() => {
        setCopiedCheckpointId((current) =>
          current === checkpoint.checkpoint_id ? null : current,
        );
      }, 1800);
    } catch (error) {
      console.error("Failed to copy checkpoint resume command:", error);
    }
  };

  return (
    <section className="memory-panel">
      <div className="memory-panel-header">
        <h3>Checkpoints</h3>
        <p>Freeze the current conversation context into resumable snapshots and promote them into handoffs when another agent needs to continue.</p>
      </div>

      <div className="memory-card-actions">
        <button type="button" className="btn btn-primary" onClick={onCreate}>
          Freeze Current Context
        </button>
      </div>

      {checkpoints.length === 0 ? (
        <div className="empty-state">
          <div className="empty-state-icon">C</div>
          <div className="empty-state-text">No checkpoints saved for this repository yet.</div>
        </div>
      ) : (
        <div className="memory-card-list">
          {checkpoints.map((checkpoint) => {
            const availableTargets = allAgents.filter(
              (agent) => agent !== checkpoint.source_agent,
            );
            const selectedTarget =
              targetByCheckpoint[checkpoint.checkpoint_id] ?? availableTargets[0] ?? "";
            const canPromote = checkpoint.status === "active" && Boolean(selectedTarget);

            return (
              <article key={checkpoint.checkpoint_id} className="memory-card">
                <div className="memory-card-header">
                  <div>
                    <strong>{checkpoint.summary}</strong>
                    <div className="memory-card-kind">
                      {checkpoint.source_agent}
                      {" -> "}
                      {checkpoint.conversation_id}
                    </div>
                  </div>
                  <span
                    className={`checkpoint-status-pill checkpoint-status-${checkpoint.status}`}
                  >
                    {checkpoint.status}
                  </span>
                </div>

                <div className="checkpoint-meta-grid">
                  <div className="memory-list-inline">
                    <strong>Created:</strong> {new Date(checkpoint.created_at).toLocaleString()}
                  </div>
                  <div className="memory-list-inline">
                    <strong>Resume command:</strong>{" "}
                    {checkpoint.resume_command ?? "No resume command captured"}
                  </div>
                  {checkpoint.handoff_id && (
                    <div className="memory-list-inline">
                      <strong>Promoted handoff:</strong> {checkpoint.handoff_id}
                    </div>
                  )}
                </div>

                <div className="checkpoint-actions-row">
                  <button
                    type="button"
                    className="btn btn-secondary"
                    onClick={() => void handleResume(checkpoint)}
                    disabled={!checkpoint.resume_command}
                  >
                    {copiedCheckpointId === checkpoint.checkpoint_id ? "Resume Copied" : "Resume"}
                  </button>

                  <div className="checkpoint-promote-controls">
                    <label className="checkpoint-target-label">
                      <span>Target agent</span>
                      <select
                        aria-label="Target agent"
                        value={selectedTarget}
                        onChange={(event) =>
                          setTargetByCheckpoint((current) => ({
                            ...current,
                            [checkpoint.checkpoint_id]: event.target.value,
                          }))
                        }
                        disabled={availableTargets.length === 0 || checkpoint.status !== "active"}
                      >
                        {availableTargets.length === 0 ? (
                          <option value="">No target agents</option>
                        ) : (
                          availableTargets.map((target) => (
                            <option key={target} value={target}>
                              {target}
                            </option>
                          ))
                        )}
                      </select>
                    </label>

                    <button
                      type="button"
                      className="btn btn-secondary"
                      onClick={() => onPromote(checkpoint, selectedTarget)}
                      disabled={!canPromote}
                    >
                      Promote to Handoff
                    </button>
                  </div>
                </div>
              </article>
            );
          })}
        </div>
      )}
    </section>
  );
}
