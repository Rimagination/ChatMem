import { useEffect, useState, type ReactNode } from "react";
import type { Locale } from "../i18n/types";
import { open as openDialog } from "@tauri-apps/api/dialog";
import {
  APP_FONT_OPTIONS,
  type AppFontFamily,
  type DownloadMode,
  type SyncProvider,
  type SyncSettings,
} from "../settings/storage";
import type { UpdateState } from "../updater/updater";

export type SettingsSyncCopy = {
  title: string;
  methodLabel: string;
  webdavLabel: string;
  protocolLabel: string;
  serverPathLabel: string;
  usernameLabel: string;
  passwordLabel: string;
  showPasswordLabel: string;
  hidePasswordLabel: string;
  downloadFilesLabel: string;
  onSyncDownloadLabel: string;
  asNeededDownloadLabel: string;
  verifyServerLabel: string;
  verifyingServerLabel: string;
  verifySuccessLabel: string;
  verifyMissingFieldsLabel: string;
  verifyFailedPrefix: string;
  syncNowLabel: string;
  syncingNowLabel: string;
  syncSuccessPrefix: string;
  syncSuccessSuffix: string;
  syncTargetLabel: string;
  syncFailedPrefix: string;
};

export type WebDavVerificationInput = {
  syncSettings: SyncSettings;
  password: string;
};

export type WebDavSyncResult = {
  uploadedCount: number;
  remoteUrl: string;
};

export type LocalSyncStatusResult = {
  available: boolean;
  folder_path: string;
  remote_conversation_count: number;
  last_sync_info: string | null;
};

export type LocalSyncResult = {
  uploaded: number;
  downloaded: number;
  skipped: number;
  conflicts_resolved: number;
  folder_path: string;
};

export type UpgradeReadinessCheck = {
  key: string;
  label: string;
  status: "ok" | "warning" | "error" | string;
  detail: string;
};

export type UpgradeReadinessReport = {
  status: "ok" | "warning" | "error" | string;
  summary: string;
  checks: UpgradeReadinessCheck[];
  warnings: string[];
};

export type AgentIntegrationStatus = {
  agent: string;
  label: string;
  configPath: string;
  instructionsPath: string;
  mcpInstalled: boolean;
  instructionsInstalled: boolean;
  instructionsOutdated: boolean;
  configExists: boolean;
  status: "ready" | "partial" | "not_installed" | string;
  statusLabel: string;
  commandPreview: string;
  details: string[];
};

export type AgentIntegrationOperationResult = {
  agent: string;
  label: string;
  changed: boolean;
  message: string;
  backupPaths: string[];
  status: AgentIntegrationStatus;
};

type SettingsPanelProps = {
  open: boolean;
  title: string;
  closeLabel: string;
  languageLabel: string;
  locale: Locale;
  fontFamily: AppFontFamily;
  autoCheckUpdates: boolean;
  autoCaptureMemory: boolean;
  autoCheckLabel: string;
  autoCaptureLabel: string;
  autoCaptureHint: string;
  checkUpdatesLabel: string;
  checkingLabel: string;
  upToDateLabel: string;
  updateAvailablePrefix: string;
  installUpdateLabel: string;
  installingLabel: string;
  updateState: UpdateState;
  syncSettings: SyncSettings;
  syncCopy: SettingsSyncCopy;
  onClose: () => void;
  onLocaleChange: (locale: Locale) => void;
  onFontFamilyChange: (fontFamily: AppFontFamily) => void;
  onAutoCheckChange: (nextValue: boolean) => void;
  onAutoCaptureChange: (nextValue: boolean) => void;
  onSyncSettingsChange: (patch: Partial<SyncSettings>) => void;
  onVerifyWebDavServer: (input: WebDavVerificationInput) => Promise<void>;
  onSyncWebDavNow: (input: WebDavVerificationInput) => Promise<WebDavSyncResult>;
  onRunUpgradeReadinessCheck: () => Promise<UpgradeReadinessReport>;
  onDetectAgentIntegrations: () => Promise<AgentIntegrationStatus[]>;
  onInstallAgentIntegration: (agent: string) => Promise<AgentIntegrationOperationResult[]>;
  onUninstallAgentIntegration: (agent: string) => Promise<AgentIntegrationOperationResult[]>;
  onLoadWebDavPassword: (username: string) => Promise<string | null>;
  onSaveWebDavPassword: (input: { username: string; password: string }) => Promise<void>;
  onLocalSyncStatus: () => Promise<LocalSyncStatusResult>;
  onSyncLocalNow: () => Promise<LocalSyncResult>;
  autoBackupEnabled: boolean;
  autoBackupIntervalMinutes: number;
  onAutoBackupEnabledChange: (enabled: boolean) => void;
  onAutoBackupIntervalChange: (minutes: number) => void;
  onCheckUpdates: () => void;
  onInstallUpdate: () => void;
  aboutContent?: ReactNode;
};

