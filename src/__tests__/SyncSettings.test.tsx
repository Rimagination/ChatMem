import { fireEvent, render, screen, waitFor } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { within } from "@testing-library/react";
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

function clickSegmentedOption(groupName: string, optionName: string) {
  const group = screen.getByRole("radiogroup", { name: groupName });
  fireEvent.click(within(group).getByRole("radio", { name: optionName }));
}

function expectSegmentedOptionSelected(groupName: string, optionName: string) {
  const group = screen.getByRole("radiogroup", { name: groupName });
  expect(within(group).getByRole("radio", { name: optionName }).getAttribute("aria-checked")).toBe(
    "true",
  );
}

describe("Sync settings", () => {
  beforeEach(() => {
    localStorage.clear();
    localStorage.setItem(
      SETTINGS_STORAGE_KEY,
      JSON.stringify({ locale: "en", autoCheckUpdates: false, autoCaptureMemory: false }),
    );

    mockInvoke.mockReset();
    mockInvoke.mockResolvedValue([]);
  });

  it("persists a Zotero-style WebDAV conversation-data profile without a fake provider dropdown", async () => {
    renderApp();

    expect(screen.queryByRole("button", { name: "About us" })).toBeNull();

    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    expect(await screen.findByRole("heading", { name: "General" })).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "About ChatMem" })).toBeNull();

    fireEvent.click(screen.getByRole("button", { name: "About" }));
    expect(await screen.findByRole("heading", { name: "About ChatMem" })).toBeTruthy();
    expect(screen.getByText(/keeps local project memory organized/i)).toBeTruthy();
    expect(screen.getByText(/Current version v/)).toBeTruthy();
    expect(screen.getByText("Cleaner settings")).toBeTruthy();
    expect(screen.getByText("Smoother agent handoff")).toBeTruthy();
    expect(screen.queryByText("What changed in 1.1.4")).toBeNull();
    expect(screen.getByText("Rimagination/ChatMem")).toBeTruthy();
    expect(screen.queryByRole("heading", { name: "Acknowledgements" })).toBeNull();
    expect(screen.getByText("References")).toBeTruthy();
    expect(screen.getByText(/mem0/)).toBeTruthy();
    expect(screen.getByText(/Letta/)).toBeTruthy();
    expect(screen.getByText(/Zep/)).toBeTruthy();
    expect(screen.getByText(/LLM Wiki/)).toBeTruthy();

    fireEvent.click(screen.getByRole("button", { name: "Conversation Data Sync" }));
    expect(await screen.findByRole("heading", { name: "Conversation Data Sync" })).toBeTruthy();
    expect(screen.queryByText(/Use a generic WebDAV server/)).toBeNull();
    expect(screen.queryByText(/Account details/)).toBeNull();

    clickSegmentedOption("Conversation data sync method:", "WebDAV");
    expectSegmentedOptionSelected("Conversation data sync method:", "WebDAV");
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
    clickSegmentedOption("Download files", "As needed");
    expectSegmentedOptionSelected("Download files", "As needed");

    await waitFor(() => {
      const saved = JSON.parse(localStorage.getItem(SETTINGS_STORAGE_KEY) ?? "{}");
      expect(saved.sync).toEqual({
        provider: "webdav",
        webdavScheme: "https",
        webdavHost: "example.com",
        webdavPath: "webdav",
        username: "liang@example.com",
        remotePath: "chatmem",
        syncFolder: "",
        downloadMode: "as-needed",
      });
      expect(saved.sync.password).toBeUndefined();
      expect(JSON.stringify(saved.sync)).not.toContain("local-secret");
    });
  });

  it("restores WebDAV sync settings from the native settings file when browser storage was reset", async () => {
    localStorage.clear();
    mockInvoke.mockImplementation((command: string) => {
      if (command === "load_app_settings") {
        return Promise.resolve({
          locale: "en",
          autoCheckUpdates: false,
          sync: {
            provider: "webdav",
            webdavScheme: "https",
            webdavHost: "dav.example.com",
            webdavPath: "remote.php/dav/files/liang",
            username: "liang@example.com",
            remotePath: "chatmem",
            downloadMode: "as-needed",
          },
        });
      }
      if (command === "load_webdav_password") {
        return Promise.resolve("saved-secret");
      }
      return Promise.resolve([]);
    });

    renderApp();

    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    fireEvent.click(screen.getByRole("button", { name: "Conversation Data Sync" }));

    await waitFor(() => {
      expectSegmentedOptionSelected("Conversation data sync method:", "WebDAV");
      expect((screen.getByLabelText("Server and path") as HTMLInputElement).value).toBe(
        "dav.example.com/remote.php/dav/files/liang",
      );
      expect((screen.getByLabelText("Username") as HTMLInputElement).value).toBe(
        "liang@example.com",
      );
      expect((screen.getByLabelText("Password") as HTMLInputElement).value).toBe("saved-secret");
      expectSegmentedOptionSelected("Download files", "As needed");
    });

    expect(mockInvoke).toHaveBeenCalledWith("load_app_settings");
    expect(mockInvoke).toHaveBeenCalledWith("load_webdav_password", {
      username: "liang@example.com",
    });
    expect(JSON.parse(localStorage.getItem(SETTINGS_STORAGE_KEY) ?? "{}").sync).toEqual({
      provider: "webdav",
      webdavScheme: "https",
      webdavHost: "dav.example.com",
      webdavPath: "remote.php/dav/files/liang",
      username: "liang@example.com",
      remotePath: "chatmem",
      syncFolder: "",
      downloadMode: "as-needed",
    });
  });

  it("verifies the WebDAV server with the entered password and shows success", async () => {
    renderApp();

    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    fireEvent.click(screen.getByRole("button", { name: "Conversation Data Sync" }));
    clickSegmentedOption("Conversation data sync method:", "WebDAV");
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

    mockInvoke.mockResolvedValueOnce(undefined);
    fireEvent.click(screen.getByRole("button", { name: "Verify server" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("verify_webdav_server", {
        webdavScheme: "https",
        webdavHost: "example.com",
        webdavPath: "webdav",
        remotePath: "chatmem",
        username: "liang@example.com",
        password: "local-secret",
      });
      expect(mockInvoke).toHaveBeenCalledWith("save_webdav_password", {
        username: "liang@example.com",
        password: "local-secret",
      });
      expect(screen.getByText("Verification successful")).toBeTruthy();
    });
  });

  it("runs a real WebDAV sync after credentials are entered", async () => {
    renderApp();

    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    fireEvent.click(screen.getByRole("button", { name: "Conversation Data Sync" }));
    clickSegmentedOption("Conversation data sync method:", "WebDAV");
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

    mockInvoke.mockResolvedValueOnce({
      uploadedCount: 2,
      remoteUrl: "https://example.com/webdav/chatmem/",
    });
    fireEvent.click(screen.getByRole("button", { name: "Sync now" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("sync_webdav_now", {
        webdavScheme: "https",
        webdavHost: "example.com",
        webdavPath: "webdav",
        remotePath: "chatmem",
        username: "liang@example.com",
        password: "local-secret",
      });
      expect(screen.getByText("Synced 2 files to WebDAV")).toBeTruthy();
      expect(screen.getByText("Remote folder: https://example.com/webdav/chatmem/")).toBeTruthy();
    });
  });

  it("runs an upgrade readiness check from settings", async () => {
    mockInvoke.mockImplementation((command: string) => {
      if (command === "run_upgrade_readiness_check") {
        return Promise.resolve({
          status: "warning",
          summary: "Data check found 1 item that needs attention.",
          checks: [
            {
              key: "settings",
              label: "App settings",
              status: "ok",
              detail: "Settings are available.",
            },
            {
              key: "webdav_password",
              label: "WebDAV password",
              status: "warning",
              detail: "Password is missing.",
            },
            {
              key: "memory_store",
              label: "Local memory",
              status: "ok",
              detail: "Local memory can be opened.",
            },
          ],
          warnings: ["Password is missing."],
        });
      }
      return Promise.resolve([]);
    });

    renderApp();

    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    fireEvent.click(screen.getByRole("button", { name: "Updates" }));
    fireEvent.click(await screen.findByRole("button", { name: "Check data" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("run_upgrade_readiness_check");
      expect(screen.getByText("Data check found 1 item that needs attention.")).toBeTruthy();
      expect(screen.getByText("App settings")).toBeTruthy();
      expect(screen.getByText("WebDAV password")).toBeTruthy();
      expect(screen.getByText("Local memory")).toBeTruthy();
    });
  });

  it("installs ChatMem MCP and Skill into local agents from settings", async () => {
    mockInvoke.mockImplementation((command: string, payload?: Record<string, unknown>) => {
      if (command === "detect_agent_integrations") {
        return Promise.resolve([
          {
            agent: "codex",
            label: "Codex",
            configPath: "C:/Users/demo/.codex/config.toml",
            instructionsPath: "C:/Users/demo/.agents/skills/chatmem/SKILL.md",
            mcpInstalled: false,
            instructionsInstalled: false,
            instructionsOutdated: false,
            configExists: true,
            status: "not_installed",
            statusLabel: "Not installed",
            commandPreview: '"C:/Program Files/ChatMem/ChatMem.exe" --mcp',
            details: [],
          },
        ]);
      }

      if (command === "install_agent_integration") {
        expect(payload).toEqual({ agent: "all" });
        return Promise.resolve([
          {
            agent: "codex",
            label: "Codex",
            changed: true,
            message: "Codex integration installed.",
            backupPaths: [],
            status: {
              agent: "codex",
              label: "Codex",
              configPath: "C:/Users/demo/.codex/config.toml",
              instructionsPath: "C:/Users/demo/.agents/skills/chatmem/SKILL.md",
              mcpInstalled: true,
              instructionsInstalled: true,
              instructionsOutdated: false,
              configExists: true,
              status: "ready",
              statusLabel: "Ready",
              commandPreview: '"C:/Program Files/ChatMem/ChatMem.exe" --mcp',
              details: [],
            },
          },
        ]);
      }

      return Promise.resolve([]);
    });

    renderApp();

    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    fireEvent.click(screen.getByRole("button", { name: "Agent integration" }));
    expect(await screen.findByRole("heading", { name: "Agent integration" })).toBeTruthy();
    expect((await screen.findAllByText("Codex")).length).toBeGreaterThan(0);
    expect(screen.getByText(/Connect supported agents/)).toBeTruthy();
    expect(screen.getAllByText("Instructions").length).toBeGreaterThan(0);

    fireEvent.click(screen.getByRole("button", { name: "Set up all" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("install_agent_integration", { agent: "all" });
      expect(screen.getByText("All detected agents are set up.")).toBeTruthy();
      expect(screen.getByText("Ready")).toBeTruthy();
    });
  });

  it("shows outdated agent guidance as repairable from settings", async () => {
    mockInvoke.mockImplementation((command: string, payload?: Record<string, unknown>) => {
      if (command === "detect_agent_integrations") {
        return Promise.resolve([
          {
            agent: "codex",
            label: "Codex",
            configPath: "C:/Users/demo/.codex/config.toml",
            instructionsPath: "C:/Users/demo/.agents/skills/chatmem/SKILL.md",
            mcpInstalled: true,
            instructionsInstalled: true,
            instructionsOutdated: true,
            configExists: true,
            status: "partial",
            statusLabel: "Needs update",
            commandPreview: '"C:/Program Files/ChatMem/ChatMem.exe" --mcp',
            details: ["ChatMem instructions need an update."],
          },
        ]);
      }

      if (command === "install_agent_integration") {
        expect(payload).toEqual({ agent: "codex" });
        return Promise.resolve([]);
      }

      return Promise.resolve([]);
    });

    renderApp();

    fireEvent.click(await screen.findByRole("button", { name: "Settings" }));
    fireEvent.click(screen.getByRole("button", { name: "Agent integration" }));
    expect((await screen.findAllByText("Needs update")).length).toBeGreaterThan(0);

    fireEvent.click(screen.getByRole("button", { name: "Update" }));

    await waitFor(() => {
      expect(mockInvoke).toHaveBeenCalledWith("install_agent_integration", { agent: "codex" });
    });
  });
});
