import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import type { MouseEvent as ReactMouseEvent } from "react";
import { invoke } from "@tauri-apps/api/tauri";
import { appWindow } from "@tauri-apps/api/window";
import ConversationDetail from "./components/ConversationDetail";
import MigrateModal from "./components/MigrateModal";
import SettingsPanel, {
  type SettingsSyncCopy,
  type WebDavSyncResult,
  type WebDavVerificationInput,
} from "./components/SettingsPanel";
import HandoffComposerModal from "./components/HandoffComposerModal";
import LibraryPanel from "./components/LibraryPanel";
import MemoryInboxPanel from "./components/MemoryInboxPanel";
import ProjectIndexStatus from "./components/ProjectIndexStatus";
import RepoMemoryPanel from "./components/RepoMemoryPanel";
import { useI18n } from "./i18n/I18nProvider";
import type { Locale } from "./i18n/types";
import { loadSettings, updateSettings, type AppSettings } from "./settings/storage";
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
  getRepoMemoryHealth,
  listMemoryCandidates,
  listRepoMemories,
  markHandoffConsumed,
  rebuildRepoWiki,
  scanRepoConversations,
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
type TopPage = "continue" | "review" | "history" | "help";
type HistoryView = "conversations" | "recovery" | "transfers" | "outputs";
type MemoryDrawerTab = "inbox" | "approved" | "wiki";
type MigrateMode = "copy" | "cut";
type CopyTarget = "location" | "resume";
type CopyState = {
  target: CopyTarget | null;
  status: "idle" | "success" | "error";
};
type LibraryArrangement = "projects" | "timeline" | "chats-first";
type LibrarySort = "updated" | "created";
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

type ShellCopy = {
  nav: Record<TopPage, string>;
  navAria: string;
  projectSection: string;
  chatSection: string;
  settings: string;
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
    default:
      return "对话";
  }
}

