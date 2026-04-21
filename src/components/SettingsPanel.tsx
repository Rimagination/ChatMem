import { useState } from "react";
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
};

export type WebDavVerificationInput = {
  syncSettings: SyncSettings;
  password: string;
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
  onCheckUpdates: () => void;
  onInstallUpdate: () => void;
};

type WebDavVerificationState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "success" }
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
  onCheckUpdates,
  onInstallUpdate,
}: SettingsPanelProps) {
  const [password, setPassword] = useState("");
  const [showPassword, setShowPassword] = useState(false);
  const [webDavVerification, setWebDavVerification] = useState<WebDavVerificationState>({
    kind: "idle",
  });
  const fileSyncEnabled = syncSettings.provider === "webdav";
  const canVerifyWebDav =
    fileSyncEnabled &&
    syncSettings.webdavHost.trim().length > 0 &&
    syncSettings.username.trim().length > 0 &&
    password.trim().length > 0;

  const handleSyncSettingsChange = (patch: Partial<SyncSettings>) => {
    setWebDavVerification({ kind: "idle" });
    onSyncSettingsChange(patch);
  };

  const handlePasswordChange = (nextPassword: string) => {
    setWebDavVerification({ kind: "idle" });
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
      setWebDavVerification({ kind: "success" });
    } catch (error) {
      const message = error instanceof Error ? error.message : String(error);
      setWebDavVerification({
        kind: "error",
        message: `${syncCopy.verifyFailedPrefix}: ${message}`,
      });
    }
  };

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
                  onChange={(event) => handleSyncSettingsChange({ username: event.target.value })}
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
                  disabled={webDavVerification.kind === "checking"}
                >
                  {webDavVerification.kind === "checking"
                    ? syncCopy.verifyingServerLabel
                    : syncCopy.verifyServerLabel}
                </button>
              </div>

              {webDavVerification.kind === "success" ? (
                <p className="settings-notice is-success">{syncCopy.verifySuccessLabel}</p>
              ) : null}

              {webDavVerification.kind === "error" ? (
                <p className="settings-notice is-danger">{webDavVerification.message}</p>
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
