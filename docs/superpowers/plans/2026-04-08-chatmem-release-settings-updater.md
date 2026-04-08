# ChatMem Release, Settings, I18n, and Updater Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Ship ChatMem as a public Windows desktop app with a settings panel, Simplified Chinese and English UI, in-app update checks, auto-check-on-launch, and a GitHub-based release pipeline.

**Architecture:** Keep the current Tauri + React desktop app as the only product line, add a small client-side i18n and settings layer, wrap Tauri updater JS APIs behind a local service, and let GitHub Releases act as both the user download page and the updater JSON host. Because this repo does not yet have `.git`, initialize git first and execute in the existing workspace instead of a separate worktree.

**Tech Stack:** React 18, TypeScript, Vitest, Testing Library, Tauri 1.x, Rust, GitHub Actions, PowerShell

---

## File Map

### Frontend files to create

- `D:\VSP\agentswap-gui\src\i18n\types.ts`
  Shared locale and translation-key types.
- `D:\VSP\agentswap-gui\src\i18n\strings.ts`
  Chinese and English string dictionaries.
- `D:\VSP\agentswap-gui\src\i18n\I18nProvider.tsx`
  Locale state, `t(key)` lookup, localStorage persistence, context hook.
- `D:\VSP\agentswap-gui\src\settings\storage.ts`
  Read and write persisted settings such as language and auto-update.
- `D:\VSP\agentswap-gui\src\updater\updater.ts`
  Wrapper around `@tauri-apps/api/updater` and `@tauri-apps/api/process`.
- `D:\VSP\agentswap-gui\src\components\SettingsPanel.tsx`
  Settings drawer/modal with language selector, manual update action, and auto-check toggle.

### Frontend files to modify

- `D:\VSP\agentswap-gui\src\App.tsx`
  Wire i18n, settings entry, update-check flow, and launch-time auto-check.
- `D:\VSP\agentswap-gui\src\main.tsx`
  Mount the i18n provider at the app root.
- `D:\VSP\agentswap-gui\src\styles.css`
  Add settings panel, switch, notice, and updater-state styles.
- `D:\VSP\agentswap-gui\src\__tests__\App.test.tsx`
  Cover settings entry, language switching, manual update states, and launch auto-check.

### Tauri and release files to modify

- `D:\VSP\agentswap-gui\src-tauri\Cargo.toml`
  Enable the Tauri updater feature.
- `D:\VSP\agentswap-gui\src-tauri\tauri.conf.json`
  Add updater config, public key, endpoint, and Windows install mode.
- `D:\VSP\agentswap-gui\.gitignore`
  Ignore build outputs, portable bundles, logs, and Python caches, while keeping source and docs.
- `D:\VSP\agentswap-gui\README.md`
  Rewrite for public GitHub usage, Windows downloads, build instructions, and updater behavior.
- `D:\VSP\agentswap-gui\DEVELOPMENT.md`
  Document local release workflow, updater key generation, and GitHub Secrets.

### Release automation files to create

- `D:\VSP\agentswap-gui\.github\workflows\release.yml`
  Windows release workflow using `tauri-apps/tauri-action@v1`.
- `D:\VSP\agentswap-gui\scripts\build-portable.ps1`
  Stage `ChatMem.exe` plus docs into a portable folder and zip it.
- `D:\VSP\agentswap-gui\LICENSE`
  MIT license for the public repo.

### Files to delete from the public product repo

- `D:\VSP\agentswap-gui\app.py`
- `D:\VSP\agentswap-gui\app_fixed.py`
- `D:\VSP\agentswap-gui\backend.py`
- `D:\VSP\agentswap-gui\backend.log`
- `D:\VSP\agentswap-gui\server.py`
- `D:\VSP\agentswap-gui\start.py`
- `D:\VSP\agentswap-gui\start.bat`
- `D:\VSP\agentswap-gui\viewer.html`

These files belong to an older Python/Web path that the approved product scope explicitly excludes.

---

### Task 1: Add I18n and Persistent Settings Foundation

