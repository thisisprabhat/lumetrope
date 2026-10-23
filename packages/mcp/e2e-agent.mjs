// Manual end-to-end check: behaves like an MCP agent against a running Lumetrope window.
import { Client } from "@modelcontextprotocol/sdk/client/index.js";
import { StdioClientTransport } from "@modelcontextprotocol/sdk/client/stdio.js";
import { writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const out = process.argv[2];
const client = new Client({ name: "e2e-agent", version: "0" });
await client.connect(new StdioClientTransport({ command: process.execPath, args: [fileURLToPath(new URL("./dist/lumetrope-mcp.mjs", import.meta.url))], stderr: "inherit" }));
const call = async (name, args = {}) => {
  const r = await client.callTool({ name, arguments: args });
  if (r.isError) throw new Error(`${name}: ${r.content[0].text}`);
  return r.content[0].type === "text" ? JSON.parse(r.content[0].text) : r.content;
};
for (let i = 0; i < 20; i++) {
  try { await call("get_editor_state"); break; } catch (e) { if (i === 19) throw e; await new Promise((r) => setTimeout(r, 1000)); }
}
await call("stop_state_machine_preview");
await call("set_scene_settings", { width: 400, height: 300, fps: 30, duration_frames: 60, name: "Agent demo" });
const bg = await call("create_solid", { color: "#14213d", name: "Background" });
const btn = await call("create_shape", { type: "rect", name: "Button", position: [200, 150], size: [180, 70], roundness: 35, fill: "#fca311" });
await call("create_text", { text: "Press me", position: [200, 160], font_size: 26, color: "#14213d", name: "Label" });
await call("apply_motion_preset", { layer: "Button", preset: "pop", start_frame: 0, duration_frames: 20 });
await call("add_segment", { name: "intro", start: 0, end: 20 });
await call("add_segment", { name: "pressed", start: 20, end: 40 });
await call("set_keyframe", { layer: "Button", property: "rotation", frame: 20, value: 0 });
await call("set_keyframe", { layer: "Button", property: "rotation", frame: 40, value: 360, easing: "ease_in_out" });
const sm = await call("add_state_machine", { name: "Button" });
await call("add_input", { machine: sm.machine, name: "press", type: "event" });
await call("add_state", { machine: sm.machine, name: "Intro", segment: "intro" });
await call("add_state", { machine: sm.machine, name: "Spin", segment: "pressed" });
await call("add_transition", { machine: sm.machine, from: "Intro", to: "Spin", guards: [{ input: "press" }] });
await call("add_interaction", { machine: sm.machine, type: "Click", layer: "Button", actions: [{ type: "fire", input: "press" }] });
const scene = await call("read_scene");
await call("save_version", { message: "Agent built an interactive button" });
const p0 = await call("preview_state_machine", { machine: sm.machine });
const afterMiss = await call("state_machine_pointer", { kind: "click", x: 20, y: 20 });
const afterHit = await call("state_machine_pointer", { kind: "click", x: 200, y: 150 });
const done = await call("state_machine_advance", { seconds: 0.4 });
await call("stop_state_machine_preview");
const img = await client.callTool({ name: "get_frame_image", arguments: { frame: 30, width: 400 } });
writeFileSync(out, Buffer.from(img.content[0].data, "base64"));
console.log(JSON.stringify({ layers: scene.layers.map((l) => `${l.id} ${l.type} ${l.name}`), bg: bg.id, btn: btn.id, start: p0.state, miss: afterMiss.state, hit: afterHit.state, later: { state: done.state, frame: Math.round(done.frame) } }));
await client.close();
