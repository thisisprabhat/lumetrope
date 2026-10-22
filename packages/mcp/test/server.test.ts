import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import WebSocket from "ws";
import { fileURLToPath } from "node:url";

const port = 46000 + Math.floor(Math.random() * 900);
let client: Client;
let app: WebSocket | null = null;

beforeAll(async () => {
  const transport = new StdioClientTransport({
    command: process.execPath,
    args: [fileURLToPath(new URL("../dist/lumetrope-mcp.mjs", import.meta.url))],
    env: { ...process.env, LUMETROPE_MCP_PORT: String(port) } as Record<string, string>,
    stderr: "ignore",
  });
  client = new Client({ name: "test", version: "0" });
  await client.connect(transport);
});

afterAll(async () => {
  app?.close();
  await client.close();
});

describe("lumetrope-mcp over stdio", () => {
  it("lists catalogue and host tools", async () => {
    const { tools } = await client.listTools();
    const names = tools.map((t) => t.name);
    expect(names.length).toBeGreaterThan(100);
    for (const n of ["read_scene", "create_shape", "set_keyframe", "add_state_machine", "get_frame_image", "save_version", "preview_state_machine"]) expect(names).toContain(n);
    expect(tools.every((t) => t.inputSchema.type === "object")).toBe(true);
  });

  it("explains how to connect when the app is not running", async () => {
    const r = await client.callTool({ name: "read_scene", arguments: {} });
    expect(r.isError).toBe(true);
    expect(JSON.stringify(r.content)).toContain("Local MCP");
  }, 10000);

  it("forwards calls to the connected app", async () => {
    app = new WebSocket(`ws://127.0.0.1:${port}`, { origin: "http://localhost:5173" });
    await new Promise((r) => app!.on("open", r));
    app.on("message", (raw) => {
      const m = JSON.parse(String(raw));
      app!.send(JSON.stringify({ type: "tool_response", id: m.id, result: m.tool === "get_frame_image" ? { png: "iVBORw0KGgo=", frame: 3, width: 1, height: 1 } : { layers: [], tool: m.tool } }));
    });
    const r = await client.callTool({ name: "read_scene", arguments: {} });
    expect(r.isError).toBeFalsy();
    expect(JSON.stringify(r.content)).toContain("read_scene");
    const img = await client.callTool({ name: "get_frame_image", arguments: { frame: 3 } });
    expect((img.content as any[])[0].type).toBe("image");
  });
});
