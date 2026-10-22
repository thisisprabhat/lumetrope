import { describe, expect, it } from "vitest";
import { detectOS, mcpSnippets, shellQuote } from "./mcpSetup";

const byId = (s: ReturnType<typeof mcpSnippets>) => Object.fromEntries(s.map((x) => [x.id, x]));

describe("detectOS", () => {
  it("reads navigator platform data", () => {
    expect(detectOS({ userAgentData: { platform: "macOS" } })).toBe("mac");
    expect(detectOS({ platform: "Win32", userAgent: "Mozilla/5.0 (Windows NT 10.0; Win64; x64)" })).toBe("windows");
    expect(detectOS({ platform: "Linux x86_64", userAgent: "Mozilla/5.0 (X11; Linux x86_64)" })).toBe("linux");
  });
});

describe("shellQuote", () => {
  it("quotes for POSIX shells, escaping what double quotes expand", () => {
    expect(shellQuote("/Users/me/My Apps/x.mjs", "mac")).toBe('"/Users/me/My Apps/x.mjs"');
    expect(shellQuote('/tmp/a"b$c`d\\e', "linux")).toBe('"/tmp/a\\"b\\$c\\`d\\\\e"');
  });
  it("quotes Windows paths as-is (cmd and PowerShell)", () => {
    expect(shellQuote("C:\\Users\\Me\\AppData\\Roaming\\x.mjs", "windows")).toBe('"C:\\Users\\Me\\AppData\\Roaming\\x.mjs"');
  });
});

describe("mcpSnippets", () => {
  it("gives exact macOS commands and config", () => {
    const s = byId(mcpSnippets("/Users/me/Library/Application Support/app.lumetrope.editor/mcp/lumetrope-mcp.mjs", "mac"));
    expect(s.claude.code).toBe('claude mcp add --scope user lumetrope -- node "/Users/me/Library/Application Support/app.lumetrope.editor/mcp/lumetrope-mcp.mjs"');
    expect(s.codex.code).toBe('codex mcp add lumetrope -- node "/Users/me/Library/Application Support/app.lumetrope.editor/mcp/lumetrope-mcp.mjs"');
    expect(JSON.parse(s.json.code)).toEqual({ mcpServers: { lumetrope: { command: "node", args: ["/Users/me/Library/Application Support/app.lumetrope.editor/mcp/lumetrope-mcp.mjs"] } } });
    expect(s.json.where).toContain("~/Library/Application Support/Claude/claude_desktop_config.json");
    expect(s.toml.code).toBe('[mcp_servers.lumetrope]\ncommand = "node"\nargs = ["/Users/me/Library/Application Support/app.lumetrope.editor/mcp/lumetrope-mcp.mjs"]');
  });

  it("escapes Windows backslashes in JSON and TOML but not in the shell command", () => {
    const p = "C:\\Users\\Me\\AppData\\Roaming\\app.lumetrope.editor\\mcp\\lumetrope-mcp.mjs";
    const s = byId(mcpSnippets(p, "windows"));
    expect(s.claude.code).toBe(`claude mcp add --scope user lumetrope -- node "${p}"`);
    expect(s.json.code).toContain('"C:\\\\Users\\\\Me\\\\AppData');
    expect(JSON.parse(s.json.code).mcpServers.lumetrope.args[0]).toBe(p);
    expect(s.json.where).toContain("%APPDATA%\\Claude\\claude_desktop_config.json");
    expect(s.toml.code).toContain('args = ["C:\\\\Users\\\\Me');
    expect(s.toml.where).toContain("%USERPROFILE%\\.codex\\config.toml");
  });

  it("omits Claude Desktop paths on Linux, where it isn't available", () => {
    const s = byId(mcpSnippets("/home/me/.local/share/app.lumetrope.editor/mcp/lumetrope-mcp.mjs", "linux"));
    expect(s.json.where).not.toContain("claude_desktop_config");
    expect(s.json.where).toContain("~/.cursor/mcp.json");
  });
});