**Files:**
- Create: `D:\VSP\agentswap-gui\src\i18n\types.ts`
- Create: `D:\VSP\agentswap-gui\src\i18n\strings.ts`
- Create: `D:\VSP\agentswap-gui\src\i18n\I18nProvider.tsx`
- Create: `D:\VSP\agentswap-gui\src\settings\storage.ts`
- Modify: `D:\VSP\agentswap-gui\src\main.tsx`
- Test: `D:\VSP\agentswap-gui\src\__tests__\App.test.tsx`

- [ ] **Step 1: Write the failing language-persistence test**

```tsx
it("restores the saved language and renders the English shell copy", async () => {
  localStorage.setItem(
    "chatmem.settings",
    JSON.stringify({ locale: "en", autoCheckUpdates: true }),
  );

  render(<App />);

  expect(await screen.findByText("Your local AI conversations, ready to resume")).toBeTruthy();
  expect(screen.getByPlaceholderText("Search conversations...")).toBeTruthy();
});
```

- [ ] **Step 2: Run the test to verify RED**

Run:

```powershell
npm run test:run -- src/__tests__/App.test.tsx
```

Expected:

```text
FAIL
Unable to find text "Your local AI conversations, ready to resume"
```

- [ ] **Step 3: Write the minimal persistence and i18n modules**

```ts
// src/i18n/types.ts
export type Locale = "zh-CN" | "en";

export type TranslationKey =
  | "brand.subtitle"
  | "search.placeholder"
  | "settings.open"
  | "settings.short"
  | "settings.title"
  | "settings.language"
  | "settings.checkUpdates"
  | "settings.autoCheck"
  | "settings.updateError"
  | "common.close";
```

```ts
// src/settings/storage.ts
import type { Locale } from "../i18n/types";

export type AppSettings = {
  locale: Locale;
  autoCheckUpdates: boolean;
};

export const DEFAULT_SETTINGS: AppSettings = {
  locale: "zh-CN",
  autoCheckUpdates: true,
};

const STORAGE_KEY = "chatmem.settings";

export function loadSettings(): AppSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return DEFAULT_SETTINGS;
    const parsed = JSON.parse(raw);
    return {
      locale: parsed.locale === "en" ? "en" : "zh-CN",
      autoCheckUpdates: parsed.autoCheckUpdates !== false,
    };
  } catch {
    return DEFAULT_SETTINGS;
  }
}

export function saveSettings(settings: AppSettings) {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
}
```

```tsx
// src/i18n/I18nProvider.tsx
import { createContext, useContext, useMemo, useState } from "react";
import { strings } from "./strings";
import type { Locale, TranslationKey } from "./types";
import { loadSettings, saveSettings } from "../settings/storage";

type I18nContextValue = {
  locale: Locale;
  t: (key: TranslationKey) => string;
  setLocale: (locale: Locale) => void;
};

const I18nContext = createContext<I18nContextValue | null>(null);

export function I18nProvider({ children }: { children: React.ReactNode }) {
  const [locale, setLocaleState] = useState<Locale>(() => loadSettings().locale);

  const value = useMemo<I18nContextValue>(
    () => ({
      locale,
      t: (key) => strings[locale][key] ?? strings["zh-CN"][key],
      setLocale: (nextLocale) => {
        const current = loadSettings();
        saveSettings({ ...current, locale: nextLocale });
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
```

- [ ] **Step 4: Mount the provider in the app root**

```tsx
// src/main.tsx
import { I18nProvider } from "./i18n/I18nProvider";

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <I18nProvider>
      <App />
    </I18nProvider>
  </React.StrictMode>,
);
```

- [ ] **Step 5: Run the test to verify GREEN**

Run:

```powershell
npm run test:run -- src/__tests__/App.test.tsx
```

Expected:

```text
PASS src/__tests__/App.test.tsx
```

- [ ] **Step 6: Commit**

```powershell
git add src/i18n src/settings src/main.tsx src/__tests__/App.test.tsx
git commit -m "feat: add i18n and persisted app settings"
```

---

### Task 2: Add the Settings Panel and Language Switch UI

