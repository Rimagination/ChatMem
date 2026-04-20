import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import ConversationList from "./components/ConversationList";
import ConversationDetail from "./components/ConversationDetail";
import MigrateModal from "./components/MigrateModal";
import SettingsPanel from "./components/SettingsPanel";
import RepoMemoryPanel from "./components/RepoMemoryPanel";
import MemoryInboxPanel from "./components/MemoryInboxPanel";
import ApprovalsPanel from "./components/ApprovalsPanel";
import EpisodesPanel from "./components/EpisodesPanel";
import RunsPanel from "./components/RunsPanel";
import ArtifactsPanel from "./components/ArtifactsPanel";
import CheckpointsPanel from "./components/CheckpointsPanel";
import HandoffsPanel from "./components/HandoffsPanel";
import HandoffComposerModal from "./components/HandoffComposerModal";
import { useI18n } from "./i18n/I18nProvider";
import type { Locale } from "./i18n/types";
import { loadSettings, updateSettings, type AppSettings } from "./settings/storage";
import { installAvailableUpdate, runUpdateCheck, type UpdateState } from "./updater/updater";
import { normalizeConversationTitle, truncateWorkspaceTitle } from "./utils/titleUtils";
import {
  createCheckpoint,
  createHandoffPacket,
  listArtifacts,
  listCheckpoints,
  listEpisodes,
  listHandoffs,
  listMemoryCandidates,
  listRuns,
  markHandoffConsumed,
  listRepoMemories,
  reverifyMemory,
  reviewMemoryCandidate,
} from "./chatmem-memory/api";
import type {
  ApprovedMemory,
  ArtifactRecord,
  CheckpointRecord,
  EpisodeRecord,
  HandoffPacket,
  HandoffTargetProfileOption,
  MemoryCandidate,
  RunRecord,
} from "./chatmem-memory/types";

interface ConversationSummary {
  id: string;
  source_agent: string;
  project_dir: string;
  created_at: string;
  updated_at: string;
  summary: string | null;
  message_count: number;
  file_count: number;
}

interface Conversation {
  id: string;
  source_agent: string;
  project_dir: string;
  created_at: string;
  updated_at: string;
  summary: string | null;
  storage_path?: string | null;
  resume_command?: string | null;
  messages: Message[];
  file_changes: FileChange[];
}

interface Message {
  id: string;
  timestamp: string;
  role: string;
  content: string;
  tool_calls: ToolCall[];
  metadata: Record<string, unknown>;
}

interface ToolCall {
  name: string;
  input: unknown;
  output: string | null;
  status: string;
}

interface FileChange {
  path: string;
  change_type: string;
  timestamp: string;
  message_id: string;
}

type AgentType = "claude" | "codex" | "gemini";
type MigrateMode = "copy" | "cut";
type CopyTarget = "location" | "resume";
type CopyState = {
  target: CopyTarget | null;
  status: "idle" | "success" | "error";
};
type WorkspaceView =
  | "conversation"
  | "checkpoints"
  | "repo-memory"
  | "memory-inbox"
  | "approvals"
  | "episodes"
  | "runs"
  | "artifacts"
  | "handoffs";
type HandoffComposerState = {
  targetAgent: string;
  profileOptions: HandoffTargetProfileOption[];
  checkpoint?: {
    checkpointId: string;
    repoRoot: string;
    sourceAgent: string;
    summary: string;
  };
} | null;

const COPY_RESET_DELAY_MS = 1800;
const TARGET_PROFILE_OPTIONS: Record<string, HandoffTargetProfileOption[]> = {
  claude: [
    {
      value: "claude_contextual",
      label: "Claude Contextual",
      description: "Carry narrative context, open questions, and review-ready notes for Claude.",
    },
    {
      value: "claude_reviewer",
      label: "Claude Reviewer",
      description: "Bias the packet toward auditability, edge cases, and validation checkpoints.",
    },
  ],
  codex: [
    {
      value: "codex_execution",
      label: "Codex Execution",
      description: "Emphasize concrete next steps, commands, and file-level action items.",
    },
    {
      value: "codex_debugger",
      label: "Codex Debugger",
      description: "Highlight repro steps, likely fault lines, and verification commands.",
    },
  ],
  gemini: [
    {
      value: "gemini_summarizer",
      label: "Gemini Summarizer",
      description: "Compress the latest repo context into a compact summary for quick catch-up.",
    },
    {
      value: "gemini_research",
      label: "Gemini Research",
      description: "Focus on history, related context, and cross-cutting background information.",
    },
  ],
};

