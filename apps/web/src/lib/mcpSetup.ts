// Exact, copy-paste MCP setup text for the user's OS: shell commands quoted
// for that platform, JSON/TOML with the real server path escaped.

export type OS = "mac" | "windows" | "linux";

interface NavLike {
  platform?: string;
  userAgent?: string;
  userAgentData?: { platform?: string };
}

export function detectOS(nav: NavLike = typeof navigator !== "undefined" ? (navigator as NavLike) : {}): OS {
  const p = `${nav.userAgentData?.platform ?? ""} ${nav.platform ?? ""} ${nav.userAgent ?? ""}`;
  if (/mac|iphone|ipad/i.test(p)) return "mac";
  if (/win/i.test(p)) return "windows";
  return "linux";
}

/** Quote one argument for the platform's shell (POSIX sh/zsh/bash, or cmd/PowerShell). */
export function shellQuote(arg: string, os: OS): string {
  if (os === "windows") return `"${arg}"`;
  return `"${arg.replace(/([\\"$`])/g, "\\$1")}"`;
}

export interface Snippet {
  id: "claude" | "codex" | "json" | "toml";
  title: string;
  /** Where the text goes (terminal, or which config file). */
  where: string;
  code: string;
}

const CONFIG_FILES: Record<OS, { json: string; toml: string }> = {
  mac: {
    json: "Claude Desktop: ~/Library/Application Support/Claude/claude_desktop_config.json · Cursor: ~/.cursor/mcp.json",
    toml: "~/.codex/config.toml",
  },
  windows: {
    json: "Claude Desktop: %APPDATA%\\Claude\\claude_desktop_config.json · Cursor: %USERPROFILE%\\.cursor\\mcp.json",
    toml: "%USERPROFILE%\\.codex\\config.toml",
  },
  linux: {
    json: "Cursor: ~/.cursor/mcp.json · or any client's mcpServers config",
    toml: "~/.codex/config.toml",
  },
};

export function mcpSnippets(serverPath: string, os: OS): Snippet[] {
  const q = shellQuote(serverPath, os);
  const terminal = os === "windows" ? "Run in PowerShell or Command Prompt" : "Run in a terminal";
  return [
    { id: "claude", title: "Claude Code", where: terminal, code: `claude mcp add --scope user lumetrope -- node ${q}` },
    { id: "codex", title: "Codex CLI", where: terminal, code: `codex mcp add lumetrope -- node ${q}` },
    {
      id: "json",
      title: "Claude Desktop, Cursor & other clients (JSON)",
      where: CONFIG_FILES[os].json,
      code: JSON.stringify({ mcpServers: { lumetrope: { command: "node", args: [serverPath] } } }, null, 2),
    },
    // JSON string escapes are valid TOML basic-string escapes.
    { id: "toml", title: "Codex (config.toml)", where: CONFIG_FILES[os].toml, code: `[mcp_servers.lumetrope]\ncommand = "node"\nargs = [${JSON.stringify(serverPath)}]` },
  ];
}
