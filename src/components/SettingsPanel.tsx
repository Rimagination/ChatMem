import { useEffect, useState } from "react";
import type { Locale } from "../i18n/types";
import type { SyncSettings } from "../settings/storage";
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

type SettingsPanelProps = {
  open: boolean;
  title: string;
  closeLabel: string;
  languageLabel: string;
  locale: Locale;
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
  onAutoCheckChange: (nextValue: boolean) => void;
  onSyncSettingsChange: (patch: Partial<SyncSettings>) => void;
  onVerifyWebDavServer: (input: WebDavVerificationInput) => Promise<void>;
  onSyncWebDavNow: (input: WebDavVerificationInput) => Promise<WebDavSyncResult>;
  onRunUpgradeReadinessCheck: () => Promise<UpgradeReadinessReport>;
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

const ACKNOWLEDGED_SYSTEMS = [
  "mem0",
  "Letta / MemGPT",
  "Zep",
  "Cognee",
  "LangGraph / LangMem",
  "LLM Wiki / DeepWiki / CodeWiki",
  "OpenAI / Claude native memory",
];

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
  onAutoCheckChange,
  onSyncSettingsChange,
  onVerifyWebDavServer,
  onSyncWebDavNow,
  onRunUpgradeReadinessCheck,
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

  const upgradeCopy = {
    title: isEnglish ? "Install and upgrade check" : "\u5b89\u88c5\u4e0e\u5347\u7ea7\u81ea\u68c0",
    helper: isEnglish
      ? "Checks whether settings, WebDAV credentials, and the memory database survived an upgrade."
      : "\u68c0\u67e5\u8bbe\u7f6e\u3001WebDAV \u51ed\u636e\u548c\u8bb0\u5fc6\u6570\u636e\u5e93\u5728\u5347\u7ea7\u540e\u662f\u5426\u4ecd\u7136\u53ef\u7528\u3002",
    run: isEnglish ? "Run upgrade check" : "\u8fd0\u884c\u5347\u7ea7\u81ea\u68c0",
    checking: isEnglish ? "Checking..." : "\u6b63\u5728\u68c0\u67e5...",
    failed: isEnglish ? "Upgrade check failed" : "\u5347\u7ea7\u81ea\u68c0\u5931\u8d25",
  };

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
    <div className="settings-overlay" role="presentation" onClick={onClose}>
      <section
        className="settings-panel"
        role="dialog"
        aria-modal="true"
        aria-labelledby="settings-title"
        onClick={(event) => event.stopPropagation()}
      >
        <div className="settings-panel-header">
          <h3 id="settings-title">{title}</h3>
          <button type="button" className="toolbar-button" onClick={onClose}>
            {closeLabel}
          </button>
        </div>

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

        <section
          className="settings-section settings-acknowledgements"
          aria-labelledby="settings-acknowledgements-title"
        >
          <div>
            <h4 id="settings-acknowledgements-title">
              {locale === "en" ? "Acknowledgements" : "设计参考与致谢"}
            </h4>
            <p className="settings-helper">
              {locale === "en"
                ? "ChatMem draws from memory, agent, and wiki systems; it is not a clone of a single project."
                : "ChatMem 借鉴了多类记忆、Agent 和 Wiki 项目的设计，不是某一个项目的复刻。"}
            </p>
          </div>
          <ul
            className="acknowledgement-list"
            aria-label={locale === "en" ? "Acknowledged projects" : "致谢项目"}
          >
            {ACKNOWLEDGED_SYSTEMS.map((system) => (
              <li key={system} className="acknowledgement-pill">
                {system}
              </li>
            ))}
          </ul>
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

        <section className="settings-section upgrade-check-section" aria-labelledby="settings-upgrade-title">
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
        </section>

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

        <button type="button" className="btn btn-primary" onClick={onCheckUpdates}>
          {checkUpdatesLabel}
        </button>

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
      </section>
    </div>
  );
}
