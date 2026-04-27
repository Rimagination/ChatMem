import { useEffect, useState } from "react";
import type { Locale } from "../i18n/types";
import {
  APP_FONT_OPTIONS,
  type AppFontFamily,
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
  autoCheckLabel: string;
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
  onSyncSettingsChange: (patch: Partial<SyncSettings>) => void;
  onVerifyWebDavServer: (input: WebDavVerificationInput) => Promise<void>;
  onSyncWebDavNow: (input: WebDavVerificationInput) => Promise<WebDavSyncResult>;
  onRunUpgradeReadinessCheck: () => Promise<UpgradeReadinessReport>;
  onDetectAgentIntegrations: () => Promise<AgentIntegrationStatus[]>;
  onInstallAgentIntegration: (agent: string) => Promise<AgentIntegrationOperationResult[]>;
  onUninstallAgentIntegration: (agent: string) => Promise<AgentIntegrationOperationResult[]>;
  onLoadWebDavPassword: (username: string) => Promise<string | null>;
  onSaveWebDavPassword: (input: { username: string; password: string }) => Promise<void>;
  onCheckUpdates: () => void;
  onInstallUpdate: () => void;
};

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
  autoCheckLabel,
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
  onSyncSettingsChange,
  onVerifyWebDavServer,
  onSyncWebDavNow,
  onRunUpgradeReadinessCheck,
  onDetectAgentIntegrations,
  onInstallAgentIntegration,
  onUninstallAgentIntegration,
  onLoadWebDavPassword,
  onSaveWebDavPassword,
  onCheckUpdates,
  onInstallUpdate,
}: SettingsPanelProps) {
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
  const isEnglish = locale === "en";
  const fileSyncEnabled = syncSettings.provider === "webdav";
  const canVerifyWebDav =
    fileSyncEnabled &&
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
            ? "Installed or repaired all detected integrations."
            : "已安装或修复全部 Agent 集成。"
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
    title: isEnglish ? "Upgrade self-check" : "\u5347\u7ea7\u81ea\u68c0",
    helper: isEnglish
      ? "Checks whether settings, WebDAV credentials, and the memory database survived an upgrade."
      : "\u68c0\u67e5\u8bbe\u7f6e\u3001WebDAV \u51ed\u636e\u548c\u8bb0\u5fc6\u6570\u636e\u5e93\u5728\u5347\u7ea7\u540e\u662f\u5426\u4ecd\u7136\u53ef\u7528\u3002",
    run: isEnglish ? "Run upgrade check" : "\u8fd0\u884c\u5347\u7ea7\u81ea\u68c0",
    checking: isEnglish ? "Checking..." : "\u6b63\u5728\u68c0\u67e5...",
    failed: isEnglish ? "Upgrade check failed" : "\u5347\u7ea7\u81ea\u68c0\u5931\u8d25",
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
    title: isEnglish ? "Updates and diagnostics" : "\u66f4\u65b0\u4e0e\u8bca\u65ad",
    helper: isEnglish
      ? "Keep the desktop app current and verify that local data still works after upgrades."
      : "\u68c0\u67e5\u684c\u9762\u7aef\u66f4\u65b0\uff0c\u5e76\u786e\u8ba4\u5347\u7ea7\u540e\u672c\u5730\u6570\u636e\u4ecd\u53ef\u7528\u3002",
  };

  const integrationCopy = {
    title: isEnglish ? "Agent integration" : "Agent 集成",
    helper: isEnglish
      ? "Install MCP plus each agent's native guidance entry, so recall questions are routed to ChatMem before the agent asks you to redescribe history."
      : "安装 MCP 和各 Agent 原生的引导入口，让“记得吗 / 继续 / 迁移”这类问题先查 ChatMem，而不是让你重述历史。",
    installAll: isEnglish ? "Install all" : "一键安装到全部",
    rescan: isEnglish ? "Rescan" : "重新检测",
    loading: isEnglish ? "Checking integrations..." : "正在检查集成状态...",
    install: isEnglish ? "Install" : "安装",
    repair: isEnglish ? "Repair" : "修复",
    uninstall: isEnglish ? "Uninstall" : "卸载",
    mcp: "MCP",
    instructions: isEnglish ? "Guidance" : "引导",
    config: isEnglish ? "Config" : "配置",
    guidance: isEnglish ? "Guidance entry" : "引导入口",
    notDetected: isEnglish
      ? "No integrations returned by the native layer."
      : "本机层还没有返回可用集成。",
  };

  useEffect(() => {
    if (!open) {
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
        setIntegrationState({ kind: "idle" });
      })
      .catch((error) => {
        if (cancelled) {
          return;
        }
        const message = error instanceof Error ? error.message : String(error);
        setIntegrationState({ kind: "error", message });
      });

    return () => {
      cancelled = true;
    };
  }, [open]);

  useEffect(() => {
    if (!open || !fileSyncEnabled || !syncSettings.username.trim()) {
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
  }, [fileSyncEnabled, onLoadWebDavPassword, open, syncSettings.username]);

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
            <p className="settings-helper settings-field-hint">{generalCopy.fontHint}</p>
          </section>
        </div>

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
                const statusLabel = isEnglish
                  ? integration.status === "ready"
                    ? "Ready"
                    : integration.status === "partial"
                      ? "Needs repair"
                      : "Not installed"
                  : integration.statusLabel;

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
                          : integration.status === "ready"
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

        <section className="settings-section file-sync-section" aria-labelledby="settings-sync-title">
          <div>
            <h4 id="settings-sync-title">{syncCopy.title}</h4>
          </div>

          <div className="sync-method-row">
            <label className="sync-method-label">
              <input
                type="checkbox"
                checked={fileSyncEnabled}
                aria-label={syncCopy.methodLabel}
                onChange={(event) =>
                  handleSyncSettingsChange({
                    provider: event.target.checked ? "webdav" : "off",
                  })
                }
              />
              <span>{syncCopy.methodLabel}</span>
            </label>
            <span className="sync-method-value">{syncCopy.webdavLabel}</span>
          </div>

          {fileSyncEnabled ? (
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

          <div className="download-row">
            <span>{syncCopy.downloadFilesLabel}</span>
            <select
              className="settings-select download-select"
              aria-label={syncCopy.downloadFilesLabel}
              value={syncSettings.downloadMode}
              onChange={(event) =>
                handleSyncSettingsChange({
                  downloadMode: event.target.value === "as-needed" ? "as-needed" : "on-sync",
                })
              }
            >
              <option value="on-sync">{syncCopy.onSyncDownloadLabel}</option>
              <option value="as-needed">{syncCopy.asNeededDownloadLabel}</option>
            </select>
          </div>
        </section>

        <section className="settings-section update-settings-section" aria-labelledby="settings-update-title">
          <div>
            <h4 id="settings-update-title">{updateCopy.title}</h4>
            <p className="settings-helper">{updateCopy.helper}</p>
          </div>

          <label className="settings-toggle-row">
            <div className="settings-toggle-copy">
              <span className="settings-label">{autoCheckLabel}</span>
            </div>
            <input
              type="checkbox"
              checked={autoCheckUpdates}
              onChange={(event) => onAutoCheckChange(event.target.checked)}
            />
          </label>

          <div className="settings-inline-actions">
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
            <div className="settings-section-heading">
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

      </section>
  );
}
