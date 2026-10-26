// Workspaces: separate libraries of projects you can switch between. The
// default one lives in this browser's storage (as before workspaces existed);
// others are extra browser libraries or folders on disk (desktop app, or a
// Chromium browser via the File System Access API).
import { isTauri } from "../platform";
import { createLibrary, DEFAULT_DB, type Library } from "./db";
import { createFolderLibrary, handleDir, initFolder, pathDir, readManifest, type Dir, type HandleStore } from "./folder";

export interface WorkspaceInfo {
  id: string;
  name: string;
  kind: "browser" | "folder";
  /** Folder path (desktop app). Browser folders keep their handle in IndexedDB. */
  path?: string;
  created: number;
}

export type WorkspaceStatus = "ready" | "needs-permission" | "missing" | "error";

export const REGISTRY_KEY = "lumetrope.workspaces";
export const CURRENT_KEY = "lumetrope.workspace";
export const DEFAULT_WORKSPACE: WorkspaceInfo = { id: "default", name: "This device", kind: "browser", created: 0 };

function ls(): Storage | null {
  try {
    return localStorage;
  } catch {
    return null;
  }
}

export function listWorkspaces(): WorkspaceInfo[] {
  let saved: WorkspaceInfo[] = [];
  try {
    const v = JSON.parse(ls()?.getItem(REGISTRY_KEY) ?? "[]");
    if (Array.isArray(v)) saved = v.filter((w) => w && typeof w.id === "string" && w.id !== "default" && (w.kind === "browser" || w.kind === "folder"));
  } catch {
    /* corrupt registry: just the default */
  }
  return [DEFAULT_WORKSPACE, ...saved];
}

function saveRegistry(list: WorkspaceInfo[]) {
  try {
    ls()?.setItem(REGISTRY_KEY, JSON.stringify(list.filter((w) => w.id !== "default")));
  } catch {
    /* not remembered */
  }
}

export function currentWorkspace(): WorkspaceInfo {
  const id = ls()?.getItem(CURRENT_KEY);
  const all = listWorkspaces();
  return all.find((w) => w.id === id) ?? all[0];
}

export function setCurrentWorkspace(id: string): void {
  try {
    ls()?.setItem(CURRENT_KEY, id);
  } catch {
    /* not remembered */
  }
}

export function addWorkspace(w: WorkspaceInfo): void {
  saveRegistry([...listWorkspaces().filter((x) => x.id !== w.id), w]);
}

export function renameWorkspace(id: string, name: string): void {
  if (id === "default") return;
  saveRegistry(listWorkspaces().map((w) => (w.id === id ? { ...w, name } : w)));
}

