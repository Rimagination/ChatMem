import { useEffect, useMemo, useState } from "react";

type AgentType = "claude" | "codex" | "gemini" | "opencode";
type MigrateMode = "copy" | "cut";

interface MigrateModalProps {
  sourceAgent: AgentType;
  onMigrate: (targetAgent: AgentType, mode: MigrateMode) => void;
  onClose: () => void;
}

const agents: { value: AgentType; label: string }[] = [
  { value: "claude", label: "Claude Code" },
  { value: "codex", label: "Codex" },
  { value: "gemini", label: "Gemini" },
  { value: "opencode", label: "OpenCode" },
];

function firstTargetFor(sourceAgent: AgentType) {
  return agents.find((agent) => agent.value !== sourceAgent)?.value ?? "claude";
}

function MigrateModal({ sourceAgent, onMigrate, onClose }: MigrateModalProps) {
  const availableTargets = useMemo(
    () => agents.filter((agent) => agent.value !== sourceAgent),
    [sourceAgent],
  );
  const [targetAgent, setTargetAgent] = useState<AgentType>(() => firstTargetFor(sourceAgent));
  const [mode, setMode] = useState<MigrateMode>("copy");

  useEffect(() => {
    if (!availableTargets.some((agent) => agent.value === targetAgent)) {
      setTargetAgent(firstTargetFor(sourceAgent));
    }
  }, [availableTargets, sourceAgent, targetAgent]);

  return (
    <div className="modal-overlay" onClick={onClose}>
      <div className="modal" onClick={(event) => event.stopPropagation()}>
        <h3>迁移对话</h3>
        <div className="modal-content">
          <p className="modal-helper-text">
            可在 Claude、Codex、Gemini 和 OpenCode 之间迁移。写入后会自动读回验证，验证失败时不会删除原对话。
          </p>

          <div className="form-group">
            <label>目标平台</label>
            <select
              value={targetAgent}
              onChange={(event) => setTargetAgent(event.target.value as AgentType)}
            >
              {availableTargets.map((agent) => (
                <option key={agent.value} value={agent.value}>
                  {agent.label}
                </option>
              ))}
            </select>
          </div>

          <div className="form-group">
            <label>迁移方式</label>
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
                  <span>复制</span>
                  <small>保留原对话，在目标平台创建副本。</small>
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
                  <span>移动</span>
                  <small>目标平台验证成功后，再把原对话移入垃圾箱。</small>
                </span>
              </label>
            </div>
          </div>
        </div>
        <div className="modal-actions">
          <button className="btn btn-secondary" onClick={onClose}>
            取消
          </button>
          <button className="btn btn-primary" onClick={() => onMigrate(targetAgent, mode)}>
            {mode === "copy" ? "复制" : "移动"}
          </button>
        </div>
      </div>
    </div>
  );
}

export default MigrateModal;