**Files:**
- Create: `D:\VSP\agentswap-gui\src\components\SettingsPanel.tsx`
- Modify: `D:\VSP\agentswap-gui\src\App.tsx`
- Modify: `D:\VSP\agentswap-gui\src\styles.css`
- Test: `D:\VSP\agentswap-gui\src\__tests__\App.test.tsx`

- [ ] **Step 1: Write the failing settings-panel test**

```tsx
it("opens settings and switches the interface language to English", async () => {
  render(<App />);

  fireEvent.click(screen.getByRole("button", { name: "打开设置" }));
  expect(screen.getByRole("heading", { name: "设置" })).toBeTruthy();

  fireEvent.change(screen.getByLabelText("语言 Language"), {
    target: { value: "en" },
  });

  expect(await screen.findByRole("heading", { name: "Settings" })).toBeTruthy();
  expect(screen.getByRole("button", { name: "Check for updates" })).toBeTruthy();
});
```

- [ ] **Step 2: Run the test to verify RED**

Run:

```powershell
npm run test:run -- src/__tests__/App.test.tsx
```

Expected:

```text
FAIL
Unable to find role "button" with name "打开设置"
```

- [ ] **Step 3: Implement the settings panel component**

```tsx
// src/components/SettingsPanel.tsx
import type { Locale } from "../i18n/types";

type SettingsPanelProps = {
  open: boolean;
  locale: Locale;
  autoCheckUpdates: boolean;
  onClose: () => void;
  onLocaleChange: (locale: Locale) => void;
  onAutoCheckChange: (value: boolean) => void;
  onCheckUpdates: () => void;
  labels: {
    title: string;
    language: string;
    checkUpdates: string;
    autoCheck: string;
    close: string;
  };
};

export default function SettingsPanel(props: SettingsPanelProps) {
  if (!props.open) return null;

  return (
    <div className="settings-overlay" role="dialog" aria-modal="true" aria-labelledby="settings-title">
      <section className="settings-panel">
        <div className="settings-header">
          <h3 id="settings-title">{props.labels.title}</h3>
          <button type="button" className="toolbar-button" onClick={props.onClose}>
            {props.labels.close}
          </button>
        </div>

        <label className="settings-field">
          <span>{props.labels.language}</span>
          <select value={props.locale} onChange={(event) => props.onLocaleChange(event.target.value as Locale)}>
            <option value="zh-CN">简体中文</option>
            <option value="en">English</option>
          </select>
        </label>

        <label className="settings-toggle">
          <input
            type="checkbox"
            checked={props.autoCheckUpdates}
            onChange={(event) => props.onAutoCheckChange(event.target.checked)}
          />
          <span>{props.labels.autoCheck}</span>
        </label>

        <button type="button" className="btn btn-primary" onClick={props.onCheckUpdates}>
          {props.labels.checkUpdates}
        </button>
      </section>
    </div>
  );
}
```

- [ ] **Step 4: Wire the settings trigger into `App.tsx`**

```tsx
const [showSettings, setShowSettings] = useState(false);
const { locale, setLocale, t } = useI18n();
const [appSettings, setAppSettings] = useState(() => loadSettings());

function updateAutoCheckUpdates(nextValue: boolean) {
  const nextSettings = { ...appSettings, autoCheckUpdates: nextValue };
  setAppSettings(nextSettings);
  saveSettings(nextSettings);
}

<button
  type="button"
  className="toolbar-button"
  onClick={() => setShowSettings(true)}
  aria-label={t("settings.open")}
  title={t("settings.open")}
>
  <span className="toolbar-button-icon" aria-hidden="true">⚙</span>
  <span>{t("settings.short")}</span>
</button>

<SettingsPanel
  open={showSettings}
  locale={locale}
  autoCheckUpdates={appSettings.autoCheckUpdates}
  onClose={() => setShowSettings(false)}
  onLocaleChange={(nextLocale) => {
    setLocale(nextLocale);
    setAppSettings((current) => {
      const nextSettings = { ...current, locale: nextLocale };
      saveSettings(nextSettings);
      return nextSettings;
    });
  }}
  onAutoCheckChange={updateAutoCheckUpdates}
  onCheckUpdates={() => {}}
  labels={{
    title: t("settings.title"),
    language: t("settings.language"),
    checkUpdates: t("settings.checkUpdates"),
    autoCheck: t("settings.autoCheck"),
    close: t("common.close"),
  }}
/>
```

