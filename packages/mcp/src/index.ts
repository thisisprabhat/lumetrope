#!/usr/bin/env node
// lumetrope-mcp: stdio MCP server exposing Lumetrope's editing tools to AI agents.
import { Server } from "@modelcontextprotocol/sdk/server/index.js";
import { StdioServerTransport } from "@modelcontextprotocol/sdk/server/stdio.js";
import { CallToolRequestSchema, ListToolsRequestSchema } from "@modelcontextprotocol/sdk/types.js";
import { Bridge, DEFAULT_PORT } from "./bridge.js";
import { HOST_TOOLS } from "./hostTools.js";
import catalogJson from "../catalog.json" with { type: "json" };

const VERSION = "0.1.1";
const log = (m: string) => process.stderr.write(`[lumetrope-mcp] ${m}\n`);

interface ToolDef {
  name: string;
  description: string;
  inputSchema: Record<string, unknown>;
}

const catalog = catalogJson as unknown as ToolDef[];
const tools: ToolDef[] = [...catalog.map(({ name, description, inputSchema }) => ({ name, description, inputSchema })), ...(HOST_TOOLS as unknown as ToolDef[])];
const known = new Set(tools.map((t) => t.name));

const bridge = new Bridge({ port: Number(process.env.LUMETROPE_MCP_PORT) || DEFAULT_PORT, log });

const server = new Server(
  { name: "lumetrope", version: VERSION },
  {
    capabilities: { tools: {} },
    instructions:
      "Lumetrope is a Lottie/dotLottie animation editor. Start with read_scene to get layer ids (e.g. 'root:3'), then edit with the tools. " +
      "Frames are integers on the scene timeline. After visual changes call get_frame_image to look at the result. " +
      "Interactivity is built with state machines (add_state_machine, add_state, add_input, add_transition, add_interaction) over named segments (add_segment); " +
      "test them with preview_state_machine + state_machine_pointer/state_machine_input. Save milestones with save_version.",
  },
);

server.setRequestHandler(ListToolsRequestSchema, async () => ({ tools }));

server.setRequestHandler(CallToolRequestSchema, async (req) => {
  const { name, arguments: args } = req.params;
  if (!known.has(name)) return { content: [{ type: "text", text: `Unknown tool '${name}'` }], isError: true };
  try {
    const result: any = await bridge.call(name, args ?? {});
    if (name === "get_frame_image" && result?.png) {
      return { content: [{ type: "image", data: result.png, mimeType: "image/png" }, { type: "text", text: `frame ${result.frame} · ${result.width}×${result.height}` }] };
    }
    return { content: [{ type: "text", text: typeof result === "string" ? result : JSON.stringify(result ?? null, null, 2) }] };
  } catch (e) {
    return { content: [{ type: "text", text: e instanceof Error ? e.message : String(e) }], isError: true };
  }
});

async function main() {
  try {
    await bridge.start();
  } catch (e) {
    log(String(e instanceof Error ? e.message : e));
    process.exit(1);
  }
  await server.connect(new StdioServerTransport());
  log(`ready · ${tools.length} tools · waiting for the Lumetrope app on port ${bridge.port}`);
  const shutdown = async () => {
    await bridge.stop();
    process.exit(0);
  };
  process.stdin.on("close", shutdown);
  process.on("SIGINT", shutdown);
  process.on("SIGTERM", shutdown);
}

void main();
