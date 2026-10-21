import { afterEach, describe, expect, it } from "vitest";
import WebSocket from "ws";
import { Bridge, isAllowedOrigin } from "../src/bridge";

let bridge: Bridge | null = null;
afterEach(async () => {
  await bridge?.stop();
  bridge = null;
});

const port = () => 47000 + Math.floor(Math.random() * 900);

function app(p: number, origin = "http://localhost:5173"): Promise<WebSocket> {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(`ws://127.0.0.1:${p}`, { origin });
    ws.on("open", () => resolve(ws));
    ws.on("error", reject);
  });
}

describe("origins", () => {
  it("allows local dev servers and the desktop shell only", () => {
    expect(isAllowedOrigin("http://localhost:5173")).toBe(true);
    expect(isAllowedOrigin("http://127.0.0.1:4173")).toBe(true);
    expect(isAllowedOrigin("tauri://localhost")).toBe(true);
    expect(isAllowedOrigin("http://tauri.localhost")).toBe(true);
    expect(isAllowedOrigin("https://evil.example")).toBe(false);
    expect(isAllowedOrigin("http://localhost.evil.example")).toBe(false);
    expect(isAllowedOrigin(undefined)).toBe(false);
  });
});

describe("bridge", () => {
  it("rejects_disallowed_origin", async () => {
    const p = port();
    bridge = new Bridge({ port: p });
    await bridge.start();
    const ws = new WebSocket(`ws://127.0.0.1:${p}`, { origin: "https://evil.example" });
    const code = await new Promise<number>((r) => ws.on("close", (c) => r(c)));
    expect(code).toBe(1008);
    expect(bridge.connected).toBe(false);
  });

  it("forwards_request_and_resolves_response", async () => {
    const p = port();
    bridge = new Bridge({ port: p });
    await bridge.start();
    const ws = await app(p);
    ws.on("message", (raw) => {
      const m = JSON.parse(String(raw));
      if (m.type === "tool_request") ws.send(JSON.stringify({ type: "tool_response", id: m.id, result: { echoed: m.tool, args: m.args } }));
    });
    ws.send(JSON.stringify({ type: "hello", app: "lumetrope", version: "test" }));
    await new Promise((r) => setTimeout(r, 50));
    expect(bridge.connected).toBe(true);
    await expect(bridge.call("read_scene", { scene: "root" })).resolves.toEqual({ echoed: "read_scene", args: { scene: "root" } });
    ws.close();
  });

  it("propagates tool errors", async () => {
    const p = port();
    bridge = new Bridge({ port: p });
    await bridge.start();
    const ws = await app(p);
    ws.on("message", (raw) => {
      const m = JSON.parse(String(raw));
      ws.send(JSON.stringify({ type: "tool_response", id: m.id, error: "layer 'x' not found" }));
    });
    await new Promise((r) => setTimeout(r, 30));
    await expect(bridge.call("delete_layer", { layer: "x" })).rejects.toThrow("layer 'x' not found");
    ws.close();
  });

  it("times_out_without_app", async () => {
    const p = port();
    bridge = new Bridge({ port: p, timeoutMs: 150, connectWaitMs: 50 });
    await bridge.start();
    await expect(bridge.call("read_scene", {})).rejects.toThrow(/Lumetrope is not connected/);
  });

  it("times out when the app does not answer", async () => {
    const p = port();
    bridge = new Bridge({ port: p, timeoutMs: 100 });
    await bridge.start();
    const ws = await app(p);
    await new Promise((r) => setTimeout(r, 30));
    await expect(bridge.call("read_scene", {})).rejects.toThrow(/timed out/);
    ws.close();
  });

  it("keeps only one app connection", async () => {
    const p = port();
    bridge = new Bridge({ port: p });
    await bridge.start();
    const a = await app(p);
    await new Promise((r) => setTimeout(r, 30));
    const b = new WebSocket(`ws://127.0.0.1:${p}`, { origin: "http://localhost:5173" });
    const code = await new Promise<number>((r) => b.on("close", (c) => r(c)));
    expect(code).toBe(4001);
    a.close();
  });
});