- [ ] **Step 5: Add the settings layout styles**

```css
.settings-overlay {
  position: fixed;
  inset: 0;
  display: flex;
  justify-content: flex-end;
  background: rgba(18, 26, 40, 0.22);
  backdrop-filter: blur(8px);
  z-index: 1200;
}

.settings-panel {
  width: min(420px, calc(100vw - 24px));
  height: 100%;
  padding: 24px;
  background: rgba(255, 255, 255, 0.96);
  border-left: 1px solid var(--border-soft);
  box-shadow: var(--shadow-lg);
  display: grid;
  align-content: start;
  gap: 18px;
}

.settings-field,
.settings-toggle {
  display: grid;
  gap: 10px;
}
```

- [ ] **Step 6: Run the test to verify GREEN**

Run:

```powershell
npm run test:run -- src/__tests__/App.test.tsx
```

Expected:

```text
PASS src/__tests__/App.test.tsx
```

- [ ] **Step 7: Commit**

```powershell
git add src/components/SettingsPanel.tsx src/App.tsx src/styles.css src/__tests__/App.test.tsx
git commit -m "feat: add settings panel and language switcher"
```

---

### Task 3: Add Manual Update Checks and Auto-Check on Launch

**Files:**
- Create: `D:\VSP\agentswap-gui\src\updater\updater.ts`
- Modify: `D:\VSP\agentswap-gui\src\App.tsx`
- Modify: `D:\VSP\agentswap-gui\src\components\SettingsPanel.tsx`
- Modify: `D:\VSP\agentswap-gui\src\styles.css`
- Test: `D:\VSP\agentswap-gui\src\__tests__\App.test.tsx`

- [ ] **Step 1: Write the failing update-state tests**

```tsx
it("shows the up-to-date state after a manual check", async () => {
  mockCheckUpdate.mockResolvedValue({ shouldUpdate: false });

  render(<App />);
  fireEvent.click(screen.getByRole("button", { name: "打开设置" }));
  fireEvent.click(screen.getByRole("button", { name: "检查更新" }));

  expect(await screen.findByText("当前已是最新版本")).toBeTruthy();
});

it("auto-checks for updates on launch when enabled", async () => {
  vi.useFakeTimers();
  mockCheckUpdate.mockResolvedValue({ shouldUpdate: true, manifest: { version: "0.1.1", date: "2026-04-08T12:00:00Z", body: "Bug fixes" } });

  render(<App />);
  vi.advanceTimersByTime(4000);

  await waitFor(() => {
    expect(mockCheckUpdate).toHaveBeenCalledTimes(1);
    expect(screen.getByText("发现新版本 0.1.1")).toBeTruthy();
  });
});
```

- [ ] **Step 2: Run the tests to verify RED**

Run:

```powershell
npm run test:run -- src/__tests__/App.test.tsx
```

Expected:

```text
FAIL
mockCheckUpdate was not called
```

- [ ] **Step 3: Create the updater wrapper**

```ts
// src/updater/updater.ts
import { checkUpdate, installUpdate } from "@tauri-apps/api/updater";
import { relaunch } from "@tauri-apps/api/process";

export type UpdateCheckState =
  | { kind: "idle" }
  | { kind: "checking" }
  | { kind: "up-to-date" }
  | { kind: "available"; version: string; date?: string; notes?: string }
  | { kind: "installing"; version: string }
  | { kind: "error"; message: string };

export async function runUpdateCheck(): Promise<UpdateCheckState> {
  const result = await checkUpdate();
  if (!result.shouldUpdate) {
    return { kind: "up-to-date" };
  }
  return {
    kind: "available",
    version: result.manifest?.version ?? "",
    date: result.manifest?.date,
    notes: result.manifest?.body,
  };
}

export async function installAvailableUpdate(version: string) {
  await installUpdate();
  await relaunch();
  return { kind: "installing", version } as const;
}
```

- [ ] **Step 4: Wire manual and delayed auto-check behavior in `App.tsx`**