function getAgentHeading(agent: AgentType) {
  switch (agent) {
    case "claude":
      return "CLAUDE 瀵硅瘽";
    case "codex":
      return "CODEX 瀵硅瘽";
    case "gemini":
      return "GEMINI 瀵硅瘽";
    default:
      return "瀵硅瘽";
  }
}

function App() {
  const { setLocale, t } = useI18n();
  const [selectedAgent, setSelectedAgent] = useState<AgentType>("claude");
  const [conversations, setConversations] = useState<ConversationSummary[]>([]);
  const [selectedConversation, setSelectedConversation] = useState<Conversation | null>(null);
  const [showMigrateModal, setShowMigrateModal] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [listLoading, setListLoading] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);
  const [searchQuery, setSearchQuery] = useState("");
  const [copyState, setCopyState] = useState<CopyState>({ target: null, status: "idle" });
  const [appSettings, setAppSettings] = useState<AppSettings>(() => loadSettings());
  const [updateState, setUpdateState] = useState<UpdateState>({ kind: "idle" });
  const [workspaceView, setWorkspaceView] = useState<WorkspaceView>("conversation");
  const [memoryLoading, setMemoryLoading] = useState(false);
  const [repoMemories, setRepoMemories] = useState<ApprovedMemory[]>([]);
  const [memoryCandidates, setMemoryCandidates] = useState<MemoryCandidate[]>([]);
  const [episodes, setEpisodes] = useState<EpisodeRecord[]>([]);
  const [runs, setRuns] = useState<RunRecord[]>([]);
  const [artifacts, setArtifacts] = useState<ArtifactRecord[]>([]);
  const [checkpoints, setCheckpoints] = useState<CheckpointRecord[]>([]);
  const [handoffs, setHandoffs] = useState<HandoffPacket[]>([]);
  const [handoffComposer, setHandoffComposer] = useState<HandoffComposerState>(null);
  const activeRepoRoot = selectedConversation?.project_dir ?? null;
  const allAgents = ["claude", "codex", "gemini"];
  const availableHandoffTargets = ["claude", "codex", "gemini"].filter(
    (agent) => agent !== selectedAgent,
  );
  const workspaceTitle = selectedConversation
    ? normalizeConversationTitle(selectedConversation.summary || selectedConversation.id)
    : "选择一段对话";
  const visibleWorkspaceTitle = truncateWorkspaceTitle(workspaceTitle);

  useEffect(() => {
    setSelectedConversation(null);
    setCopyState({ target: null, status: "idle" });
    setWorkspaceView("conversation");
  }, [selectedAgent]);

  useEffect(() => {
    loadConversations(searchQuery, selectedAgent);
  }, [searchQuery, selectedAgent]);

  useEffect(() => {
    setCopyState({ target: null, status: "idle" });
    setWorkspaceView("conversation");
  }, [selectedConversation?.id]);

  useEffect(() => {
    if (!appSettings.autoCheckUpdates) {
      return;
    }

    const timer = window.setTimeout(async () => {
      try {
        const nextState = await runUpdateCheck();
        if (nextState.kind === "available") {
          setUpdateState(nextState);
        }
      } catch {
        // Keep launch-time update checks silent on failure.
      }
    }, 3500);

    return () => window.clearTimeout(timer);
  }, [appSettings.autoCheckUpdates]);

  useEffect(() => {
    if (!activeRepoRoot) {
      setRepoMemories([]);
      setMemoryCandidates([]);
      setEpisodes([]);
      setRuns([]);
      setArtifacts([]);
      setCheckpoints([]);
      setHandoffs([]);
      return;
    }

    if (workspaceView === "conversation") {
      return;
    }

    const loadWorkspaceData = async () => {
      setMemoryLoading(true);
      try {
        if (workspaceView === "checkpoints") {
          setCheckpoints(await listCheckpoints(activeRepoRoot));
        } else if (workspaceView === "repo-memory") {
          setRepoMemories(await listRepoMemories(activeRepoRoot));
        } else if (workspaceView === "memory-inbox") {
          setMemoryCandidates(await listMemoryCandidates(activeRepoRoot, "pending_review"));
        } else if (workspaceView === "approvals") {
          const [nextMemories, nextCandidates] = await Promise.all([
            listRepoMemories(activeRepoRoot),
            listMemoryCandidates(activeRepoRoot, "pending_review"),
          ]);
          setRepoMemories(nextMemories);
          setMemoryCandidates(nextCandidates);
        } else if (workspaceView === "episodes") {
          setEpisodes(await listEpisodes(activeRepoRoot));
        } else if (workspaceView === "runs") {
          setRuns(await listRuns(activeRepoRoot));
        } else if (workspaceView === "artifacts") {
          setArtifacts(await listArtifacts(activeRepoRoot));
        } else if (workspaceView === "handoffs") {
          setHandoffs(await listHandoffs(activeRepoRoot));
        }
      } catch (error) {
        console.error(`Failed to load ${workspaceView}:`, error);
      } finally {
        setMemoryLoading(false);
      }
    };

    void loadWorkspaceData();
  }, [activeRepoRoot, workspaceView]);

  const loadConversations = async (query = searchQuery, agent = selectedAgent) => {
    setListLoading(true);
    try {
      const trimmedQuery = query.trim();
      const result = trimmedQuery
        ? await invoke<ConversationSummary[]>("search_conversations", {
            agent,
            query: trimmedQuery,
          })
        : await invoke<ConversationSummary[]>("list_conversations", {
            agent,
          });
      setConversations(result);
    } catch (error) {
      console.error("Failed to load conversations:", error);
    } finally {
      setListLoading(false);
    }
  };

  const loadConversationDetail = async (id: string, agent = selectedAgent) => {
    setDetailLoading(true);
    try {
      const result = await invoke<Conversation>("read_conversation", {
        agent,
        id,
      });
      setSelectedConversation(result);
    } catch (error) {
      console.error("Failed to load conversation:", error);
    } finally {
      setDetailLoading(false);
    }
  };

  const handleMigrate = async (targetAgent: AgentType, mode: MigrateMode) => {
    if (!selectedConversation) {
      return;
    }

    setDetailLoading(true);
    try {
      const newId = await invoke<string>("migrate_conversation", {
        source: selectedAgent,
        target: targetAgent,
        id: selectedConversation.id,
        mode,
      });
      const modeText = mode === "copy" ? "复制" : "剪切";
      alert(`对话${modeText}成功，新 ID: ${newId}`);
      setShowMigrateModal(false);
      setSearchQuery("");
      setSelectedConversation(null);
      setSelectedAgent(targetAgent);
      await loadConversations("", targetAgent);
      await loadConversationDetail(newId, targetAgent);
    } catch (error) {
      console.error("Failed to migrate conversation:", error);
      alert("对话迁移失败");
    } finally {
      setDetailLoading(false);
    }
  };

  const handleDelete = async () => {
    if (!selectedConversation) {
      return;
    }

    const confirmMessage = `确定要删除这段对话吗？\n\n"${selectedConversation.summary || selectedConversation.id}"\n\n此操作不可撤销。`;
    if (!confirm(confirmMessage)) {
      return;
    }

    setDetailLoading(true);
    try {
      await invoke("delete_conversation", {
        agent: selectedAgent,
        id: selectedConversation.id,
      });
      alert("对话已删除");
      setSelectedConversation(null);
      await loadConversations();
    } catch (error) {
      console.error("Failed to delete conversation:", error);
      alert("删除失败");
    } finally {
      setDetailLoading(false);
    }
  };

  const handleCopy = async (target: CopyTarget, value: string | null | undefined) => {
    if (!value) {
      return;
    }

    try {
      if (!navigator.clipboard?.writeText) {
        throw new Error("Clipboard unavailable");
      }
      await navigator.clipboard.writeText(value);
      setCopyState({ target, status: "success" });
    } catch (error) {
      console.error(`Failed to copy ${target}:`, error);
      setCopyState({ target, status: "error" });
    } finally {
      window.setTimeout(() => {
        setCopyState((current) =>
          current.target === target ? { target: null, status: "idle" } : current,
        );
      }, COPY_RESET_DELAY_MS);
    }
  };

  const handleApproveCandidate = async (candidate: MemoryCandidate) => {
    if (!activeRepoRoot) {
      return;
    }

    setMemoryLoading(true);
    try {
      await reviewMemoryCandidate({
        candidateId: candidate.candidate_id,
        action: "approve",
        editedTitle: candidate.summary,
        editedUsageHint: candidate.why_it_matters,
      });
      const [nextCandidates, nextMemories] = await Promise.all([
        listMemoryCandidates(activeRepoRoot, "pending_review"),
        listRepoMemories(activeRepoRoot),
      ]);
      setMemoryCandidates(nextCandidates);
      setRepoMemories(nextMemories);
    } catch (error) {
      console.error("Failed to approve memory candidate:", error);
    } finally {
      setMemoryLoading(false);
    }
  };

  const handleRejectCandidate = async (candidateId: string) => {
    if (!activeRepoRoot) {
      return;
    }

    setMemoryLoading(true);
    try {
      await reviewMemoryCandidate({
        candidateId,
        action: "reject",
      });
      setMemoryCandidates(await listMemoryCandidates(activeRepoRoot, "pending_review"));
    } catch (error) {
      console.error("Failed to reject memory candidate:", error);
    } finally {
      setMemoryLoading(false);
    }
  };

  const handleCreateHandoff = async (targetAgent: string) => {
    const profileOptions = TARGET_PROFILE_OPTIONS[targetAgent] ?? [];
    setHandoffComposer({
      targetAgent,
      profileOptions,
    });
  };

  const handleCreateCheckpoint = async () => {
    if (!activeRepoRoot || !selectedConversation) {
      return;
    }

    setMemoryLoading(true);
    try {
      const checkpoint = await createCheckpoint({
        repoRoot: activeRepoRoot,
        conversationId: `${selectedAgent}:${selectedConversation.id}`,
        sourceAgent: selectedAgent,
        summary: selectedConversation.summary ?? selectedConversation.id,
        resumeCommand: selectedConversation.resume_command ?? undefined,
        metadataJson: JSON.stringify({
          storage_path: selectedConversation.storage_path ?? null,
        }),
      });
      setCheckpoints((current) => [checkpoint, ...current]);
      setWorkspaceView("checkpoints");
    } catch (error) {
      console.error("Failed to create checkpoint:", error);
    } finally {
      setMemoryLoading(false);
    }
  };

  const handlePromoteCheckpoint = (checkpoint: CheckpointRecord, targetAgent: string) => {
    const profileOptions = TARGET_PROFILE_OPTIONS[targetAgent] ?? [];
    setHandoffComposer({
      targetAgent,
      profileOptions,
      checkpoint: {
        checkpointId: checkpoint.checkpoint_id,
        repoRoot: checkpoint.repo_root,
        sourceAgent: checkpoint.source_agent,
        summary: checkpoint.summary,
      },
    });
  };

  const handleConfirmCreateHandoff = async (targetProfile: string) => {
    if (!activeRepoRoot && !handoffComposer?.checkpoint) {
      return;
    }
    if (!handoffComposer) {
      return;
    }

    setMemoryLoading(true);
    try {
      const packet = await createHandoffPacket({
        repoRoot: handoffComposer.checkpoint?.repoRoot ?? activeRepoRoot ?? "",
        fromAgent: handoffComposer.checkpoint?.sourceAgent ?? selectedAgent,
        toAgent: handoffComposer.targetAgent,
        goalHint: handoffComposer.checkpoint?.summary ?? selectedConversation?.summary ?? undefined,
        targetProfile,
        checkpointId: handoffComposer.checkpoint?.checkpointId,
      });
      setHandoffs((current) => [packet, ...current]);
      if (handoffComposer.checkpoint) {
        setCheckpoints((current) =>
          current.map((checkpoint) =>
            checkpoint.checkpoint_id === handoffComposer.checkpoint?.checkpointId
              ? {
                  ...checkpoint,
                  status: "promoted",
                  handoff_id: packet.handoff_id,
                }
              : checkpoint,
          ),
        );
      }
      setWorkspaceView("handoffs");
      setHandoffComposer(null);
    } catch (error) {
      console.error("Failed to create handoff packet:", error);
    } finally {
      setMemoryLoading(false);
    }
  };

  const handleReverifyMemory = async (memoryId: string) => {
    if (!activeRepoRoot) {
      return;
    }

    setMemoryLoading(true);
    try {
      await reverifyMemory({
        memoryId,
        verifiedBy: selectedAgent,
      });
      setRepoMemories(await listRepoMemories(activeRepoRoot));
    } catch (error) {
      console.error("Failed to re-verify memory:", error);
    } finally {
      setMemoryLoading(false);
    }
  };

  const handleMarkHandoffConsumed = async (handoffId: string) => {
    setMemoryLoading(true);
    try {
      await markHandoffConsumed({
        handoffId,
        consumedBy: selectedAgent,
      });
      setHandoffs((current) =>
        current.map((handoff) =>
          handoff.handoff_id === handoffId
            ? {
                ...handoff,
                status: "consumed",
                consumed_by: selectedAgent,
                consumed_at: new Date().toISOString(),
              }
            : handoff,
        ),
      );
    } catch (error) {
      console.error("Failed to mark handoff consumed:", error);
    } finally {
      setMemoryLoading(false);
    }
  };

  const locationButtonLabel =
    copyState.target === "location" && copyState.status === "success"
      ? "位置已复制"
      : copyState.target === "location" && copyState.status === "error"
        ? "复制失败"
        : "复制位置";

  const resumeButtonLabel =
    copyState.target === "resume" && copyState.status === "success"
      ? "命令已复制"
      : copyState.target === "resume" && copyState.status === "error"
        ? "复制失败"
        : "复制恢复命令";

  return (
    <div className="app-shell">
      <aside className="sidebar">
        <header className="sidebar-topbar">
          <div className="brand-lockup">
            <div className="brand-mark" aria-hidden="true">
              <span className="brand-mark-bubble"></span>
              <span className="brand-mark-node"></span>
            </div>
            <div className="brand-copy">
              <h1>ChatMem</h1>
              <p>{t("brand.subtitle")}</p>
            </div>
          </div>
          <div className="sidebar-toolbar">
            <button
              type="button"
              className="toolbar-button"
              onClick={() => setShowSettings(true)}
              aria-label={t("settings.open")}
              title={t("settings.open")}
            >
              <span className="toolbar-button-icon" aria-hidden="true">
                鈿?
              </span>
              <span>{t("settings.short")}</span>
            </button>
            <button
              type="button"
              className="toolbar-button"
              onClick={() => loadConversations()}
              aria-label="鍒锋柊浼氳瘽鍒楄〃"
              title="鍒锋柊浼氳瘽鍒楄〃"
            >
              <span className="toolbar-button-icon" aria-hidden="true">
                鈫?
              </span>
              <span>鍒锋柊</span>
            </button>
          </div>
        </header>

        <div className="sidebar-controls">
          <div className="agent-tabs">
            <button
              className={`agent-tab ${selectedAgent === "claude" ? "active" : ""}`}
              onClick={() => setSelectedAgent("claude")}
            >
              Claude
            </button>
            <button
              className={`agent-tab ${selectedAgent === "codex" ? "active" : ""}`}
              onClick={() => setSelectedAgent("codex")}
            >
              Codex
            </button>
            <button
              className={`agent-tab ${selectedAgent === "gemini" ? "active" : ""}`}
              onClick={() => setSelectedAgent("gemini")}
            >
              Gemini
            </button>
          </div>

          <input
            type="text"
            className="search-box"
            placeholder={t("search.placeholder")}
            value={searchQuery}
            onChange={(event) => setSearchQuery(event.target.value)}
          />
        </div>

        <ConversationList
          conversations={conversations}
          selectedId={selectedConversation?.id || null}
          onSelect={loadConversationDetail}
          loading={listLoading}
        />
      </aside>

      <section className="workspace">
        <div className="workspace-header">
          <div className="workspace-title-block">
            <span className="workspace-eyebrow">{getAgentHeading(selectedAgent)}</span>
            <h2 title={workspaceTitle}>{visibleWorkspaceTitle}</h2>
          </div>

          {selectedConversation && (
            <div className="content-header-actions">
              <button
                className="btn btn-secondary"
                onClick={() => setShowMigrateModal(true)}
                disabled={detailLoading}
              >
                杩佺Щ
              </button>
              <button className="btn btn-danger" onClick={handleDelete} disabled={detailLoading}>
                鍒犻櫎
              </button>
            </div>
          )}
        </div>

        {selectedConversation && (
          <div className="conversation-meta-strip">
            <div className="conversation-meta-copy">
              <span className="conversation-meta-label">瀵硅瘽鏂囦欢浣嶇疆</span>
              <span
                className={`conversation-meta-value ${selectedConversation.storage_path ? "" : "is-muted"}`}
                title={selectedConversation.storage_path || "褰撳墠鏉ユ簮涓嶅彲鎻愪緵鏂囦欢浣嶇疆"}
              >
                {selectedConversation.storage_path || "褰撳墠鏉ユ簮涓嶅彲鎻愪緵鏂囦欢浣嶇疆"}
              </span>
            </div>

            <div className="conversation-meta-actions-block">
              <span className="conversation-meta-label">鎿嶄綔</span>
              <div className="conversation-meta-actions">
                <button
                  className="btn btn-secondary btn-action"
                  onClick={() => handleCopy("location", selectedConversation.storage_path)}
                  disabled={!selectedConversation.storage_path}
                >
                  {locationButtonLabel}
                </button>
                <button
                  className="btn btn-secondary btn-action"
                  onClick={() => handleCopy("resume", selectedConversation.resume_command)}
                  disabled={!selectedConversation.resume_command}
                >
                  {resumeButtonLabel}
                </button>
              </div>
            </div>
          </div>
        )}

        <div className="workspace-mode-tabs">
          <button
            type="button"
            className={`workspace-mode-tab ${workspaceView === "conversation" ? "active" : ""}`}
            onClick={() => setWorkspaceView("conversation")}
          >
            Conversation
          </button>
          <button
            type="button"
            className={`workspace-mode-tab ${workspaceView === "checkpoints" ? "active" : ""}`}
            onClick={() => setWorkspaceView("checkpoints")}
            disabled={!activeRepoRoot}
          >
            Checkpoints
          </button>
          <button
            type="button"
            className={`workspace-mode-tab ${workspaceView === "repo-memory" ? "active" : ""}`}
            onClick={() => setWorkspaceView("repo-memory")}
            disabled={!activeRepoRoot}
          >
            Repo Memory
          </button>
          <button
            type="button"
            className={`workspace-mode-tab ${workspaceView === "memory-inbox" ? "active" : ""}`}
            onClick={() => setWorkspaceView("memory-inbox")}
            disabled={!activeRepoRoot}
          >
            Memory Inbox
          </button>
          <button
            type="button"
            className={`workspace-mode-tab ${workspaceView === "approvals" ? "active" : ""}`}
            onClick={() => setWorkspaceView("approvals")}
            disabled={!activeRepoRoot}
          >
            Approvals
          </button>
          <button
            type="button"
            className={`workspace-mode-tab ${workspaceView === "episodes" ? "active" : ""}`}
            onClick={() => setWorkspaceView("episodes")}
            disabled={!activeRepoRoot}
          >
            Episodes
          </button>
          <button
            type="button"
            className={`workspace-mode-tab ${workspaceView === "runs" ? "active" : ""}`}
            onClick={() => setWorkspaceView("runs")}
            disabled={!activeRepoRoot}
          >
            Runs
          </button>
          <button
            type="button"
            className={`workspace-mode-tab ${workspaceView === "artifacts" ? "active" : ""}`}
            onClick={() => setWorkspaceView("artifacts")}
            disabled={!activeRepoRoot}
          >
            Artifacts
          </button>
          <button
            type="button"
            className={`workspace-mode-tab ${workspaceView === "handoffs" ? "active" : ""}`}
            onClick={() => setWorkspaceView("handoffs")}
            disabled={!activeRepoRoot}
          >
            Handoffs
          </button>
        </div>

        <div className="workspace-body">
          {workspaceView === "conversation" && detailLoading ? (
            <div className="detail-loading">
              <div className="spinner"></div>
            </div>
          ) : selectedConversation ? (
            workspaceView === "conversation" ? (
            <ConversationDetail conversation={selectedConversation} />
            ) : workspaceView === "checkpoints" ? (
              <CheckpointsPanel
                checkpoints={checkpoints}
                loading={memoryLoading}
                allAgents={allAgents}
                onCreate={handleCreateCheckpoint}
                onPromote={handlePromoteCheckpoint}
              />
            ) : workspaceView === "repo-memory" ? (
              <RepoMemoryPanel
                memories={repoMemories}
                loading={memoryLoading}
                onReverify={handleReverifyMemory}
              />
            ) : workspaceView === "memory-inbox" ? (
              <MemoryInboxPanel
                candidates={memoryCandidates}
                loading={memoryLoading}
                onApprove={handleApproveCandidate}
                onReject={handleRejectCandidate}
              />
            ) : workspaceView === "approvals" ? (
              <ApprovalsPanel
                candidates={memoryCandidates}
                memories={repoMemories}
                loading={memoryLoading}
                onOpenInbox={() => setWorkspaceView("memory-inbox")}
                onOpenRepoMemory={() => setWorkspaceView("repo-memory")}
                onReverify={handleReverifyMemory}
              />
            ) : workspaceView === "episodes" ? (
              <EpisodesPanel episodes={episodes} loading={memoryLoading} />
            ) : workspaceView === "runs" ? (
              <RunsPanel runs={runs} loading={memoryLoading} />
            ) : workspaceView === "artifacts" ? (
              <ArtifactsPanel artifacts={artifacts} loading={memoryLoading} />
            ) : (
              <HandoffsPanel
                handoffs={handoffs}
                loading={memoryLoading}
                currentAgent={selectedAgent}
                availableTargets={availableHandoffTargets}
                onCreate={handleCreateHandoff}
                onMarkConsumed={handleMarkHandoffConsumed}
              />
            )
          ) : (
            <div className="empty-state empty-state-large">
              <div className="empty-state-icon">○</div>
              <div className="empty-state-text">从左侧选择一段对话，查看上下文、文件位置和恢复命令。</div>
            </div>
          )}
        </div>
      </section>

      {updateState.kind === "available" && (
        <div className="update-toast" role="status" aria-live="polite">
          <div className="update-toast-copy">
            <strong>
              {t("settings.updateAvailablePrefix")} {updateState.version}
            </strong>
            {updateState.notes ? <p>{updateState.notes}</p> : null}
          </div>
          <button
            type="button"
            className="btn btn-primary"
            onClick={async () => {
              setUpdateState({ kind: "installing", version: updateState.version });
              try {
                const nextState = await installAvailableUpdate(updateState.version);
                setUpdateState(nextState);
              } catch {
                setUpdateState({ kind: "error", message: t("settings.updateError") });
              }
            }}
          >
            {t("settings.updateNow")}
          </button>
        </div>
      )}

      {showMigrateModal && selectedConversation && (
        <MigrateModal
          sourceAgent={selectedAgent}
          onMigrate={handleMigrate}
          onClose={() => setShowMigrateModal(false)}
        />
      )}

      {handoffComposer && (
        <HandoffComposerModal
          targetAgent={handoffComposer.targetAgent}
          profileOptions={handoffComposer.profileOptions}
          onClose={() => setHandoffComposer(null)}
          onCreate={handleConfirmCreateHandoff}
        />
      )}

      <SettingsPanel
        open={showSettings}
        title={t("settings.title")}
        closeLabel={t("common.close")}
        languageLabel={t("settings.language")}
        locale={appSettings.locale}
        autoCheckUpdates={appSettings.autoCheckUpdates}
        autoCheckLabel={t("settings.autoCheck")}
        checkUpdatesLabel={t("settings.checkUpdates")}
        checkingLabel={t("settings.checking")}
        upToDateLabel={t("settings.upToDate")}
        updateAvailablePrefix={t("settings.updateAvailablePrefix")}
        installUpdateLabel={t("settings.updateNow")}
        installingLabel={t("settings.installing")}
        updateState={updateState}
        onClose={() => setShowSettings(false)}
        onLocaleChange={(locale: Locale) => {
          setLocale(locale);
          const nextSettings = { ...appSettings, locale };
          setAppSettings(nextSettings);
        }}
        onAutoCheckChange={(autoCheckUpdates: boolean) => {
          const nextSettings = updateSettings({ autoCheckUpdates });
          setAppSettings(nextSettings);
        }}
        onCheckUpdates={async () => {
          setUpdateState({ kind: "checking" });
          try {
            const nextState = await runUpdateCheck();
            setUpdateState(nextState);
          } catch {
            setUpdateState({ kind: "error", message: t("settings.updateError") });
          }
        }}
        onInstallUpdate={async () => {
          if (updateState.kind !== "available") {
            return;
          }

          setUpdateState({ kind: "installing", version: updateState.version });
          try {
            const nextState = await installAvailableUpdate(updateState.version);
            setUpdateState(nextState);
          } catch {
            setUpdateState({ kind: "error", message: t("settings.updateError") });
          }
        }}
      />
    </div>
  );
}

export default App;

