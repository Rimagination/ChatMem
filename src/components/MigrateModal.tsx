import { useEffect, useMemo, useState } from "react";

type AgentType =
  | "claude"
  | "codex"
  | "gemini"
  | "opencode"
  | "zcode"
  | "hermes";
type MigrateMode = "copy" | "cut";
type MigrateKind = "full" | "brief";

interface MigrateModalProps {
  sourceAgent: AgentType;
  onMigrate: (targetAgent: AgentType, mode: MigrateMode) => void;
  onCopyContinuationBrief: () => void;
  continuationBriefAvailable: boolean;
  onClose: () => void;
}

const agents: { value: AgentType; label: string }[] = [
  { value: "claude", label: "Claude Code" },
  { value: "codex", label: "Codex" },
  { value: "gemini", label: "Gemini" },
  { value: "opencode", label: "OpenCode" },
  { value: "zcode", label: "ZCode" },
];

function firstTargetFor(sourceAgent: AgentType) {
  return agents.find((agent) => agent.value !== sourceAgent)?.value ?? "claude";
}

function MigrateModal({
  sourceAgent,
  onMigrate,
  onCopyContinuationBrief,
  continuationBriefAvailable,
  onClose,
}: MigrateModalProps) {
  const availableTargets = useMemo(
    () => agents.filter((agent) => agent.value !== sourceAgent),
    [sourceAgent],
  );
  const [targetAgent, setTargetAgent] = useState<AgentType>(() => firstTargetFor(sourceAgent));
  const [mode, setMode] = useState<MigrateMode>("copy");
  const [kind, setKind] = useState<MigrateKind>("full");

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
            <label>迁移内容</label>
            <div className="radio-group">
              <label className="radio-label">
                <input
                  type="radio"
                  name="migration-kind"
                  value="full"
                  checked={kind === "full"}
                  onChange={() => setKind("full")}
                />
                <span className="radio-text">
                  <span>完整对话迁移</span>
                  <small>保留原始消息和记录结构，并写入目标平台。</small>
                </span>
              </label>
              <label className="radio-label">
                <input
                  type="radio"
                  name="migration-kind"
                  value="brief"
                  checked={kind === "brief"}
                  onChange={() => setKind("brief")}
                />
                <span className="radio-text">
                  <span>总结式迁移</span>
                  <small>只复制继续卡片，用更少 token 在另一个 agent 里接着做。</small>
                </span>
              </label>
            </div>
          </div>

          {kind === "full" ? (
            <>
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
            </>
          ) : (
            <p className="modal-helper-text">
              总结式迁移不会写入目标平台，也不会删除原对话；它只把继续卡片复制到剪贴板。
            </p>
          )}
        </div>
        <div className="modal-actions">
          <button className="btn btn-secondary" onClick={onClose}>
            取消
          </button>
          <button
            className="btn btn-primary"
            onClick={() => {
              if (kind === "brief") {
                onCopyContinuationBrief();
                return;
              }
              onMigrate(targetAgent, mode);
            }}
            disabled={kind === "brief" && !continuationBriefAvailable}
          >
            {kind === "brief" ? "复制继续卡片" : mode === "copy" ? "复制" : "移动"}
          </button>
        </div>
      </div>
    </div>
  );
}

export default MigrateModal;