```tsx
const [updateState, setUpdateState] = useState<UpdateCheckState>({ kind: "idle" });

async function handleCheckForUpdates() {
  setUpdateState({ kind: "checking" });
  try {
    setUpdateState(await runUpdateCheck());
  } catch {
    setUpdateState({ kind: "error", message: t("settings.updateError") });
  }
}

useEffect(() => {
  if (!appSettings.autoCheckUpdates) return;

  const timer = window.setTimeout(async () => {
    try {
      const nextState = await runUpdateCheck();
      if (nextState.kind === "available") {
        setUpdateState(nextState);
      }
    } catch {
      // auto-check stays silent on startup failure
    }
  }, 3500);

  return () => window.clearTimeout(timer);
}, [appSettings.autoCheckUpdates]);
```

- [ ] **Step 5: Render the update status and install button in the settings panel**

```tsx
{props.updateState.kind === "up-to-date" && (
  <p className="settings-notice is-success">当前已是最新版本</p>
)}

{props.updateState.kind === "available" && (
  <div className="settings-notice is-accent">
    <strong>发现新版本 {props.updateState.version}</strong>
    {props.updateState.notes ? <p>{props.updateState.notes}</p> : null}
    <button type="button" className="btn btn-primary" onClick={props.onInstallUpdate}>
      立即更新
    </button>
  </div>
)}

{props.updateState.kind === "error" && (
  <p className="settings-notice is-danger">{props.updateState.message}</p>
)}
```

- [ ] **Step 6: Run the tests to verify GREEN**

Run:

```powershell
npm run test:run -- src/__tests__/App.test.tsx
```

Expected:

```text
PASS src/__tests__/App.test.tsx
```

- [ ] **Step 7: Commit**

```powershell
git add src/updater src/App.tsx src/components/SettingsPanel.tsx src/styles.css src/__tests__/App.test.tsx
git commit -m "feat: add manual and startup update checks"
```

---

### Task 4: Configure Tauri Updater and Automated Windows Releases

**Files:**
- Modify: `D:\VSP\agentswap-gui\src-tauri\Cargo.toml`
- Modify: `D:\VSP\agentswap-gui\src-tauri\tauri.conf.json`
- Create: `D:\VSP\agentswap-gui\.github\workflows\release.yml`
- Create: `D:\VSP\agentswap-gui\scripts\build-portable.ps1`
- Modify: `D:\VSP\agentswap-gui\DEVELOPMENT.md`

- [ ] **Step 1: Generate the updater key locally**

Run:

```powershell
cd D:\VSP\agentswap-gui
npm run tauri signer generate -- -w $HOME\.tauri\chatmem.key
```

Expected:

```text
Private key written to C:\Users\<user>\.tauri\chatmem.key
Public key: <multi-line public key block>
```

- [ ] **Step 2: Enable updater support in Rust and Tauri config**

```toml
# src-tauri/Cargo.toml
tauri = { version = "1", features = ["shell-open", "updater"] }
```

```json
// src-tauri/tauri.conf.json
{
  "tauri": {
    "updater": {
      "active": true,
      "dialog": false,
      "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6IDM3Nzk4MjNFOEJDQzZGODcKUldTSGI4eUxQb0o1TjlhVmRYcjhDc2VibjNEaU9ueUdPMzJjb3pPMm1xYXdXM0t0eGpJSnpocFQK",
      "endpoints": [
        "https://github.com/Rimagination/ChatMem/releases/latest/download/latest.json"
      ],
      "windows": {
        "installMode": "passive"
      }
    }
  }
}
```

- [ ] **Step 3: Add the portable bundle script**

```powershell
# scripts/build-portable.ps1
param(
  [string]$Version = "0.1.0"
)

$Root = Split-Path -Parent $PSScriptRoot
$PortableRoot = Join-Path $Root "dist-portable\ChatMem"
$ZipPath = Join-Path $Root "dist-portable\ChatMem-v$Version-portable.zip"
$ExePath = Join-Path $Root "src-tauri\target\release\ChatMem.exe"
$ReadmePath = Join-Path $Root "启动说明.md"

New-Item -ItemType Directory -Force -Path $PortableRoot | Out-Null
Copy-Item -LiteralPath $ExePath -Destination (Join-Path $PortableRoot "ChatMem.exe") -Force
Copy-Item -LiteralPath $ReadmePath -Destination (Join-Path $PortableRoot "使用说明.txt") -Force
if (Test-Path $ZipPath) { Remove-Item -LiteralPath $ZipPath -Force }
Compress-Archive -Path (Join-Path $PortableRoot "*") -DestinationPath $ZipPath
```

