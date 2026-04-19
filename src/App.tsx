import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import ConversationList from "./components/ConversationList";
import ConversationDetail from "./components/ConversationDetail";
import MigrateModal from "./components/MigrateModal";
import SettingsPanel from "./components/SettingsPanel";
import RepoMemoryPanel from "./components/RepoMemoryPanel";
import MemoryInboxPanel from "./components/MemoryInboxPanel";
import EpisodesPanel from "./components/EpisodesPanel";
import HandoffsPanel from "./components/HandoffsPanel";
import { useI18n } from "./i18n/I18nProvider";
import type { Locale } from "./i18n/types";
import { loadSettings, updateSettings, type AppSettings } from "./settings/storage";
import { installAvailableUpdate, runUpdateCheck, type UpdateState } from "./updater/updater";
import {
  createHandoffPacket,
  listEpisodes,
  listHandoffs,
  listMemoryCandidates,
  listRepoMemories,
  reviewMemoryCandidate,
} from "./chatmem-memory/api";
import type {
  ApprovedMemory,
  EpisodeRecord,
  HandoffPacket,
  MemoryCandidate,
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
type WorkspaceView = "conversation" | "repo-memory" | "memory-inbox" | "episodes" | "handoffs";

const COPY_RESET_DELAY_MS = 1800;

function getAgentHeading(agent: AgentType) {
  switch (agent) {
    case "claude":
      return "CLAUDE 对话";
    case "codex":
      return "CODEX 对话";
    case "gemini":
      return "GEMINI 对话";
    default:
      return "对话";
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
  const [handoffs, setHandoffs] = useState<HandoffPacket[]>([]);
  const activeRepoRoot = selectedConversation?.project_dir ?? null;

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
      setHandoffs([]);
      return;
    }

    if (workspaceView === "conversation") {
      return;
    }

    const loadWorkspaceData = async () => {
      setMemoryLoading(true);
      try {
        if (workspaceView === "repo-memory") {
          setRepoMemories(await listRepoMemories(activeRepoRoot));
        } else if (workspaceView === "memory-inbox") {
          setMemoryCandidates(await listMemoryCandidates(activeRepoRoot, "pending_review"));
        } else if (workspaceView === "episodes") {
          setEpisodes(await listEpisodes(activeRepoRoot));
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

  const loadConversationDetail = async (id: string) => {
    setDetailLoading(true);
    try {
      const result = await invoke<Conversation>("read_conversation", {
        agent: selectedAgent,
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
      if (mode === "cut") {
        setSelectedConversation(null);
        await loadConversations();
      }
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
    if (!activeRepoRoot) {
      return;
    }

    setMemoryLoading(true);
    try {
      const packet = await createHandoffPacket({
        repoRoot: activeRepoRoot,
        fromAgent: selectedAgent,
        toAgent: targetAgent,
        goalHint: selectedConversation?.summary ?? undefined,
      });
      setHandoffs((current) => [packet, ...current]);
      setWorkspaceView("handoffs");
    } catch (error) {
      console.error("Failed to create handoff packet:", error);
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
                ⚙
              </span>
              <span>{t("settings.short")}</span>
            </button>
            <button
              type="button"
              className="toolbar-button"
              onClick={() => loadConversations()}
              aria-label="刷新会话列表"
              title="刷新会话列表"
            >
              <span className="toolbar-button-icon" aria-hidden="true">
                ↻
              </span>
              <span>刷新</span>
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
            <h2>{selectedConversation ? selectedConversation.summary || selectedConversation.id : "选择一段对话"}</h2>
          </div>

          {selectedConversation && (
            <div className="content-header-actions">
              <button
                className="btn btn-secondary"
                onClick={() => setShowMigrateModal(true)}
                disabled={detailLoading}
              >
                迁移
              </button>
              <button className="btn btn-danger" onClick={handleDelete} disabled={detailLoading}>
                删除
              </button>
            </div>
          )}
        </div>

        {selectedConversation && (
          <div className="conversation-meta-strip">
            <div className="conversation-meta-copy">
              <span className="conversation-meta-label">对话文件位置</span>
              <span
                className={`conversation-meta-value ${selectedConversation.storage_path ? "" : "is-muted"}`}
                title={selectedConversation.storage_path || "当前来源不可提供文件位置"}
              >
                {selectedConversation.storage_path || "当前来源不可提供文件位置"}
              </span>
            </div>

            <div className="conversation-meta-actions-block">
              <span className="conversation-meta-label">操作</span>
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
            className={`workspace-mode-tab ${workspaceView === "episodes" ? "active" : ""}`}
            onClick={() => setWorkspaceView("episodes")}
            disabled={!activeRepoRoot}
          >
            Episodes
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
            ) : workspaceView === "repo-memory" ? (
              <RepoMemoryPanel memories={repoMemories} loading={memoryLoading} />
            ) : workspaceView === "memory-inbox" ? (
              <MemoryInboxPanel
                candidates={memoryCandidates}
                loading={memoryLoading}
                onApprove={handleApproveCandidate}
                onReject={handleRejectCandidate}
              />
            ) : workspaceView === "episodes" ? (
              <EpisodesPanel episodes={episodes} loading={memoryLoading} />
            ) : (
              <HandoffsPanel
                handoffs={handoffs}
                loading={memoryLoading}
                availableTargets={["claude", "codex", "gemini"].filter((agent) => agent !== selectedAgent)}
                onCreate={handleCreateHandoff}
              />
            )
          ) : (
            <div className="empty-state empty-state-large">
              <div className="empty-state-icon">⌘</div>
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