type SettingsSectionId = "general" | "sync" | "integrations" | "updates" | "about";

type WebDavVerificationState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "success" }
  | { kind: "error"; message: string };

type WebDavSyncState =
  | { kind: "idle" }
  | { kind: "syncing" }
  | { kind: "success"; uploadedCount: number; remoteUrl: string }
  | { kind: "error"; message: string };

type LocalSyncState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "syncing" }
  | { kind: "success"; uploaded: number; downloaded: number; folderPath: string }
  | { kind: "error"; message: string };

type UpgradeCheckState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "success"; report: UpgradeReadinessReport }
  | { kind: "error"; message: string };

type IntegrationState =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "working"; agent: string }
  | { kind: "success"; message: string }
  | { kind: "error"; message: string };

function joinServerPath(syncSettings: SyncSettings) {
  return [syncSettings.webdavHost, syncSettings.webdavPath]
    .filter(Boolean)
    .join("/")
    .replace(/^\/+/, "");
}

function splitServerPath(value: string) {
  const normalized = value.trim().replace(/^https?:\/\//, "").replace(/^\/+/, "");
  const [host = "", ...pathParts] = normalized.split("/");
  return {
    webdavHost: host,
    webdavPath: pathParts.join("/").replace(/\/+$/g, ""),
  };
}

export default function SettingsPanel({
  open,
  title,
  closeLabel,
  languageLabel,
  locale,
  fontFamily,
  autoCheckUpdates,
  autoCaptureMemory,
  autoCheckLabel,
  autoCaptureLabel,
  autoCaptureHint,
  checkUpdatesLabel,
  checkingLabel,
  upToDateLabel,
  updateAvailablePrefix,
  installUpdateLabel,
  installingLabel,
  updateState,
  syncSettings,
  syncCopy,
  onClose,
  onLocaleChange,
  onFontFamilyChange,
  onAutoCheckChange,
  onAutoCaptureChange,
  onSyncSettingsChange,
  onVerifyWebDavServer,
  onSyncWebDavNow,
  onRunUpgradeReadinessCheck,
  onDetectAgentIntegrations,
  onInstallAgentIntegration,
  onUninstallAgentIntegration,
  onLoadWebDavPassword,
  onSaveWebDavPassword,
  onLocalSyncStatus,
  onSyncLocalNow,
  autoBackupEnabled,
  autoBackupIntervalMinutes,
  onAutoBackupEnabledChange,
  onAutoBackupIntervalChange,
  onCheckUpdates,
  onInstallUpdate,
  aboutContent,
}: SettingsPanelProps) {
  const [activeSettingsSection, setActiveSettingsSection] = useState<SettingsSectionId>("general");
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [webDavVerification, setWebDavVerification] = useState<WebDavVerificationState>({
    kind: "idle",
  });
  const [webDavSync, setWebDavSync] = useState<WebDavSyncState>({
    kind: "idle",
  });
  const [upgradeCheck, setUpgradeCheck] = useState<UpgradeCheckState>({
    kind: "idle",
  });
  const [agentIntegrations, setAgentIntegrations] = useState<AgentIntegrationStatus[]>([]);
  const [integrationState, setIntegrationState] = useState<IntegrationState>({
    kind: "idle",
  });
  const [localSyncStatus, setLocalSyncStatus] = useState<LocalSyncStatusResult | null>(null);
  const [localSyncState, setLocalSyncState] = useState<LocalSyncState>({
    kind: "idle",
  });
  const [agentIntegrationsLoaded, setAgentIntegrationsLoaded] = useState(false);
  const isEnglish = locale === "en";
  const isWebDav = syncSettings.provider === "webdav";
  const isOneDrive = syncSettings.provider === "onedrive";
  const canVerifyWebDav =
    isWebDav &&
    syncSettings.webdavHost.trim().length > 0 &&
    syncSettings.username.trim().length > 0 &&
    password.trim().length > 0;

  const handleSyncSettingsChange = (patch: Partial<SyncSettings>) => {
    setWebDavVerification({ kind: "idle" });
    setWebDavSync({ kind: "idle" });
    onSyncSettingsChange(patch);
  };

  const handleUsernameChange = (nextUsername: string) => {
    setPassword("");
    handleSyncSettingsChange({ username: nextUsername });
  };

  const handlePasswordChange = (nextPassword: string) => {
    setWebDavVerification({ kind: "idle" });
    setWebDavSync({ kind: "idle" });
    setPassword(nextPassword);
  };

  const handleVerifyWebDavServer = async () => {
    if (!canVerifyWebDav) {
      setWebDavVerification({ kind: "error", message: syncCopy.verifyMissingFieldsLabel });
      return;
    }

    setWebDavVerification({ kind: "checking" });
    try {
      await onVerifyWebDavServer({ syncSettings, password });
      await onSaveWebDavPassword({ username: syncSettings.username, password });
      setWebDavVerification({ kind: "success" });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setWebDavVerification({
        kind: "error",
        message: `${syncCopy.verifyFailedPrefix}: ${message}`,
      });
    }
  };

  const handleSyncWebDavNow = async () => {
    if (!canVerifyWebDav) {
      setWebDavSync({ kind: "error", message: syncCopy.verifyMissingFieldsLabel });
      return;
    }

    setWebDavSync({ kind: "syncing" });
    try {
      const result = await onSyncWebDavNow({ syncSettings, password });
      await onSaveWebDavPassword({ username: syncSettings.username, password });
      setWebDavSync({
        kind: "success",
        uploadedCount: result.uploadedCount,
        remoteUrl: result.remoteUrl,
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setWebDavSync({
        kind: "error",
        message: `${syncCopy.syncFailedPrefix}: ${message}`,
      });
    }
  };

  const handleSyncLocalNow = async () => {
    setLocalSyncState({ kind: "syncing" });
    try {
      const result = await onSyncLocalNow();
      setLocalSyncState({
        kind: "success",
        uploaded: result.uploaded,
        downloaded: result.downloaded,
        folderPath: result.folder_path,
      });
      // Refresh status after sync
      const status = await onLocalSyncStatus();
      setLocalSyncStatus(status);
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setLocalSyncState({ kind: "error", message });
    }
  };

  const handleRunUpgradeReadinessCheck = async () => {
    setUpgradeCheck({ kind: "checking" });
    try {
      const report = await onRunUpgradeReadinessCheck();
      setUpgradeCheck({ kind: "success", report });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setUpgradeCheck({ kind: "error", message });
    }
  };

  const refreshAgentIntegrations = async () => {
    setIntegrationState({ kind: "loading" });
    try {
      const integrations = await onDetectAgentIntegrations();
      setAgentIntegrations(integrations);
      setIntegrationState({ kind: "idle" });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setIntegrationState({ kind: "error", message });
    }
  };

  const handleInstallAgentIntegration = async (agent: string) => {
    setIntegrationState({ kind: "working", agent });
    try {
      const results = await onInstallAgentIntegration(agent);
      setAgentIntegrations((current) => {
        const statusByAgent = new Map(results.map((result) => [result.agent, result.status]));
        return current.map((item) => statusByAgent.get(item.agent) ?? item);
      });
      const message =
        agent === "all"
          ? isEnglish
            ? "All detected agents are set up."
            : "已完成全部可用 Agent 设置。"
          : results[0]?.message ?? (isEnglish ? "Integration updated." : "集成已更新。");
      setIntegrationState({ kind: "success", message });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setIntegrationState({ kind: "error", message });
    }
  };

  const handleUninstallAgentIntegration = async (agent: string) => {
    setIntegrationState({ kind: "working", agent });
    try {
      const results = await onUninstallAgentIntegration(agent);
      setAgentIntegrations((current) => {
        const statusByAgent = new Map(results.map((result) => [result.agent, result.status]));
        return current.map((item) => statusByAgent.get(item.agent) ?? item);
      });
      setIntegrationState({
        kind: "success",
        message: results[0]?.message ?? (isEnglish ? "Integration removed." : "集成已卸载。"),
      });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setIntegrationState({ kind: "error", message });
    }
  };

  const upgradeCopy = {
    title: isEnglish ? "Local data check" : "本地数据检查",
    helper: isEnglish
      ? "Confirm local data is available after updates."
      : "确认更新后本地数据可用。",
    run: isEnglish ? "Check data" : "检查数据",
    checking: isEnglish ? "Checking..." : "\u6b63\u5728\u68c0\u67e5...",
    failed: isEnglish ? "Data check failed" : "数据检查失败",
  };

  const generalCopy = {
    title: isEnglish ? "General" : "\u901a\u7528",
    helper: isEnglish
      ? "Choose the interface language and typeface."
      : "设置界面语言和字体。",
    fontLabel: isEnglish ? "Typeface" : "字体",
    fontHint: isEnglish
      ? "Open-source commercial-safe families are used first when installed, then system fonts."
      : "优先使用本机已安装的可商用开源字体，未安装时回退到系统字体。",
  };

  const updateCopy = {
    title: isEnglish ? "Updates" : "更新",
    helper: isEnglish
      ? "Keep ChatMem up to date."
      : "保持 ChatMem 为最新版本。",
  };

  const integrationCopy = {
    title: isEnglish ? "Agent integration" : "Agent 集成",
    helper: isEnglish
      ? "Connect supported agents so they can use ChatMem when continuing a project."
      : "连接支持的 Agent，让它们继续项目时能使用 ChatMem。",
    installAll: isEnglish ? "Set up all" : "全部设置",
    rescan: isEnglish ? "Check again" : "重新检查",
    loading: isEnglish ? "Checking agents..." : "正在检查 Agent...",
    install: isEnglish ? "Install" : "安装",
    repair: isEnglish ? "Update" : "更新",
    uninstall: isEnglish ? "Uninstall" : "卸载",
    mcp: "MCP",
    instructions: isEnglish ? "Instructions" : "说明",
    outdated: isEnglish ? "Needs update" : "需要更新",
    config: isEnglish ? "Settings file" : "设置文件",
    guidance: isEnglish ? "Instructions file" : "说明文件",
    notDetected: isEnglish
      ? "No supported agents found."
      : "未发现可用的 Agent。",
  };

  const navCopy = {
    general: {
      label: generalCopy.title,
      helper: isEnglish ? "Language, typeface, and capture defaults." : "语言、字体和捕获默认项。",
    },
    sync: {
      label: syncCopy.title,
      helper: isEnglish ? "WebDAV, OneDrive, and download behavior." : "WebDAV、OneDrive 和下载方式。",
    },
    integrations: {
      label: integrationCopy.title,
      helper: isEnglish ? "Supported agents and setup status." : "支持的 Agent 和连接状态。",
    },
    updates: {
      label: updateCopy.title,
      helper: isEnglish ? "Version checks and local data status." : "检查更新和本地数据状态。",
    },
    about: {
      label: isEnglish ? "About" : "关于",
      helper: isEnglish ? "Version and project information." : "版本和项目信息。",
    },
  };

  const settingsSections = [
    { id: "general" as const, ...navCopy.general },
    { id: "sync" as const, ...navCopy.sync },
    { id: "integrations" as const, ...navCopy.integrations },
    { id: "updates" as const, ...navCopy.updates },
    ...(aboutContent ? [{ id: "about" as const, ...navCopy.about }] : []),
  ];

  const syncProviderOptions: Array<{ value: SyncProvider; label: string }> = [
    { value: "off", label: isEnglish ? "Off" : "\u5173\u95ed" },
    { value: "webdav", label: "WebDAV" },
    { value: "onedrive", label: "OneDrive" },
  ];

  const downloadModeOptions: Array<{ value: DownloadMode; label: string }> = [
    { value: "on-sync", label: syncCopy.onSyncDownloadLabel },
    { value: "as-needed", label: syncCopy.asNeededDownloadLabel },
  ];

  useEffect(() => {
    if (open) {
      setActiveSettingsSection("general");
    }
  }, [open]);

  useEffect(() => {
    if (!open || activeSettingsSection !== "integrations" || agentIntegrationsLoaded) {
      return;
    }

    let cancelled = false;
    setIntegrationState({ kind: "loading" });
    void onDetectAgentIntegrations()
      .then((integrations) => {
        if (cancelled) {
          return;
        }
        setAgentIntegrations(integrations);
        setAgentIntegrationsLoaded(true);
        setIntegrationState({ kind: "idle" });
      })
      .catch((error) => {
        if (cancelled) {
          return;
        }
        const message = error instanceof Error ? error.message : String(error);
        setAgentIntegrationsLoaded(true);
        setIntegrationState({ kind: "error", message });
      });

    return () => {
      cancelled = true;
    };
  }, [activeSettingsSection, agentIntegrationsLoaded, onDetectAgentIntegrations, open]);

  useEffect(() => {
    if (!open || activeSettingsSection !== "sync" || !isWebDav || !syncSettings.username.trim()) {
      return;
    }

    let cancelled = false;
    void onLoadWebDavPassword(syncSettings.username).then((savedPassword) => {
      if (cancelled || !savedPassword) {
        return;
      }
      setPassword((currentPassword) => currentPassword || savedPassword);
    });

    return () => {
      cancelled = true;
    };
  }, [activeSettingsSection, isWebDav, onLoadWebDavPassword, open, syncSettings.username]);

  useEffect(() => {
    if (!open || activeSettingsSection !== "sync" || !isOneDrive) {
      return;
    }

    let cancelled = false;
    setLocalSyncState({ kind: "loading" });
    void onLocalSyncStatus().then((status) => {
      if (cancelled) {
        return;
      }
      setLocalSyncStatus(status);
      setLocalSyncState({ kind: "idle" });
    }).catch(() => {
      if (cancelled) {
        return;
      }
      setLocalSyncState({ kind: "idle" });
    });

    return () => {
      cancelled = true;
    };
  }, [activeSettingsSection, isOneDrive, onLocalSyncStatus, open]);

  if (!open) {
    return null;
  }

  return (
    <section
      className="settings-panel settings-page"
      role="region"
      aria-labelledby="settings-title"
    >
        <div className="settings-panel-header">
          <h3 id="settings-title">{title}</h3>
          <button type="button" className="toolbar-button" onClick={onClose}>
            {closeLabel}
          </button>
        </div>

        <div className="settings-layout">
          <nav className="settings-sidebar-nav" aria-label={isEnglish ? "Settings sections" : "设置分区"}>
            {settingsSections.map((section) => {
              const isActive = section.id === activeSettingsSection;
              return (
                <button
                  key={section.id}
                  type="button"
                  className={`settings-nav-item ${isActive ? "active" : ""}`}
                  aria-label={section.label}
                  aria-current={isActive ? "page" : undefined}
                  aria-controls={`settings-section-${section.id}`}
                  onClick={() => setActiveSettingsSection(section.id)}
                >
                  <span className="settings-nav-label">{section.label}</span>
                  <span className="settings-nav-helper" aria-hidden="true">{section.helper}</span>
                </button>
              );
            })}
          </nav>

          <div className="settings-detail" id={`settings-section-${activeSettingsSection}`}>
            {activeSettingsSection === "general" ? (

        <div className="settings-compact-grid" aria-label={isEnglish ? "Compact settings" : "常用设置"}>
          <section className="settings-section general-settings-section" aria-labelledby="settings-general-title">
            <div>
              <h4 id="settings-general-title">{generalCopy.title}</h4>
              <p className="settings-helper">{generalCopy.helper}</p>
            </div>

            <div className="settings-field-grid">
              <label className="settings-field">
                <span className="settings-label">{languageLabel}</span>
                <select
                  className="settings-select"
                  value={locale}
                  onChange={(event) => onLocaleChange(event.target.value as Locale)}
                >
                  <option value="zh-CN">简体中文</option>
                  <option value="en">English</option>
                </select>
              </label>

              <label className="settings-field">
                <span className="settings-label">{generalCopy.fontLabel}</span>
                <select
                  className="settings-select"
                  value={fontFamily}
                  onChange={(event) => onFontFamilyChange(event.target.value as AppFontFamily)}
                >
                  {APP_FONT_OPTIONS.map((option) => (
                    <option key={option.id} value={option.id}>
                      {option.label[locale]}
                    </option>
                  ))}
                </select>
              </label>
            </div>
            <label className="settings-toggle-row memory-autocapture-toggle">
              <div className="settings-toggle-copy">
                <span className="settings-label">{autoCaptureLabel}</span>
                <span className="settings-helper">{autoCaptureHint}</span>
              </div>
              <input
                type="checkbox"
                checked={autoCaptureMemory}
                onChange={(event) => onAutoCaptureChange(event.target.checked)}
              />
            </label>
            <p className="settings-helper settings-field-hint">{generalCopy.fontHint}</p>
          </section>
        </div>
            ) : null}

            {activeSettingsSection === "integrations" ? (
        <section
          className="settings-section agent-integration-section"
          aria-labelledby="settings-agent-integration-title"
        >
          <div className="settings-section-heading agent-integration-heading">
            <div>
              <h4 id="settings-agent-integration-title">{integrationCopy.title}</h4>
              <p className="settings-helper">{integrationCopy.helper}</p>
            </div>
            <div className="agent-integration-actions">
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => void refreshAgentIntegrations()}
                disabled={integrationState.kind === "loading" || integrationState.kind === "working"}
              >
                {integrationCopy.rescan}
              </button>
              <button
                type="button"
                className="btn btn-primary"
                onClick={() => void handleInstallAgentIntegration("all")}
                disabled={integrationState.kind === "loading" || integrationState.kind === "working"}
              >
                {integrationState.kind === "working" && integrationState.agent === "all"
                  ? isEnglish
                    ? "Installing..."
                    : "正在安装..."
                  : integrationCopy.installAll}
              </button>
            </div>
          </div>

          {integrationState.kind === "loading" ? (
            <p className="settings-notice">{integrationCopy.loading}</p>
          ) : null}

          {integrationState.kind === "success" ? (
            <p className="settings-notice is-success">{integrationState.message}</p>
          ) : null}

          {integrationState.kind === "error" ? (
            <p className="settings-notice is-danger">{integrationState.message}</p>
          ) : null}

          {agentIntegrations.length > 0 ? (
            <div className="agent-integration-grid">
              {agentIntegrations.map((integration) => {
                const isWorking =
                  integrationState.kind === "working" &&
                  (integrationState.agent === "all" || integrationState.agent === integration.agent);
                const hasAnyInstall =
                  integration.mcpInstalled || integration.instructionsInstalled;
                const statusLabel =
                  integration.status === "ready"
                    ? isEnglish
                      ? "Ready"
                      : "已就绪"
                    : integration.status === "partial"
                      ? isEnglish
                        ? "Needs update"
                        : "需要更新"
                      : isEnglish
                        ? "Not set up"
                        : "未设置";

                return (
                  <article
                    key={integration.agent}
                    className={`agent-integration-card is-${integration.status}`}
                  >
                    <div className="agent-integration-card-header">
                      <strong>{integration.label}</strong>
                      <span className="agent-integration-status">{statusLabel}</span>
                    </div>

                    <div className="agent-integration-pills">
                      <span className={integration.mcpInstalled ? "is-on" : ""}>
                        {integrationCopy.mcp}
                      </span>
                      <span className={integration.instructionsInstalled ? "is-on" : ""}>
                        {integrationCopy.instructions}
                      </span>
                      {integration.instructionsOutdated ? (
                        <span className="is-warning">{integrationCopy.outdated}</span>
                      ) : null}
                    </div>

                    <div className="agent-integration-paths">
                      <span>
                        {integrationCopy.config}
                        <code title={integration.configPath}>{integration.configPath}</code>
                      </span>
                      <span>
                        {integrationCopy.guidance}
                        <code title={integration.instructionsPath}>{integration.instructionsPath}</code>
                      </span>
                    </div>

                    {integration.details.length > 0 ? (
                      <div className="agent-integration-notes">
                        {integration.details.slice(0, 2).map((detail) => (
                          <span key={detail}>{detail}</span>
                        ))}
                      </div>
                    ) : null}

                    <div className="agent-integration-card-actions">
                      <button
                        type="button"
                        className="btn btn-secondary"
                        onClick={() => void handleInstallAgentIntegration(integration.agent)}
                        disabled={integrationState.kind === "loading" || isWorking}
                      >
                        {isWorking
                          ? isEnglish
                            ? "Working..."
                            : "处理中..."
                          : hasAnyInstall
                            ? integrationCopy.repair
                            : integrationCopy.install}
                      </button>
                      <button
                        type="button"
                        className="btn btn-secondary"
                        onClick={() => void handleUninstallAgentIntegration(integration.agent)}
                        disabled={
                          integrationState.kind === "loading" ||
                          isWorking ||
                          !hasAnyInstall
                        }
                      >
                        {integrationCopy.uninstall}
                      </button>
                    </div>
                  </article>
                );
              })}
            </div>
          ) : integrationState.kind !== "loading" ? (
            <p className="settings-notice">{integrationCopy.notDetected}</p>
          ) : null}
        </section>
            ) : null}

            {activeSettingsSection === "sync" ? (
        <section className="settings-section file-sync-section" aria-labelledby="settings-sync-title">
          <div>
            <h4 id="settings-sync-title">{syncCopy.title}</h4>
          </div>

          <div className="sync-method-row">
            <span className="sync-method-label" id="conversation-data-sync-method-label">
              <span>{syncCopy.methodLabel}</span>
            </span>
            <div
              className="settings-segmented-control"
              role="radiogroup"
              aria-labelledby="conversation-data-sync-method-label"
            >
              {syncProviderOptions.map((option) => (
                <button
                  key={option.value}
                  type="button"
                  className={`settings-segmented-option ${
                    syncSettings.provider === option.value ? "is-selected" : ""
                  }`}
                  role="radio"
                  aria-checked={syncSettings.provider === option.value}
                  onClick={() => handleSyncSettingsChange({ provider: option.value })}
                >
                  {option.label}
                </button>
              ))}
            </div>
          </div>

          {isWebDav ? (
            <div className="webdav-settings-box">
              <div className="webdav-row webdav-url-row">
                <span className="webdav-field-label">{syncCopy.serverPathLabel}</span>
                <select
                  className="settings-select protocol-select"
                  aria-label={syncCopy.protocolLabel}
                  value={syncSettings.webdavScheme}
                  onChange={(event) =>
                    handleSyncSettingsChange({
                      webdavScheme: event.target.value === "http" ? "http" : "https",
                    })
                  }
                >
                  <option value="https">https</option>
                  <option value="http">http</option>
                </select>
                <span className="url-divider">://</span>
                <input
                  className="settings-input webdav-server-input"
                  type="text"
                  aria-label={syncCopy.serverPathLabel}
                  value={joinServerPath(syncSettings)}
                  placeholder="example.com/webdav"
                  onChange={(event) => handleSyncSettingsChange(splitServerPath(event.target.value))}
                />
                <span className="webdav-suffix">/{syncSettings.remotePath}/</span>
              </div>

              <label className="webdav-row">
                <span className="webdav-field-label">{syncCopy.usernameLabel}</span>
                <input
                  className="settings-input webdav-short-input"
                  type="text"
                  value={syncSettings.username}
                  autoComplete="username"
                  onChange={(event) => handleUsernameChange(event.target.value)}
                />
              </label>

              <label className="webdav-row">
                <span className="webdav-field-label">{syncCopy.passwordLabel}</span>
                <span className="settings-password-row">
                  <input
                    className="settings-input webdav-short-input"
                    type={showPassword ? "text" : "password"}
                    value={password}
                    autoComplete="current-password"
                    onChange={(event) => handlePasswordChange(event.target.value)}
                  />
                  <button
                    type="button"
                    className="password-toggle-button"
                    onClick={() => setShowPassword((current) => !current)}
                  >
                    {showPassword ? syncCopy.hidePasswordLabel : syncCopy.showPasswordLabel}
                  </button>
                </span>
              </label>

              <div className="webdav-verify-row">
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={() => void handleVerifyWebDavServer()}
                  disabled={webDavVerification.kind === "checking" || webDavSync.kind === "syncing"}
                >
                  {webDavVerification.kind === "checking"
                    ? syncCopy.verifyingServerLabel
                    : syncCopy.verifyServerLabel}
                </button>
                <button
                  type="button"
                  className="btn btn-primary"
                  onClick={() => void handleSyncWebDavNow()}
                  disabled={webDavVerification.kind === "checking" || webDavSync.kind === "syncing"}
                >
                  {webDavSync.kind === "syncing" ? syncCopy.syncingNowLabel : syncCopy.syncNowLabel}
                </button>
              </div>

              {webDavVerification.kind === "success" ? (
                <p className="settings-notice is-success">{syncCopy.verifySuccessLabel}</p>
              ) : null}

              {webDavVerification.kind === "error" ? (
                <p className="settings-notice is-danger">{webDavVerification.message}</p>
              ) : null}

              {webDavSync.kind === "success" ? (
                <div className="settings-notice is-success">
                  <p>
                    {syncCopy.syncSuccessPrefix} {webDavSync.uploadedCount}{" "}
                    {syncCopy.syncSuccessSuffix}
                  </p>
                  <p className="settings-notice-detail">
                    {syncCopy.syncTargetLabel}: {webDavSync.remoteUrl}
                  </p>
                </div>
              ) : null}

              {webDavSync.kind === "error" ? (
                <p className="settings-notice is-danger">{webDavSync.message}</p>
              ) : null}
            </div>
          ) : null}

          {isOneDrive ? (
            <div className="local-sync-settings-box">
              <div className="local-sync-info-row">
                <span className="local-sync-label">
                  {isEnglish ? "Sync folder" : "同步文件夹"}
                </span>
                <code className="local-sync-path">
                  {syncSettings.syncFolder || localSyncStatus?.folder_path || (isEnglish ? "Not selected" : "未选择")}
                </code>
              </div>

              <div className="webdav-verify-row" style={{ marginBottom: 8 }}>
                <button
                  type="button"
                  className="btn btn-secondary"
                  onClick={async () => {
                    const selected = await openDialog({ directory: true, title: isEnglish ? "Select sync folder" : "选择同步文件夹" });
                    if (selected && typeof selected === "string") {
                      handleSyncSettingsChange({ syncFolder: selected });
                    }
                  }}
                >
                  {isEnglish ? "Select Folder" : "选择文件夹"}
                </button>
                {syncSettings.syncFolder ? (
                  <button
                    type="button"
                    className="btn btn-secondary"
                    style={{ marginLeft: 8 }}
                    onClick={() => handleSyncSettingsChange({ syncFolder: "" })}
                  >
                    {isEnglish ? "Clear" : "清除"}
                  </button>
                ) : null}
              </div>

              <div className="local-sync-info-row">
                <span className="local-sync-label">
                  {isEnglish ? "Conversations synced" : "已同步对话数"}
                </span>
                <span className="local-sync-count">
                  {localSyncStatus?.remote_conversation_count ?? 0}
                </span>
              </div>

              {!syncSettings.syncFolder ? (
                <p className="settings-notice is-danger">
                  {isEnglish
                    ? "Please select a sync folder first."
                    : "请先选择同步文件夹。"}
                </p>
              ) : !localSyncStatus?.available ? (
                <p className="settings-notice is-danger">
                  {isEnglish
                    ? "Sync folder not accessible. Please check the path."
                    : "同步文件夹不可访问，请检查路径。"}
                </p>
              ) : null}

              <div className="webdav-verify-row">
                <button
                  type="button"
                  className="btn btn-primary"
                  onClick={() => void handleSyncLocalNow()}
                  disabled={localSyncState.kind === "syncing" || localSyncState.kind === "loading" || !syncSettings.syncFolder}
                >
                  {localSyncState.kind === "syncing"
                    ? (isEnglish ? "Syncing..." : "正在同步...")
                    : syncCopy.syncNowLabel}
                </button>
              </div>

              {localSyncState.kind === "success" ? (
                <div className="settings-notice is-success">
                  <p>
                    {isEnglish ? "Sync complete" : "同步完成"}: ↑{localSyncState.uploaded} ↓{localSyncState.downloaded}
                  </p>
                  <p className="settings-notice-detail">
                    {syncCopy.syncTargetLabel}: {localSyncState.folderPath}
                  </p>
                </div>
              ) : null}

              {localSyncState.kind === "error" ? (
                <p className="settings-notice is-danger">{localSyncState.message}</p>
              ) : null}

              <hr className="settings-divider" style={{ margin: "12px 0" }} />

              <label className="settings-toggle-row">
                <div className="settings-toggle-copy">
                  <span className="settings-label">
                    {isEnglish ? "Auto Backup" : "定时自动备份"}
                  </span>
                  <span className="settings-helper">
                    {isEnglish
                      ? "Periodically sync when cloud folder is idle"
                      : "定期检测云盘空闲时自动同步"}
                  </span>
                </div>
                <input
                  type="checkbox"
                  checked={autoBackupEnabled}
                  onChange={(e) => onAutoBackupEnabledChange(e.target.checked)}
                />
              </label>

              {autoBackupEnabled ? (
                <div className="settings-field-row" style={{ marginTop: 8 }}>
                  <span className="settings-label">
                    {isEnglish ? "Interval (minutes)" : "同步间隔（分钟）"}
                  </span>
                  <select
                    className="settings-select"
                    value={autoBackupIntervalMinutes}
                    onChange={(e) => onAutoBackupIntervalChange(Number(e.target.value))}
                  >
                    <option value={5}>5</option>
                    <option value={15}>15</option>
                    <option value={30}>30</option>
                    <option value={60}>60</option>
                    <option value={120}>120</option>
                  </select>
                </div>
              ) : null}
            </div>
          ) : null}

          <div className="download-row">
            <span id="download-files-mode-label">{syncCopy.downloadFilesLabel}</span>
            <div
              className="settings-segmented-control"
              role="radiogroup"
              aria-labelledby="download-files-mode-label"
            >
              {downloadModeOptions.map((option) => (
                <button
                  key={option.value}
                  type="button"
                  className={`settings-segmented-option ${
                    syncSettings.downloadMode === option.value ? "is-selected" : ""
                  }`}
                  role="radio"
                  aria-checked={syncSettings.downloadMode === option.value}
                  onClick={() => handleSyncSettingsChange({ downloadMode: option.value })}
                >
                  {option.label}
                </button>
              ))}
            </div>
          </div>
        </section>
            ) : null}

            {activeSettingsSection === "updates" ? (
        <section className="settings-section update-settings-section" aria-labelledby="settings-update-title">
          <div>
            <h4 id="settings-update-title">{updateCopy.title}</h4>
            <p className="settings-helper">{updateCopy.helper}</p>
          </div>

          <label className="settings-toggle-row update-auto-check-row">
            <div className="settings-toggle-copy">
              <span className="settings-label">{autoCheckLabel}</span>
            </div>
            <input
              type="checkbox"
              checked={autoCheckUpdates}
              onChange={(event) => onAutoCheckChange(event.target.checked)}
            />
          </label>

          <div className="settings-inline-actions update-check-actions">
            <button type="button" className="btn btn-primary" onClick={onCheckUpdates}>
              {checkUpdatesLabel}
            </button>
          </div>

          {updateState.kind === "checking" && <p className="settings-notice">{checkingLabel}</p>}

          {updateState.kind === "up-to-date" && (
            <p className="settings-notice is-success">{upToDateLabel}</p>
          )}

          {updateState.kind === "available" && (
            <div className="settings-notice is-accent">
              <strong>
                {updateAvailablePrefix} {updateState.version}
              </strong>
              {updateState.notes ? <p>{updateState.notes}</p> : null}
              <button type="button" className="btn btn-primary" onClick={onInstallUpdate}>
                {installUpdateLabel}
              </button>
            </div>
          )}

          {updateState.kind === "installing" && (
            <p className="settings-notice is-accent">{installingLabel}</p>
          )}

          {updateState.kind === "error" && (
            <p className="settings-notice is-danger">{updateState.message}</p>
          )}

          <div className="settings-nested-section upgrade-check-section" aria-labelledby="settings-upgrade-title">
            <div className="settings-section-heading upgrade-check-heading">
              <div>
                <h4 id="settings-upgrade-title">{upgradeCopy.title}</h4>
                <p className="settings-helper">{upgradeCopy.helper}</p>
              </div>
              <button
                type="button"
                className="btn btn-secondary"
                onClick={() => void handleRunUpgradeReadinessCheck()}
                disabled={upgradeCheck.kind === "checking"}
              >
                {upgradeCheck.kind === "checking" ? upgradeCopy.checking : upgradeCopy.run}
              </button>
            </div>

            {upgradeCheck.kind === "success" ? (
              <div className={`settings-notice upgrade-check-result is-${upgradeCheck.report.status}`}>
                <strong>{upgradeCheck.report.summary}</strong>
                <ul className="upgrade-check-list">
                  {upgradeCheck.report.checks.map((check) => (
                    <li key={check.key} className={`upgrade-check-item is-${check.status}`}>
                      <span className="upgrade-check-label">{check.label}</span>
                      <span className="upgrade-check-detail">{check.detail}</span>
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}

            {upgradeCheck.kind === "error" ? (
              <p className="settings-notice is-danger">
                {upgradeCopy.failed}: {upgradeCheck.message}
              </p>
            ) : null}
          </div>
        </section>
            ) : null}

            {activeSettingsSection === "about" ? (
              <div className="settings-about-section">{aboutContent}</div>
            ) : null}

          </div>
        </div>
      </section>
  );
}
