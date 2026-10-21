// WebSocket bridge between this MCP server (stdio) and the running Lumetrope app.
// The app connects as a client; tool calls are forwarded as
// {type:"tool_request", id, tool, args} and answered with
// {type:"tool_response", id, result | error}.
import { randomUUID } from "node:crypto";
import { WebSocketServer, type WebSocket } from "ws";

export const DEFAULT_PORT = 47321;

/** Only local dev servers and the desktop shell may connect. */
export function isAllowedOrigin(origin: string | undefined): boolean {
  if (!origin) return false;
  if (origin === "tauri://localhost") return true;
  return /^https?:\/\/(localhost|127\.0\.0\.1|\[::1\]|tauri\.localhost)(:\d+)?$/.test(origin);
}

export interface BridgeOptions {
  port?: number;
  host?: string;
  /** Per-call timeout once the app is connected. */
  timeoutMs?: number;
  /** How long a call waits for the app to connect before failing. */
  connectWaitMs?: number;
  log?: (msg: string) => void;
}

interface Pending {
  resolve: (v: unknown) => void;
  reject: (e: Error) => void;
  timer: NodeJS.Timeout;
}

export class Bridge {
  private wss: WebSocketServer | null = null;
  private sock: WebSocket | null = null;
  private pending = new Map<string, Pending>();
  private waiters: (() => void)[] = [];
  private ping: NodeJS.Timeout | null = null;
  appVersion: string | null = null;
  readonly port: number;

  constructor(private opts: BridgeOptions = {}) {
    this.port = opts.port ?? DEFAULT_PORT;
  }

  get connected(): boolean {
    return !!this.sock && this.sock.readyState === this.sock.OPEN;
  }

  private log(m: string) {
    this.opts.log?.(m);
  }

  start(): Promise<void> {
    return new Promise((resolve, reject) => {
      const wss = new WebSocketServer({ port: this.port, host: this.opts.host ?? "127.0.0.1", maxPayload: 32 * 1024 * 1024 });
      this.wss = wss;
      wss.on("listening", () => {
        this.log(`listening on ws://127.0.0.1:${this.port}`);
        resolve();
      });
      wss.on("error", (e: NodeJS.ErrnoException) => {
        reject(e.code === "EADDRINUSE" ? new Error(`Port ${this.port} is already in use (is another lumetrope-mcp running?). Set LUMETROPE_MCP_PORT to change it.`) : e);
      });
      wss.on("connection", (ws, req) => {
        if (!isAllowedOrigin(req.headers.origin)) {
          this.log(`rejected origin ${req.headers.origin}`);
          ws.close(1008, "Origin not allowed");
          return;
        }
        if (this.connected) {
          ws.close(4001, "Another Lumetrope window is already connected");
          return;
        }
        this.sock = ws;
        this.log("app connected");
        this.waiters.splice(0).forEach((w) => w());
        let alive = true;
        ws.on("pong", () => (alive = true));
        this.ping = setInterval(() => {
          if (!alive) return ws.terminate();
          alive = false;
          ws.ping();
        }, 15000);
        ws.on("message", (raw) => this.onMessage(String(raw)));
        ws.on("close", () => {
          if (this.sock === ws) {
            this.sock = null;
            if (this.ping) clearInterval(this.ping);
            for (const [id, p] of this.pending) {
              clearTimeout(p.timer);
              p.reject(new Error("Lumetrope disconnected before answering"));
              this.pending.delete(id);
            }
            this.log("app disconnected");
          }
        });
      });
    });
  }

  private onMessage(raw: string) {
    let m: any;
    try {
      m = JSON.parse(raw);
    } catch {
      return;
    }
    if (m?.type === "hello") {
      this.appVersion = String(m.version ?? "");
      return;
    }
    if (m?.type !== "tool_response" || typeof m.id !== "string") return;
    const p = this.pending.get(m.id);
    if (!p) return;
    clearTimeout(p.timer);
    this.pending.delete(m.id);
    if (m.error !== undefined && m.error !== null) p.reject(new Error(String(m.error)));
    else p.resolve(m.result);
  }

  private waitForApp(ms: number): Promise<boolean> {
    if (this.connected) return Promise.resolve(true);
    return new Promise((resolve) => {
      const t = setTimeout(() => {
        this.waiters = this.waiters.filter((w) => w !== done);
        resolve(false);
      }, ms);
      const done = () => {
        clearTimeout(t);
        resolve(true);
      };
      this.waiters.push(done);
    });
  }

  async call(tool: string, args: unknown): Promise<unknown> {
    if (!(await this.waitForApp(this.opts.connectWaitMs ?? 3000))) {
      throw new Error(`Lumetrope is not connected. Open Lumetrope and turn on Settings ▸ Local MCP (it connects to port ${this.port}).`);
    }
    const id = randomUUID();
    return new Promise((resolve, reject) => {
      const timer = setTimeout(() => {
        this.pending.delete(id);
        reject(new Error(`Lumetrope timed out after ${this.opts.timeoutMs ?? 30000} ms running '${tool}'`));
      }, this.opts.timeoutMs ?? 30000);
      this.pending.set(id, { resolve, reject, timer });
      this.sock!.send(JSON.stringify({ type: "tool_request", id, tool, args: args ?? {} }));
    });
  }

  async stop(): Promise<void> {
    if (this.ping) clearInterval(this.ping);
    for (const p of this.pending.values()) clearTimeout(p.timer);
    this.pending.clear();
    this.sock?.close();
    await new Promise<void>((r) => (this.wss ? this.wss.close(() => r()) : r()));
    this.wss = null;
  }
}
