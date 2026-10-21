import { useEffect } from "react";
import { MCP_PORT, setLocalMcp } from "../mcp/client";
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

function Snippet({ title, code }: { title: string; code: string }) {
  return (
    <div className="snippet">
      <div className="snippet-head">
        <span>{title}</span>
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
  const server = "<path-to-lumetrope>/packages/mcp/dist/index.js";
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
          <Snippet title="Claude Code" code={`claude mcp add lumetrope -- node ${server}`} />
          <Snippet title="Any MCP client (JSON config)" code={JSON.stringify({ mcpServers: { lumetrope: { command: "node", args: [server] } } }, null, 2)} />
          <div className="muted tiny">
            The server talks to this window over ws://127.0.0.1:{MCP_PORT}. Only local Lumetrope windows can connect. Build it once with <span className="num">pnpm --filter lumetrope-mcp build</span>.
          </div>
        </div>
      </div>
    </div>
  );
}