- [ ] **Step 4: Add the GitHub release workflow**

```yaml
name: release

on:
  push:
    tags:
      - "v*"

jobs:
  release-windows:
    runs-on: windows-latest
    permissions:
      contents: write
    steps:
      - uses: actions/checkout@v4

      - uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: npm

      - uses: dtolnay/rust-toolchain@stable

      - name: Install frontend dependencies
        run: npm ci

      - name: Build and publish Tauri bundles
        uses: tauri-apps/tauri-action@v1
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          TAURI_PRIVATE_KEY: ${{ secrets.TAURI_PRIVATE_KEY }}
          TAURI_KEY_PASSWORD: ${{ secrets.TAURI_KEY_PASSWORD }}
        with:
          tagName: ${{ github.ref_name }}
          releaseName: "ChatMem ${{ github.ref_name }}"
          releaseBody: "Windows downloads for ChatMem."
          releaseDraft: false
          prerelease: false
          updaterJsonPreferNsis: true
          uploadUpdaterJson: true

      - name: Build portable zip
        run: |
          $version = "${{ github.ref_name }}".TrimStart("v")
          .\scripts\build-portable.ps1 -Version $version
        shell: pwsh

      - name: Upload portable zip
        uses: softprops/action-gh-release@v2
        with:
          files: dist-portable/ChatMem-v*.zip
```

- [ ] **Step 5: Document the release secret setup**

```md
## Release Secrets

Set these repository secrets before pushing a release tag:

- `TAURI_PRIVATE_KEY`: the full contents of `C:\Users\<you>\.tauri\chatmem.key`
- `TAURI_KEY_PASSWORD`: the signer passphrase used during key generation

The updater endpoint is:

`https://github.com/Rimagination/ChatMem/releases/latest/download/latest.json`
```

- [ ] **Step 6: Verify the release config locally**

Run:

```powershell
npm run tauri build
powershell -ExecutionPolicy Bypass -File .\scripts\build-portable.ps1 -Version 0.1.0
```

Expected:

```text
Build succeeds
dist-portable\ChatMem-v0.1.0-portable.zip exists
src-tauri\target\release\bundle\nsis\*.nsis.zip.sig exists
src-tauri\target\release\bundle\msi\*.msi.zip.sig exists
```

- [ ] **Step 7: Commit**

```powershell
git add src-tauri/Cargo.toml src-tauri/tauri.conf.json .github/workflows/release.yml scripts/build-portable.ps1 DEVELOPMENT.md
git commit -m "build: add updater config and release workflow"
```

---

### Task 5: Clean the Repo, Rewrite Public Docs, Initialize Git, and Publish

**Files:**
- Modify: `D:\VSP\agentswap-gui\.gitignore`
- Modify: `D:\VSP\agentswap-gui\README.md`
- Create: `D:\VSP\agentswap-gui\LICENSE`
- Delete: `D:\VSP\agentswap-gui\app.py`
- Delete: `D:\VSP\agentswap-gui\app_fixed.py`
- Delete: `D:\VSP\agentswap-gui\backend.py`
- Delete: `D:\VSP\agentswap-gui\backend.log`
- Delete: `D:\VSP\agentswap-gui\server.py`
- Delete: `D:\VSP\agentswap-gui\start.py`
- Delete: `D:\VSP\agentswap-gui\start.bat`
- Delete: `D:\VSP\agentswap-gui\viewer.html`

- [ ] **Step 1: Tighten `.gitignore` for a public repo**

```gitignore
node_modules/
dist/
dist-portable/
src-tauri/target/
__pycache__/
*.log
.env
.env.local
.vscode/
.idea/
```

- [ ] **Step 2: Rewrite the README for public users**

```md
# ChatMem

