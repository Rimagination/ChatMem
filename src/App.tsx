import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { CSSProperties, MouseEvent as ReactMouseEvent, ReactNode } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { appWindow } from "@tauri-apps/api/window";
import ConversationDetail from "./components/ConversationDetail";
import MigrateModal from "./components/MigrateModal";
import SettingsPanel, {
  type AgentIntegrationOperationResult,
  type AgentIntegrationStatus,
  type SettingsSyncCopy,
  type UpgradeReadinessReport,
  type WebDavSyncResult,
  type WebDavVerificationInput,
} from "./components/SettingsPanel";
import HandoffComposerModal from "./components/HandoffComposerModal";
import LibraryPanel from "./components/LibraryPanel";
import MemoryInboxPanel, { type MemoryCandidateApprovalDraft } from "./components/MemoryInboxPanel";
import ProjectIndexStatus from "./components/ProjectIndexStatus";
import RepoMemoryPanel from "./components/RepoMemoryPanel";
import { useI18n } from "./i18n/I18nProvider";
import type { Locale } from "./i18n/types";
import {
  APP_FONT_OPTIONS,
  loadNativeSettings,
  loadSettings,
  loadWebDavPassword,
  saveSettings,
  saveNativeSettings,
  saveWebDavPassword,
  updateSettings,
  type AppFontFamily,
  type AppSettings,
} from "./settings/storage";
import { installAvailableUpdate, runUpdateCheck, type UpdateState } from "./updater/updater";
import { formatDateTime, formatDistanceToNow } from "./utils/dateUtils";
import {
  normalizeConversationTitle,
  truncateSidebarTitle,
  truncateWorkspaceTitle,
} from "./utils/titleUtils";
import { normalizeProjectPath, projectPathKey } from "./utils/projectPaths";
import { buildRepoLibraryRecords, type LibraryRecord } from "./library/model";
import packageInfo from "../package.json";
import brandIcon from "../src-tauri/icons/icon.png";
import {
  createCheckpoint,
  createHandoffPacket,
  getProjectContext,
  getRepoMemoryHealth,
  importAllLocalHistory,
  listCheckpoints,
  listHandoffs,
  listMemoryCandidates,
  listRepoMemories,
  listWikiPages,
  markHandoffConsumed,
  mergeRepoAlias,
  rebuildRepoWiki,
  scanRepoConversations,
  reverifyMemory,
  retireMemory,
  reviewMemoryCandidate,
} from "./chatmem-memory/api";
import type {
  ApprovedMemory,
  ArtifactRecord,
  CheckpointRecord,
  EpisodeRecord,
  HandoffPacket,
  HandoffTargetProfileOption,
  LocalHistoryImportReport,
  MemoryCandidate,
  RepoMemoryHealth,
  RunRecord,
  WikiPage,
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

type TrashTarget = Pick<ConversationSummary, "id" | "source_agent" | "summary">;

interface TrashedConversation {
  trashId: string;
  originalId: string;
  sourceAgent: string;
  projectDir: string;
  summary: string | null;
  trashedAt: string;
  expiresAt: string;
  storagePath?: string | null;
  resumeCommand?: string | null;
  remoteBackupDeleted: boolean;
  remoteBackupPath?: string | null;
  warnings: string[];
}

interface EmptyTrashResponse {
  removedCount: number;
  removedTrashIds: string[];
}

type TrashConfirmState = {
  targets: TrashTarget[];
  deleteRemoteBackup: boolean;
  busy: boolean;
  error: string | null;
} | null;

type EmptyTrashConfirmState = {
  busy: boolean;
  error: string | null;
} | null;

type AppNotice = {
  kind: "success" | "error";
  message: string;
} | null;

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

type AgentType = "claude" | "codex" | "gemini" | "opencode";
type TopPage = "continue" | "review" | "history" | "help";
type HistoryView = "conversations" | "recovery" | "transfers" | "outputs";
type MemoryDrawerTab = "inbox" | "approved" | "wiki" | "continuation";
type MigrateMode = "copy" | "cut";
type MigrationVerification = {
  readBack: boolean;
  listed: boolean;
  sourceMessageCount: number;
  targetMessageCount: number;
  sourceFileCount: number;
  targetFileCount: number;
  firstUserPreserved: boolean;
};
type MigrationResult = {
  newId: string;
  source: AgentType;
  target: AgentType;
  mode: MigrateMode;
  verified: boolean;
  verification: MigrationVerification;
  warnings: string[];
};
type CopyTarget = "location" | "resume";
type CopyState = {
  target: CopyTarget | null;
  status: "idle" | "success" | "error";
};
type LibraryArrangement = "projects" | "timeline" | "chats-first";
type LibrarySort = "updated" | "created";
type WorkspaceView = "conversation" | "history";
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

type HelpCard = {
  id: string;
  title: string;
  description: string;
  buttonLabel: string;
  answer: string;
  onSelect: () => void;
};

function readableError(error: unknown) {
  if (error instanceof Error) {
    return error.message;
  }
  if (typeof error === "string") {
    return error;
  }
  try {
    return JSON.stringify(error);
  } catch {
    return String(error);
  }
}

type ShellCopy = {
  nav: Record<TopPage, string>;
  navAria: string;
  projectSection: string;
  chatSection: string;
  settings: string;
  aboutChatMem: string;
  continueTitle: string;
  continueSubtitle: string;
  reviewTitle: string;
  reviewSubtitle: string;
  historyTitle: string;
  historySubtitle: string;
  helpTitle: string;
  helpSubtitle: string;
  searchHelpPlaceholder: string;
  recentTasks: string;
  recoverableProgress: string;
  nextStep: string;
  recentTransfers: string;
  noProgressTitle: string;
  noProgressBody: string;
  fileLocation: string;
  actionsLabel: string;
  copyLocation: string;
  copyLocationSuccess: string;
  copyResume: string;
  copyResumeSuccess: string;
  copyFailed: string;
  resumeWork: string;
  viewHistory: string;
  openConversation: string;
  chooseConversation: string;
  chooseConversationBody: string;
  suggestedConclusions: string;
  projectRules: string;
  pendingTransfers: string;
  confirmKeep: string;
  reviewLater: string;
  rejectKeep: string;
  reverifyRule: string;
  nothingToReview: string;
  nothingToReviewBody: string;
  historyFilters: Record<HistoryView, string>;
  createCheckpoint: string;
  createHandoff: string;
  createdAt: string;
  resumeCommand: string;
  promotedHandoff: string;
  outputsRuns: string;
  outputsArtifacts: string;
  outputsEpisodes: string;
  needHelp: string;
  commonQuestions: string;
  advancedTroubleshooting: string;
  connectionStatus: string;
  configLocations: string;
  relatedPaths: string;
  currentSource: string;
  noAvailablePath: string;
  workspaceSwitcherLabel: string;
  workspaceConversation: string;
  workspaceLocalHistory: string;
  filterSummary: string;
  allChats: string;
  organizeTitle: string;
  organizeArrangement: string;
  organizeSort: string;
  organizeFilters: string;
  arrangeProjects: string;
  arrangeTimeline: string;
  arrangeChatsFirst: string;
  sortUpdated: string;
  sortCreated: string;
  filterProject: string;
  filterTags: string;
  filterStatus: string;
  noTagsYet: string;
  noStatusesYet: string;
  collapseProjects: string;
  restoreProjects: string;
  openOrganizer: string;
  refreshList: string;
  trash: string;
  bulkSelect: string;
  cancelBulkSelect: string;
  bulkSelectionToolbar: string;
  selectConversation: string;
  selectVisible: string;
  clearSelection: string;
  selectedCount: (count: number) => string;
  moveSelectedToTrash: (count: number) => string;
  confirmTrashTitle: (count: number) => string;
  confirmTrashBody: (count: number) => string;
  confirmTrashLocalHint: (days: number) => string;
  confirmTrashRemoteBackup: string;
  confirmTrashRemoteUnavailable: string;
  confirmTrashRemotePasswordMissing: string;
  cancel: string;
  moveToTrash: string;
  movingToTrash: string;
  trashSuccessSingle: string;
  trashSuccessBulk: (count: number) => string;
  trashFailed: string;
  trashWorkspaceTitle: string;
  trashWorkspaceSubtitle: string;
  trashEmptyTitle: string;
  trashEmptyBody: string;
  trashRetentionDays: string;
  trashRetentionHint: string;
  emptyTrash: string;
  emptyingTrash: string;
  confirmEmptyTrashTitle: string;
  confirmEmptyTrashBody: (count: number) => string;
  emptyTrashSuccess: (count: number) => string;
  emptyTrashFailed: string;
  restore: string;
  restoring: string;
  restoreSuccess: string;
  restoreFailed: string;
  trashLoadFailed: string;
  remoteBackupDeleted: string;
  expiresAt: string;
  migrate: string;
  delete: string;
  helpHowItWorks: string;
};

type ProjectGroup = {
  id: string;
  label: string;
  fullPath: string;
  latestAt: string;
  conversations: ConversationSummary[];
};

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
  opencode: [
    {
      value: "opencode_execution",
      label: "OpenCode Execution",
      description: "Emphasize terminal-ready commands, touched files, and the next concrete step.",
    },
    {
      value: "opencode_review",
      label: "OpenCode Review",
      description: "Highlight evidence, risks, and verification notes for an OpenCode session.",
    },
  ],
};

function getAgentHeading(agent: AgentType, locale: Locale) {
  if (locale === "en") {
    return `${agent.toUpperCase()} Conversations`;
  }

  switch (agent) {
    case "claude":
      return "CLAUDE 对话";
    case "codex":
      return "CODEX 对话";
    case "gemini":
      return "GEMINI 对话";
    case "opencode":
      return "OPENCODE \u5bf9\u8bdd";
    default:
      return "对话";
  }
}

const AGENT_LABELS: Record<string, string> = {
  claude: "Claude",
  codex: "Codex",
  gemini: "Gemini",
  opencode: "OpenCode",
};

function getAgentLabel(agent: string) {
  return AGENT_LABELS[agent.toLowerCase()] ?? agent.charAt(0).toUpperCase() + agent.slice(1);
}

function getCurrentConversationLabel(agent: AgentType, locale: Locale) {
  if (locale === "en") {
    return `Current ${agent.toUpperCase()} conversation`;
  }

  return `${agent.toUpperCase()} \u5f53\u524d\u5bf9\u8bdd`;
}

function getAgentConfigLocation(agent: AgentType) {
  switch (agent) {
    case "claude":
      return "~/.claude";
    case "codex":
      return "~/.codex/config.toml";
    case "gemini":
      return "~/.gemini";
    case "opencode":
      return "$XDG_DATA_HOME/opencode or ~/.local/share/opencode";
    default:
      return "--";
  }
}

function getProjectLabel(projectDir: string) {
  const trimmed = normalizeProjectPath(projectDir).replace(/[\\/]+$/, "");
  const segments = trimmed.split(/[\\/]/).filter(Boolean);
  return segments[segments.length - 1] || projectDir;
}

