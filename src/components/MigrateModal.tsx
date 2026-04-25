import { useState } from "react";

type AgentType = "claude" | "codex" | "gemini" | "opencode";
type MigrationTargetAgent = Exclude<AgentType, "opencode">;
type MigrateMode = "copy" | "cut";

interface MigrateModalProps {
  sourceAgent: AgentType;
  onMigrate: (targetAgent: MigrationTargetAgent, mode: MigrateMode) => void;
  onClose: () => void;
}

function MigrateModal({ sourceAgent, onMigrate, onClose }: MigrateModalProps) {
  const [targetAgent, setTargetAgent] = useState<MigrationTargetAgent>(
    sourceAgent === "claude" ? "codex" : "claude"
  );
  const [mode, setMode] = useState<MigrateMode>("copy");

  const agents: { value: MigrationTargetAgent; label: string }[] = [
    { value: "claude", label: "Claude Code" },
    { value: "codex", label: "Codex CLI" },
    { value: "gemini", label: "Gemini CLI" },
  ];

  const availableTargets = agents.filter((a) => a.value !== sourceAgent);

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h3>迁移对话</h3>
        <div className="modal-content">
          <div className="form-group">
            <label>目标 Agent：</label>
            <select
              value={targetAgent}
              onChange={(e) => setTargetAgent(e.target.value as MigrationTargetAgent)}
            >
              {availableTargets.map((agent) => (
                <option key={agent.value} value={agent.value}>
                  {agent.label}
                </option>
              ))}
            </select>
          </div>
          
          <div className="form-group">
            <label>迁移方式：</label>
            <div className="radio-group">
              <label className="radio-label">
                <input
                  type="radio"
                  name="mode"
                  value="copy"
                  checked={mode === "copy"}
                  onChange={() => setMode("copy")}
                />
                <span className="radio-text">
                  <strong>复制</strong>
                  <small>保留原对话，在目标创建副本</small>
                </span>
              </label>
              <label className="radio-label">
                <input
                  type="radio"
                  name="mode"
                  value="cut"
                  checked={mode === "cut"}
                  onChange={() => setMode("cut")}
                />
                <span className="radio-text">
                  <strong>剪切</strong>
                  <small>删除原对话，移动到目标</small>
                </span>
              </label>
            </div>
          </div>
        </div>
        <div className="modal-actions">
          <button className="btn btn-secondary" onClick={onClose}>
            取消
          </button>
          <button
            className="btn btn-primary"
            onClick={() => onMigrate(targetAgent, mode)}
          >
            {mode === "copy" ? "复制" : "剪切"}
          </button>
        </div>
      </div>
    </div>
  );
}

export default MigrateModal;