function getAgentLabel(agent: string) {
  return agent.charAt(0).toUpperCase() + agent.slice(1);
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

function getWikiSourceLabel(page: WikiPage, locale: Locale) {
  const memoryCount = page.source_memory_ids.length;
  const episodeCount = page.source_episode_ids.length;
  const parts: string[] = [];

  if (memoryCount > 0) {
    parts.push(
      locale === "en"
        ? `${memoryCount} memory source${memoryCount === 1 ? "" : "s"}`
        : `${memoryCount} \u6761\u8bb0\u5fc6\u6765\u6e90`,
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
      restoreProjects: "Restore project expansion",
      openOrganizer: "Organize lists",
      refreshList: "Refresh conversations",
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
    chatSection: "聊天",
    settings: "设置",
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
    collapseProjects: "折叠全部项目",
    restoreProjects: "恢复上次展开",
    openOrganizer: "整理对话",
    refreshList: "刷新会话列表",
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

function WindowButtonIcon({
  type,
}: {
  type: "minimize" | "maximize" | "close" | "collapse" | "organize" | "chevron";
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

  if (type === "collapse") {
    return (
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <path d="M6 4L2.5 7.5 6 11" />
        <path d="M10 4l3.5 3.5L10 11" />
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
  const [memoryDrawerOpen, setMemoryDrawerOpen] = useState(false);
  const [memoryDrawerTab, setMemoryDrawerTab] = useState<MemoryDrawerTab>("inbox");
  const [listLoading, setListLoading] = useState(false);
  const [detailLoading, setDetailLoading] = useState(false);
  const [memoryLoading, setMemoryLoading] = useState(false);
  const [repoMemoryHealth, setRepoMemoryHealth] = useState<RepoMemoryHealth | null>(null);
  const [repoHealthLoading, setRepoHealthLoading] = useState(false);
  const [repoScanRunning, setRepoScanRunning] = useState(false);
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
  const [episodes] = useState<EpisodeRecord[]>([]);
  const [runs] = useState<RunRecord[]>([]);
  const [artifacts] = useState<ArtifactRecord[]>([]);
  const [checkpoints, setCheckpoints] = useState<CheckpointRecord[]>([]);
  const [handoffs, setHandoffs] = useState<HandoffPacket[]>([]);
  const [handoffComposer, setHandoffComposer] = useState<HandoffComposerState>(null);
  const [showOrganizeMenu, setShowOrganizeMenu] = useState(false);
  const [libraryArrangement, setLibraryArrangement] = useState<LibraryArrangement>("projects");
  const [librarySort, setLibrarySort] = useState<LibrarySort>("updated");
  const [projectFilters, setProjectFilters] = useState<string[]>([]);
  const [expandedProjects, setExpandedProjects] = useState<Record<string, boolean>>({});
  const [collapsedSnapshot, setCollapsedSnapshot] = useState<Record<string, boolean> | null>(null);
  const [isWindowFilled, setIsWindowFilled] = useState(false);
  const organizeMenuRef = useRef<HTMLDivElement | null>(null);
  const activeRepoRoot = selectedConversation?.project_dir ?? null;
  const activeRepoRootRef = useRef<string | null>(activeRepoRoot);
  const repoScanRequestIdRef = useRef(0);
  const repoScanActiveCountRef = useRef(0);
  const availableHandoffTargets = ["claude", "codex", "gemini"].filter(
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
    setCopyState({ target: null, status: "idle" });
    setActivePage("continue");
    setHistoryView("conversations");
  }, [selectedAgent]);

  useEffect(() => {
    void loadConversations(searchQuery, selectedAgent);
  }, [searchQuery, selectedAgent]);

  useEffect(() => {
    setCopyState({ target: null, status: "idle" });
  }, [selectedConversation?.id]);

  useEffect(() => {
    activeRepoRootRef.current = activeRepoRoot;
    if (!activeRepoRoot) {
      repoScanRequestIdRef.current += 1;
      repoScanActiveCountRef.current = 0;
      setRepoScanRunning(false);
    }
  }, [activeRepoRoot]);

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
      setRepoMemoryHealth(null);
      setRepoHealthLoading(false);
      setMemoryDrawerOpen(false);
      return;
    }

    let cancelled = false;

    const loadProjectMemory = async () => {
      setMemoryLoading(true);
      setRepoHealthLoading(true);
      const requestRepoRoot = activeRepoRoot;
      try {
        const [nextMemories, nextCandidates, nextWikiPages] = await Promise.all([
          listRepoMemories(activeRepoRoot),
          listMemoryCandidates(activeRepoRoot, "pending_review"),
          rebuildRepoWiki(activeRepoRoot),
        ]);
        if (cancelled || activeRepoRootRef.current !== requestRepoRoot) {
          return;
        }
        setRepoMemories(nextMemories);
        setMemoryCandidates(nextCandidates);
        setWikiPages(nextWikiPages);
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
  }, [activeRepoRoot]);

  const handleScanRepoConversations = async () => {
    if (!activeRepoRoot) {
      return;
    }
    const requestRepoRoot = activeRepoRoot;
    const requestId = ++repoScanRequestIdRef.current;
    repoScanActiveCountRef.current += 1;
    setRepoScanRunning(true);
    try {
      await scanRepoConversations(requestRepoRoot);
      const nextHealth = await getRepoMemoryHealth(requestRepoRoot);
      if (
        activeRepoRootRef.current === requestRepoRoot &&
        requestId === repoScanRequestIdRef.current
      ) {
        setRepoMemoryHealth(nextHealth);
      }
    } catch (error) {
      console.error("Failed to scan repo conversations:", error);
    } finally {
      repoScanActiveCountRef.current = Math.max(0, repoScanActiveCountRef.current - 1);
      setRepoScanRunning(repoScanActiveCountRef.current > 0);
    }
  };

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

  const loadConversationDetail = async (id: string, agent = selectedAgent) => {
    setDetailLoading(true);
    try {
      const result = await invoke<Conversation>("read_conversation", {
        agent,
        id,
      });
      setSelectedConversation(normalizeConversationProject(result));
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
      const [nextMemories, nextWikiPages] = await Promise.all([
        listRepoMemories(activeRepoRoot),
        rebuildRepoWiki(activeRepoRoot),
      ]);
      setRepoMemories(nextMemories);
      setWikiPages(nextWikiPages);
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

  const sortedConversations = useMemo(
    () => sortConversations(conversations, librarySort),
    [conversations, librarySort],
  );

  const availableProjects = useMemo(
    () => {
      const projects = new Map<string, string>();
      sortedConversations.forEach((conversation) => {
        const projectDir = normalizeProjectPath(conversation.project_dir);
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
    return sortedConversations.filter((conversation) =>
      filterKeys.has(projectPathKey(conversation.project_dir)),
    );
  }, [projectFilters, sortedConversations]);

  const projectConversations = useMemo(
    () => filteredConversations.filter((conversation) => normalizeProjectPath(conversation.project_dir)),
    [filteredConversations],
  );

  const chatConversations = useMemo(
    () => filteredConversations.filter((conversation) => !normalizeProjectPath(conversation.project_dir)),
    [filteredConversations],
  );

  const repoLibraryRecords = useMemo(() => {
    if (!activeRepoRoot) {
      return [];
    }

    return buildRepoLibraryRecords({
      conversations: sortedConversations.filter(
        (conversation) => projectPathKey(conversation.project_dir) === projectPathKey(activeRepoRoot),
      ),
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
      const projectDir = normalizeProjectPath(conversation.project_dir);
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

  const renderConversationRow = (
    conversation: ConversationSummary,
    extraClassName = "",
  ) => {
    const title = normalizeConversationTitle(conversation.summary) || conversation.id;
    const visibleTitle = truncateSidebarTitle(title);
    const isSelected = selectedConversation?.id === conversation.id;

    return (
      <button
        key={`${conversation.project_dir}-${conversation.id}`}
        type="button"
        className={`conversation-item ${isSelected ? "selected" : ""} ${extraClassName}`.trim()}
        onClick={() => void loadConversationDetail(conversation.id)}
      >
        <div className="conversation-item-row">
          <div className="conversation-item-main">
            <div className="conversation-item-title" title={title}>
              {visibleTitle}
            </div>
            <div className="conversation-item-path" title={conversation.project_dir}>
              {conversation.project_dir}
            </div>
          </div>
          <div className="conversation-item-time">{formatDistanceToNow(conversation.updated_at)}</div>
        </div>
      </button>
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
              <span>{conversation.project_dir}</span>
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
            <div className="empty-state-icon">○</div>
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
              <span className="meta-value">~/.codex/config.toml</span>
            </div>
            <div className="meta-block">
              <span className="meta-label">{shell.relatedPaths}</span>
              <span className="meta-value">
                {selectedConversation?.project_dir || "--"}
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

    const memoryTitle = locale === "en" ? "Project Memory" : "\u9879\u76ee\u8bb0\u5fc6";
    const drawerSubtitle =
      locale === "en"
        ? "Review only what needs attention, then tuck the rest away."
        : "\u53ea\u5904\u7406\u9700\u8981\u5173\u6ce8\u7684\u4e8b\uff0c\u5176\u4f59\u8bb0\u5fc6\u6536\u8d77\u6765\u3002";
    const wikiSubtitle =
      locale === "en"
        ? "Readable pages rebuilt from approved memory and episodes."
        : "\u7531\u5df2\u6279\u51c6\u8bb0\u5fc6\u548c\u9636\u6bb5\u8bb0\u5f55\u91cd\u5efa\u7684\u53ef\u8bfb\u9875\u9762\u3002";
    const emptyWiki =
      locale === "en"
        ? "No wiki projection has been generated yet."
        : "\u8fd8\u6ca1\u6709\u751f\u6210 Wiki \u6295\u5f71\u3002";
    const tabs: Array<{ id: MemoryDrawerTab; label: string; count: number }> = [
      {
        id: "inbox",
        label: locale === "en" ? "Inbox" : "\u6536\u4ef6\u7bb1",
        count: memoryCandidates.length,
      },
      {
        id: "approved",
        label: locale === "en" ? "Approved" : "\u5df2\u6279\u51c6",
        count: repoMemories.length,
      },
      { id: "wiki", label: locale === "en" ? "Wiki" : "Wiki", count: wikiPages.length },
    ];

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
        ) : (
          <div className="memory-card-list">
            {wikiPages.slice(0, 20).map((page) => (
              <article key={page.page_id} className="memory-card memory-card-projection">
                <div className="memory-card-header">
                  <div>
                    <strong>{page.title}</strong>
                    <div className="memory-card-kind">{page.status}</div>
                  </div>
                </div>
                <p>{getWikiPreview(page.body)}</p>
                <span>{getWikiSourceLabel(page, locale)}</span>
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
          />
        );
      }

      if (memoryDrawerTab === "wiki") {
        return renderWikiTab();
      }

      return (
        <MemoryInboxPanel
          candidates={memoryCandidates}
          loading={memoryLoading}
          locale={locale}
          onApprove={(candidate) => void handleApproveCandidate(candidate)}
          onApproveMerge={(candidate) => void handleApproveMergeCandidate(candidate)}
          onReject={(candidateId) => void handleRejectCandidate(candidateId)}
        />
      );
    };

    return (
      <div className="memory-drawer-overlay" onMouseDown={() => setMemoryDrawerOpen(false)}>
        <aside
          className="memory-drawer"
          role="complementary"
          aria-label={memoryTitle}
          onMouseDown={(event) => event.stopPropagation()}
        >
          <header className="memory-drawer-header">
            <div>
              <p className="page-eyebrow">{locale === "en" ? "Repository context" : "\u4ed3\u5e93\u4e0a\u4e0b\u6587"}</p>
              <h2>{memoryTitle}</h2>
              <span>{drawerSubtitle}</span>
            </div>
            <button
              type="button"
              className="icon-button"
              aria-label={locale === "en" ? "Close memory drawer" : "\u5173\u95ed\u8bb0\u5fc6\u62bd\u5c49"}
              onClick={() => setMemoryDrawerOpen(false)}
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
                onClick={() => setMemoryDrawerTab(tab.id)}
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

  const renderWorkspace = () => {
    if (!selectedConversation) {
      return (
        <div className="conversation-empty-state">
          <div className="empty-state-icon">○</div>
          <h1>{shell.chooseConversation}</h1>
          <div className="empty-state-text">{shell.noProgressBody}</div>
        </div>
      );
    }

    const conversationTitle =
      normalizeConversationTitle(selectedConversation.summary) || selectedConversation.id;
    const visibleConversationTitle = truncateWorkspaceTitle(conversationTitle);
    const memoryAttentionCount = memoryCandidates.length;
    const memoryButtonLabel = locale === "en" ? "Memory" : "\u8bb0\u5fc6";

    return (
      <div className="conversation-workspace">
        <header className="conversation-toolbar">
          <div className="conversation-title-block">
            <p className="page-eyebrow">{getAgentHeading(selectedAgent, locale)}</p>
            <h1 title={conversationTitle}>{visibleConversationTitle}</h1>
            <span title={selectedConversation.project_dir}>{selectedConversation.project_dir}</span>
          </div>
          <div className="conversation-toolbar-actions">
            <button
              type="button"
              className={`btn btn-secondary memory-drawer-trigger ${
                memoryAttentionCount > 0 ? "has-memory-alert" : ""
              }`}
              onClick={() => {
                setMemoryDrawerTab(memoryAttentionCount > 0 ? "inbox" : "approved");
                setMemoryDrawerOpen(true);
              }}
            >
              <span>{memoryButtonLabel}</span>
              {memoryAttentionCount > 0 ? (
                <span className="memory-drawer-trigger-badge">{memoryAttentionCount}</span>
              ) : null}
            </button>
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

        {activeRepoRoot ? (
          <ProjectIndexStatus
            health={repoMemoryHealth}
            loading={repoHealthLoading}
            scanning={repoScanRunning}
            locale={locale}
            onScan={() => void handleScanRepoConversations()}
          />
        ) : null}

        <div className="conversation-content-grid">
          <ConversationDetail conversation={selectedConversation} />
        </div>
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

  return (
    <div className={`app-shell ${isWindowFilled ? "is-window-filled" : ""}`}>
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

      <div className={`app-body ${libraryArrangement === "chats-first" ? "chats-first" : ""}`}>
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
                    className="icon-button"
                    aria-label={allProjectsCollapsed ? shell.restoreProjects : shell.collapseProjects}
                    onClick={handleToggleCollapseProjects}
                  >
                    <WindowButtonIcon type="collapse" />
                  </button>
                  <button
                    type="button"
                    className="icon-button"
                    aria-label={shell.openOrganizer}
                    onClick={() => setShowOrganizeMenu((current) => !current)}
                  >
                    <WindowButtonIcon type="organize" />
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

          <button type="button" className="settings-row" onClick={() => setShowSettings(true)}>
            {shell.settings}
          </button>
        </aside>

        <main className="workspace">
          <section className="workspace-surface">{renderWorkspace()}</section>
        </main>
      </div>

      {renderMemoryDrawer()}

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
        syncSettings={appSettings.sync}
        syncCopy={syncCopy}
        onClose={() => setShowSettings(false)}
        onLocaleChange={(nextLocale: Locale) => {
          setLocale(nextLocale);
          const nextSettings = { ...appSettings, locale: nextLocale };
          setAppSettings(nextSettings);
        }}
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
