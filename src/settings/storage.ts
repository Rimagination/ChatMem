import type { Locale } from "../i18n/types";

export type SyncProvider = "off" | "webdav";
export type WebDavScheme = "https" | "http";
export type DownloadMode = "on-sync" | "as-needed";

export type SyncSettings = {
  provider: SyncProvider;
  webdavScheme: WebDavScheme;
  webdavHost: string;
  webdavPath: string;
  username: string;
  remotePath: string;
  downloadMode: DownloadMode;
};

export type AppSettings = {
  locale: Locale;
  autoCheckUpdates: boolean;
  sync: SyncSettings;
};

export const SETTINGS_STORAGE_KEY = "chatmem.settings";

export const DEFAULT_SYNC_SETTINGS: SyncSettings = {
  provider: "off",
  webdavScheme: "https",
  webdavHost: "",
  webdavPath: "",
  username: "",
  remotePath: "chatmem",
  downloadMode: "on-sync",
};

export const DEFAULT_SETTINGS: AppSettings = {
  locale: "zh-CN",
  autoCheckUpdates: true,
  sync: DEFAULT_SYNC_SETTINGS,
};

function normalizeSyncSettings(value: unknown): SyncSettings {
  if (!value || typeof value !== "object") {
    return DEFAULT_SYNC_SETTINGS;
  }

  const parsed = value as Partial<SyncSettings> & { webdavUrl?: string; syncMode?: string };
  const parsedUrl = splitWebDavUrl(parsed.webdavUrl);

  return {
    provider: parsed.provider === "webdav" ? "webdav" : "off",
    webdavScheme:
      parsed.webdavScheme === "http" || parsed.webdavScheme === "https"
        ? parsed.webdavScheme
        : parsedUrl.webdavScheme,
    webdavHost:
      typeof parsed.webdavHost === "string" ? parsed.webdavHost : parsedUrl.webdavHost,
    webdavPath:
      typeof parsed.webdavPath === "string" ? parsed.webdavPath : parsedUrl.webdavPath,
    username: typeof parsed.username === "string" ? parsed.username : "",
    remotePath: typeof parsed.remotePath === "string" && parsed.remotePath.trim() ? parsed.remotePath : "chatmem",
    downloadMode: parsed.downloadMode === "as-needed" ? "as-needed" : "on-sync",
  };
}

function splitWebDavUrl(value: unknown): Pick<SyncSettings, "webdavScheme" | "webdavHost" | "webdavPath"> {
  if (typeof value !== "string" || !value.trim()) {
    return {
      webdavScheme: "https",
      webdavHost: "",
      webdavPath: "",
    };
  }

  try {
    const url = new URL(value);
    return {
      webdavScheme: url.protocol === "http:" ? "http" : "https",
      webdavHost: url.host,
      webdavPath: url.pathname.replace(/^\/+|\/+$/g, ""),
    };
  } catch {
    return {
      webdavScheme: "https",
      webdavHost: value.replace(/^https?:\/\//, "").replace(/^\/+|\/+$/g, ""),
      webdavPath: "",
    };
  }
}

export function loadSettings(): AppSettings {
  try {
    const raw = localStorage.getItem(SETTINGS_STORAGE_KEY);
    if (!raw) {
      return DEFAULT_SETTINGS;
    }

    const parsed = JSON.parse(raw) as Partial<AppSettings>;

    return {
      locale: parsed.locale === "en" ? "en" : "zh-CN",
      autoCheckUpdates: parsed.autoCheckUpdates !== false,
      sync: normalizeSyncSettings(parsed.sync),
    };
  } catch {
    return DEFAULT_SETTINGS;
  }
}

export function saveSettings(settings: AppSettings) {
  localStorage.setItem(SETTINGS_STORAGE_KEY, JSON.stringify(settings));
}

export function updateSettings(patch: Partial<AppSettings>) {
  const nextSettings = { ...loadSettings(), ...patch };
  saveSettings(nextSettings);
  return nextSettings;
}
