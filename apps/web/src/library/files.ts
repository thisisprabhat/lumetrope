// Reading and writing the user's own files, remembering where they live:
// desktop → absolute path (native dialogs; the persisted fs scope keeps access
// after a restart); Chromium browsers → File System Access handle (stored in
// IndexedDB); other browsers → file input / download with a cached copy.
import { isTauri } from "../platform";
import type { FileLink, ProjectMeta } from "./db";

export interface Picked {
  file: File;
  link?: FileLink;
}

const OPEN_TYPES = [
  {
    description: "Lottie, dotLottie, SVG or image",
    accept: {
      "application/json": [".json"],
      "application/zip": [".lottie"],
      "image/svg+xml": [".svg"],
      "image/png": [".png"],
      "image/jpeg": [".jpg", ".jpeg"],
    },
  },
];

const fsAccess = () => typeof window !== "undefined" && "showOpenFilePicker" in window;
const base = (p: string) => p.split(/[\\/]/).pop() || p;

function extensions(accept: string): string[] {
  return accept
    .split(",")
    .map((s) => s.trim().replace(/^\./, ""))
    .filter(Boolean);
}

/** Ask for a file to open. Resolves null when the user cancels. */
export async function pickOpen(accept: string): Promise<Picked | null> {
  if (isTauri()) {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const { readFile } = await import("@tauri-apps/plugin-fs");
    const path = await open({ multiple: false, directory: false, filters: [{ name: "Supported files", extensions: extensions(accept) }] });
    if (!path || Array.isArray(path)) return null;
    return { file: new File([await readFile(path)], base(path)), link: { kind: "path", path } };
  }
  if (fsAccess()) {
    try {
      const [handle] = (await (window as any).showOpenFilePicker({ types: OPEN_TYPES, excludeAcceptAllOption: false, multiple: false })) as FileSystemFileHandle[];
      return { file: await handle.getFile(), link: { kind: "handle", handle } };
    } catch (e) {
      if ((e as DOMException)?.name === "AbortError") return null;
      throw e;
    }
  }
  return new Promise((resolve) => {
    const input = document.createElement("input");
    input.type = "file";
    input.accept = accept;
    input.onchange = () => resolve(input.files?.[0] ? { file: input.files[0] } : null);
    input.oncancel = () => resolve(null);
    input.click();
  });
}

async function permitted(handle: FileSystemFileHandle, mode: "read" | "readwrite"): Promise<boolean> {
  const h = handle as any;
  if (!h.queryPermission) return true;
  if ((await h.queryPermission({ mode })) === "granted") return true;
  return (await h.requestPermission({ mode })) === "granted";
}

/** Read a linked file again (e.g. when reopening a project from the dashboard). */
export async function readLink(link: FileLink): Promise<File> {
  if (link.kind === "path") {
    const { readFile } = await import("@tauri-apps/plugin-fs");
    return new File([await readFile(link.path)], base(link.path));
  }
  if (!(await permitted(link.handle, "read"))) throw new Error("Permission to read the file was not granted");
  return link.handle.getFile();
}

export async function writeLink(link: FileLink, bytes: Uint8Array): Promise<void> {
  if (link.kind === "path") {
    const { writeFile } = await import("@tauri-apps/plugin-fs");
    await writeFile(link.path, bytes);
    return;
  }
  if (!(await permitted(link.handle, "readwrite"))) throw new Error("Permission to save the file was not granted");
  const w = await (link.handle as any).createWritable();
  await w.write(bytes);
  await w.close();
}

/** Ask where to save. Null when cancelled, or when the platform can only download. */
export async function pickSave(suggested: string): Promise<{ link: FileLink; name: string } | null | "download"> {
  const ext = suggested.split(".").pop() || "lottie";
  if (isTauri()) {
    const { save } = await import("@tauri-apps/plugin-dialog");
    const path = await save({ defaultPath: suggested, filters: [{ name: ext === "json" ? "Lottie JSON" : "dotLottie", extensions: [ext] }] });
    return path ? { link: { kind: "path", path }, name: base(path) } : null;
  }
  if ("showSaveFilePicker" in window) {
    try {
      const handle = (await (window as any).showSaveFilePicker({
        suggestedName: suggested,
        types: [ext === "json" ? { description: "Lottie JSON", accept: { "application/json": [".json"] } } : { description: "dotLottie", accept: { "application/zip": [".lottie"] } }],
      })) as FileSystemFileHandle;
      return { link: { kind: "handle", handle }, name: handle.name };
    } catch (e) {
      if ((e as DOMException)?.name === "AbortError") return null;
      throw e;
    }
  }
  return "download";
}

/** How ⌘S writes back to a linked file, by its name. JSON can't hold state machines or themes. */
export function saveFormat(fileName: string, interactive: boolean): "lottie" | "json" | "saveAs" {
  const ext = fileName.toLowerCase().split(".").pop();
  if (ext === "lottie") return "lottie";
  if (ext === "json") return interactive ? "saveAs" : "json";
  return "saveAs";
}

export function linkLabel(file: NonNullable<ProjectMeta["file"]>): string {
  if (file.link?.kind === "path") return file.link.path;
  if (file.link) return file.name;
  return `${file.name} (copy kept in Lumetrope)`;
}
