import { createContext, useContext, useMemo, useState, type ReactNode } from "react";
import { strings } from "./strings";
import type { Locale, TranslationKey } from "./types";
import { loadSettings, updateSettings } from "../settings/storage";

type I18nContextValue = {
  locale: Locale;
  t: (key: TranslationKey) => string;
  setLocale: (locale: Locale) => void;
};

const I18nContext = createContext<I18nContextValue | null>(null);

export function I18nProvider({ children }: { children: ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(() => loadSettings().locale);

  const value = useMemo<I18nContextValue>(
    () => ({
      locale,
      t: (key) => strings[locale][key] ?? strings["zh-CN"][key],
      setLocale: (nextLocale) => {
        updateSettings({ locale: nextLocale });
        setLocaleState(nextLocale);
      },
    }),
    [locale],
  );

  return <I18nContext.Provider value={value}>{children}</I18nContext.Provider>;
}

export function useI18n() {
  const context = useContext(I18nContext);
  if (!context) {
    throw new Error("useI18n must be used inside I18nProvider");
  }
  return context;
}
