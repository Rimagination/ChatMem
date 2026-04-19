import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const codexPlugin = JSON.parse(
  readFileSync(resolve(process.cwd(), "plugins/chatmem/.codex-plugin/plugin.json"), "utf8"),
);
const claudePlugin = JSON.parse(
  readFileSync(resolve(process.cwd(), "plugins/chatmem/.claude-plugin/plugin.json"), "utf8"),
);
const mcpConfig = JSON.parse(
  readFileSync(resolve(process.cwd(), "plugins/chatmem/.mcp.json"), "utf8"),
);
const marketplace = JSON.parse(
  readFileSync(resolve(process.cwd(), ".agents/plugins/marketplace.json"), "utf8"),
);
const setupDoc = readFileSync(resolve(process.cwd(), "docs/CHATMEM_MCP_SETUP.md"), "utf8");
const syncScript = readFileSync(resolve(process.cwd(), "scripts/sync-chatmem-plugin.ps1"), "utf8");

describe("chatmem integration manifests", () => {
  it("registers the Codex plugin with MCP and skills", () => {
    expect(codexPlugin.name).toBe("chatmem");
    expect(codexPlugin.skills).toBe("./skills/");
    expect(codexPlugin.mcpServers).toBe("./.mcp.json");
    expect(codexPlugin.interface.displayName).toBe("ChatMem");
  });

  it("registers the Claude plugin shell for shared skills", () => {
    expect(claudePlugin.name).toBe("chatmem");
    expect(claudePlugin.skills).toBe("./skills/");
  });

  it("defines a local MCP server entry for chatmem", () => {
    expect(mcpConfig.mcpServers.chatmem).toBeDefined();
    expect(mcpConfig.mcpServers.chatmem.command).toBe("powershell");
    expect(mcpConfig.mcpServers.chatmem.args.join(" ")).toContain("run-chatmem-mcp.ps1");
  });

  it("adds chatmem to the local marketplace catalog", () => {
    expect(marketplace.plugins.some((plugin: { name: string }) => plugin.name === "chatmem")).toBe(
      true,
    );
  });

  it("ships a sync helper for installing the local plugin bundle", () => {
    expect(syncScript).toContain("[string]$CodexWorkspaceRoot");
    expect(syncScript).toContain('Join-Path $WorkspaceRoot "plugins"');
    expect(syncScript).toContain(".agents\\plugins\\marketplace.json");
    expect(syncScript).toContain(".claude\\plugins");
  });

  it("documents Codex app setup and review workflow", () => {
    expect(setupDoc).toContain("Codex App");
    expect(setupDoc).toContain("Memory Inbox");
    expect(setupDoc).toContain("chatmem-mcp");
    expect(setupDoc).toContain("-CodexWorkspaceRoot");
    expect(setupDoc).toContain("D:\\VSP");
  });
});
