// Connects the app to a local lumetrope-mcp server so AI agents can drive the
// editor, and registers the same tools with WebMCP when the browser offers it.
import { engine } from "../engine";
import { useEditor } from "../store";
import { runTool } from "./host";
import { backoffMs, parseRequest, response } from "./protocol";
import { localMcpSupported } from "../platform";

export const MCP_PORT = 47321;
let ws: WebSocket | null = null;
let attempt = 0;
let timer = 0;
let wanted = false;

function setStatus(mcpStatus: "off" | "connecting" | "connected" | "error") {
  useEditor.setState({ mcpStatus });
}

function connect() {
  if (!wanted) return;
  setStatus("connecting");
  try {
    ws = new WebSocket(`ws://127.0.0.1:${MCP_PORT}`);
  } catch {
    return retry();
  }
  ws.onopen = () => {
    attempt = 0;
    setStatus("connected");
    ws?.send(JSON.stringify({ type: "hello", app: "lumetrope", version: "0.1.0" }));
    useEditor.getState().toast("Local MCP bridge connected");
  };
  ws.onmessage = async (ev) => {
    const req = parseRequest(String(ev.data));
    if (!req) return;
    try {
      const result = await runTool(req.tool, req.args);
      ws?.send(response(req.id, result));
    } catch (e) {
      ws?.send(response(req.id, undefined, e instanceof Error ? e.message : String(e)));
    }
  };
  ws.onclose = (e) => {
    ws = null;
    if (e.code === 4001) {
      setStatus("error");
      useEditor.getState().toast("Another Lumetrope window is already connected to MCP", "error");
      return;
    }
    retry();
  };
  ws.onerror = () => undefined;
}

function retry() {
  if (!wanted) return setStatus("off");
  setStatus("connecting");
  clearTimeout(timer);
  timer = window.setTimeout(connect, backoffMs(attempt++));
}

export function setLocalMcp(on: boolean) {
  wanted = on;
  try {
    localStorage.setItem("lumetrope.mcp", on ? "1" : "0");
  } catch {
    /* ignore */
  }
  clearTimeout(timer);
  if (on) {
    attempt = 0;
    connect();
  } else {
    ws?.close();
    ws = null;
    setStatus("off");
  }
}

export function initMcp() {
  // In-browser WebMCP works on any origin that supports it; the local bridge only on this machine.
  registerWebMcp();
  if (!localMcpSupported()) return;
  let on = false;
  try {
    on = localStorage.getItem("lumetrope.mcp") === "1";
  } catch {
    /* ignore */
  }
  if (on) setLocalMcp(true);
}

/** WebMCP (navigator.modelContext): expose the same tools to in-browser agents. */
function registerWebMcp() {
  const mc = (navigator as any).modelContext;
  if (!mc) return;
  const tools = (JSON.parse(engine().catalog()) as { name: string; description: string; inputSchema: object }[]).map((t) => ({
    name: t.name,
    description: t.description,
    inputSchema: t.inputSchema,
    execute: async (args: Record<string, unknown>) => ({ content: [{ type: "text", text: JSON.stringify(await runTool(t.name, args ?? {})) }] }),
  }));
  try {
    if (typeof mc.provideContext === "function") mc.provideContext({ tools });
    else if (typeof mc.registerTool === "function") tools.forEach((t) => mc.registerTool(t));
    useEditor.setState({ webMcp: true });
  } catch {
    /* WebMCP API shape differs; ignore */
  }
}
