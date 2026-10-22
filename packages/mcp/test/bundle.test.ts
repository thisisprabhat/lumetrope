import { afterAll, beforeAll, describe, expect, it } from "vitest";
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { copyFileSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

// The shipped server is one file: it must start from an empty folder (no
// node_modules, no catalog.json beside it), as it does inside the desktop app.
const dir = mkdtempSync(join(tmpdir(), "lumetrope-mcp-"));
const port = 47000 + Math.floor(Math.random() * 900);
let client: Client;

beforeAll(async () => {
  const bundle = join(dir, "lumetrope-mcp.mjs");
  copyFileSync(fileURLToPath(new URL("../dist/lumetrope-mcp.mjs", import.meta.url)), bundle);
  client = new Client({ name: "test", version: "0" });
  await client.connect(
    new StdioClientTransport({ command: process.execPath, args: [bundle], cwd: dir, env: { ...process.env, LUMETROPE_MCP_PORT: String(port) } as Record<string, string>, stderr: "ignore" }),
  );
});

afterAll(async () => {
  await client?.close();
  rmSync(dir, { recursive: true, force: true });
});

describe("single-file MCP server", () => {
  it("runs standalone and lists every tool", async () => {
    const { tools } = await client.listTools();
    expect(tools.length).toBeGreaterThan(100);
    expect(tools.map((t) => t.name)).toContain("create_shape");
  });
});
