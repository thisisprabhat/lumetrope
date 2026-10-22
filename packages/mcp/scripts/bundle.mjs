// Bundle the MCP server into one self-contained file (deps + tool catalogue
// inlined) so it runs anywhere Node 20+ is installed, including from inside
// the desktop app's resources.
import { build } from "esbuild";
import { rmSync } from "node:fs";

rmSync(new URL("../dist", import.meta.url), { recursive: true, force: true });
await build({
  entryPoints: [new URL("../src/index.ts", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1")],
  outfile: new URL("../dist/lumetrope-mcp.mjs", import.meta.url).pathname.replace(/^\/([A-Za-z]:)/, "$1"),
  bundle: true,
  platform: "node",
  format: "esm",
  target: "node20",
  // ws loads these optional native speed-ups with require() inside try/catch.
  external: ["bufferutil", "utf-8-validate"],
  // CommonJS dependencies call require(); give the ESM bundle a real one.
  banner: { js: "import { createRequire as __lumetropeRequire } from 'node:module'; const require = __lumetropeRequire(import.meta.url);" },
  legalComments: "none",
  logLevel: "warning",
});
