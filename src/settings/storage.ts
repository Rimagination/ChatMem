import type { Locale } from "../i18n/types";

export type AppSettings = {
  locale: Locale;
  autoCheckUpdates: boolean;
};

export const SETTINGS_STORAGE_KEY = "chatmem.settings";

export const DEFAULT_SETTINGS: AppSettings = {
  locale: "zh-CN",
  autoCheckUpdates: true,
};

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