export function newWorkspaceId(): string {
  return `ws${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
}

export const dbName = (w: WorkspaceInfo) => (w.id === "default" ? DEFAULT_DB : `${DEFAULT_DB}-ws-${w.id}`);

/** Can this platform keep a workspace in a folder? */
export function foldersSupported(): boolean {
  return isTauri() || (typeof window !== "undefined" && "showDirectoryPicker" in window);
}

// ---------------------------------------------------- handles in IndexedDB

const HANDLES_DB = "lumetrope-handles";

function handlesDb(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const r = indexedDB.open(HANDLES_DB, 1);
    r.onupgradeneeded = () => {
      r.result.createObjectStore("dirs");
      r.result.createObjectStore("files");
    };
    r.onsuccess = () => resolve(r.result);
    r.onerror = () => reject(r.error);
  });
}

async function idb<T>(store: "dirs" | "files", mode: IDBTransactionMode, fn: (s: IDBObjectStore) => IDBRequest): Promise<T> {
  const db = await handlesDb();
  return new Promise((resolve, reject) => {
    const r = fn(db.transaction(store, mode).objectStore(store));
    r.onsuccess = () => resolve(r.result as T);
    r.onerror = () => reject(r.error);
  });
}

const dirHandle = (ws: string) => idb<FileSystemDirectoryHandle | undefined>("dirs", "readonly", (s) => s.get(ws));
const putDirHandle = (ws: string, h: FileSystemDirectoryHandle) => idb("dirs", "readwrite", (s) => s.put(h, ws));

function fileHandles(ws: string): HandleStore {
  return {
    get: (p) => idb("files", "readonly", (s) => s.get(`${ws}/${p}`)),
    put: (p, h) => idb("files", "readwrite", (s) => s.put(h, `${ws}/${p}`)),
    delete: (p) => idb("files", "readwrite", (s) => s.delete(`${ws}/${p}`)),
  };
}

// ---------------------------------------------------------------- opening

async function permission(h: FileSystemDirectoryHandle, request: boolean): Promise<boolean> {
  const x = h as any;
  if (!x.queryPermission) return true;
  if ((await x.queryPermission({ mode: "readwrite" })) === "granted") return true;
  return request && (await x.requestPermission({ mode: "readwrite" })) === "granted";
}

async function folderDir(w: WorkspaceInfo, request: boolean): Promise<{ dir?: Dir; status: WorkspaceStatus }> {
  if (w.path) {
    try {
      const { exists } = await import("@tauri-apps/plugin-fs");
      return (await exists(w.path)) ? { dir: pathDir(w.path), status: "ready" } : { status: "missing" };
    } catch {
      return { status: "error" };
    }
  }
  const h = await dirHandle(w.id).catch(() => undefined);
  if (!h) return { status: "missing" };
  if (!(await permission(h, request).catch(() => false))) return { status: "needs-permission" };
  return { dir: handleDir(h), status: "ready" };
}

/** The library behind a workspace. `request` may prompt for folder access (needs a click). */
export async function openWorkspace(w: WorkspaceInfo, request = false): Promise<{ lib?: Library; status: WorkspaceStatus }> {
  if (w.kind === "browser") return { lib: createLibrary(undefined, dbName(w)), status: "ready" };
  const { dir, status } = await folderDir(w, request);
  if (!dir) return { status };
  try {
    await initFolder(dir, w.name);
  } catch {
    return { status: "error" };
  }
  return { lib: createFolderLibrary(dir, fileHandles(w.id)), status: "ready" };
}

/** Ask for a folder and register it as a workspace (reusing the entry when it's already known). */
export async function pickFolderWorkspace(): Promise<WorkspaceInfo | null> {
  const base = (p: string) => p.split(/[\\/]/).filter(Boolean).pop() || p;
  if (isTauri()) {
    const { open } = await import("@tauri-apps/plugin-dialog");
    const path = await open({ directory: true, multiple: false, recursive: true, title: "Choose a workspace folder" });
    if (!path || Array.isArray(path)) return null;
    const known = listWorkspaces().find((w) => w.kind === "folder" && w.path === path);
    if (known) return known;
    const manifest = await readManifest(pathDir(path)).catch(() => null);
    const w: WorkspaceInfo = { id: newWorkspaceId(), name: manifest?.name || base(path), kind: "folder", path, created: Date.now() };
    addWorkspace(w);
    return w;
  }
  let h: FileSystemDirectoryHandle;
  try {
    h = await (window as any).showDirectoryPicker({ id: "lumetrope-workspace", mode: "readwrite" });
  } catch (e) {
    if ((e as DOMException)?.name === "AbortError") return null;
    throw e;
  }
  for (const w of listWorkspaces()) {
    if (w.kind !== "folder" || w.path) continue;
    const other = await dirHandle(w.id).catch(() => undefined);
    if (other && (await other.isSameEntry(h).catch(() => false))) {
      await putDirHandle(w.id, h); // the fresh handle carries the permission just granted
      return w;
    }
  }
  const manifest = await readManifest(handleDir(h)).catch(() => null);
  const w: WorkspaceInfo = { id: newWorkspaceId(), name: manifest?.name || h.name, kind: "folder", created: Date.now() };
  await putDirHandle(w.id, h);
  addWorkspace(w);
  return w;
}

/** Take a workspace off the list. A browser workspace's projects are deleted with it; a folder is left on disk. */
export async function forgetWorkspace(w: WorkspaceInfo): Promise<void> {
  if (w.id === "default") return;
  saveRegistry(listWorkspaces().filter((x) => x.id !== w.id));
  if (w.kind === "browser") {
    await new Promise<void>((resolve) => {
      const r = indexedDB.deleteDatabase(dbName(w));
      r.onsuccess = r.onerror = r.onblocked = () => resolve();
    });
  } else {
    await idb("dirs", "readwrite", (s) => s.delete(w.id)).catch(() => undefined);
  }
}

/** Where a workspace lives, for display. */
export function workspaceWhere(w: WorkspaceInfo): string {
  if (w.kind === "browser") return w.id === "default" ? "Saved in this browser" : "Saved in this browser (separate library)";
  return w.path ?? "Folder on disk";
}