ChatMem is a Windows desktop app for browsing, searching, and resuming local AI coding conversations from Claude, Codex, and Gemini.

## Downloads

- `ChatMem_<version>_x64-setup.exe`: recommended installer
- `ChatMem_<version>_x64_en-US.msi`: MSI package
- `ChatMem-v<version>-portable.zip`: portable build

## Features

- Full-text search across conversation content
- Resume-command and storage-path copy actions
- Simplified Chinese and English UI
- In-app update checks

## Development

```powershell
npm ci
npm run test:run
npm run tauri build
```
```

- [ ] **Step 3: Add the MIT license**

```text
MIT License

Copyright (c) 2026 Rimagination

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files...
```

- [ ] **Step 4: Delete the obsolete Python/web files**

Run:

```powershell
Remove-Item -LiteralPath D:\VSP\agentswap-gui\app.py
Remove-Item -LiteralPath D:\VSP\agentswap-gui\app_fixed.py
Remove-Item -LiteralPath D:\VSP\agentswap-gui\backend.py
Remove-Item -LiteralPath D:\VSP\agentswap-gui\backend.log
Remove-Item -LiteralPath D:\VSP\agentswap-gui\server.py
Remove-Item -LiteralPath D:\VSP\agentswap-gui\start.py
Remove-Item -LiteralPath D:\VSP\agentswap-gui\start.bat
Remove-Item -LiteralPath D:\VSP\agentswap-gui\viewer.html
```

- [ ] **Step 5: Verify the full test and build suite**

Run:

```powershell
npm run test:run
cargo test --manifest-path D:\VSP\agentswap-gui\src-tauri\Cargo.toml
npm run tauri build
```

Expected:

```text
All Vitest suites pass
All Rust tests pass
Tauri build succeeds
```

- [ ] **Step 6: Initialize git and create the first public commit**

Run:

```powershell
cd D:\VSP\agentswap-gui
git init -b main
git add .
git commit -m "feat: prepare ChatMem for public release"
```

- [ ] **Step 7: Create and push the GitHub repo**

Preferred authenticated path if GitHub CLI is available:

```powershell
gh repo create Rimagination/ChatMem --public --source . --remote origin --push
```

Fallback authenticated path if the browser session is already signed in:

1. Create `https://github.com/new` in the authenticated browser session.
2. Set owner to `Rimagination`.
3. Set repository name to `ChatMem`.
4. Leave README, `.gitignore`, and license unchecked because the local repo already contains them.
5. After creation, run:

```powershell
git remote add origin https://github.com/Rimagination/ChatMem.git
git push -u origin main
```

- [ ] **Step 8: Tag and publish the first release**

Run:

```powershell
git tag v0.1.0
git push origin v0.1.0
```

Expected:

```text
GitHub Actions release workflow starts
Release assets include setup.exe, .msi, portable zip, updater latest.json, and updater signatures
```

---

## Self-Review

### Spec coverage

- Public repo cleanup and product-line narrowing: covered by Task 5.
- Settings entry and settings page: covered by Task 2.
- Language switching and persistence: covered by Tasks 1 and 2.
- Manual update checks and auto-check on launch: covered by Task 3.
- Tauri updater config and signing: covered by Task 4.
- GitHub Releases and Windows download assets: covered by Tasks 4 and 5.
- Automated release pipeline: covered by Task 4.

No spec gaps remain.

### Placeholder scan

- No `TODO`, `TBD`, or “implement later” markers remain.
- Each behavior task includes a concrete failing test, verification command, and minimal implementation sketch.
- Conditional publication logic is explicit rather than deferred.

### Type consistency

- Locale type is consistently `zh-CN | en`.
- Persisted settings shape stays `AppSettings`.
- Updater state is centralized as `UpdateCheckState`.

Plan complete and saved to `docs/superpowers/plans/2026-04-08-chatmem-release-settings-updater.md`. Two execution options:

**1. Subagent-Driven (recommended)** - I dispatch a fresh subagent per task, review between tasks, fast iteration

**2. Inline Execution** - Execute tasks in this session using executing-plans, batch execution with checkpoints

Which approach?
