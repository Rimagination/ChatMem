import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { describe, expect, it } from "vitest";

const mcpConfig = JSON.parse(
  readFileSync(resolve(process.cwd(), ".mcp.json"), "utf8"),
);
const setupDoc = readFileSync(resolve(process.cwd(), "docs/CHATMEM_MCP_SETUP.md"), "utf8");
const skillDoc = readFileSync(resolve(process.cwd(), "skills/chatmem/SKILL.md"), "utf8");
const skillOpenAiYaml = readFileSync(
  resolve(process.cwd(), "skills/chatmem/agents/openai.yaml"),
  "utf8",
);
const tauriConfig = JSON.parse(
  readFileSync(resolve(process.cwd(), "src-tauri/tauri.conf.json"), "utf8"),
);
const appStyles = readFileSync(resolve(process.cwd(), "src/styles.css"), "utf8");

describe("chatmem integration manifests", () => {
  it("defines a local MCP server entry for chatmem", () => {
    expect(mcpConfig.mcpServers.chatmem).toBeDefined();
    expect(mcpConfig.mcpServers.chatmem.command).toBe("powershell");
    expect(mcpConfig.mcpServers.chatmem.args.join(" ")).toContain("run-chatmem-mcp.ps1");
    expect(mcpConfig.mcpServers.chatmem.args.join(" ")).toContain("./mcp/run-chatmem-mcp.ps1");
  });

  it("keeps ChatMem as MCP plus skill without local plugin wrappers", () => {
    expect(existsSync(resolve(process.cwd(), "mcp/run-chatmem-mcp.ps1"))).toBe(true);
    expect(existsSync(resolve(process.cwd(), "skills/chatmem/SKILL.md"))).toBe(true);
    expect(existsSync(resolve(process.cwd(), "plugins/chatmem"))).toBe(false);
    expect(existsSync(resolve(process.cwd(), ".agents/plugins/marketplace.json"))).toBe(false);
    expect(existsSync(resolve(process.cwd(), "scripts/sync-chatmem-plugin.ps1"))).toBe(false);
  });

  it("documents Codex app setup and review workflow", () => {
    expect(setupDoc).toContain("Codex App");
    expect(setupDoc).toContain("There is no local plugin wrapper");
    expect(setupDoc).toContain("Skill");
    expect(setupDoc).toContain("chatmem-mcp");
    expect(setupDoc).toContain(".mcp.json");
    expect(setupDoc).toContain("mcp\\run-chatmem-mcp.ps1");
    expect(setupDoc).not.toContain("marketplace");
  });

  it("defines a ChatMem skill around MCP-first memory, checkpoint, and handoff flows", () => {
    expect(skillDoc).toContain("get_repo_memory");
    expect(skillDoc).toContain("search_repo_history");
    expect(skillDoc).toContain("create_memory_candidate");
    expect(skillDoc).toContain("build_handoff_packet");
    expect(skillDoc).toContain("get_project_context");
    expect(skillDoc).toContain("history evidence");
    expect(skillDoc).toContain("approved startup rules");
    expect(skillDoc.toLowerCase()).toContain("checkpoint");
    expect(skillDoc).toContain("@chatmem");
    expect(skillDoc).toContain("desktop app");
  });

  it("declares the chatmem MCP dependency in skill metadata", () => {
    expect(skillOpenAiYaml).toContain('display_name: "ChatMem"');
    expect(skillOpenAiYaml).toContain('default_prompt: "Use $chatmem');
    expect(skillOpenAiYaml).toContain('type: "mcp"');
    expect(skillOpenAiYaml).toContain('value: "chatmem"');
  });

  it("allows native window controls for the frameless ChatMem window", () => {
    expect(tauriConfig.tauri.windows[0].decorations).toBe(false);
    expect(tauriConfig.tauri.windows[0].transparent).toBe(true);
    expect(tauriConfig.tauri.allowlist.window?.all).toBe(true);
  });

  it("keeps the desktop shell fixed while sidebar and workspace scroll independently", () => {
    expect(appStyles).toMatch(/html,\s*body,\s*#root\s*\{[\s\S]*height:\s*100%;[\s\S]*overflow:\s*hidden;/);
    expect(appStyles).toMatch(/#root\s*\{[\s\S]*background:\s*transparent;/);
    expect(appStyles).toMatch(/\.app-shell\s*\{[\s\S]*width:\s*calc\(100% - 16px\);[\s\S]*height:\s*calc\(100% - 16px\);[\s\S]*margin:\s*8px;[\s\S]*overflow:\s*hidden;[\s\S]*border-radius:\s*12px;/);
    expect(appStyles).toMatch(/\.app-shell\.is-window-filled\s*\{[\s\S]*width:\s*100%;[\s\S]*height:\s*100%;[\s\S]*margin:\s*0;[\s\S]*border-radius:\s*0;/);
    expect(appStyles).toMatch(/\.sidebar-scroll\s*\{[\s\S]*overflow-y:\s*auto;/);
    expect(appStyles).toMatch(/\.workspace-surface\s*\{[\s\S]*overflow-y:\s*auto;/);
    expect(appStyles).toMatch(/\.settings-row\s*\{[\s\S]*flex:\s*0 0 auto;/);
  });
});
