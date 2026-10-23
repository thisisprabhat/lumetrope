// Platform layer: native open/save dialogs in the Tauri desktop shell, file
// inputs and downloads in the browser. Tauri modules load only on desktop.

export function isTauri(win: any = typeof window !== "undefined" ? window : {}): boolean {
  return !!win && "__TAURI_INTERNALS__" in win;
}

export function dialogFilters(accept: string): { name: string; extensions: string[] }[] {
  const ext = accept
    .split(",")
    .map((s) => s.trim().replace(/^\./, ""))
    .filter(Boolean);
  return ext.length ? [{ name: "Supported files", extensions: ext }] : [];
}

const base = (p: string) => p.split(/[\\/]/).pop() || p;

/** Ask the user for a file. */
export async function pickFile(accept: string): Promise<File | null> {
  if (isTauri()) {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const { readFile } = await import("@tauri-apps/plugin-fs");
    const path = await open({ multiple: false, directory: false, filters: dialogFilters(accept) });
    if (!path || Array.isArray(path)) return null;
    const bytes = await readFile(path);
    return new File([bytes], base(path));
  }
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = accept;
    input.onchange = () => resolve(input.files?.[0] ?? null);
    input.click();
  });
}

/** Save data under a suggested name (native save dialog on desktop, download in the browser). */
export async function saveFile(name: string, data: BlobPart, mime: string): Promise<boolean> {
  if (isTauri()) {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const { writeFile } = await import("@tauri-apps/plugin-fs");
    const ext = name.includes(".") ? name.split(".").pop()! : "";
    const path = await save({ defaultPath: name, filters: ext ? [{ name: ext.toUpperCase(), extensions: [ext] }] : [] });
    if (!path) return false;
    const bytes = typeof data === "string" ? new TextEncoder().encode(data) : data instanceof Uint8Array ? data : new Uint8Array(await new Blob([data]).arrayBuffer());
    await writeFile(path, bytes);
    return true;
  }
  const url = URL.createObjectURL(new Blob([data], { type: mime }));
  const a = document.createElement("a");
  a.href = url;
  a.download = name;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
  return true;
}

/** Does this webview's Canvas2D support `ctx.filter` (blur, shadows, luma mattes on the GPU path)? */
export function canvasFilterSupported(): boolean {
  return typeof CanvasRenderingContext2D !== "undefined" && "filter" in CanvasRenderingContext2D.prototype;
}

/** A saved choice wins; otherwise GPU where Canvas2D filters work, else the exact CPU renderer. */
export function defaultRenderer(saved: string | null, canvasFilter: boolean): "gpu" | "cpu" {
  if (saved === "gpu" || saved === "cpu") return saved;
  return canvasFilter ? "gpu" : "cpu";
}

/** Path of the MCP server agents should launch: the desktop app's installed copy, or the repo build. */
export async function mcpServerPath(): Promise<{ path: string; source: "app" | "build" }> {
  if (isTauri()) {
    const { invoke } = await import("@tauri-apps/api/core");
    return { path: await invoke<string>("mcp_server_path"), source: "app" };
  }
  return { path: __LUMETROPE_MCP_PATH__, source: "build" };
}

/**
 * Can this window use the local MCP bridge (ws://127.0.0.1)? Only the desktop app and pages
 * served from this machine: the bridge accepts local origins only, and browsers restrict public
 * websites from reaching localhost. A deployed web copy hides MCP instead of offering a dead end.
 */
export function localMcpSupported(win: any = typeof window !== "undefined" ? window : {}): boolean {
  if (isTauri(win)) return true;
  const host = String(win?.location?.hostname ?? "").toLowerCase();
  return host === "localhost" || host === "127.0.0.1" || host === "[::1]" || host === "::1" || host.endsWith(".localhost");
}
