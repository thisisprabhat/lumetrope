import { useEffect, useState } from "react";
import { detectOS, mcpSnippets, type OS } from "../lib/mcpSetup";
import { MCP_PORT, setLocalMcp } from "../mcp/client";
import { mcpServerPath } from "../platform";
import { useEditor } from "../store";
import { Toggle } from "../ui/controls";
import { Icon } from "./Icon";

const STATUS: Record<string, string> = { off: "Off", connecting: "Waiting for lumetrope-mcp…", connected: "Connected", error: "Error" };

export function McpBadge() {
  const status = useEditor((s) => s.mcpStatus);
  const webMcp = useEditor((s) => s.webMcp);
  return (
    <button className={`mcp-badge s-${status}`} onClick={() => useEditor.setState({ mcpHelpOpen: true })} data-tip="AI agents via MCP">
      <span className="dot" />
      MCP{webMcp ? " · Web" : ""}
    </button>
  );
}

const OS_NAME: Record<OS, string> = { mac: "macOS", windows: "Windows", linux: "Linux" };

function Snippet({ title, where, code }: { title: string; where?: string; code: string }) {
  return (
    <div className="snippet">
      <div className="snippet-head">
        <span>
          {title}
          {where && <span className="snippet-where">{where}</span>}
        </span>
        <button className="chip-btn" onClick={() => void navigator.clipboard?.writeText(code).then(() => useEditor.getState().toast("Copied"))}>
          Copy
        </button>
      </div>
      <pre className="num">{code}</pre>
    </div>
  );
}

export function McpDialog() {
  const status = useEditor((s) => s.mcpStatus);
  const close = () => useEditor.setState({ mcpHelpOpen: false });
  useEffect(() => {
    const esc = (e: KeyboardEvent) => e.key === "Escape" && close();
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, []);
  const os = detectOS();
  const [server, setServer] = useState<{ path: string; source: "app" | "build" } | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    mcpServerPath().then(setServer, (e) => setError(String(e)));
  }, []);
  return (
    <div className="modal-backdrop" onPointerDown={(e) => e.target === e.currentTarget && close()}>
      <div className="modal wide" role="dialog" aria-label="MCP">
        <header className="modal-head">
          <span>Connect AI agents (MCP)</span>
          <button className="icon-btn" onClick={close} aria-label="Close">
            <Icon name="close" />
          </button>
        </header>
        <div className="modal-body">
          <p className="muted">
            Lumetrope exposes 110+ editing tools — scenes, shapes, keyframes, effects, themes, segments, state machines, history and live previews — to agents like Claude Code, Cursor or Codex through the Model Context Protocol.
          </p>
          <div className="row" style={{ justifyContent: "space-between" }}>
            <Toggle label="Enable Local MCP" value={status !== "off"} onChange={setLocalMcp} />
            <span className={`mcp-badge s-${status}`}>
              <span className="dot" />
              {STATUS[status]}
            </span>
          </div>
          <div className="mcp-steps muted">
            <span>
              1. Copy the setup for your agent ({OS_NAME[os]}) · 2. Turn on <b>Enable Local MCP</b> above · 3. Ask your agent to read the scene.
            </span>
          </div>
          {error && <div className="bad tiny">Could not locate the MCP server: {error}</div>}
          {server && mcpSnippets(server.path, os).map((s) => <Snippet key={s.id} title={s.title} where={s.where} code={s.code} />)}
          {server && (
            <div className="muted tiny">
              Server: <span className="num path-text">{server.path}</span>
              {server.source === "app" ? " (installed by this app, kept up to date)." : " (built from this repository; rebuild with "}
              {server.source === "build" && <span className="num">pnpm --filter lumetrope-mcp build</span>}
              {server.source === "build" && ")."} Needs Node.js 20+ on your PATH. The server talks to this window over ws://127.0.0.1:{MCP_PORT}; only local Lumetrope windows can connect.
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
