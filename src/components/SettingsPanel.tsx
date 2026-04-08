import type { Locale } from "../i18n/types";
import type { UpdateState } from "../updater/updater";

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
  onClose: () => void;
  onLocaleChange: (locale: Locale) => void;
  onAutoCheckChange: (nextValue: boolean) => void;
  onCheckUpdates: () => void;
  onInstallUpdate: () => void;
};

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
  onClose,
  onLocaleChange,
  onAutoCheckChange,
  onCheckUpdates,
  onInstallUpdate,
}: SettingsPanelProps) {
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

        {updateState.kind === "checking" && (
          <p className="settings-notice">{checkingLabel}</p>
        )}

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