function getWikiPreview(body: string) {
  return body
    .replace(/^#\s+[^\n]+\n*/u, "")
    .replace(/\n{2,}/g, "\n")
    .trim()
    .slice(0, 180);
}

function cleanWikiInline(text: string) {
  return text
    .replace(/\*\*([^*]+)\*\*/g, "$1")
    .replace(/`([^`]+)`/g, "$1")
    .trim();
}

function renderWikiBody(body: string) {
  const nodes: ReactNode[] = [];
  let listItems: string[] = [];

  const flushList = () => {
    if (listItems.length === 0) {
      return;
    }

    const currentItems = listItems;
    listItems = [];
    nodes.push(
      <ul key={`wiki-list-${nodes.length}`}>
        {currentItems.map((item, index) => (
          <li key={`${item}-${index}`}>{cleanWikiInline(item)}</li>
        ))}
      </ul>,
    );
  };

  body.split(/\r?\n/u).forEach((line) => {
    const trimmed = line.trim();

    if (!trimmed) {
      flushList();
      return;
    }

    if (/^#\s+/u.test(trimmed)) {
      return;
    }

    if (/^##\s+/u.test(trimmed)) {
      flushList();
      nodes.push(<h5 key={`wiki-heading-${nodes.length}`}>{cleanWikiInline(trimmed.replace(/^##\s+/u, ""))}</h5>);
      return;
    }

    if (/^-\s+/u.test(trimmed)) {
      listItems.push(trimmed.replace(/^-\s+/u, ""));
      return;
    }

    flushList();
    nodes.push(<p key={`wiki-paragraph-${nodes.length}`}>{cleanWikiInline(trimmed)}</p>);
  });

  flushList();
  return nodes;
}

function getWikiSourceLabel(page: WikiPage, locale: Locale) {
  const memoryCount = page.source_memory_ids.length;
  const episodeCount = page.source_episode_ids.length;
  const parts: string[] = [];

  if (memoryCount > 0) {
    parts.push(
      locale === "en"
        ? `${memoryCount} startup rule source${memoryCount === 1 ? "" : "s"}`
        : `${memoryCount} \u6761\u542f\u52a8\u89c4\u5219\u6765\u6e90`,
    );
  }

  if (episodeCount > 0) {
    parts.push(
      locale === "en"
        ? `${episodeCount} episode source${episodeCount === 1 ? "" : "s"}`
        : `${episodeCount} \u6761\u9636\u6bb5\u6765\u6e90`,
    );
  }

  if (parts.length === 0) {
    return locale === "en" ? "No linked sources" : "\u6682\u65e0\u5173\u8054\u6765\u6e90";
  }

  return parts.join(" / ");
}

function normalizeConversationProject<T extends { project_dir: string }>(conversation: T): T {
  const projectDir = normalizeProjectPath(conversation.project_dir);
  if (projectDir === conversation.project_dir) {
    return conversation;
  }

  return {
    ...conversation,
    project_dir: projectDir,
  };
}

const CODEX_DOCUMENTS_MARKER = "/documents/codex/";
const CODEX_DATE_FOLDER_PATTERN = /^\d{4}-\d{2}-\d{2}$/;
const CODEX_NEW_CHAT_LEAF_PATTERN = /^new-chat(?:-\d+)?$/i;
const CODEX_FLAT_NEW_CHAT_PATTERN =
  /^(?:new-chat(?:-\d+)?|\d{4}-\d{2}-\d{2}-new-chat(?:-\d+)?)$/i;

function isCodexGeneratedChatPath(projectDir: string) {
  const normalized = normalizeProjectPath(projectDir);
  const markerIndex = normalized.toLowerCase().lastIndexOf(CODEX_DOCUMENTS_MARKER);
  if (markerIndex < 0) {
    return false;
  }

  const relativePath = normalized
    .slice(markerIndex + CODEX_DOCUMENTS_MARKER.length)
    .replace(/^\/+|\/+$/g, "");
  const segments = relativePath.split("/").filter(Boolean);

  if (segments.length === 1) {
    return CODEX_FLAT_NEW_CHAT_PATTERN.test(segments[0]);
  }

  return (
    segments.length === 2 &&
    CODEX_DATE_FOLDER_PATTERN.test(segments[0]) &&
    CODEX_NEW_CHAT_LEAF_PATTERN.test(segments[1])
  );
}

function isRootProjectPlaceholder(projectDir: string) {
  const normalized = normalizeProjectPath(projectDir);
  return normalized === "/" || /^[a-zA-Z]:\/?$/.test(normalized);
}

function getConversationProjectDir(
  conversation: Pick<ConversationSummary, "project_dir" | "source_agent">,
) {
  const projectDir = normalizeProjectPath(conversation.project_dir);
  if (!projectDir) {
    return "";
  }

  if (isRootProjectPlaceholder(projectDir)) {
    return "";
  }

  if (conversation.source_agent.toLowerCase() === "codex" && isCodexGeneratedChatPath(projectDir)) {
    return "";
  }

  return projectDir;
}

function isProjectConversation(
  conversation: Pick<ConversationSummary, "project_dir" | "source_agent">,
) {
  return getConversationProjectDir(conversation).length > 0;
}

function getConversationKey(
  conversation: Pick<ConversationSummary, "id" | "source_agent"> | Pick<Conversation, "id" | "source_agent">,
) {
  return `${conversation.source_agent}:${conversation.id}`;
}

function sortConversations(conversations: ConversationSummary[], sortMode: LibrarySort) {
  const field = sortMode === "created" ? "created_at" : "updated_at";
  return [...conversations].sort((left, right) =>
    right[field].localeCompare(left[field]),
  );
}

function getShellCopy(locale: Locale): ShellCopy {
  if (locale === "en") {
    return {
      nav: {
        continue: "Continue Work",
        review: "Needs Review",
        history: "History",
        help: "Help",
      },
      navAria: "Primary navigation",
      projectSection: "Projects",
      chatSection: "Chats",
      settings: "Settings",
      aboutChatMem: "About us",
      continueTitle: "Continue Work",
      continueSubtitle: "Pick up the latest progress, commands, and next steps.",
      reviewTitle: "Needs Review",
      reviewSubtitle: "Keep human decisions in one place.",
      historyTitle: "History",
      historySubtitle: "Open deeper records only when you need them.",
      helpTitle: "Need help?",
      helpSubtitle: "Start with the most common questions.",
      searchHelpPlaceholder: "Search questions",
      recentTasks: "Recent Tasks",
      recoverableProgress: "Recoverable Progress",
      nextStep: "Suggested Next Step",
      recentTransfers: "Recent Transfers",
      noProgressTitle: "No recoverable progress yet",
      noProgressBody: "Choose a conversation from the left to continue.",
      fileLocation: "Conversation file location",
      actionsLabel: "Actions",
      copyLocation: "Copy location",
      copyLocationSuccess: "Location copied",
      copyResume: "Copy resume command",
      copyResumeSuccess: "Command copied",
      copyFailed: "Copy failed",
      resumeWork: "Resume this work",
      viewHistory: "View History",
      openConversation: "Open",
      chooseConversation: "Choose a conversation",
      chooseConversationBody: "Select a conversation from the left to unlock recovery and review.",
      suggestedConclusions: "Suggested conclusions to keep",
      projectRules: "Project rules to re-check",
      pendingTransfers: "Transfer summaries waiting",
      confirmKeep: "Confirm",
      reviewLater: "Review later",
      rejectKeep: "Do not keep",
      reverifyRule: "Re-verify",
      nothingToReview: "Nothing needs your review",
      nothingToReviewBody: "Items that need a decision will appear here.",
      historyFilters: {
        conversations: "Conversations",
        recovery: "Recovery",
        transfers: "Transfers",
        outputs: "Outputs",
      },
      createCheckpoint: "Freeze current context",
      createHandoff: "Create handoff",
      createdAt: "Created",
      resumeCommand: "Resume command",
      promotedHandoff: "Promoted handoff",
      outputsRuns: "Runs",
      outputsArtifacts: "Artifacts",
      outputsEpisodes: "Episodes",
      needHelp: "Need help?",
      commonQuestions: "Most common questions",
      advancedTroubleshooting: "Advanced troubleshooting",
      connectionStatus: "Current source",
      configLocations: "Configuration locations",
      relatedPaths: "Related paths",
      currentSource: "Current source",
      noAvailablePath: "No file path is available from this source",
      workspaceSwitcherLabel: "Workspace view",
      workspaceConversation: "Current conversation",
      workspaceLocalHistory: "Local history",
      filterSummary: "Filtered",
      allChats: "All chats",
      organizeTitle: "Organize",
      organizeArrangement: "Arrangement",
      organizeSort: "Sort",
      organizeFilters: "Filters",
      arrangeProjects: "By project",
      arrangeTimeline: "Timeline list",
      arrangeChatsFirst: "Chats first",
      sortUpdated: "Recently updated",
      sortCreated: "Recently created",
      filterProject: "Project",
      filterTags: "Tags",
      filterStatus: "Status",
      noTagsYet: "No tags yet",
      noStatusesYet: "No status filters yet",
      collapseProjects: "Collapse all projects",
      restoreProjects: "Restore previous expansion",
      openOrganizer: "Filter, sort, and organize conversations",
      refreshList: "Refresh conversations",
      trash: "Trash",
      bulkSelect: "Select conversations",
      cancelBulkSelect: "Cancel selection",
      bulkSelectionToolbar: "Bulk conversation actions",
      selectConversation: "Select",
      selectVisible: "Select visible",
      clearSelection: "Clear",
      selectedCount: (count) => `${count} selected`,
      moveSelectedToTrash: (count) =>
        count > 0 ? `Move ${count} selected to Trash` : "Move selected to Trash",
      confirmTrashTitle: (count) =>
        count === 1 ? "Move this conversation to Trash?" : `Move ${count} conversations to Trash?`,
      confirmTrashBody: (count) =>
        count === 1
          ? "ChatMem will keep a recovery snapshot and move the local conversation to the system Trash when possible."
          : "ChatMem will keep recovery snapshots and move local conversations to the system Trash when possible.",
      confirmTrashLocalHint: (days) =>
        `Recovery snapshots are kept for ${days} day${days === 1 ? "" : "s"}.`,
      confirmTrashRemoteBackup: "Also delete the WebDAV cloud backup",
      confirmTrashRemoteUnavailable: "WebDAV sync is not configured, so no cloud backup will be deleted.",
      confirmTrashRemotePasswordMissing: "WebDAV password is missing. Save it in Settings before deleting cloud backups.",
      cancel: "Cancel",
      moveToTrash: "Move to Trash",
      movingToTrash: "Moving...",
      trashSuccessSingle: "Conversation moved to Trash.",
      trashSuccessBulk: (count) =>
        `${count} conversation${count === 1 ? "" : "s"} moved to Trash.`,
      trashFailed: "Could not move conversation to Trash",
      trashWorkspaceTitle: "Trash",
      trashWorkspaceSubtitle:
        "Deleted conversations stay recoverable here before the retention window expires.",
      trashEmptyTitle: "Trash is empty",
      trashEmptyBody: "Deleted conversations will appear here with a restore action.",
      trashRetentionDays: "Retention days",
      trashRetentionHint: "Applies to new deletions. Existing items keep their current expiry.",
      emptyTrash: "Empty Trash",
      emptyingTrash: "Emptying...",
      confirmEmptyTrashTitle: "Empty Trash?",
      confirmEmptyTrashBody: (count) =>
        `This permanently removes ${count} recovery snapshot${
          count === 1 ? "" : "s"
        }. You will not be able to restore ${count === 1 ? "it" : "them"} from ChatMem.`,
      emptyTrashSuccess: (count) =>
        count === 1 ? "Trash emptied. 1 snapshot removed." : `Trash emptied. ${count} snapshots removed.`,
      emptyTrashFailed: "Could not empty Trash",
      restore: "Restore",
      restoring: "Restoring...",
      restoreSuccess: "Conversation restored.",
      restoreFailed: "Could not restore conversation",
      trashLoadFailed: "Could not load Trash",
      remoteBackupDeleted: "WebDAV backup deleted",
      expiresAt: "Expires",
      migrate: "Migrate",
      delete: "Delete",
      helpHowItWorks: "How ChatMem works in the background",
    };
  }

  return {
    nav: {
      continue: "继续工作",
      review: "待确认",
      history: "历史",
      help: "帮助",
    },
    navAria: "主导航",
    projectSection: "项目",
    chatSection: "对话",
    settings: "设置",
    aboutChatMem: "关于我们",
    continueTitle: "继续工作",
    continueSubtitle: "把最近的进度、恢复命令和下一步放在一起。",
    reviewTitle: "待确认",
    reviewSubtitle: "只把需要你判断的内容放在这里。",
    historyTitle: "历史",
    historySubtitle: "需要下钻时再看详细记录。",
    helpTitle: "需要帮助？",
    helpSubtitle: "先从最常见的问题开始。",
    searchHelpPlaceholder: "搜索问题",
    recentTasks: "最近任务",
    recoverableProgress: "可恢复进度",
    nextStep: "建议下一步",
    recentTransfers: "最近移交",
    noProgressTitle: "还没有可恢复的进度",
    noProgressBody: "先从左侧选择一段对话开始。",
    fileLocation: "对话文件位置",
    actionsLabel: "操作",
    copyLocation: "复制位置",
    copyLocationSuccess: "位置已复制",
    copyResume: "复制恢复命令",
    copyResumeSuccess: "命令已复制",
    copyFailed: "复制失败",
    resumeWork: "继续这段工作",
    viewHistory: "查看历史",
    openConversation: "打开",
    chooseConversation: "先选择一段对话",
    chooseConversationBody: "从左侧选择一段对话，再继续恢复、审批或移交。",
    suggestedConclusions: "建议记住的结论",
    projectRules: "需要复核的项目规则",
    pendingTransfers: "等待确认的移交摘要",
    confirmKeep: "确认保留",
    reviewLater: "稍后再看",
    rejectKeep: "不保留",
    reverifyRule: "重新核验",
    nothingToReview: "暂时没有待确认内容",
    nothingToReviewBody: "需要你决定的内容会集中出现在这里。",
    historyFilters: {
      conversations: "对话",
      recovery: "恢复",
      transfers: "移交",
      outputs: "输出",
    },
    createCheckpoint: "冻结当前上下文",
    createHandoff: "创建交接包",
    createdAt: "创建时间",
    resumeCommand: "恢复命令",
    promotedHandoff: "已提升交接包",
    outputsRuns: "运行记录",
    outputsArtifacts: "产物",
    outputsEpisodes: "阶段记录",
    needHelp: "需要帮助？",
    commonQuestions: "最常见的问题",
    advancedTroubleshooting: "高级排查",
    connectionStatus: "当前来源",
    configLocations: "配置位置",
    relatedPaths: "相关路径",
    currentSource: "当前来源",
    noAvailablePath: "当前来源不可提供文件位置",
    workspaceSwitcherLabel: "工作区视图",
    workspaceConversation: "当前对话",
    workspaceLocalHistory: "本地历史",
    filterSummary: "已筛选",
    allChats: "全部聊天",
    organizeTitle: "整理",
    organizeArrangement: "整理方式",
    organizeSort: "排序条件",
    organizeFilters: "显示",
    arrangeProjects: "按项目",
    arrangeTimeline: "时间顺序列表",
    arrangeChatsFirst: "聊天优先",
    sortUpdated: "已更新",
    sortCreated: "已创建",
    filterProject: "项目",
    filterTags: "标签",
    filterStatus: "状态",
    noTagsYet: "暂无可用标签",
    noStatusesYet: "暂无可用状态",
    collapseProjects: "全部收起",
    restoreProjects: "恢复之前展开的分组",
    openOrganizer: "筛选、排序和整理对话",
    refreshList: "刷新会话列表",
    trash: "垃圾箱",
    bulkSelect: "\u6279\u91cf\u9009\u62e9",
    cancelBulkSelect: "\u53d6\u6d88\u9009\u62e9",
    bulkSelectionToolbar: "\u6279\u91cf\u5bf9\u8bdd\u64cd\u4f5c",
    selectConversation: "\u9009\u62e9",
    selectVisible: "\u5168\u9009\u53ef\u89c1",
    clearSelection: "\u6e05\u7a7a",
    selectedCount: (count) => `\u5df2\u9009 ${count}`,
    moveSelectedToTrash: () => "\u79fb\u5230\u5783\u573e\u7bb1",
    confirmTrashTitle: (count) =>
      count === 1 ? "移动这段对话到垃圾箱？" : `移动 ${count} 段对话到垃圾箱？`,
    confirmTrashBody: (count) =>
      count === 1
        ? "ChatMem 会保留一份可恢复快照，并尽量把本地对话移入系统回收站。"
        : "ChatMem 会保留可恢复快照，并尽量把这些本地对话移入系统回收站。",
    confirmTrashLocalHint: (days) => `可恢复快照会保留 ${days} 天。`,
    confirmTrashRemoteBackup: "同时删除 WebDAV 网盘备份",
    confirmTrashRemoteUnavailable: "未配置 WebDAV 同步，不会处理云端备份。",
    confirmTrashRemotePasswordMissing: "缺少 WebDAV 密码。请先在设置里保存密码，再删除云端备份。",
    cancel: "取消",
    moveToTrash: "移到垃圾箱",
    movingToTrash: "正在移动...",
    trashSuccessSingle: "\u5bf9\u8bdd\u5df2\u79fb\u5230\u5783\u573e\u7bb1\u3002",
    trashSuccessBulk: (count) => `${count} \u6bb5\u5bf9\u8bdd\u5df2\u79fb\u5230\u5783\u573e\u7bb1\u3002`,
    trashFailed: "\u79fb\u5230\u5783\u573e\u7bb1\u5931\u8d25",
    trashWorkspaceTitle: "垃圾箱",
    trashWorkspaceSubtitle: "误删的对话先放在这里，保留期内可以恢复。",
    trashEmptyTitle: "垃圾箱是空的",
    trashEmptyBody: "删除后的对话会出现在这里，并提供恢复操作。",
    trashRetentionDays: "保留天数",
    trashRetentionHint: "影响之后删除的对话；已有项目保留原到期时间。",
    emptyTrash: "清空垃圾箱",
    emptyingTrash: "正在清空...",
    confirmEmptyTrashTitle: "清空垃圾箱？",
    confirmEmptyTrashBody: (count) =>
      `这会永久移除 ${count} 份恢复快照，之后不能再从 ChatMem 恢复。`,
    emptyTrashSuccess: (count) => `垃圾箱已清空，移除了 ${count} 份恢复快照。`,
    emptyTrashFailed: "清空垃圾箱失败",
    restore: "恢复",
    restoring: "正在恢复...",
    restoreSuccess: "对话已恢复。",
    restoreFailed: "恢复对话失败",
    trashLoadFailed: "加载垃圾箱失败",
    remoteBackupDeleted: "已删除 WebDAV 备份",
    expiresAt: "到期",
    migrate: "迁移",
    delete: "删除",
    helpHowItWorks: "了解后台工作方式",
  };
}

function getSyncCopy(locale: Locale): SettingsSyncCopy {
  if (locale === "en") {
    return {
      title: "Conversation Data Sync",
      methodLabel: "Conversation data sync method:",
      webdavLabel: "WebDAV",
      protocolLabel: "Protocol",
      serverPathLabel: "Server and path",
      usernameLabel: "Username",
      passwordLabel: "Password",
      showPasswordLabel: "Show",
      hidePasswordLabel: "Hide",
      downloadFilesLabel: "Download files",
      onSyncDownloadLabel: "At sync time",
      asNeededDownloadLabel: "As needed",
      verifyServerLabel: "Verify server",
      verifyingServerLabel: "Verifying...",
      verifySuccessLabel: "Verification successful",
      verifyMissingFieldsLabel: "Fill in the server, username, and password first.",
      verifyFailedPrefix: "Verification failed",
      syncNowLabel: "Sync now",
      syncingNowLabel: "Syncing...",
      syncSuccessPrefix: "Synced",
      syncSuccessSuffix: "files to WebDAV",
      syncTargetLabel: "Remote folder",
      syncFailedPrefix: "Sync failed",
    };
  }

  return {
    title: "\u5bf9\u8bdd\u6570\u636e\u540c\u6b65",
    methodLabel: "\u5bf9\u8bdd\u6570\u636e\u540c\u6b65\u65b9\u5f0f\uff1a",
    webdavLabel: "WebDAV",
    protocolLabel: "\u534f\u8bae",
    serverPathLabel: "\u7f51\u5740",
    usernameLabel: "\u7528\u6237\u540d",
    passwordLabel: "\u5bc6\u7801",
    showPasswordLabel: "\u663e\u793a",
    hidePasswordLabel: "\u9690\u85cf",
    downloadFilesLabel: "\u4e0b\u8f7d\u6587\u4ef6",
    onSyncDownloadLabel: "\u5728\u540c\u6b65\u65f6",
    asNeededDownloadLabel: "\u9700\u8981\u65f6",
    verifyServerLabel: "\u9a8c\u8bc1\u670d\u52a1\u5668",
    verifyingServerLabel: "\u6b63\u5728\u9a8c\u8bc1...",
    verifySuccessLabel: "\u9a8c\u8bc1\u6210\u529f",
    verifyMissingFieldsLabel: "\u8bf7\u5148\u586b\u5199\u7f51\u5740\u3001\u7528\u6237\u540d\u548c\u5bc6\u7801",
    verifyFailedPrefix: "\u9a8c\u8bc1\u5931\u8d25",
    syncNowLabel: "\u7acb\u5373\u540c\u6b65",
    syncingNowLabel: "\u6b63\u5728\u540c\u6b65...",
    syncSuccessPrefix: "\u5df2\u540c\u6b65",
    syncSuccessSuffix: "\u4e2a\u6587\u4ef6\u5230 WebDAV",
    syncTargetLabel: "\u8fdc\u7a0b\u76ee\u5f55",
    syncFailedPrefix: "\u540c\u6b65\u5931\u8d25",
  };
}

const ACKNOWLEDGED_SYSTEMS = [
  "mem0",
  "Letta / MemGPT",
  "Zep",
  "Cognee",
  "LangGraph / LangMem",
  "LLM Wiki / DeepWiki / CodeWiki",
  "OpenAI / Claude native memory",
];

function WindowButtonIcon({
  type,
}: {
  type:
    | "minimize"
    | "maximize"
    | "close"
    | "collapseAll"
    | "restoreExpansion"
    | "organize"
    | "bulkSelect"
    | "trash"
    | "settings"
    | "help"
    | "chevron";
}) {
  if (type === "minimize") {
    return (
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <path d="M3 8.5h10" />
      </svg>
    );
  }

  if (type === "maximize") {
    return (
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <rect x="3.5" y="3.5" width="9" height="9" rx="1.2" />
      </svg>
    );
  }

  if (type === "close") {
    return (
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <path d="M4 4l8 8" />
        <path d="M12 4l-8 8" />
      </svg>
    );
  }

  if (type === "collapseAll") {
    return (
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <path d="M6.5 3.5 3.5 6.5" />
        <path d="M3.5 3.5v3h3" />
        <path d="M9.5 12.5l3-3" />
        <path d="M12.5 12.5v-3h-3" />
      </svg>
    );
  }

  if (type === "restoreExpansion") {
    return (
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <path d="M10.5 4H12v1.5" />
        <path d="M5.5 12H4v-1.5" />
      </svg>
    );
  }

  if (type === "organize") {
    return (
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <path d="M3 4h10" />
        <path d="M5 8h6" />
        <path d="M7 12h2" />
      </svg>
    );
  }

  if (type === "bulkSelect") {
    return (
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <rect x="3" y="3" width="4" height="4" rx="0.8" />
        <path d="M9 5h4" />
        <rect x="3" y="9" width="4" height="4" rx="0.8" />
        <path d="M9 11h4" />
      </svg>
    );
  }

  if (type === "trash") {
    return (
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <path d="M5.5 4.5h5" />
        <path d="M6.5 4.5V3.3h3v1.2" />
        <path d="M4.5 6h7" />
        <path d="M5.2 6.2l.5 6.3h4.6l.5-6.3" />
        <path d="M7.1 8v3" />
        <path d="M8.9 8v3" />
      </svg>
    );
  }

  if (type === "settings") {
    return (
      <svg viewBox="0 0 24 24" aria-hidden="true">
        <path d="M12.2 2h-.4a2 2 0 0 0-2 2v.2a2 2 0 0 1-1 1.7l-.4.2a2 2 0 0 1-2 0l-.2-.1a2 2 0 0 0-2.7.7l-.2.4a2 2 0 0 0 .7 2.7l.2.1a2 2 0 0 1 1 1.7v.6a2 2 0 0 1-1 1.7l-.2.1a2 2 0 0 0-.7 2.7l.2.4a2 2 0 0 0 2.7.7l.2-.1a2 2 0 0 1 2 0l.4.2a2 2 0 0 1 1 1.7v.2a2 2 0 0 0 2 2h.4a2 2 0 0 0 2-2v-.2a2 2 0 0 1 1-1.7l.4-.2a2 2 0 0 1 2 0l.2.1a2 2 0 0 0 2.7-.7l.2-.4a2 2 0 0 0-.7-2.7l-.2-.1a2 2 0 0 1-1-1.7v-.6a2 2 0 0 1 1-1.7l.2-.1a2 2 0 0 0 .7-2.7l-.2-.4a2 2 0 0 0-2.7-.7l-.2.1a2 2 0 0 1-2 0l-.4-.2a2 2 0 0 1-1-1.7V4a2 2 0 0 0-2-2z" />
        <circle cx="12" cy="12" r="3" />
      </svg>
    );
  }

  if (type === "help") {
    return (
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <circle cx="8" cy="8" r="5.3" />
        <path d="M6.6 6.4A1.5 1.5 0 0 1 8.1 5c.9 0 1.5.5 1.5 1.3 0 .6-.3 1-.9 1.4-.6.4-.8.7-.8 1.4" />
        <path d="M8 11.1h.01" />
      </svg>
    );
  }

  return (
    <svg viewBox="0 0 16 16" aria-hidden="true">
      <path d="M6 4l4 4-4 4" />
    </svg>
  );
}

function App() {
  const { locale, setLocale, t } = useI18n();
  const shell = useMemo(() => getShellCopy(locale), [locale]);
  const syncCopy = useMemo(() => getSyncCopy(locale), [locale]);
  const [selectedAgent, setSelectedAgent] = useState<AgentType>("claude");
  const [conversations, setConversations] = useState<ConversationSummary[]>([]);
  const [selectedConversation, setSelectedConversation] = useState<Conversation | null>(null);
  const [showMigrateModal, setShowMigrateModal] = useState(false);
  const [showSettings, setShowSettings] = useState(false);
  const [showTrash, setShowTrash] = useState(false);
  const [showAbout, setShowAbout] = useState(false);
  const [memoryDrawerOpen, setMemoryDrawerOpen] = useState(false);
  const [memoryDrawerTab, setMemoryDrawerTab] = useState<MemoryDrawerTab>("inbox");
  const [listLoading, setListLoading] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);
  const [deletingConversationId, setDeletingConversationId] = useState<string | null>(null);
  const [bulkSelectionMode, setBulkSelectionMode] = useState(false);
  const [selectedConversationKeys, setSelectedConversationKeys] = useState<string[]>([]);
  const [bulkDeleting, setBulkDeleting] = useState(false);
  const [trashConfirm, setTrashConfirm] = useState<TrashConfirmState>(null);
  const [emptyTrashConfirm, setEmptyTrashConfirm] = useState<EmptyTrashConfirmState>(null);
  const [trashedConversations, setTrashedConversations] = useState<TrashedConversation[]>([]);
  const [trashLoading, setTrashLoading] = useState(false);
  const [restoringTrashId, setRestoringTrashId] = useState<string | null>(null);
  const [appNotice, setAppNotice] = useState<AppNotice>(null);
  const [memoryLoading, setMemoryLoading] = useState(false);
  const [repoMemoryHealth, setRepoMemoryHealth] = useState<RepoMemoryHealth | null>(null);
  const [lastLocalHistoryImportReport, setLastLocalHistoryImportReport] =
    useState<LocalHistoryImportReport | null>(null);
  const [repoHealthLoading, setRepoHealthLoading] = useState(false);
  const [repoScanRunning, setRepoScanRunning] = useState(false);
  const [mergingAliasRoot, setMergingAliasRoot] = useState<string | null>(null);
  const [bootstrapReadyConversationId, setBootstrapReadyConversationId] = useState<string | null>(
    null,
  );
  const [pendingApprovedMemoryAutofocusConversationId, setPendingApprovedMemoryAutofocusConversationId] =
    useState<string | null>(null);
  const [searchQuery, setSearchQuery] = useState("");
  const [copyState, setCopyState] = useState<CopyState>({ target: null, status: "idle" });
  const [appSettings, setAppSettings] = useState<AppSettings>(() => loadSettings());
  const [updateState, setUpdateState] = useState<UpdateState>({ kind: "idle" });
  const [, setActivePage] = useState<TopPage>("continue");
  const [historyView, setHistoryView] = useState<HistoryView>("conversations");
  const [helpQuery, setHelpQuery] = useState("");
  const [advancedHelpOpen, setAdvancedHelpOpen] = useState(false);
  const [repoMemories, setRepoMemories] = useState<ApprovedMemory[]>([]);
  const [memoryCandidates, setMemoryCandidates] = useState<MemoryCandidate[]>([]);
  const [wikiPages, setWikiPages] = useState<WikiPage[]>([]);
  const [selectedWikiPageId, setSelectedWikiPageId] = useState<string | null>(null);
  const [episodes] = useState<EpisodeRecord[]>([]);
  const [runs] = useState<RunRecord[]>([]);
  const [artifacts] = useState<ArtifactRecord[]>([]);
  const [checkpoints, setCheckpoints] = useState<CheckpointRecord[]>([]);
  const [handoffs, setHandoffs] = useState<HandoffPacket[]>([]);
  const [handoffComposer, setHandoffComposer] = useState<HandoffComposerState>(null);
  const [showOrganizeMenu, setShowOrganizeMenu] = useState(false);
  const [libraryArrangement, setLibraryArrangement] = useState<LibraryArrangement>("projects");
  const [librarySort, setLibrarySort] = useState<LibrarySort>("updated");
  const [workspaceView, setWorkspaceView] = useState<WorkspaceView>("conversation");
  const [projectFilters, setProjectFilters] = useState<string[]>([]);
  const [expandedProjects, setExpandedProjects] = useState<Record<string, boolean>>({});
  const [collapsedSnapshot, setCollapsedSnapshot] = useState<Record<string, boolean> | null>(null);
  const [isWindowFilled, setIsWindowFilled] = useState(false);
  const organizeMenuRef = useRef<HTMLDivElement | null>(null);
  const activeConversationId = selectedConversation?.id ?? null;
  const activeConversationIdRef = useRef<string | null>(activeConversationId);
  const activeRepoRoot = selectedConversation
    ? getConversationProjectDir(selectedConversation) || null
    : null;
  const activeRepoRootRef = useRef<string | null>(activeRepoRoot);
  const repoScanRequestIdRef = useRef(0);
  const repoScanActiveCountRef = useRef(0);
  const conversationDetailCacheRef = useRef<Record<string, Conversation>>({});
  const conversationDetailRequestIdRef = useRef(0);
  const autoBootstrapAttemptedReposRef = useRef<Record<string, true>>({});
  const globalHistoryImportAttemptedRef = useRef(false);
  const selectedFontOption =
    APP_FONT_OPTIONS.find((option) => option.id === appSettings.fontFamily) ?? APP_FONT_OPTIONS[0];
  const appShellStyle = {
    "--font-sans": selectedFontOption.cssFamily,
    fontFamily: selectedFontOption.cssFamily,
  } as CSSProperties & Record<"--font-sans", string>;
  const availableHandoffTargets = ["claude", "codex", "gemini", "opencode"].filter(
    (agent) => agent !== selectedAgent,
  );

  const syncNativeWindowState = useCallback(async () => {
    try {
      const [isMaximized, isFullscreen] = await Promise.all([
        appWindow.isMaximized(),
        appWindow.isFullscreen(),
      ]);
      setIsWindowFilled(isMaximized || isFullscreen);
    } catch {
      setIsWindowFilled(false);
    }
  }, []);

  useEffect(() => {
    let isDisposed = false;
    let unlistenResize: (() => void) | null = null;

    const syncIfMounted = async () => {
      if (!isDisposed) {
        await syncNativeWindowState();
      }
    };

    void syncIfMounted();
    const onResized =
      typeof appWindow.onResized === "function" ? appWindow.onResized.bind(appWindow) : null;

    if (onResized) {
      void onResized(() => {
        void syncIfMounted();
      })
        .then((unlisten) => {
          if (isDisposed) {
            unlisten();
            return;
          }
          unlistenResize = unlisten;
        })
        .catch(() => {
          // Browser tests and web previews can run without a native Tauri window.
        });
    }

    return () => {
      isDisposed = true;
      unlistenResize?.();
    };
  }, [syncNativeWindowState]);

  useEffect(() => {
    setSelectedConversation(null);
    setShowSettings(false);
    setShowTrash(false);
    setShowAbout(false);
    setCopyState({ target: null, status: "idle" });
    setBulkSelectionMode(false);
    setSelectedConversationKeys([]);
    setActivePage("continue");
    setHistoryView("conversations");
  }, [selectedAgent]);

  useEffect(() => {
    if (!appNotice) {
      return;
    }
    const timer = window.setTimeout(() => setAppNotice(null), 2600);
    return () => window.clearTimeout(timer);
  }, [appNotice]);

  useEffect(() => {
    void loadConversations(searchQuery, selectedAgent);
  }, [searchQuery, selectedAgent]);

  useEffect(() => {
    const availableKeys = new Set(conversations.map(getConversationKey));
    setSelectedConversationKeys((current) => current.filter((key) => availableKeys.has(key)));
  }, [conversations]);

  useEffect(() => {
    setCopyState({ target: null, status: "idle" });
    setBootstrapReadyConversationId((current) =>
      current === activeConversationId ? current : null,
    );
    setPendingApprovedMemoryAutofocusConversationId((current) =>
      current === activeConversationId ? current : null,
    );
  }, [activeConversationId]);

  useEffect(() => {
    activeConversationIdRef.current = activeConversationId;
  }, [activeConversationId]);

  useEffect(() => {
    activeRepoRootRef.current = activeRepoRoot;
    if (!activeRepoRoot) {
      repoScanRequestIdRef.current += 1;
      repoScanActiveCountRef.current = 0;
      setRepoScanRunning(false);
      setMergingAliasRoot(null);
      setWorkspaceView("conversation");
    }
  }, [activeRepoRoot]);

  const runRepoScan = useCallback(
    async (
      requestRepoRoot: string,
      options?: {
        announceBootstrapReady?: boolean;
        forceGlobalImport?: boolean;
        includeGlobalImport?: boolean;
        requestConversationId?: string | null;
      },
    ) => {
    const requestId = ++repoScanRequestIdRef.current;
    repoScanActiveCountRef.current += 1;
    setRepoScanRunning(true);
    try {
      const shouldImportGlobalHistory =
        options?.includeGlobalImport !== false &&
        (options?.forceGlobalImport === true || !globalHistoryImportAttemptedRef.current);
      if (shouldImportGlobalHistory) {
        globalHistoryImportAttemptedRef.current = true;
        try {
          const report = await importAllLocalHistory();
          setLastLocalHistoryImportReport(report);
        } catch (error) {
          console.error("Failed to import all local history:", error);
        }
      }
      await scanRepoConversations(requestRepoRoot);
      const nextHealth = await getRepoMemoryHealth(requestRepoRoot);
      if (
        activeRepoRootRef.current === requestRepoRoot &&
        requestId === repoScanRequestIdRef.current
      ) {
        setRepoMemoryHealth(nextHealth);
        if (
          options?.announceBootstrapReady === true &&
          nextHealth.indexed_chunk_count > 0 &&
          activeConversationIdRef.current
        ) {
          const readyConversationId = activeConversationIdRef.current;
          setBootstrapReadyConversationId(readyConversationId);
          setPendingApprovedMemoryAutofocusConversationId(readyConversationId);
        }
      }
      return nextHealth;
    } catch (error) {
      console.error("Failed to scan repo conversations:", error);
      return null;
    } finally {
      repoScanActiveCountRef.current = Math.max(0, repoScanActiveCountRef.current - 1);
      setRepoScanRunning(repoScanActiveCountRef.current > 0);
    }
    },
    [],
  );

  const closeMemoryDrawer = useCallback(() => {
    setMemoryDrawerOpen(false);
    setPendingApprovedMemoryAutofocusConversationId((current) =>
      current === activeConversationIdRef.current ? null : current,
    );
  }, []);

  const handleMemoryDrawerTabChange = useCallback((nextTab: MemoryDrawerTab) => {
    setMemoryDrawerTab(nextTab);
    if (nextTab === "inbox") {
      setPendingApprovedMemoryAutofocusConversationId((current) =>
        current === activeConversationIdRef.current ? null : current,
      );
    }
  }, []);

  useEffect(() => {
    let cancelled = false;

    void loadNativeSettings().then((nativeSettings) => {
      if (cancelled) {
        return;
      }

      if (nativeSettings) {
        saveSettings(nativeSettings);
        setAppSettings(nativeSettings);
        if (nativeSettings.locale !== locale) {
          setLocale(nativeSettings.locale);
        }
      } else {
        void saveNativeSettings(appSettings);
      }
    });

    return () => {
      cancelled = true;
    };
  }, []);

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
    if (!showOrganizeMenu) {
      return;
    }

    const handlePointerDown = (event: MouseEvent) => {
      if (!organizeMenuRef.current?.contains(event.target as Node)) {
        setShowOrganizeMenu(false);
      }
    };

    document.addEventListener("mousedown", handlePointerDown);
    return () => document.removeEventListener("mousedown", handlePointerDown);
  }, [showOrganizeMenu]);

  useEffect(() => {
    if (!activeRepoRoot) {
      setRepoMemories([]);
      setMemoryCandidates([]);
      setWikiPages([]);
      setSelectedWikiPageId(null);
      setCheckpoints([]);
      setHandoffs([]);
      setRepoMemoryHealth(null);
      setRepoHealthLoading(false);
      setMemoryDrawerOpen(false);
      setPendingApprovedMemoryAutofocusConversationId(null);
      return;
    }

    let cancelled = false;

    const loadProjectMemory = async () => {
      setMemoryLoading(true);
      setRepoHealthLoading(true);
      const requestRepoRoot = activeRepoRoot;
      const requestConversationId = activeConversationIdRef.current;
      try {
        const [nextMemories, nextCandidates, nextWikiPages, nextCheckpoints, nextHandoffs] = await Promise.all([
          listRepoMemories(activeRepoRoot),
          listMemoryCandidates(activeRepoRoot, "pending_review"),
          listWikiPages(activeRepoRoot),
          listCheckpoints(activeRepoRoot),
          listHandoffs(activeRepoRoot),
        ]);
        if (cancelled || activeRepoRootRef.current !== requestRepoRoot) {
          return;
        }
        setRepoMemories(nextMemories);
        setMemoryCandidates(nextCandidates);
        setWikiPages(nextWikiPages);
        setCheckpoints(nextCheckpoints);
        setHandoffs(nextHandoffs);
      } catch (error) {
        console.error("Failed to load project memory:", error);
      } finally {
        if (!cancelled) {
          setMemoryLoading(false);
        }
      }

      try {
        const nextHealth = await getRepoMemoryHealth(requestRepoRoot);
        if (cancelled || activeRepoRootRef.current !== requestRepoRoot) {
          return;
        }
        setRepoMemoryHealth(nextHealth);
        const bootstrapKey = nextHealth.canonical_repo_root || requestRepoRoot;
        if (
          nextHealth.indexed_chunk_count === 0 &&
          autoBootstrapAttemptedReposRef.current[bootstrapKey] !== true
        ) {
          autoBootstrapAttemptedReposRef.current[bootstrapKey] = true;
          void runRepoScan(requestRepoRoot, {
            announceBootstrapReady: true,
            requestConversationId,
          });
        }
      } catch (error) {
        console.error("Failed to load repo memory health:", error);
      } finally {
        if (!cancelled) {
          setRepoHealthLoading(false);
        }
      }
    };

    void loadProjectMemory();

    return () => {
      cancelled = true;
    };
  }, [activeRepoRoot, runRepoScan]);

  const handleScanRepoConversations = async () => {
    if (!activeRepoRoot) {
      return;
    }
    await runRepoScan(activeRepoRoot, { forceGlobalImport: true });
  };

  const handleMergeRepoAlias = useCallback(
    async (aliasRoot: string) => {
      const repoRoot = activeRepoRootRef.current;
      if (!repoRoot) {
        return;
      }

      setMergingAliasRoot(aliasRoot);
      try {
        await mergeRepoAlias(repoRoot, aliasRoot);
        await runRepoScan(repoRoot, {
          announceBootstrapReady: true,
          includeGlobalImport: false,
          requestConversationId: activeConversationIdRef.current,
        });
      } catch (error) {
        console.error("Failed to merge repo alias:", error);
      } finally {
        setMergingAliasRoot(null);
      }
    },
    [runRepoScan],
  );

  const loadConversations = async (query = searchQuery, agent = selectedAgent) => {
    setListLoading(true);
    try {
      const trimmedQuery = query.trim();
      const result = trimmedQuery
        ? await invoke<ConversationSummary[]>("search_conversations", {
            agent,
            query: trimmedQuery,
          })
        : await invoke<ConversationSummary[]>("list_conversations", { agent });
      setConversations(result.map(normalizeConversationProject));
    } catch (error) {
      console.error("Failed to load conversations:", error);
    } finally {
      setListLoading(false);
    }
  };

  const loadConversationDetail = async (
    id: string,
    agent = selectedAgent,
    options: { throwOnError?: boolean } = {},
  ) => {
    const requestId = ++conversationDetailRequestIdRef.current;
    const cacheKey = `${agent}:${id}`;
    if (id !== activeConversationIdRef.current) {
      setBootstrapReadyConversationId(null);
    }
    setShowSettings(false);
    setShowTrash(false);
    setShowAbout(false);
    const cachedConversation = conversationDetailCacheRef.current[cacheKey];
    if (cachedConversation) {
      setSelectedConversation(cachedConversation);
      setDetailLoading(false);
      return true;
    }
    setDetailLoading(true);
    try {
      const result = await invoke<Conversation>("read_conversation", {
        agent,
        id,
      });
      if (requestId !== conversationDetailRequestIdRef.current) {
        return false;
      }
      const normalizedConversation = normalizeConversationProject(result);
      conversationDetailCacheRef.current[cacheKey] = normalizedConversation;
      setSelectedConversation(normalizedConversation);
      return true;
    } catch (error) {
      console.error("Failed to load conversation:", error);
      if (options.throwOnError) {
        throw error;
      }
      return false;
    } finally {
      if (requestId === conversationDetailRequestIdRef.current) {
        setDetailLoading(false);
      }
    }
  };

  const loadTrashConversations = async () => {
    setTrashLoading(true);
    try {
      const result = await invoke<TrashedConversation[]>("list_trashed_conversations");
      setTrashedConversations(result);
    } catch (error) {
      console.error("Failed to load Trash:", error);
      setAppNotice({ kind: "error", message: shell.trashLoadFailed });
    } finally {
      setTrashLoading(false);
    }
  };

  useEffect(() => {
    if (showTrash) {
      void loadTrashConversations();
    }
  }, [showTrash]);

  useEffect(() => {
    void loadTrashConversations();
  }, []);

  const handleMigrate = async (targetAgent: AgentType, mode: MigrateMode) => {
    if (!selectedConversation) {
      return;
    }

    const sourceAgent = selectedAgent;
    const sourceConversation = selectedConversation;
    setDetailLoading(true);
    try {
      const result = await invoke<MigrationResult>("migrate_conversation", {
        source: sourceAgent,
        target: targetAgent,
        id: sourceConversation.id,
        mode,
      });
      setSearchQuery("");
      setSelectedAgent(targetAgent);
      await loadConversations("", targetAgent);
      await loadConversationDetail(result.newId, targetAgent, { throwOnError: true });
      setShowMigrateModal(false);
      const modeText = mode === "copy" ? "复制" : "移动";
      const verificationText =
        result.warnings.length > 0
          ? `，但有 ${result.warnings.length} 条需要检查：${result.warnings[0]}`
          : "，已通过读回和列表验证";
      setAppNotice({
        kind: "success",
        message: `对话已${modeText}到 ${targetAgent}${verificationText}。`,
      });
    } catch (error) {
      console.error("Failed to migrate conversation:", error);
      setSelectedAgent(sourceAgent);
      setSelectedConversation(sourceConversation);
      await loadConversations("", sourceAgent);
      const message = readableError(error);
      setAppNotice({
        kind: "error",
        message: `对话迁移失败：${message}`,
      });
      alert(`对话迁移失败：${message}`);
    } finally {
      setDetailLoading(false);
    }
  };

  const moveConversationsToTrash = (
    targets: Array<
      Pick<ConversationSummary, "id" | "source_agent" | "summary"> |
        Pick<Conversation, "id" | "source_agent" | "summary">
    >,
  ) => {
    if (targets.length === 0) {
      return;
    }

    setTrashConfirm({
      targets: targets.map((target) => ({
        id: target.id,
        source_agent: target.source_agent || selectedAgent,
        summary: target.summary ?? null,
      })),
      deleteRemoteBackup: false,
      busy: false,
      error: null,
    });
  };

  const confirmMoveConversationsToTrash = async () => {
    if (!trashConfirm || trashConfirm.targets.length === 0) {
      return;
    }

    const targets = trashConfirm.targets;
    const isBulk = targets.length > 1;
    const deletingSelectedConversation = selectedConversation
      ? targets.some((conversation) => conversation.id === selectedConversation.id)
      : false;
    const syncSettings = appSettings.sync;
    const shouldDeleteRemote =
      trashConfirm.deleteRemoteBackup &&
      syncSettings.provider === "webdav" &&
      syncSettings.webdavHost.trim().length > 0 &&
      syncSettings.username.trim().length > 0;
    let webdavPassword: string | null = null;

    if (shouldDeleteRemote) {
      webdavPassword = await loadWebDavPassword(syncSettings.username);
      if (!webdavPassword) {
        setTrashConfirm((current) =>
          current
            ? { ...current, busy: false, error: shell.confirmTrashRemotePasswordMissing }
            : current,
        );
        return;
      }
    }

    if (isBulk) {
      setBulkDeleting(true);
    } else {
      setDeletingConversationId(targets[0].id);
    }
    if (deletingSelectedConversation) {
      setDetailLoading(true);
    }
    setTrashConfirm((current) => (current ? { ...current, busy: true, error: null } : current));
    try {
      for (const conversation of targets) {
        await invoke("trash_conversation", {
          agent: conversation.source_agent || selectedAgent,
          id: conversation.id,
          retentionDays: appSettings.trashRetentionDays,
          deleteRemoteBackup: shouldDeleteRemote,
          webdavScheme: syncSettings.webdavScheme,
          webdavHost: syncSettings.webdavHost,
          webdavPath: syncSettings.webdavPath,
          remotePath: syncSettings.remotePath,
          username: syncSettings.username,
          password: webdavPassword ?? "",
        });
      }
      setAppNotice({
        kind: "success",
        message: isBulk ? shell.trashSuccessBulk(targets.length) : shell.trashSuccessSingle,
      });
      if (deletingSelectedConversation) {
        setSelectedConversation(null);
      }
      setSelectedConversationKeys([]);
      setBulkSelectionMode(false);
      setTrashConfirm(null);
      await loadConversations();
      if (showTrash) {
        await loadTrashConversations();
      }
    } catch (error) {
      console.error("Failed to move conversation to Trash:", error);
      setTrashConfirm((current) =>
        current
          ? {
              ...current,
              busy: false,
              error: `${shell.trashFailed}: ${String(error)}`,
            }
          : current,
      );
    } finally {
      setDeletingConversationId(null);
      setBulkDeleting(false);
      if (deletingSelectedConversation) {
        setDetailLoading(false);
      }
    }
  };

  const handleDeleteConversation = async (
    conversation: Pick<ConversationSummary, "id" | "source_agent" | "summary"> | Pick<Conversation, "id" | "source_agent" | "summary"> | null =
      selectedConversation,
  ) => {
    if (!conversation) {
      return;
    }

    await moveConversationsToTrash([conversation]);
  };

  const handleDelete = async () => {
    await handleDeleteConversation();
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

  const handleVerifyWebDavServer = async ({
    syncSettings,
    password,
  }: WebDavVerificationInput) => {
    await invoke("verify_webdav_server", {
      webdavScheme: syncSettings.webdavScheme,
      webdavHost: syncSettings.webdavHost,
      webdavPath: syncSettings.webdavPath,
      remotePath: syncSettings.remotePath,
      username: syncSettings.username,
      password,
    });
  };

  const handleSyncWebDavNow = async ({
    syncSettings,
    password,
  }: WebDavVerificationInput): Promise<WebDavSyncResult> => {
    return invoke<WebDavSyncResult>("sync_webdav_now", {
      webdavScheme: syncSettings.webdavScheme,
      webdavHost: syncSettings.webdavHost,
      webdavPath: syncSettings.webdavPath,
      remotePath: syncSettings.remotePath,
      username: syncSettings.username,
      password,
    });
  };

  const handleRunUpgradeReadinessCheck = async (): Promise<UpgradeReadinessReport> => {
    return invoke<UpgradeReadinessReport>("run_upgrade_readiness_check");
  };

  const handleDetectAgentIntegrations = async (): Promise<AgentIntegrationStatus[]> => {
    return invoke<AgentIntegrationStatus[]>("detect_agent_integrations");
  };

  const handleInstallAgentIntegration = async (
    agent: string,
  ): Promise<AgentIntegrationOperationResult[]> => {
    return invoke<AgentIntegrationOperationResult[]>("install_agent_integration", { agent });
  };

  const handleUninstallAgentIntegration = async (
    agent: string,
  ): Promise<AgentIntegrationOperationResult[]> => {
    return invoke<AgentIntegrationOperationResult[]>("uninstall_agent_integration", { agent });
  };

  const handleApproveCandidate = async (
    candidate: MemoryCandidate,
    reviewDraft?: MemoryCandidateApprovalDraft,
  ) => {
    if (!activeRepoRoot) {
      return;
    }

    const editedTitle = reviewDraft?.title ?? candidate.summary;
    const editedValue = reviewDraft?.value ?? candidate.value;
    const editedUsageHint = reviewDraft?.usageHint ?? candidate.why_it_matters;
    const hasEditedDraft = Boolean(
      reviewDraft &&
        (editedTitle !== candidate.summary ||
          editedValue !== candidate.value ||
          editedUsageHint !== candidate.why_it_matters),
    );

    setMemoryLoading(true);
    try {
      await reviewMemoryCandidate({
        candidateId: candidate.candidate_id,
        action: hasEditedDraft ? "approve_with_edit" : "approve",
        editedTitle,
        ...(hasEditedDraft ? { editedValue } : {}),
        editedUsageHint,
      });
      const [nextCandidates, nextMemories, nextWikiPages] = await Promise.all([
        listMemoryCandidates(activeRepoRoot, "pending_review"),
        listRepoMemories(activeRepoRoot),
        rebuildRepoWiki(activeRepoRoot),
      ]);
      setMemoryCandidates(nextCandidates);
      setRepoMemories(nextMemories);
      setWikiPages(nextWikiPages);
    } catch (error) {
      console.error("Failed to approve memory candidate:", error);
    } finally {
      setMemoryLoading(false);
    }
  };

  const handleApproveMergeCandidate = async (candidate: MemoryCandidate) => {
    if (!activeRepoRoot || !candidate.merge_suggestion?.proposed_value) {
      return;
    }

    setMemoryLoading(true);
    try {
      await reviewMemoryCandidate({
        candidateId: candidate.candidate_id,
        action: "approve_merge",
        mergeMemoryId: candidate.merge_suggestion.memory_id,
        editedTitle: candidate.merge_suggestion.proposed_title ?? candidate.merge_suggestion.memory_title,
        editedValue: candidate.merge_suggestion.proposed_value,
        editedUsageHint: candidate.merge_suggestion.proposed_usage_hint ?? candidate.why_it_matters,
      });
      const [nextCandidates, nextMemories, nextWikiPages] = await Promise.all([
        listMemoryCandidates(activeRepoRoot, "pending_review"),
        listRepoMemories(activeRepoRoot),
        rebuildRepoWiki(activeRepoRoot),
      ]);
      setMemoryCandidates(nextCandidates);
      setRepoMemories(nextMemories);
      setWikiPages(nextWikiPages);
    } catch (error) {
      console.error("Failed to approve memory merge:", error);
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

  const handleSnoozeCandidate = async (candidateId: string) => {
    if (!activeRepoRoot) {
      return;
    }

    setMemoryLoading(true);
    try {
      await reviewMemoryCandidate({
        candidateId,
        action: "snooze",
      });
      setMemoryCandidates(await listMemoryCandidates(activeRepoRoot, "pending_review"));
    } catch (error) {
      console.error("Failed to snooze memory candidate:", error);
    } finally {
      setMemoryLoading(false);
    }
  };

  const handleCreateHandoff = (targetAgent: string) => {
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
      setActivePage("history");
      setHistoryView("recovery");
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
      setActivePage("history");
      setHistoryView("transfers");
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
      const [nextMemories, nextWikiPages, nextHealth] = await Promise.all([
        listRepoMemories(activeRepoRoot),
        rebuildRepoWiki(activeRepoRoot),
        getRepoMemoryHealth(activeRepoRoot),
      ]);
      setRepoMemories(nextMemories);
      setWikiPages(nextWikiPages);
      setRepoMemoryHealth(nextHealth);
    } catch (error) {
      console.error("Failed to re-verify memory:", error);
    } finally {
      setMemoryLoading(false);
    }
  };

  const handleRetireMemory = async (memoryId: string) => {
    if (!activeRepoRoot) {
      return;
    }

    setMemoryLoading(true);
    try {
      await retireMemory({
        memoryId,
        retiredBy: selectedAgent,
      });
      const [nextMemories, nextCandidates, nextWikiPages, nextHealth] = await Promise.all([
        listRepoMemories(activeRepoRoot),
        listMemoryCandidates(activeRepoRoot, "pending_review"),
        rebuildRepoWiki(activeRepoRoot),
        getRepoMemoryHealth(activeRepoRoot),
      ]);
      setRepoMemories(nextMemories);
      setMemoryCandidates(nextCandidates);
      setWikiPages(nextWikiPages);
      setRepoMemoryHealth(nextHealth);
    } catch (error) {
      console.error("Failed to retire startup rule:", error);
    } finally {
      setMemoryLoading(false);
    }
  };

  const handleRetireManyMemories = async (memoryIds: string[]) => {
    if (!activeRepoRoot || memoryIds.length === 0) {
      return;
    }

    setMemoryLoading(true);
    try {
      await Promise.all(
        memoryIds.map((memoryId) =>
          retireMemory({
            memoryId,
            retiredBy: selectedAgent,
          }),
        ),
      );
      const [nextMemories, nextCandidates, nextHealth] = await Promise.all([
        listRepoMemories(activeRepoRoot),
        listMemoryCandidates(activeRepoRoot, "pending_review"),
        getRepoMemoryHealth(activeRepoRoot),
      ]);
      setRepoMemories(nextMemories);
      setMemoryCandidates(nextCandidates);
      setRepoMemoryHealth(nextHealth);
    } catch (error) {
      console.error("Failed to retire startup rules:", error);
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

  const sortedConversations = useMemo(
    () => sortConversations(conversations, librarySort),
    [conversations, librarySort],
  );

  const availableProjects = useMemo(
    () => {
      const projects = new Map<string, string>();
      sortedConversations.forEach((conversation) => {
        const projectDir = getConversationProjectDir(conversation);
        if (projectDir) {
          projects.set(projectPathKey(projectDir), projectDir);
        }
      });

      return Array.from(projects.values()).sort((left, right) => left.localeCompare(right));
    },
    [sortedConversations],
  );

  const filteredConversations = useMemo(() => {
    if (projectFilters.length === 0) {
      return sortedConversations;
    }

    const filterKeys = new Set(projectFilters.map(projectPathKey));
    return sortedConversations.filter((conversation) => {
      const projectDir = getConversationProjectDir(conversation);
      return projectDir ? filterKeys.has(projectPathKey(projectDir)) : false;
    });
  }, [projectFilters, sortedConversations]);

  const selectedConversationKeySet = useMemo(
    () => new Set(selectedConversationKeys),
    [selectedConversationKeys],
  );

  const selectedConversationsForBulkAction = useMemo(
    () =>
      filteredConversations.filter((conversation) =>
        selectedConversationKeySet.has(getConversationKey(conversation)),
      ),
    [filteredConversations, selectedConversationKeySet],
  );

  const selectedConversationCount = selectedConversationsForBulkAction.length;
  const allVisibleConversationsSelected =
    filteredConversations.length > 0 &&
    filteredConversations.every((conversation) =>
      selectedConversationKeySet.has(getConversationKey(conversation)),
    );

  const projectConversations = useMemo(
    () => filteredConversations.filter(isProjectConversation),
    [filteredConversations],
  );

  const chatConversations = useMemo(
    () => filteredConversations.filter((conversation) => !isProjectConversation(conversation)),
    [filteredConversations],
  );

  const repoLibraryRecords = useMemo(() => {
    if (!activeRepoRoot) {
      return [];
    }

    return buildRepoLibraryRecords({
      conversations: sortedConversations.filter((conversation) => {
        const projectDir = getConversationProjectDir(conversation);
        return projectDir ? projectPathKey(projectDir) === projectPathKey(activeRepoRoot) : false;
      }),
      memories: repoMemories,
      checkpoints,
      handoffs,
      runs,
      artifacts,
      episodes,
    });
  }, [
    activeRepoRoot,
    artifacts,
    checkpoints,
    episodes,
    handoffs,
    repoMemories,
    runs,
    sortedConversations,
  ]);

  const projectGroups = useMemo<ProjectGroup[]>(() => {
    const groups = new Map<string, ProjectGroup>();

    projectConversations.forEach((conversation) => {
      const projectDir = getConversationProjectDir(conversation);
      if (!projectDir) {
        return;
      }
      const groupKey = projectPathKey(projectDir);
      const normalizedConversation = normalizeConversationProject(conversation);
      const existing = groups.get(groupKey);
      if (existing) {
        existing.conversations.push(normalizedConversation);
        if (conversation.updated_at > existing.latestAt) {
          existing.latestAt = conversation.updated_at;
        }
        return;
      }

      groups.set(groupKey, {
        id: groupKey,
        label: getProjectLabel(projectDir),
        fullPath: projectDir,
        latestAt: conversation.updated_at,
        conversations: [normalizedConversation],
      });
    });

    return Array.from(groups.values()).sort((left, right) =>
      right.latestAt.localeCompare(left.latestAt),
    );
  }, [projectConversations]);

  useEffect(() => {
    setExpandedProjects((current) => {
      const next: Record<string, boolean> = {};

      projectGroups.forEach((group) => {
        next[group.id] = current[group.id] ?? true;
      });

      const currentKeys = Object.keys(current);
      const nextKeys = Object.keys(next);
      if (
        currentKeys.length === nextKeys.length &&
        nextKeys.every((key) => current[key] === next[key])
      ) {
        return current;
      }

      return next;
    });
  }, [projectGroups]);

  const allProjectsCollapsed =
    projectGroups.length > 0 && projectGroups.every((group) => expandedProjects[group.id] === false);
  const projectCollapseActionLabel = allProjectsCollapsed
    ? shell.restoreProjects
    : shell.collapseProjects;
  const activeFilterCount = projectFilters.length;

  const handleOpenLibraryRecord = async (record: LibraryRecord) => {
    if (record.destination === "review") {
      setActivePage("review");
    } else {
      setActivePage("history");

      if (record.destination === "history-conversations") {
        setHistoryView("conversations");
      } else if (record.destination === "history-recovery") {
        setHistoryView("recovery");
      } else if (record.destination === "history-transfers") {
        setHistoryView("transfers");
      } else {
        setHistoryView("outputs");
      }
    }

    if (record.conversationId && record.conversationId !== selectedConversation?.id) {
      await loadConversationDetail(record.conversationId);
    }
  };

  const locationButtonLabel =
    copyState.target === "location" && copyState.status === "success"
      ? shell.copyLocationSuccess
      : copyState.target === "location" && copyState.status === "error"
        ? shell.copyFailed
        : shell.copyLocation;
  const resumeButtonLabel =
    copyState.target === "resume" && copyState.status === "success"
      ? shell.copyResumeSuccess
      : copyState.target === "resume" && copyState.status === "error"
        ? shell.copyFailed
        : shell.copyResume;

  const helpCards = useMemo<HelpCard[]>(
    () => [
      {
        id: "continue",
        title: locale === "en" ? "Continue Previous Work" : "继续之前的工作",
        description:
          locale === "en" ? "Jump back to the latest recoverable progress." : "回到最近一次可恢复的进度。",
        buttonLabel: locale === "en" ? "View Progress" : "查看进度",
        answer:
          locale === "en"
            ? "Start from Continue Work. If a conversation is selected, you'll see its resume command and latest context in one place."
            : "先从“继续工作”开始。只要选中一段对话，你就能在同一页看到恢复命令和最近上下文。",
        onSelect: () => setActivePage("continue"),
      },
      {
        id: "switch-agent",
        title: locale === "en" ? "Switch Agent" : "切换代理",
        description:
          locale === "en"
            ? "Pass the current task to another agent without losing context."
            : "把当前任务移交给另一个代理，不丢上下文。",
        buttonLabel: locale === "en" ? "Start Transfer" : "开始移交",
        answer:
          locale === "en"
            ? "Transfers work best after you select a conversation. From Continue Work or History you can freeze context or create a handoff packet."
            : "先选中一段对话，再进行移交最顺手。你可以在“继续工作”或“历史”里冻结上下文，或者创建交接包。",
        onSelect: () => {
          setActivePage("continue");
          if (availableHandoffTargets[0]) {
            handleCreateHandoff(availableHandoffTargets[0]);
          }
        },
      },
      {
        id: "remembered",
        title: locale === "en" ? "Why wasn't this remembered?" : "为什么没有被记住？",
        description:
          locale === "en"
            ? "Some memory proposals need review before they become durable."
            : "有些记忆建议需要先经过你的确认，才会真正留下。",
        buttonLabel: locale === "en" ? "Open Review Queue" : "打开待确认",
        answer:
          locale === "en"
            ? "ChatMem keeps reviewable suggestions separate from durable project rules. The Needs Review page is where those decisions belong."
            : "ChatMem 会把“建议记住”与“已经成为规则”的内容分开。需要你判断的东西，都集中在“待确认”里。",
        onSelect: () => setActivePage("review"),
      },
      {
        id: "chatmem",
        title: locale === "en" ? "Why can't I find @chatmem?" : "为什么找不到 @chatmem?",
        description:
          locale === "en"
            ? "ChatMem often works through MCP and background flows rather than chat mentions."
            : "ChatMem 往往通过 MCP 和后台流程工作，而不是靠对话里 @ 出来。",
        buttonLabel: locale === "en" ? "See How It Works" : "查看工作方式",
        answer:
          locale === "en"
            ? "For agents, ChatMem is usually an MCP surface. The desktop app is the human recovery and review layer, not the main operating interface for agents."
            : "对 agent 来说，ChatMem 通常是一个 MCP 能力。桌面端更像是给人看的恢复与审批台，而不是 agent 的主操作界面。",
        onSelect: () => setAdvancedHelpOpen(true),
      },
      {
        id: "start",
        title: locale === "en" ? "Where should I start?" : "我应该先从哪里开始？",
        description:
          locale === "en" ? "Start with Continue Work unless you're reviewing." : "除非你在审批内容，否则先从“继续工作”开始。",
        buttonLabel: locale === "en" ? "Go to Continue Work" : "去继续工作",
        answer:
          locale === "en"
            ? "When in doubt, the fastest path is Continue Work. It gives you the current command, recent tasks, and next-step guidance."
            : "如果不确定，从“继续工作”开始最快。它会把恢复命令、最近任务和建议下一步放在一起。",
        onSelect: () => setActivePage("continue"),
      },
    ],
    [availableHandoffTargets, locale],
  );

  const visibleHelpCards = helpCards.filter((card) => {
    const query = helpQuery.trim().toLowerCase();
    if (!query) {
      return true;
    }

    return (
      card.title.toLowerCase().includes(query) ||
      card.description.toLowerCase().includes(query) ||
      card.answer.toLowerCase().includes(query)
    );
  });

  const recentTransfers = handoffs.slice(0, 3);
  const staleRules = repoMemories
    .filter((memory) => memory.freshness_status !== "fresh")
    .slice(0, 3);
  const pendingTransfers = handoffs
    .filter((handoff) => !handoff.consumed_at)
    .slice(0, 3);

  const toggleProjectFilter = (projectDir: string) => {
    setProjectFilters((current) =>
      current.includes(projectDir)
        ? current.filter((item) => item !== projectDir)
        : [...current, projectDir],
    );
  };

  const handleToggleCollapseProjects = () => {
    if (!allProjectsCollapsed) {
      const snapshot = projectGroups.reduce<Record<string, boolean>>((accumulator, group) => {
        accumulator[group.id] = expandedProjects[group.id] ?? true;
        return accumulator;
      }, {});
      setCollapsedSnapshot(snapshot);
      setExpandedProjects((current) =>
        projectGroups.reduce<Record<string, boolean>>((accumulator, group) => {
          accumulator[group.id] = false;
          return accumulator;
        }, { ...current }),
      );
      return;
    }

    const nextSnapshot =
      collapsedSnapshot ??
      projectGroups.reduce<Record<string, boolean>>((accumulator, group) => {
        accumulator[group.id] = true;
        return accumulator;
      }, {});
    setExpandedProjects((current) => ({ ...current, ...nextSnapshot }));
    setCollapsedSnapshot(null);
  };

  const handleToggleBulkSelectionMode = () => {
    setShowOrganizeMenu(false);
    setBulkSelectionMode((current) => {
      if (current) {
        setSelectedConversationKeys([]);
      }
      return !current;
    });
  };

  const handleToggleConversationSelection = (
    conversation: Pick<ConversationSummary, "id" | "source_agent">,
  ) => {
    const key = getConversationKey(conversation);
    setSelectedConversationKeys((current) =>
      current.includes(key)
        ? current.filter((item) => item !== key)
        : [...current, key],
    );
  };

  const handleSelectVisibleConversations = () => {
    const visibleKeys = filteredConversations.map(getConversationKey);
    setSelectedConversationKeys((current) => {
      const next = new Set(current);
      visibleKeys.forEach((key) => next.add(key));
      return Array.from(next);
    });
  };

  const handleClearConversationSelection = () => {
    setSelectedConversationKeys([]);
  };

  const handleBulkTrash = async () => {
    await moveConversationsToTrash(selectedConversationsForBulkAction);
  };

  const getConversationProjectDisplay = (
    conversation: Pick<ConversationSummary, "project_dir" | "source_agent">,
  ) => getConversationProjectDir(conversation) || (locale === "en" ? "No project" : "无项目");

  const handleEmptyTrashClick = () => {
    if (trashLoading || trashedConversations.length === 0) {
      return;
    }
    setEmptyTrashConfirm({ busy: false, error: null });
  };

  const confirmEmptyTrash = async () => {
    if (!emptyTrashConfirm) {
      return;
    }

    setEmptyTrashConfirm({ busy: true, error: null });
    try {
      const result = await invoke<EmptyTrashResponse>("empty_trash");
      setTrashedConversations([]);
      setEmptyTrashConfirm(null);
      await loadTrashConversations();
      setAppNotice({
        kind: "success",
        message: shell.emptyTrashSuccess(result.removedCount),
      });
    } catch (error) {
      console.error("Failed to empty Trash:", error);
      setEmptyTrashConfirm({
        busy: false,
        error: `${shell.emptyTrashFailed}: ${String(error)}`,
      });
    }
  };

  const handleRestoreTrashConversation = async (trashId: string) => {
    setRestoringTrashId(trashId);
    try {
      await invoke("restore_trashed_conversation", { trashId });
      setAppNotice({ kind: "success", message: shell.restoreSuccess });
      await loadTrashConversations();
      await loadConversations();
    } catch (error) {
      console.error("Failed to restore conversation:", error);
      setAppNotice({ kind: "error", message: `${shell.restoreFailed}: ${String(error)}` });
    } finally {
      setRestoringTrashId(null);
    }
  };

  const renderConversationRow = (
    conversation: ConversationSummary,
    extraClassName = "",
  ) => {
    const title = normalizeConversationTitle(conversation.summary) || conversation.id;
    const visibleTitle = truncateSidebarTitle(title);
    const isSelected = selectedConversation?.id === conversation.id;
    const conversationKey = getConversationKey(conversation);
    const isBulkSelected = selectedConversationKeySet.has(conversationKey);
    const projectDisplay = getConversationProjectDisplay(conversation);

    return (
      <div
        key={`${conversation.project_dir}-${conversation.id}`}
        className={`conversation-item ${isSelected ? "selected" : ""} ${
          bulkSelectionMode ? "selection-mode" : ""
        } ${isBulkSelected ? "is-bulk-selected" : ""} ${extraClassName}`.trim()}
      >
        {bulkSelectionMode ? (
          <label className="conversation-item-checkbox">
            <input
              type="checkbox"
              aria-label={`${shell.selectConversation} ${title}`}
              checked={isBulkSelected}
              onChange={() => handleToggleConversationSelection(conversation)}
            />
            <span aria-hidden="true"></span>
          </label>
        ) : null}
        <button
          type="button"
          className="conversation-item-select"
          onClick={() =>
            bulkSelectionMode
              ? handleToggleConversationSelection(conversation)
              : void loadConversationDetail(conversation.id)
          }
        >
          <div className="conversation-item-row">
            <div className="conversation-item-main">
              <div className="conversation-item-title" title={title}>
                {visibleTitle}
              </div>
              <div className="conversation-item-path" title={conversation.project_dir || projectDisplay}>
                {projectDisplay}
              </div>
            </div>
            <div className="conversation-item-time">{formatDistanceToNow(conversation.updated_at)}</div>
          </div>
        </button>
        {!bulkSelectionMode ? (
          <button
            type="button"
            className="conversation-item-delete"
            aria-label={`${shell.delete} ${title}`}
            title={shell.delete}
            disabled={deletingConversationId === conversation.id}
            onClick={(event) => {
              event.stopPropagation();
              void handleDeleteConversation(conversation);
            }}
          >
            {shell.delete}
          </button>
        ) : null}
      </div>
    );
  };

  const renderRecentTasks = () => {
    if (filteredConversations.length === 0) {
      return (
        <div className="inline-empty-state">
          <div className="inline-empty-title">{shell.noProgressTitle}</div>
          <div className="inline-empty-body">{shell.noProgressBody}</div>
        </div>
      );
    }

    return (
      <div className="task-list">
        {filteredConversations.slice(0, 5).map((conversation) => (
          <button
            key={`recent-${conversation.id}`}
            type="button"
            className="task-list-item"
            onClick={() => void loadConversationDetail(conversation.id)}
          >
            <div>
              <strong>{normalizeConversationTitle(conversation.summary) || conversation.id}</strong>
              <span>{getConversationProjectDisplay(conversation)}</span>
            </div>
            <span>{formatDistanceToNow(conversation.updated_at)}</span>
          </button>
        ))}
      </div>
    );
  };

  const renderContinuePage = () => {
    if (!selectedConversation) {
      return (
        <div className="page-layout page-layout-empty">
          <div className="empty-state empty-state-page quiet-empty-state">
            <div className="empty-state-icon brand-empty-icon" aria-hidden="true">
              <img src={brandIcon} alt="" />
            </div>
            <h1>{shell.chooseConversation}</h1>
            <div className="empty-state-text">{shell.noProgressBody}</div>
          </div>
        </div>
      );
    }

    return (
      <div className="page-layout">
      <header className="page-header">
        <div>
          <p className="page-eyebrow">{getAgentHeading(selectedAgent, locale)}</p>
          <h1>{shell.continueTitle}</h1>
        </div>
      </header>

      <div className="page-grid">
        <section className="task-panel task-panel-hero">
          <div className="task-panel-header">
            <div>
              <span className="task-panel-label">{shell.recoverableProgress}</span>
              <h2>
                {selectedConversation
                  ? normalizeConversationTitle(selectedConversation.summary) || selectedConversation.id
                  : shell.chooseConversation}
              </h2>
            </div>
          </div>
          <p className="task-panel-copy">
            {selectedConversation
              ? selectedConversation.summary || selectedConversation.id
              : shell.chooseConversationBody}
          </p>
          <div className="task-actions">
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => void handleCopy("resume", selectedConversation?.resume_command)}
              disabled={!selectedConversation?.resume_command}
            >
              {shell.resumeWork}
            </button>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => setActivePage("history")}
              disabled={!selectedConversation}
            >
              {shell.viewHistory}
            </button>
            {availableHandoffTargets.map((target) => (
              <button
                key={target}
                type="button"
                className="btn btn-secondary"
                onClick={() => handleCreateHandoff(target)}
                disabled={!selectedConversation}
              >
                {locale === "en" ? `Transfer to ${getAgentLabel(target)}` : `转给 ${getAgentLabel(target)}`}
              </button>
            ))}
          </div>
        </section>

        <section className="task-panel">
          <div className="task-panel-header">
            <div>
              <span className="task-panel-label">{shell.recentTasks}</span>
              <h2>{shell.recentTasks}</h2>
            </div>
          </div>
          {renderRecentTasks()}
        </section>

        <section className="task-panel">
          <div className="task-panel-header">
            <div>
              <span className="task-panel-label">{shell.fileLocation}</span>
              <h2>{shell.actionsLabel}</h2>
            </div>
          </div>
          <div className="meta-stack">
            <div className="meta-block">
              <span className="meta-label">{shell.fileLocation}</span>
              <span className={`meta-value ${selectedConversation?.storage_path ? "" : "is-muted"}`}>
                {selectedConversation?.storage_path || shell.noAvailablePath}
              </span>
            </div>
            <div className="meta-block">
              <span className="meta-label">{shell.resumeCommand}</span>
              <span className={`meta-value ${selectedConversation?.resume_command ? "" : "is-muted"}`}>
                {selectedConversation?.resume_command || "--"}
              </span>
            </div>
          </div>
          <div className="task-actions">
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => void handleCopy("location", selectedConversation?.storage_path)}
              disabled={!selectedConversation?.storage_path}
            >
              {locationButtonLabel}
            </button>
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => void handleCopy("resume", selectedConversation?.resume_command)}
              disabled={!selectedConversation?.resume_command}
            >
              {resumeButtonLabel}
            </button>
          </div>
        </section>

        <section className="task-panel">
          <div className="task-panel-header">
            <div>
              <span className="task-panel-label">{shell.nextStep}</span>
              <h2>{shell.nextStep}</h2>
            </div>
          </div>
          <p className="task-panel-copy">
            {selectedConversation
              ? locale === "en"
                ? "Start by restoring the current command or opening History for deeper records."
                : "先恢复当前命令，或者打开“历史”查看更完整的记录。"
              : shell.noProgressBody}
          </p>
        </section>

        <section className="task-panel">
          <div className="task-panel-header">
            <div>
              <span className="task-panel-label">{shell.recentTransfers}</span>
              <h2>{shell.recentTransfers}</h2>
            </div>
          </div>
          {memoryLoading ? (
            <div className="loading-inline">
              <div className="spinner"></div>
            </div>
          ) : recentTransfers.length === 0 ? (
            <div className="inline-empty-state">
              <div className="inline-empty-body">
                {locale === "en" ? "No recent transfer packets yet." : "还没有最近移交记录。"}
              </div>
            </div>
          ) : (
            <div className="task-list">
              {recentTransfers.map((handoff) => (
                <div key={handoff.handoff_id} className="task-list-card">
                  <strong>{handoff.current_goal}</strong>
                  <span>
                    {getAgentLabel(handoff.from_agent)}
                    {" -> "}
                    {getAgentLabel(handoff.to_agent)}
                  </span>
                </div>
              ))}
            </div>
          )}
        </section>
      </div>
      </div>
    );
  };

  const renderReviewPage = () => (
    <div className="page-layout">
      <header className="page-header">
        <div>
          <p className="page-eyebrow">{shell.nav.review}</p>
          <h1>{shell.reviewTitle}</h1>
          <p>{shell.reviewSubtitle}</p>
        </div>
      </header>

      {!selectedConversation ? (
        <div className="empty-state empty-state-page">
          <div className="empty-state-icon">?</div>
          <div className="empty-state-text">{shell.chooseConversationBody}</div>
        </div>
      ) : (
        <div className="review-grid">
          <section className="task-panel">
            <div className="task-panel-header">
              <div>
                <span className="task-panel-label">{shell.suggestedConclusions}</span>
                <h2>{shell.suggestedConclusions}</h2>
              </div>
            </div>
            {memoryLoading ? (
              <div className="loading-inline">
                <div className="spinner"></div>
              </div>
            ) : memoryCandidates.length === 0 ? (
              <div className="inline-empty-state">
                <div className="inline-empty-title">{shell.nothingToReview}</div>
                <div className="inline-empty-body">{shell.nothingToReviewBody}</div>
              </div>
            ) : (
              <div className="review-card-list">
                {memoryCandidates.slice(0, 4).map((candidate) => (
                  <article key={candidate.candidate_id} className="review-card">
                    <strong>{candidate.summary}</strong>
                    <p>{candidate.why_it_matters}</p>
                    <div className="task-actions">
                      <button
                        type="button"
                        className="btn btn-primary"
                        onClick={() => void handleApproveCandidate(candidate)}
                      >
                        {shell.confirmKeep}
                      </button>
                      <button
                        type="button"
                        className="btn btn-secondary"
                        onClick={() => void handleSnoozeCandidate(candidate.candidate_id)}
                      >
                        {shell.reviewLater}
                      </button>
                      <button
                        type="button"
                        className="btn btn-secondary"
                        onClick={() => void handleRejectCandidate(candidate.candidate_id)}
                      >
                        {shell.rejectKeep}
                      </button>
                    </div>
                  </article>
                ))}
              </div>
            )}
          </section>

          <section className="task-panel">
            <div className="task-panel-header">
              <div>
                <span className="task-panel-label">{shell.projectRules}</span>
                <h2>{shell.projectRules}</h2>
              </div>
            </div>
            {memoryLoading ? (
              <div className="loading-inline">
                <div className="spinner"></div>
              </div>
            ) : staleRules.length === 0 ? (
              <div className="inline-empty-state">
                <div className="inline-empty-body">
                  {locale === "en" ? "No project rules need re-verification." : "暂时没有需要重新核验的项目规则。"}
                </div>
              </div>
            ) : (
              <div className="review-card-list">
                {staleRules.map((memory) => (
                  <article key={memory.memory_id} className="review-card">
                    <strong>{memory.title}</strong>
                    <p>{memory.usage_hint}</p>
                    <div className="task-actions">
                      <button
                        type="button"
                        className="btn btn-secondary"
                        onClick={() => void handleReverifyMemory(memory.memory_id)}
                      >
                        {shell.reverifyRule}
                      </button>
                    </div>
                  </article>
                ))}
              </div>
            )}
          </section>

          <section className="task-panel">
            <div className="task-panel-header">
              <div>
                <span className="task-panel-label">{shell.pendingTransfers}</span>
                <h2>{shell.pendingTransfers}</h2>
              </div>
            </div>
            {memoryLoading ? (
              <div className="loading-inline">
                <div className="spinner"></div>
              </div>
            ) : pendingTransfers.length === 0 ? (
              <div className="inline-empty-state">
                <div className="inline-empty-body">
                  {locale === "en" ? "No transfer summaries are waiting." : "暂时没有等待确认的移交摘要。"}
                </div>
              </div>
            ) : (
              <div className="review-card-list">
                {pendingTransfers.map((handoff) => (
                  <article key={handoff.handoff_id} className="review-card">
                    <strong>{handoff.current_goal}</strong>
                    <p>
                      {getAgentLabel(handoff.from_agent)}
                      {" -> "}
                      {getAgentLabel(handoff.to_agent)}
                    </p>
                    <div className="task-actions">
                      {handoff.to_agent === selectedAgent && !handoff.consumed_at ? (
                        <button
                          type="button"
                          className="btn btn-primary"
                          onClick={() => void handleMarkHandoffConsumed(handoff.handoff_id)}
                        >
                          {locale === "en" ? "Mark reviewed" : "标记已查看"}
                        </button>
                      ) : (
                        <button
                          type="button"
                          className="btn btn-secondary"
                          onClick={() => {
                            setActivePage("history");
                            setHistoryView("transfers");
                          }}
                        >
                          {shell.viewHistory}
                        </button>
                      )}
                    </div>
                  </article>
                ))}
              </div>
            )}
          </section>
        </div>
      )}
    </div>
  );

  const renderHistoryConversations = () => {
    if (!selectedConversation) {
      return (
        <div className="empty-state empty-state-page">
          <div className="empty-state-icon">○</div>
          <div className="empty-state-text">{shell.chooseConversationBody}</div>
        </div>
      );
    }

    return (
      <div className="history-stack">
        <div className="task-panel compact-panel">
          <div className="task-panel-header">
            <div>
              <span className="task-panel-label">{shell.actionsLabel}</span>
              <h2>{normalizeConversationTitle(selectedConversation.summary) || selectedConversation.id}</h2>
            </div>
            <div className="task-actions">
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => setShowMigrateModal(true)}
                disabled={detailLoading}
              >
                {shell.migrate}
              </button>
              <button
                type="button"
                className="btn btn-danger"
                onClick={handleDelete}
                disabled={detailLoading}
              >
                {shell.delete}
              </button>
            </div>
          </div>
          <div className="meta-strip">
            <div className="meta-block">
              <span className="meta-label">{shell.fileLocation}</span>
              <span className={`meta-value ${selectedConversation.storage_path ? "" : "is-muted"}`}>
                {selectedConversation.storage_path || shell.noAvailablePath}
              </span>
            </div>
            <div className="task-actions">
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => void handleCopy("location", selectedConversation.storage_path)}
                disabled={!selectedConversation.storage_path}
              >
                {locationButtonLabel}
              </button>
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => void handleCopy("resume", selectedConversation.resume_command)}
                disabled={!selectedConversation.resume_command}
              >
                {resumeButtonLabel}
              </button>
            </div>
          </div>
        </div>
        {detailLoading ? (
          <div className="detail-loading">
            <div className="spinner"></div>
          </div>
        ) : (
          <ConversationDetail conversation={selectedConversation} />
        )}
      </div>
    );
  };

  const renderRecoveryHistory = () => {
    if (!selectedConversation) {
      return (
        <div className="empty-state empty-state-page">
          <div className="empty-state-icon">○</div>
          <div className="empty-state-text">{shell.chooseConversationBody}</div>
        </div>
      );
    }

    return (
      <div className="history-stack">
        <section className="task-panel">
          <div className="task-panel-header">
            <div>
              <span className="task-panel-label">{shell.recoverableProgress}</span>
              <h2>{shell.recoverableProgress}</h2>
            </div>
            <button
              type="button"
              className="btn btn-primary"
              onClick={() => void handleCreateCheckpoint()}
            >
              {shell.createCheckpoint}
            </button>
          </div>
          {memoryLoading ? (
            <div className="loading-inline">
              <div className="spinner"></div>
            </div>
          ) : checkpoints.length === 0 ? (
            <div className="inline-empty-state">
              <div className="inline-empty-body">
                {locale === "en" ? "No checkpoints for this project yet." : "这个项目还没有检查点。"}
              </div>
            </div>
          ) : (
            <div className="review-card-list">
              {checkpoints.map((checkpoint) => (
                <article key={checkpoint.checkpoint_id} className="review-card">
                  <strong>{checkpoint.summary}</strong>
                  <p>
                    {shell.createdAt}: {formatDateTime(checkpoint.created_at)}
                  </p>
                  <p>
                    {shell.resumeCommand}: {checkpoint.resume_command ?? "--"}
                  </p>
                  {checkpoint.handoff_id ? (
                    <p>
                      {shell.promotedHandoff}: {checkpoint.handoff_id}
                    </p>
                  ) : null}
                  <div className="task-actions">
                    <button
                      type="button"
                      className="btn btn-secondary"
                      onClick={() => void handleCopy("resume", checkpoint.resume_command)}
                      disabled={!checkpoint.resume_command}
                    >
                      {resumeButtonLabel}
                    </button>
                    {availableHandoffTargets.map((target) => (
                      <button
                        key={`${checkpoint.checkpoint_id}-${target}`}
                        type="button"
                        className="btn btn-secondary"
                        onClick={() => handlePromoteCheckpoint(checkpoint, target)}
                        disabled={checkpoint.status !== "active"}
                      >
                        {locale === "en" ? `Promote to ${getAgentLabel(target)}` : `转给 ${getAgentLabel(target)}`}
                      </button>
                    ))}
                  </div>
                </article>
              ))}
            </div>
          )}
        </section>
      </div>
    );
  };

  const renderTransferHistory = () => (
    <div className="history-stack">
      <section className="task-panel">
        <div className="task-panel-header">
          <div>
            <span className="task-panel-label">{shell.pendingTransfers}</span>
            <h2>{shell.pendingTransfers}</h2>
          </div>
          {availableHandoffTargets.map((target) => (
            <button
              key={target}
              type="button"
              className="btn btn-secondary"
              onClick={() => handleCreateHandoff(target)}
              disabled={!selectedConversation}
            >
              {locale === "en" ? `Create for ${getAgentLabel(target)}` : `创建给 ${getAgentLabel(target)}`}
            </button>
          ))}
        </div>
        {memoryLoading ? (
          <div className="loading-inline">
            <div className="spinner"></div>
          </div>
        ) : handoffs.length === 0 ? (
          <div className="inline-empty-state">
            <div className="inline-empty-body">
              {locale === "en" ? "No handoffs yet." : "还没有交接包。"}
            </div>
          </div>
        ) : (
          <div className="review-card-list">
            {handoffs.map((handoff) => (
              <article key={handoff.handoff_id} className="review-card">
                <strong>{handoff.current_goal}</strong>
                <p>
                  {getAgentLabel(handoff.from_agent)}
                  {" -> "}
                  {getAgentLabel(handoff.to_agent)}
                </p>
                {handoff.next_items.length > 0 ? <p>{handoff.next_items[0]}</p> : null}
                <div className="task-actions">
                  {!handoff.consumed_at && handoff.to_agent === selectedAgent ? (
                    <button
                      type="button"
                      className="btn btn-primary"
                      onClick={() => void handleMarkHandoffConsumed(handoff.handoff_id)}
                    >
                      {locale === "en" ? "Mark as consumed" : "标记已接收"}
                    </button>
                  ) : null}
                </div>
              </article>
            ))}
          </div>
        )}
      </section>
    </div>
  );

  const renderOutputHistory = () => (
    <div className="history-stack outputs-grid">
      <section className="task-panel">
        <div className="task-panel-header">
          <div>
            <span className="task-panel-label">{shell.outputsRuns}</span>
            <h2>{shell.outputsRuns}</h2>
          </div>
        </div>
        {memoryLoading ? (
          <div className="loading-inline">
            <div className="spinner"></div>
          </div>
        ) : runs.length === 0 ? (
          <div className="inline-empty-state">
            <div className="inline-empty-body">
              {locale === "en" ? "No run records yet." : "还没有运行记录。"}
            </div>
          </div>
        ) : (
          <div className="task-list">
            {runs.map((run) => (
              <div key={run.run_id} className="task-list-card">
                <strong>{run.task_hint || run.summary}</strong>
                <span>{run.summary}</span>
                <span>
                  {run.status}
                  {" · "}
                  {locale === "en"
                    ? `${run.artifact_count} artifact${run.artifact_count === 1 ? "" : "s"}`
                    : `${run.artifact_count} 个产物`}
                </span>
              </div>
            ))}
          </div>
        )}
      </section>

      <section className="task-panel">
        <div className="task-panel-header">
          <div>
            <span className="task-panel-label">{shell.outputsArtifacts}</span>
            <h2>{shell.outputsArtifacts}</h2>
          </div>
        </div>
        {memoryLoading ? (
          <div className="loading-inline">
            <div className="spinner"></div>
          </div>
        ) : artifacts.length === 0 ? (
          <div className="inline-empty-state">
            <div className="inline-empty-body">
              {locale === "en" ? "No artifacts yet." : "还没有产物记录。"}
            </div>
          </div>
        ) : (
          <div className="task-list">
            {artifacts.map((artifact) => (
              <div key={artifact.artifact_id} className="task-list-card">
                <strong>{artifact.title}</strong>
                <span>{artifact.summary}</span>
              </div>
            ))}
          </div>
        )}
      </section>

      <section className="task-panel">
        <div className="task-panel-header">
          <div>
            <span className="task-panel-label">{shell.outputsEpisodes}</span>
            <h2>{shell.outputsEpisodes}</h2>
          </div>
        </div>
        {memoryLoading ? (
          <div className="loading-inline">
            <div className="spinner"></div>
          </div>
        ) : episodes.length === 0 ? (
          <div className="inline-empty-state">
            <div className="inline-empty-body">
              {locale === "en" ? "No episode records yet." : "还没有阶段记录。"}
            </div>
          </div>
        ) : (
          <div className="task-list">
            {episodes.map((episode) => (
              <div key={episode.episode_id} className="task-list-card">
                <strong>{episode.title}</strong>
                <span>{episode.summary}</span>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );

  const renderHistoryPage = () => (
    <div className="page-layout">
      <header className="page-header">
        <div>
          <p className="page-eyebrow">{shell.nav.history}</p>
          <h1>{shell.historyTitle}</h1>
          <p>{shell.historySubtitle}</p>
        </div>
      </header>

      {activeRepoRoot ? (
        <LibraryPanel
          locale={locale}
          repoLabel={getProjectLabel(activeRepoRoot)}
          repoPath={activeRepoRoot}
          records={repoLibraryRecords}
          onOpenRecord={(record) => void handleOpenLibraryRecord(record)}
        />
      ) : null}

      <div className="history-filter-row">
        {(Object.keys(shell.historyFilters) as HistoryView[]).map((view) => (
          <button
            key={view}
            type="button"
            className={`history-filter-chip ${historyView === view ? "active" : ""}`}
            onClick={() => setHistoryView(view)}
          >
            {shell.historyFilters[view]}
          </button>
        ))}
      </div>

      {historyView === "conversations"
        ? renderHistoryConversations()
        : historyView === "recovery"
          ? renderRecoveryHistory()
          : historyView === "transfers"
            ? renderTransferHistory()
            : renderOutputHistory()}
    </div>
  );

  const renderHelpPage = () => (
    <div className="page-layout">
      <header className="page-header">
        <div>
          <p className="page-eyebrow">{shell.needHelp}</p>
          <h1>{shell.helpTitle}</h1>
          <p>{shell.helpSubtitle}</p>
        </div>
      </header>

      <div className="help-search-row">
        <input
          type="text"
          className="search-box help-search-box"
          value={helpQuery}
          onChange={(event) => setHelpQuery(event.target.value)}
          placeholder={shell.searchHelpPlaceholder}
        />
      </div>

      <div className="help-card-grid">
        {visibleHelpCards.map((card) => (
          <article key={card.id} className="help-card">
            <div>
              <strong>{card.title}</strong>
              <p>{card.description}</p>
            </div>
            <button type="button" className="btn btn-secondary" onClick={card.onSelect}>
              {card.buttonLabel}
            </button>
          </article>
        ))}
      </div>

      <section className="task-panel">
        <div className="task-panel-header">
          <div>
            <span className="task-panel-label">{shell.commonQuestions}</span>
            <h2>{shell.helpHowItWorks}</h2>
          </div>
        </div>
        <div className="help-answer-list">
          {visibleHelpCards.map((card) => (
            <article key={`answer-${card.id}`} className="help-answer">
              <strong>{card.title}</strong>
              <p>{card.answer}</p>
            </article>
          ))}
        </div>
      </section>

      <section className="task-panel">
        <button
          type="button"
          className="advanced-toggle"
          onClick={() => setAdvancedHelpOpen((current) => !current)}
        >
          <span>{shell.advancedTroubleshooting}</span>
          <WindowButtonIcon type="chevron" />
        </button>

        {advancedHelpOpen && (
          <div className="advanced-panel">
            <div className="meta-block">
              <span className="meta-label">{shell.connectionStatus}</span>
              <span className="meta-value">{getAgentHeading(selectedAgent, locale)}</span>
            </div>
            <div className="meta-block">
              <span className="meta-label">{shell.configLocations}</span>
              <span className="meta-value">{getAgentConfigLocation(selectedAgent)}</span>
            </div>
            <div className="meta-block">
              <span className="meta-label">{shell.relatedPaths}</span>
              <span className="meta-value">
                {selectedConversation ? getConversationProjectDisplay(selectedConversation) : "--"}
                {"\n"}
                {selectedConversation?.storage_path || "--"}
              </span>
            </div>
            <div className="meta-block">
              <span className="meta-label">{shell.resumeCommand}</span>
              <span className="meta-value">{selectedConversation?.resume_command || "--"}</span>
            </div>
          </div>
        )}
      </section>
    </div>
  );

  const detachedLegacyPageRenderers = [
    renderContinuePage,
    renderReviewPage,
    renderHistoryPage,
    renderHelpPage,
  ];
  void detachedLegacyPageRenderers;

  const renderMemoryDrawer = () => {
    if (!memoryDrawerOpen || !activeRepoRoot) {
      return null;
    }

    const memoryTitle = locale === "en" ? "Startup Rules" : "\u542f\u52a8\u89c4\u5219\u7ba1\u7406";
    const drawerSubtitle =
      locale === "en"
        ? "Review only durable rules that should be carried into new tasks. Local history search is already available from the top of the workspace."
        : "\u8fd9\u91cc\u53ea\u5904\u7406\u65b0\u4efb\u52a1\u9700\u8981\u5e26\u4e0a\u7684\u7a33\u5b9a\u89c4\u5219\u3002\u672c\u5730\u5386\u53f2\u68c0\u7d22\u5df2\u7ecf\u5728\u5de5\u4f5c\u533a\u9876\u90e8\u53ef\u7528\u3002";
    const wikiSubtitle =
      locale === "en"
        ? "Readable pages rebuilt from approved startup rules and local-history episodes."
        : "\u7531\u5df2\u6279\u51c6\u542f\u52a8\u89c4\u5219\u548c\u672c\u5730\u5386\u53f2\u9636\u6bb5\u8bb0\u5f55\u91cd\u5efa\u7684\u53ef\u8bfb\u9875\u9762\u3002";
    const emptyWiki =
      locale === "en"
        ? "No wiki projection has been generated yet."
        : "\u8fd8\u6ca1\u6709\u751f\u6210 Wiki \u6295\u5f71\u3002";
    const continuationSubtitle =
      locale === "en"
        ? "Checkpoints and handoff packets are temporary continuation state, not durable startup rules."
        : "\u68c0\u67e5\u70b9\u548c\u4ea4\u63a5\u5305\u662f\u7ee7\u7eed\u5de5\u4f5c\u7528\u7684\u4e34\u65f6\u72b6\u6001\uff0c\u4e0d\u662f\u957f\u671f\u542f\u52a8\u89c4\u5219\u3002";
    const emptyContinuation =
      locale === "en"
        ? "No checkpoints or handoff packets for this project yet."
        : "\u8fd9\u4e2a\u9879\u76ee\u8fd8\u6ca1\u6709\u68c0\u67e5\u70b9\u6216\u4ea4\u63a5\u5305\u3002";
    const continuationCount = checkpoints.length + handoffs.length;
    const tabs: Array<{ id: MemoryDrawerTab; label: string; count: number }> = [
      {
        id: "inbox",
        label: locale === "en" ? "Review" : "\u5019\u9009\u89c4\u5219",
        count: memoryCandidates.length,
      },
      {
        id: "approved",
        label: locale === "en" ? "Rules" : "\u542f\u52a8\u89c4\u5219",
        count: repoMemories.length,
      },
      { id: "wiki", label: locale === "en" ? "Wiki" : "Wiki", count: wikiPages.length },
      {
        id: "continuation",
        label: locale === "en" ? "Continue" : "\u7ee7\u7eed",
        count: continuationCount,
      },
    ];
    const shouldAutoFocusFirstApprovedMemory =
      memoryDrawerOpen &&
      memoryDrawerTab === "approved" &&
      pendingApprovedMemoryAutofocusConversationId === activeConversationId;

    const renderWikiTab = () => (
      <section className="memory-panel">
        <div className="memory-panel-header">
          <h3>{locale === "en" ? "Project Wiki" : "\u9879\u76ee Wiki"}</h3>
          <p>{wikiSubtitle}</p>
        </div>
        {memoryLoading ? (
          <div className="loading">
            <div className="spinner"></div>
          </div>
        ) : wikiPages.length === 0 ? (
          <div className="empty-state">
            <div className="empty-state-icon">W</div>
            <div className="empty-state-text">{emptyWiki}</div>
          </div>
        ) : (() => {
          const selectedWikiPage =
            wikiPages.find((page) => page.page_id === selectedWikiPageId) ?? wikiPages[0];

          return (
            <div className="wiki-page-browser">
              <nav
                className="wiki-page-list"
                aria-label={locale === "en" ? "Wiki pages" : "Wiki \u9875\u9762"}
              >
                {wikiPages.slice(0, 20).map((page) => {
                  const isSelected = page.page_id === selectedWikiPage.page_id;
                  return (
                    <button
                      key={page.page_id}
                      type="button"
                      className={`wiki-page-list-item ${isSelected ? "is-selected" : ""}`}
                      aria-current={isSelected ? "page" : undefined}
                      onClick={() => setSelectedWikiPageId(page.page_id)}
                    >
                      <strong>{page.title}</strong>
                      <span>{page.status}</span>
                      <small>{getWikiPreview(page.body)}</small>
                    </button>
                  );
                })}
              </nav>
              <article className="wiki-page-reader">
                <header className="wiki-page-reader-header">
                  <div>
                    <span className="memory-card-kind">{selectedWikiPage.status}</span>
                    <h4>{selectedWikiPage.title}</h4>
                  </div>
                  <div className="wiki-page-reader-meta">
                    <span>{getWikiSourceLabel(selectedWikiPage, locale)}</span>
                    <span>{formatDateTime(selectedWikiPage.updated_at)}</span>
                  </div>
                </header>
                <div className="wiki-page-body">{renderWikiBody(selectedWikiPage.body)}</div>
              </article>
            </div>
          );
        })()}
      </section>
    );

    const renderContinuationTab = () => (
      <section className="memory-panel">
        <div className="memory-panel-header">
          <h3>{locale === "en" ? "Continue Work" : "\u7ee7\u7eed\u5de5\u4f5c"}</h3>
          <p>{continuationSubtitle}</p>
        </div>
        {memoryLoading ? (
          <div className="loading">
            <div className="spinner"></div>
          </div>
        ) : continuationCount === 0 ? (
          <div className="empty-state">
            <div className="empty-state-icon">C</div>
            <div className="empty-state-text">{emptyContinuation}</div>
          </div>
        ) : (
          <div className="memory-card-list">
            {checkpoints.map((checkpoint) => (
              <article key={checkpoint.checkpoint_id} className="memory-card">
                <div className="memory-card-header">
                  <div>
                    <strong>{checkpoint.summary}</strong>
                    <div className="memory-card-kind">
                      {locale === "en" ? "Checkpoint" : "\u68c0\u67e5\u70b9"}
                      {" · "}
                      {checkpoint.status}
                    </div>
                  </div>
                </div>
                <p className="memory-card-copy">{checkpoint.resume_command ?? "--"}</p>
                <div className="memory-card-meta">
                  <span>{checkpoint.source_agent}</span>
                  <span>{formatDateTime(checkpoint.created_at)}</span>
                </div>
              </article>
            ))}
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
                  <span className={`handoff-status-pill handoff-status-${handoff.status}`}>
                    {handoff.status}
                  </span>
                </div>
                {handoff.next_items.length > 0 ? (
                  <p className="memory-card-copy">{handoff.next_items[0]}</p>
                ) : null}
                <div className="memory-card-meta">
                  <span>{formatDateTime(handoff.created_at)}</span>
                  {handoff.target_profile ? <span>{handoff.target_profile}</span> : null}
                </div>
                {!handoff.consumed_at && handoff.to_agent === selectedAgent ? (
                  <div className="memory-card-actions">
                    <button
                      type="button"
                      className="btn btn-secondary"
                      onClick={() => void handleMarkHandoffConsumed(handoff.handoff_id)}
                    >
                      {locale === "en" ? "Mark as consumed" : "\u6807\u8bb0\u5df2\u63a5\u6536"}
                    </button>
                  </div>
                ) : null}
              </article>
            ))}
          </div>
        )}
      </section>
    );

    const renderDrawerTab = () => {
      if (memoryDrawerTab === "approved") {
        return (
          <RepoMemoryPanel
            memories={repoMemories}
            loading={memoryLoading}
            locale={locale}
            onReverify={(memoryId) => void handleReverifyMemory(memoryId)}
            onRetire={(memoryId) => void handleRetireMemory(memoryId)}
            onRetireMany={(memoryIds) => void handleRetireManyMemories(memoryIds)}
            autoFocusFirstMemory={shouldAutoFocusFirstApprovedMemory || undefined}
            onAutoFocusHandled={
              shouldAutoFocusFirstApprovedMemory
                ? () => setPendingApprovedMemoryAutofocusConversationId(null)
                : undefined
            }
          />
        );
      }

      if (memoryDrawerTab === "wiki") {
        return renderWikiTab();
      }

      if (memoryDrawerTab === "continuation") {
        return renderContinuationTab();
      }

      return (
        <MemoryInboxPanel
          candidates={memoryCandidates}
          loading={memoryLoading}
          locale={locale}
          onApprove={(candidate, reviewDraft) => void handleApproveCandidate(candidate, reviewDraft)}
          onApproveMerge={(candidate) => void handleApproveMergeCandidate(candidate)}
          onReject={(candidateId) => void handleRejectCandidate(candidateId)}
        />
      );
    };

    return (
      <div className="memory-drawer-overlay" onMouseDown={closeMemoryDrawer}>
        <aside
          className="memory-drawer"
          role="complementary"
          aria-label={memoryTitle}
          onMouseDown={(event) => event.stopPropagation()}
        >
          <header className="memory-drawer-header">
            <div>
              <p className="page-eyebrow">{locale === "en" ? "Repository rules" : "\u4ed3\u5e93\u89c4\u5219"}</p>
              <h2>{memoryTitle}</h2>
              <span>{drawerSubtitle}</span>
            </div>
            <button
              type="button"
              className="icon-button"
              aria-label={locale === "en" ? "Close startup rules drawer" : "\u5173\u95ed\u542f\u52a8\u89c4\u5219\u62bd\u5c49"}
              onClick={closeMemoryDrawer}
            >
              <WindowButtonIcon type="close" />
            </button>
          </header>

          <div className="memory-drawer-tabs" role="tablist" aria-label={memoryTitle}>
            {tabs.map((tab) => (
              <button
                key={tab.id}
                type="button"
                role="tab"
                aria-selected={memoryDrawerTab === tab.id}
                className={`memory-drawer-tab ${memoryDrawerTab === tab.id ? "active" : ""}`}
                onClick={() => handleMemoryDrawerTabChange(tab.id)}
              >
                <span>{tab.label}</span>
                <span className="memory-drawer-tab-count">{tab.count}</span>
              </button>
            ))}
          </div>

          <div className="memory-drawer-body" role="tabpanel">
            {renderDrawerTab()}
          </div>
        </aside>
      </div>
    );
  };

  const renderAboutWorkspace = () => (
    <section className="about-workspace-page" aria-labelledby="about-chatmem-title">
      <header className="settings-panel-header about-page-header">
        <div className="about-title-cluster">
          <div className="about-brand-mark" aria-hidden="true">
            <img src={brandIcon} alt="" />
          </div>
          <div>
            <p className="page-eyebrow">{shell.aboutChatMem}</p>
            <h1 id="about-chatmem-title">{locale === "en" ? "About ChatMem" : "关于 ChatMem"}</h1>
          </div>
        </div>
        <button type="button" className="toolbar-button" onClick={() => setShowAbout(false)}>
          {locale === "en" ? "Back" : "返回"}
        </button>
      </header>

      <section className="about-summary-section">
        <p>
          {locale === "en"
            ? "ChatMem is a local-first memory layer for people who work with AI coding agents every day."
            : "ChatMem 是给长期使用 AI 编程 Agent 的人准备的本地优先记忆层。"}
        </p>
        <p>
          {locale === "en"
            ? "It indexes local Claude, Codex, Gemini, and OpenCode history, keeps the original evidence traceable, and turns stable project knowledge into startup rules and Wiki context."
            : "它索引本机 Claude、Codex、Gemini 和 OpenCode 历史，保留可追溯的原始证据，并把稳定的项目知识整理成启动规则和 Wiki 上下文。"}
        </p>
      </section>

      <section className="about-detail-grid" aria-label={locale === "en" ? "Product details" : "产品说明"}>
        <article className="about-detail-card">
          <span>{locale === "en" ? "Version" : "版本"}</span>
          <strong>v{packageInfo.version}</strong>
        </article>
        <article className="about-detail-card">
          <span>{locale === "en" ? "Agent layer" : "Agent 层"}</span>
          <strong>{locale === "en" ? "MCP + Native Guidance" : "MCP + 原生引导"}</strong>
        </article>
        <article className="about-detail-card">
          <span>{locale === "en" ? "Data posture" : "数据方式"}</span>
          <strong>{locale === "en" ? "Local-first" : "本地优先"}</strong>
        </article>
        <article className="about-detail-card">
          <span>GitHub</span>
          <a
            className="about-github-link"
            href="https://github.com/Rimagination/ChatMem"
            target="_blank"
            rel="noreferrer"
          >
            Rimagination/ChatMem
          </a>
        </article>
      </section>

      <section className="about-prose-section" aria-label={locale === "en" ? "Product positioning" : "产品定位"}>
        <article className="about-prose-block">
          <h2>{locale === "en" ? "Why it exists" : "项目简介"}</h2>
          <p>
            {locale === "en"
              ? "Model-native memory is useful, but it is usually tied to one product account. ChatMem treats local conversations as the durable source of truth, so past decisions, project paths, tool calls, and recovery commands can be found again without waiting for a model to remember them."
              : "模型自带记忆很有用，但通常绑定在某一个产品账号里。ChatMem 把本地对话当成长期可信的来源，让过去的决策、项目路径、工具调用和恢复命令可以被重新找到，而不是只能寄希望于模型自己想起来。"}
          </p>
        </article>

        <article className="about-prose-block">
          <h2>{locale === "en" ? "Positioning and advantages" : "优势与生态位"}</h2>
          <ul className="about-text-list">
            {(locale === "en"
              ? [
                  "More portable than native memory: one local layer works across Claude, Codex, Gemini, and OpenCode.",
                  "More auditable than pure vector memory: answers can point back to original conversations and files.",
                  "Closer to coding work than generic memory stores: history search, startup rules, Wiki context, trash recovery, and agent installation are built around real project workflows.",
                  "Local-first by default: WebDAV is optional backup, not a requirement for daily recall.",
                ]
              : [
                  "比模型自带记忆更可迁移：同一层本地记忆可以跨 Claude、Codex、Gemini 和 OpenCode 使用。",
                  "比单纯向量记忆更可审计：检索结果可以回到原始对话和文件证据。",
                  "比通用记忆库更贴近编程工作流：历史检索、启动规则、Wiki 上下文、垃圾箱恢复和 Agent 安装都围绕真实项目展开。",
                  "默认本地优先：WebDAV 只是可选备份，不是日常回忆能力的前提。",
                ]
            ).map((item) => (
              <li key={item}>{item}</li>
            ))}
          </ul>
        </article>
      </section>

      <section className="settings-section settings-about" aria-labelledby="about-acknowledgements-title">
        <div className="about-section-heading">
          <h2 id="about-acknowledgements-title">
            {locale === "en" ? "Design references and acknowledgements" : "设计参考与致谢"}
          </h2>
          <p className="settings-helper">
            {locale === "en"
              ? "ChatMem learns from several memory, agent-state, and code-wiki directions, but it is not a clone of any single project. These references are acknowledgements, not dependencies or endorsements."
              : "ChatMem 参考了多个记忆、Agent 状态管理和代码知识库方向，但它不是某一个项目的复刻。下面是设计灵感与致谢，不表示依赖、复刻或由相关项目背书。"}
          </p>
        </div>
        <ul
          className="acknowledgement-list"
          aria-label={locale === "en" ? "Acknowledged projects" : "致谢项目"}
        >
          {ACKNOWLEDGED_SYSTEMS.map((system) => (
            <li key={system} className="acknowledgement-item">
              {system}
            </li>
          ))}
        </ul>
      </section>
    </section>
  );

  const renderTrashWorkspace = () => (
    <div className="trash-workspace-page">
      <header className="trash-page-header">
        <div>
          <h1>{shell.trashWorkspaceTitle}</h1>
          <p>{shell.trashWorkspaceSubtitle}</p>
        </div>
        <div className="trash-page-actions">
          <button
            type="button"
            className="btn btn-secondary trash-empty-button"
            onClick={handleEmptyTrashClick}
            disabled={trashLoading || trashedConversations.length === 0}
          >
            {shell.emptyTrash}
          </button>
          <label className="trash-retention-control" htmlFor="trash-retention-days">
            {shell.trashRetentionDays}
            <input
              id="trash-retention-days"
              type="number"
              min={1}
              max={365}
              value={appSettings.trashRetentionDays}
              onChange={(event) => handleTrashRetentionDaysChange(Number(event.target.value))}
            />
          </label>
        </div>
      </header>

      <p className="trash-retention-hint">{shell.trashRetentionHint}</p>

      {trashLoading ? (
        <div className="loading">
          <div className="spinner"></div>
        </div>
      ) : trashedConversations.length === 0 ? (
        <div className="empty-state empty-state-page">
          <div className="empty-state-icon">
            <WindowButtonIcon type="trash" />
          </div>
          <h2>{shell.trashEmptyTitle}</h2>
          <div className="empty-state-text">{shell.trashEmptyBody}</div>
        </div>
      ) : (
        <div className="trash-card-list">
          {trashedConversations.map((item) => {
            const title = normalizeConversationTitle(item.summary) || item.originalId;
            const isRestoring = restoringTrashId === item.trashId;
            return (
              <article key={item.trashId} className="trash-card">
                <div className="trash-card-main">
                  <div>
                    <span className="trash-card-agent">{item.sourceAgent}</span>
                    <h3 title={title}>{title}</h3>
                  </div>
                  <p title={item.projectDir}>{item.projectDir}</p>
                  <div className="trash-card-meta">
                    <span>
                      {shell.expiresAt}: {formatDateTime(item.expiresAt)}
                    </span>
                    {item.remoteBackupDeleted ? <span>{shell.remoteBackupDeleted}</span> : null}
                  </div>
                </div>
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => void handleRestoreTrashConversation(item.trashId)}
                  disabled={isRestoring}
                >
                  {isRestoring ? shell.restoring : shell.restore}
                </button>
              </article>
            );
          })}
        </div>
      )}
    </div>
  );

  const renderTrashConfirmModal = () => {
    if (!trashConfirm) {
      return null;
    }

    const remoteAvailable =
      appSettings.sync.provider === "webdav" &&
      appSettings.sync.webdavHost.trim().length > 0 &&
      appSettings.sync.username.trim().length > 0;
    const previewTargets = trashConfirm.targets.slice(0, 4);

    return (
      <div className="modal-overlay" onClick={() => !trashConfirm.busy && setTrashConfirm(null)}>
        <div
          className="modal trash-confirm-modal"
          role="dialog"
          aria-modal="true"
          aria-labelledby="trash-confirm-title"
          onClick={(event) => event.stopPropagation()}
        >
          <div className="modal-content">
            <p className="page-eyebrow">{shell.trash}</p>
            <h3 id="trash-confirm-title">{shell.confirmTrashTitle(trashConfirm.targets.length)}</h3>
            <p>{shell.confirmTrashBody(trashConfirm.targets.length)}</p>
            <p className="modal-helper-text">
              {shell.confirmTrashLocalHint(appSettings.trashRetentionDays)}
            </p>

            <div className="trash-confirm-list">
              {previewTargets.map((target) => (
                <div key={`${target.source_agent}-${target.id}`} className="trash-confirm-item">
                  <strong>{normalizeConversationTitle(target.summary) || target.id}</strong>
                  <span>{target.source_agent}</span>
                </div>
              ))}
              {trashConfirm.targets.length > previewTargets.length ? (
                <div className="trash-confirm-more">
                  +{trashConfirm.targets.length - previewTargets.length}
                </div>
              ) : null}
            </div>

            <label className={`trash-remote-option ${remoteAvailable ? "" : "is-disabled"}`}>
              <input
                type="checkbox"
                checked={trashConfirm.deleteRemoteBackup && remoteAvailable}
                disabled={!remoteAvailable || trashConfirm.busy}
                onChange={(event) =>
                  setTrashConfirm((current) =>
                    current
                      ? {
                          ...current,
                          deleteRemoteBackup: event.target.checked,
                          error: null,
                        }
                      : current,
                  )
                }
              />
              <span>{shell.confirmTrashRemoteBackup}</span>
            </label>
            {!remoteAvailable ? (
              <p className="trash-remote-note">{shell.confirmTrashRemoteUnavailable}</p>
            ) : null}
            {trashConfirm.error ? (
              <p className="settings-notice is-danger">{trashConfirm.error}</p>
            ) : null}
          </div>
          <div className="modal-actions">
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => setTrashConfirm(null)}
              disabled={trashConfirm.busy}
            >
              {shell.cancel}
            </button>
            <button
              type="button"
              className="btn btn-danger"
              onClick={() => void confirmMoveConversationsToTrash()}
              disabled={trashConfirm.busy}
            >
              {trashConfirm.busy ? shell.movingToTrash : shell.moveToTrash}
            </button>
          </div>
        </div>
      </div>
    );
  };

  const renderEmptyTrashConfirmModal = () => {
    if (!emptyTrashConfirm) {
      return null;
    }

    const count = trashedConversations.length;

    return (
      <div
        className="modal-overlay"
        onClick={() => !emptyTrashConfirm.busy && setEmptyTrashConfirm(null)}
      >
        <div
          className="modal trash-confirm-modal"
          role="dialog"
          aria-modal="true"
          aria-labelledby="empty-trash-confirm-title"
          onClick={(event) => event.stopPropagation()}
        >
          <div className="modal-content">
            <p className="page-eyebrow">{shell.trash}</p>
            <h3 id="empty-trash-confirm-title">{shell.confirmEmptyTrashTitle}</h3>
            <p>{shell.confirmEmptyTrashBody(count)}</p>
            {emptyTrashConfirm.error ? (
              <p className="settings-notice is-danger">{emptyTrashConfirm.error}</p>
            ) : null}
          </div>
          <div className="modal-actions">
            <button
              type="button"
              className="btn btn-secondary"
              onClick={() => setEmptyTrashConfirm(null)}
              disabled={emptyTrashConfirm.busy}
            >
              {shell.cancel}
            </button>
            <button
              type="button"
              className="btn btn-danger"
              onClick={() => void confirmEmptyTrash()}
              disabled={emptyTrashConfirm.busy}
            >
              {emptyTrashConfirm.busy ? shell.emptyingTrash : shell.emptyTrash}
            </button>
          </div>
        </div>
      </div>
    );
  };

  const renderWorkspace = () => {
    if (!selectedConversation) {
      return (
        <div className="conversation-empty-state">
          <div className="empty-state-icon brand-empty-icon" aria-hidden="true">
            <img src={brandIcon} alt="" />
          </div>
          <h1>{shell.chooseConversation}</h1>
          <div className="empty-state-text">{shell.noProgressBody}</div>
        </div>
      );
    }

    const conversationTitle =
      normalizeConversationTitle(selectedConversation.summary) || selectedConversation.id;
    const visibleConversationTitle = truncateWorkspaceTitle(conversationTitle);
    const selectedConversationProjectDisplay = getConversationProjectDisplay(selectedConversation);
    const memoryAttentionCount = memoryCandidates.length;
    const currentWorkspaceView: WorkspaceView = activeRepoRoot ? workspaceView : "conversation";
    const localHistoryPanel = activeRepoRoot ? (
      <ProjectIndexStatus
        health={repoMemoryHealth}
        importReport={lastLocalHistoryImportReport}
        loading={repoHealthLoading}
        scanning={repoScanRunning}
        bootstrapReady={bootstrapReadyConversationId === selectedConversation.id}
        locale={locale}
        onScan={() => void handleScanRepoConversations()}
        onMergeAlias={(aliasRoot) => void handleMergeRepoAlias(aliasRoot)}
        mergingAliasRoot={mergingAliasRoot}
        onOpenRules={() => {
          handleMemoryDrawerTabChange(memoryAttentionCount > 0 ? "inbox" : "approved");
          setMemoryDrawerOpen(true);
        }}
        onRecallHistory={async (query) => {
          const context = await getProjectContext({
            repoRoot: activeRepoRoot,
            query,
            intent: "recall",
            limit: 5,
          });
          return context.relevant_history;
        }}
      />
    ) : null;

    return (
      <div className={`conversation-workspace workspace-view-${currentWorkspaceView}`}>
        {activeRepoRoot ? (
          <div
            className="workspace-view-switcher"
            role="tablist"
            aria-label={shell.workspaceSwitcherLabel}
          >
            <button
              type="button"
              role="tab"
              id="workspace-tab-conversation"
              aria-selected={currentWorkspaceView === "conversation"}
              aria-controls="workspace-panel-conversation"
              className={`workspace-view-tab ${currentWorkspaceView === "conversation" ? "active" : ""}`}
              onClick={() => setWorkspaceView("conversation")}
            >
              {shell.workspaceConversation}
            </button>
            <button
              type="button"
              role="tab"
              id="workspace-tab-history"
              aria-selected={currentWorkspaceView === "history"}
              aria-controls="workspace-panel-history"
              className={`workspace-view-tab ${currentWorkspaceView === "history" ? "active" : ""}`}
              onClick={() => setWorkspaceView("history")}
            >
              {shell.workspaceLocalHistory}
            </button>
          </div>
        ) : null}

        {currentWorkspaceView === "history" && localHistoryPanel ? (
          <div
            id="workspace-panel-history"
            className="workspace-view-panel workspace-view-panel-history"
            role="tabpanel"
            aria-labelledby="workspace-tab-history"
          >
            {localHistoryPanel}
          </div>
        ) : (
          <div
            id="workspace-panel-conversation"
            className="workspace-view-panel workspace-view-panel-conversation"
            role="tabpanel"
            aria-labelledby={activeRepoRoot ? "workspace-tab-conversation" : undefined}
          >
            <header className="conversation-toolbar">
              <div className="conversation-title-block">
                <p className="page-eyebrow">{getCurrentConversationLabel(selectedAgent, locale)}</p>
                <h1 title={conversationTitle}>{visibleConversationTitle}</h1>
                <span title={selectedConversation.project_dir || selectedConversationProjectDisplay}>
                  {selectedConversationProjectDisplay}
                </span>
              </div>
              <div className="conversation-toolbar-actions">
                <button
                  type="button"
                  className="btn btn-primary"
                  onClick={() => setShowMigrateModal(true)}
                  disabled={detailLoading}
                >
                  {shell.migrate}
                </button>
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => void handleCopy("location", selectedConversation.storage_path)}
                  disabled={!selectedConversation.storage_path}
                >
                  {locationButtonLabel}
                </button>
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => void handleCopy("resume", selectedConversation.resume_command)}
                  disabled={!selectedConversation.resume_command}
                >
                  {resumeButtonLabel}
                </button>
              </div>
            </header>

            <div className="conversation-meta-strip compact">
              <div className="meta-block">
                <span className="meta-label">{shell.fileLocation}</span>
                <span className={`meta-value ${selectedConversation.storage_path ? "" : "is-muted"}`}>
                  {selectedConversation.storage_path || shell.noAvailablePath}
                </span>
              </div>
              <div className="meta-block">
                <span className="meta-label">{shell.resumeCommand}</span>
                <span className={`meta-value ${selectedConversation.resume_command ? "" : "is-muted"}`}>
                  {selectedConversation.resume_command || "--"}
                </span>
              </div>
            </div>

            <div className="conversation-content-grid">
              <ConversationDetail conversation={selectedConversation} />
            </div>
          </div>
        )}
      </div>
    );
  };

  const handleTopbarMouseDown = (event: ReactMouseEvent<HTMLElement>) => {
    if (event.button !== 0) {
      return;
    }

    const target = event.target as HTMLElement | null;
    if (target?.closest("button,input,select,textarea,a,[role='button']")) {
      return;
    }

    event.preventDefault();
    void appWindow.startDragging();
  };

  const handleToggleWindowSize = async () => {
    await appWindow.toggleMaximize();
    window.setTimeout(() => {
      void syncNativeWindowState();
    }, 0);
  };

  const handleTrashRetentionDaysChange = (nextDays: number) => {
    const trashRetentionDays = Math.min(365, Math.max(1, Math.round(nextDays || 1)));
    const nextSettings = updateSettings({ trashRetentionDays });
    setAppSettings(nextSettings);
  };

  const handleFontFamilyChange = (fontFamily: AppFontFamily) => {
    const nextSettings = updateSettings({ fontFamily });
    setAppSettings(nextSettings);
  };

  const renderSettingsPanel = () => (
    <SettingsPanel
      open={showSettings}
      title={t("settings.title")}
      closeLabel={locale === "en" ? "Back" : "返回"}
      languageLabel={t("settings.language")}
      locale={appSettings.locale}
      fontFamily={appSettings.fontFamily}
      autoCheckUpdates={appSettings.autoCheckUpdates}
      autoCheckLabel={t("settings.autoCheck")}
      checkUpdatesLabel={t("settings.checkUpdates")}
      checkingLabel={t("settings.checking")}
      upToDateLabel={t("settings.upToDate")}
      updateAvailablePrefix={t("settings.updateAvailablePrefix")}
      installUpdateLabel={t("settings.updateNow")}
      installingLabel={t("settings.installing")}
      updateState={updateState}
      syncSettings={appSettings.sync}
      syncCopy={syncCopy}
      onClose={() => setShowSettings(false)}
      onLocaleChange={(nextLocale: Locale) => {
        setLocale(nextLocale);
        const nextSettings = { ...appSettings, locale: nextLocale };
        setAppSettings(nextSettings);
      }}
      onFontFamilyChange={handleFontFamilyChange}
      onAutoCheckChange={(autoCheckUpdates: boolean) => {
        const nextSettings = updateSettings({ autoCheckUpdates });
        setAppSettings(nextSettings);
      }}
      onSyncSettingsChange={(patch) => {
        const nextSettings = updateSettings({
          sync: {
            ...appSettings.sync,
            ...patch,
          },
        });
        setAppSettings(nextSettings);
      }}
      onVerifyWebDavServer={handleVerifyWebDavServer}
      onSyncWebDavNow={handleSyncWebDavNow}
      onRunUpgradeReadinessCheck={handleRunUpgradeReadinessCheck}
      onDetectAgentIntegrations={handleDetectAgentIntegrations}
      onInstallAgentIntegration={handleInstallAgentIntegration}
      onUninstallAgentIntegration={handleUninstallAgentIntegration}
      onLoadWebDavPassword={loadWebDavPassword}
      onSaveWebDavPassword={({ username, password }) => saveWebDavPassword(username, password)}
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
  );

  return (
    <div className={`app-shell ${isWindowFilled ? "is-window-filled" : ""}`} style={appShellStyle}>
      <header className="app-topbar" data-tauri-drag-region="true" onMouseDown={handleTopbarMouseDown}>
        <div className="topbar-left" data-tauri-drag-region="true">
          <img className="topbar-app-icon" src={brandIcon} alt="ChatMem icon" />
          <span className="topbar-version">ChatMem v{packageInfo.version}</span>
        </div>

        <div className="topbar-drag-space" data-tauri-drag-region="true" />

        <div className="window-controls">
          <button
            type="button"
            className="window-control-button"
            aria-label="Minimize window"
            onClick={() => void appWindow.minimize()}
          >
            <WindowButtonIcon type="minimize" />
          </button>
          <button
            type="button"
            className="window-control-button"
            aria-label="Toggle window size"
            onClick={() => void handleToggleWindowSize()}
          >
            <WindowButtonIcon type="maximize" />
          </button>
          <button
            type="button"
            className="window-control-button is-close"
            aria-label="Close window"
            onClick={() => void appWindow.close()}
          >
            <WindowButtonIcon type="close" />
          </button>
        </div>
      </header>

      <div
        className={`app-body ${libraryArrangement === "chats-first" ? "chats-first" : ""} ${
          showSettings || showAbout ? "is-full-page" : ""
        }`}
      >
        <aside className="sidebar">
          <div className="sidebar-scroll">
            <div className="sidebar-controls">
              <div className="agent-tabs">
                <button
                  type="button"
                  className={`agent-tab ${selectedAgent === "claude" ? "active" : ""}`}
                  onClick={() => setSelectedAgent("claude")}
                >
                  Claude
                </button>
                <button
                  type="button"
                  className={`agent-tab ${selectedAgent === "codex" ? "active" : ""}`}
                  onClick={() => setSelectedAgent("codex")}
                >
                  Codex
                </button>
                <button
                  type="button"
                  className={`agent-tab ${selectedAgent === "gemini" ? "active" : ""}`}
                  onClick={() => setSelectedAgent("gemini")}
                >
                  Gemini
                </button>
                <button
                  type="button"
                  className={`agent-tab ${selectedAgent === "opencode" ? "active" : ""}`}
                  onClick={() => setSelectedAgent("opencode")}
                >
                  OpenCode
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

            <section className="library-section">
              <div className="library-section-header">
                <div className="library-section-title-row">
                  <h2>{shell.projectSection}</h2>
                  <span className="library-count-pill">{projectGroups.length}</span>
                </div>
                <div className="library-section-actions" ref={organizeMenuRef}>
                  <button
                    type="button"
                    className={`icon-button sidebar-action-button ${
                      allProjectsCollapsed ? "is-restore" : "is-collapse"
                    }`}
                    aria-label={projectCollapseActionLabel}
                    title={projectCollapseActionLabel}
                    onClick={handleToggleCollapseProjects}
                  >
                    <WindowButtonIcon type={allProjectsCollapsed ? "restoreExpansion" : "collapseAll"} />
                    <span className="sidebar-action-tooltip" aria-hidden="true">
                      {projectCollapseActionLabel}
                    </span>
                  </button>
                  <button
                    type="button"
                    className={`icon-button sidebar-action-button ${showOrganizeMenu ? "is-active" : ""}`}
                    aria-label={shell.openOrganizer}
                    title={shell.openOrganizer}
                    aria-haspopup="menu"
                    aria-expanded={showOrganizeMenu}
                    onClick={() => setShowOrganizeMenu((current) => !current)}
                  >
                    <WindowButtonIcon type="organize" />
                    <span className="sidebar-action-tooltip" aria-hidden="true">
                      {shell.openOrganizer}
                    </span>
                  </button>
                  <button
                    type="button"
                    className={`icon-button sidebar-action-button ${bulkSelectionMode ? "is-active" : ""}`}
                    aria-label={bulkSelectionMode ? shell.cancelBulkSelect : shell.bulkSelect}
                    title={bulkSelectionMode ? shell.cancelBulkSelect : shell.bulkSelect}
                    onClick={handleToggleBulkSelectionMode}
                  >
                    <WindowButtonIcon type="bulkSelect" />
                    <span className="sidebar-action-tooltip" aria-hidden="true">
                      {bulkSelectionMode ? shell.cancelBulkSelect : shell.bulkSelect}
                    </span>
                  </button>
                  {showOrganizeMenu && (
                    <div className="organize-menu">
                      <div className="organize-group">
                        <div className="organize-group-title">{shell.organizeArrangement}</div>
                        {([
                          ["projects", shell.arrangeProjects],
                          ["timeline", shell.arrangeTimeline],
                          ["chats-first", shell.arrangeChatsFirst],
                        ] as Array<[LibraryArrangement, string]>).map(([value, label]) => (
                          <button
                            key={value}
                            type="button"
                            className={`organize-item ${libraryArrangement === value ? "active" : ""}`}
                            onClick={() => setLibraryArrangement(value)}
                          >
                            <span>{label}</span>
                            {libraryArrangement === value ? <span className="organize-check">✓</span> : null}
                          </button>
                        ))}
                      </div>

                      <div className="organize-group">
                        <div className="organize-group-title">{shell.organizeSort}</div>
                        {([
                          ["updated", shell.sortUpdated],
                          ["created", shell.sortCreated],
                        ] as Array<[LibrarySort, string]>).map(([value, label]) => (
                          <button
                            key={value}
                            type="button"
                            className={`organize-item ${librarySort === value ? "active" : ""}`}
                            onClick={() => setLibrarySort(value)}
                          >
                            <span>{label}</span>
                            {librarySort === value ? <span className="organize-check">✓</span> : null}
                          </button>
                        ))}
                      </div>

                      <div className="organize-group">
                        <div className="organize-group-title">{shell.organizeFilters}</div>
                        <div className="organize-subtitle">{shell.filterProject}</div>
                        {availableProjects.map((projectDir) => (
                          <button
                            key={projectDir}
                            type="button"
                            className={`organize-item ${projectFilters.includes(projectDir) ? "active" : ""}`}
                            onClick={() => toggleProjectFilter(projectDir)}
                          >
                            <span>{getProjectLabel(projectDir)}</span>
                            {projectFilters.includes(projectDir) ? <span className="organize-check">✓</span> : null}
                          </button>
                        ))}
                        <div className="organize-subtitle">{shell.filterTags}</div>
                        <div className="organize-placeholder">{shell.noTagsYet}</div>
                        <div className="organize-subtitle">{shell.filterStatus}</div>
                        <div className="organize-placeholder">{shell.noStatusesYet}</div>
                      </div>
                    </div>
                  )}
                </div>
              </div>

              {activeFilterCount > 0 ? (
                <div className="filter-summary-chip">
                  {shell.filterSummary} {activeFilterCount}
                </div>
              ) : null}

              {bulkSelectionMode ? (
                <div
                  className="bulk-selection-bar"
                  role="toolbar"
                  aria-label={shell.bulkSelectionToolbar}
                >
                  <span className="bulk-selection-count">
                    {shell.selectedCount(selectedConversationCount)}
                  </span>
                  <div className="bulk-selection-actions">
                    <button
                      type="button"
                      className="bulk-selection-action"
                      onClick={handleSelectVisibleConversations}
                      disabled={allVisibleConversationsSelected || filteredConversations.length === 0}
                    >
                      {shell.selectVisible}
                    </button>
                    <button
                      type="button"
                      className="bulk-selection-action"
                      onClick={handleClearConversationSelection}
                      disabled={selectedConversationCount === 0 || bulkDeleting}
                    >
                      {shell.clearSelection}
                    </button>
                    <button
                      type="button"
                      className="bulk-selection-action danger"
                      onClick={() => void handleBulkTrash()}
                      disabled={selectedConversationCount === 0 || bulkDeleting}
                    >
                      {shell.moveSelectedToTrash(selectedConversationCount)}
                    </button>
                  </div>
                </div>
              ) : null}

              {listLoading ? (
                <div className="loading">
                  <div className="spinner"></div>
                </div>
              ) : projectGroups.length === 0 ? (
                <div className="inline-empty-state sidebar-empty">
                  <div className="inline-empty-body">{shell.noProgressBody}</div>
                </div>
              ) : (
                <div className="project-group-list">
                  {projectGroups.map((group) => {
                    const isExpanded = expandedProjects[group.id] ?? true;
                    return (
                      <div key={group.id} className="project-group">
                        <button
                          type="button"
                          className="project-group-header"
                          onClick={() =>
                            setExpandedProjects((current) => ({
                              ...current,
                              [group.id]: !isExpanded,
                            }))
                          }
                        >
                          <div className="project-group-title-wrap">
                            <span className={`project-group-chevron ${isExpanded ? "expanded" : ""}`}>
                              <WindowButtonIcon type="chevron" />
                            </span>
                            <div className="project-group-copy">
                              <span className="project-group-title">{group.label}</span>
                              <span className="project-group-path" title={group.fullPath}>
                                {group.fullPath}
                              </span>
                            </div>
                          </div>
                          <span className="library-count-pill">{group.conversations.length}</span>
                        </button>
                        {isExpanded ? (
                          <div className="project-group-items">
                            {group.conversations.map((conversation) => renderConversationRow(conversation))}
                          </div>
                        ) : null}
                      </div>
                    );
                  })}
                </div>
              )}
            </section>

            {chatConversations.length > 0 ? (
              <section className="library-section chats-section">
                <div className="library-section-header">
                  <div className="library-section-title-row">
                    <h2>{shell.chatSection}</h2>
                    <span className="library-count-pill">{chatConversations.length}</span>
                  </div>
                </div>
                {listLoading ? null : (
                  <div className="chat-list">
                    {chatConversations.map((conversation) => renderConversationRow(conversation))}
                  </div>
                )}
              </section>
            ) : null}
          </div>

          <nav className="sidebar-utility-nav" aria-label={locale === "en" ? "App pages" : "应用页面"}>
            <button
              type="button"
              className={`utility-nav-button ${showTrash ? "active" : ""}`}
              aria-label={shell.trash}
              onClick={() => {
                setShowSettings(false);
                setShowAbout(false);
                setShowTrash(true);
              }}
            >
              <WindowButtonIcon type="trash" />
              <span className="utility-nav-label">{shell.trash}</span>
              {trashedConversations.length > 0 ? (
                <span className="utility-nav-count">{trashedConversations.length}</span>
              ) : null}
            </button>

            <button
              type="button"
              className={`utility-nav-button ${showSettings ? "active" : ""}`}
              aria-label={shell.settings}
              onClick={() => {
                setShowTrash(false);
                setShowAbout(false);
                setShowSettings(true);
              }}
            >
              <WindowButtonIcon type="settings" />
              <span className="utility-nav-label">{shell.settings}</span>
            </button>

            <button
              type="button"
              className={`utility-nav-button ${showAbout ? "active" : ""}`}
              aria-label={shell.aboutChatMem}
              onClick={() => {
                setShowTrash(false);
                setShowSettings(false);
                setShowAbout(true);
              }}
            >
              <WindowButtonIcon type="help" />
              <span className="utility-nav-label">{shell.aboutChatMem}</span>
            </button>
          </nav>
        </aside>

        <main
          className={`workspace ${showSettings ? "settings-workspace" : ""} ${
            showAbout ? "about-workspace" : ""
          } ${showTrash ? "trash-workspace" : ""}`}
        >
          <section className="workspace-surface">
            {showSettings
              ? renderSettingsPanel()
              : showAbout
                ? renderAboutWorkspace()
                : showTrash
                  ? renderTrashWorkspace()
                  : renderWorkspace()}
          </section>
        </main>
      </div>

      {renderMemoryDrawer()}
      {renderTrashConfirmModal()}
      {renderEmptyTrashConfirmModal()}

      {appNotice ? (
        <div className={`app-notice-toast is-${appNotice.kind}`} role="status" aria-live="polite">
          {appNotice.message}
        </div>
      ) : null}

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

      {showMigrateModal && selectedConversation ? (
        <MigrateModal
          sourceAgent={selectedAgent}
          onMigrate={handleMigrate}
          onClose={() => setShowMigrateModal(false)}
        />
      ) : null}

      {handoffComposer ? (
        <HandoffComposerModal
          targetAgent={handoffComposer.targetAgent}
          profileOptions={handoffComposer.profileOptions}
          onClose={() => setHandoffComposer(null)}
          onCreate={handleConfirmCreateHandoff}
        />
      ) : null}

    </div>
  );
}

export default App;
