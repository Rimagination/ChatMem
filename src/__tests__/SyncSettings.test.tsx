import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import App from "../App";
import { I18nProvider } from "../i18n/I18nProvider";
import { SETTINGS_STORAGE_KEY } from "../settings/storage";

const mockInvoke = vi.fn();

vi.mock("@tauri-apps/api/tauri", () => ({
  invoke: (...args: unknown[]) => mockInvoke(...args),
}));

vi.mock("@tauri-apps/api/updater", () => ({
  checkUpdate: vi.fn().mockResolvedValue({ shouldUpdate: false }),
  installUpdate: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@tauri-apps/api/process", () => ({
  relaunch: vi.fn().mockResolvedValue(undefined),
}));

vi.mock("@tauri-apps/api/window", () => ({
  appWindow: {
    minimize: vi.fn(),
    toggleMaximize: vi.fn(),
    close: vi.fn(),
  },
}));

function renderApp() {
  return render(
    <I18nProvider>
      <App />
    </I18nProvider>,
  );
}

describe("Sync settings", () => {
  beforeEach(() => {
    localStorage.clear();
    localStorage.setItem(
      SETTINGS_STORAGE_KEY,
      JSON.stringify({ locale: "en", autoCheckUpdates: false }),
    );

    mockInvoke.mockReset();
    mockInvoke.mockResolvedValue([]);
  });

  it("persists a Zotero-style WebDAV conversation-data profile without a fake provider dropdown", async () => {
    renderApp();

    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));

    expect(await screen.findByRole("heading", { name: "Conversation Data Sync" })).toBeTruthy();
    expect(screen.queryByText(/Use a generic WebDAV server/)).toBeNull();
    expect(screen.queryByText(/Account details/)).toBeNull();

    fireEvent.click(screen.getByLabelText("Conversation data sync method:"));
    const webdavLabel = screen.getByText("WebDAV");
    expect(webdavLabel.closest("select")).toBeNull();
    expect(screen.queryByText(/Passwords are kept/)).toBeNull();
    fireEvent.change(screen.getByLabelText("Protocol"), {
      target: { value: "https" },
    });
    fireEvent.change(screen.getByLabelText("Server and path"), {
      target: { value: "example.com/webdav" },
    });
    fireEvent.change(screen.getByLabelText("Username"), {
      target: { value: "liang@example.com" },
    });
    fireEvent.change(screen.getByLabelText("Password"), {
      target: { value: "local-secret" },
    });
    fireEvent.change(screen.getByLabelText("Download files"), {
      target: { value: "as-needed" },
    });

    await waitFor(() => {
      const saved = JSON.parse(localStorage.getItem(SETTINGS_STORAGE_KEY) ?? "{}");
      expect(saved.sync).toEqual({
        provider: "webdav",
        webdavScheme: "https",
        webdavHost: "example.com",
        webdavPath: "webdav",
        username: "liang@example.com",
        remotePath: "chatmem",
        downloadMode: "as-needed",
      });
      expect(saved.sync.password).toBeUndefined();
      expect(JSON.stringify(saved.sync)).not.toContain("local-secret");
    });
  });
});
