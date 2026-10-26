// A workspace kept in a folder on disk instead of IndexedDB. The layout is the
// same as a backup zip, so an unpacked backup opens as a workspace:
//
//   lumetrope-workspace.json          { format, version, name, created }
//   projects/<id>/meta.json           listing metadata (name, activity, thumbnail, linked file)
//   projects/<id>/project.json        the project document
//   projects/<id>/history.bin         version history (compressed)
//
// Browser file handles can't be written to JSON, so links to files picked in a
// Chromium browser are kept beside the folder in IndexedDB (`handles`).
import { sameLink, type FileLink, type Library, type ProjectData, type ProjectMeta } from "./db";

export const MANIFEST = "lumetrope-workspace.json";
export const FORMAT = "lumetrope-workspace";

export interface Manifest {
  format: typeof FORMAT;
  version: 1;
  name: string;
  created: number;
  exported?: number;
  app?: string;
  projects?: number;
}

/** The few file operations a workspace needs; paths are relative and use "/". */
export interface Dir {
  /** File bytes, or null when it doesn't exist. */
  read(path: string): Promise<Uint8Array | null>;
  /** Write a file, creating parent folders. */
  write(path: string, bytes: Uint8Array): Promise<void>;
  /** Entries of a folder ([] when it doesn't exist). */
  list(path: string): Promise<{ name: string; dir: boolean }[]>;
  /** Delete a file or folder recursively (no-op when missing). */
  remove(path: string): Promise<void>;
}

export interface HandleStore {
  get(project: string): Promise<FileSystemFileHandle | undefined>;
  put(project: string, h: FileSystemFileHandle): Promise<void>;
  delete(project: string): Promise<void>;
}

const enc = new TextEncoder();
const dec = new TextDecoder();

/** Project ids become folder names; anything unusual is rejected. */
export const safeId = (id: unknown): id is string => typeof id === "string" && /^[A-Za-z0-9_-]{1,100}$/.test(id);

export const projectFile = (id: string, file: "meta.json" | "project.json" | "history.bin") => `projects/${id}/${file}`;

/** Metadata as JSON: path links are kept; browser handles are dropped (`handle: true` marks one). */
export function metaToJson(m: ProjectMeta): string {
  const { file, ...rest } = m;
  const out: any = { ...rest };
  if (file) out.file = { name: file.name, synced: file.synced, ...(file.link?.kind === "path" ? { link: file.link } : file.link ? { handle: true } : {}) };
  return JSON.stringify(out);
}

export function metaFromJson(text: string, id?: string): ProjectMeta | null {
  try {
    const m = JSON.parse(text);
    if (!m || !safeId(m.id) || (id && m.id !== id) || typeof m.name !== "string") return null;
    const now = Date.now();
    const meta: ProjectMeta = { ...m, created: Number(m.created) || now, updated: Number(m.updated) || now, opened: Number(m.opened) || now, origin: ["new", "file", "sample"].includes(m.origin) ? m.origin : "new" };
    if (m.file) {
      const link: FileLink | undefined = m.file.link?.kind === "path" && typeof m.file.link.path === "string" ? { kind: "path", path: m.file.link.path } : undefined;
      meta.file = { name: String(m.file.name ?? ""), synced: !!m.file.synced, ...(link ? { link } : {}) };
      if (m.file.handle) (meta as any).__handle = true;
    }
    return meta;
  } catch {
    return null;
  }
}

export function parseManifest(bytes: Uint8Array | null): Manifest | null {
  if (!bytes) return null;
  try {
    const m = JSON.parse(dec.decode(bytes));
    return m?.format === FORMAT ? m : null;
  } catch {
    return null;
  }
}

export async function readManifest(dir: Dir): Promise<Manifest | null> {
  return parseManifest(await dir.read(MANIFEST));
}

/** Make `dir` a workspace (keeps an existing manifest). */
export async function initFolder(dir: Dir, name: string): Promise<Manifest> {
  const cur = await readManifest(dir);
  if (cur) return cur;
  const m: Manifest = { format: FORMAT, version: 1, name, created: Date.now() };
  await dir.write(MANIFEST, enc.encode(JSON.stringify(m, null, 2)));
  return m;
}

export function createFolderLibrary(dir: Dir, handles?: HandleStore): Library {
  const withHandle = async (m: ProjectMeta | null): Promise<ProjectMeta | undefined> => {
    if (!m) return undefined;
    if ((m as any).__handle) {
      delete (m as any).__handle;
      const h = await handles?.get(m.id).catch(() => undefined);
      if (h && m.file) m.file = { ...m.file, link: { kind: "handle", handle: h } };
    }
    return m;
  };
  const getMeta = async (id: string) => {
    if (!safeId(id)) return undefined;
    const b = await dir.read(projectFile(id, "meta.json"));
    return b ? withHandle(metaFromJson(dec.decode(b), id)) : undefined;
  };
  const writeMeta = async (m: ProjectMeta) => {
    if (m.file?.link?.kind === "handle") await handles?.put(m.id, m.file.link.handle);
    await dir.write(projectFile(m.id, "meta.json"), enc.encode(metaToJson(m)));
  };
  const lib: Library = {
    async list() {
      const entries = await dir.list("projects");
      const metas = await Promise.all(entries.filter((e) => e.dir && safeId(e.name)).map((e) => getMeta(e.name).catch(() => undefined)));
      return metas.filter((m): m is ProjectMeta => !!m);
    },
    getMeta,
    async getData(id) {
      if (!safeId(id)) return undefined;
      const [p, h] = await Promise.all([dir.read(projectFile(id, "project.json")), dir.read(projectFile(id, "history.bin"))]);
      return p ? { id, project: dec.decode(p), history: h ?? new Uint8Array() } : undefined;
    },
    async put(meta, data?: ProjectData) {
      if (!safeId(meta.id)) throw new Error(`invalid project id ${meta.id}`);
      if (data) {
        await dir.write(projectFile(meta.id, "project.json"), enc.encode(data.project));
        await dir.write(projectFile(meta.id, "history.bin"), data.history);
      }
      // Metadata last: a project is listed only once its data is on disk.
      await writeMeta(meta);
    },
    async patch(id, changes) {
      const cur = await getMeta(id);
      if (!cur) return undefined;
      const next = { ...cur, ...changes, id };
      await writeMeta(next);
      return next;
    },
    async remove(id) {
      if (!safeId(id)) return;
      await dir.remove(`projects/${id}`);
      await handles?.delete(id).catch(() => undefined);
    },
    async findByLink(link) {
      for (const m of await lib.list()) if (await sameLink(m.file?.link, link)) return m;
      return undefined;
    },
  };
  return lib;
}

// ------------------------------------------------------------ Dir backends

/** In-memory folder (tests). */
export function memoryDir(files = new Map<string, Uint8Array>()): Dir & { files: Map<string, Uint8Array> } {
  const norm = (p: string) => p.replace(/^\/+|\/+$/g, "");
  return {
    files,
    async read(p) {
      return files.get(norm(p)) ?? null;
    },
    async write(p, b) {
      files.set(norm(p), b);
    },
    async list(p) {
      const pre = norm(p) ? norm(p) + "/" : "";
      const out = new Map<string, boolean>();
      for (const k of files.keys()) {
        if (!k.startsWith(pre)) continue;
        const rest = k.slice(pre.length).split("/");
        out.set(rest[0], out.get(rest[0]) || rest.length > 1);
      }
      return [...out].map(([name, dir]) => ({ name, dir }));
    },
    async remove(p) {
      const n = norm(p);
      for (const k of [...files.keys()]) if (k === n || k.startsWith(n + "/")) files.delete(k);
    },
  };
}

/** A folder picked with the File System Access API (Chromium browsers). */
export function handleDir(root: FileSystemDirectoryHandle): Dir {
  const split = (p: string) => p.split("/").filter(Boolean);
  const folder = async (parts: string[], create: boolean): Promise<FileSystemDirectoryHandle | null> => {
    let d = root;
    for (const s of parts) {
      try {
        d = await d.getDirectoryHandle(s, { create });
      } catch (e) {
        if (!create && ((e as DOMException).name === "NotFoundError" || (e as DOMException).name === "TypeMismatchError")) return null;
        throw e;
      }
    }
    return d;
  };
  return {
    async read(p) {
      const parts = split(p);
      const d = await folder(parts.slice(0, -1), false);
      if (!d) return null;
      try {
        const f = await (await d.getFileHandle(parts[parts.length - 1])).getFile();
        return new Uint8Array(await f.arrayBuffer());
      } catch (e) {
        if ((e as DOMException).name === "NotFoundError" || (e as DOMException).name === "TypeMismatchError") return null;
        throw e;
      }
    },
    async write(p, b) {
      const parts = split(p);
      const d = (await folder(parts.slice(0, -1), true))!;
      const w = await (await d.getFileHandle(parts[parts.length - 1], { create: true })).createWritable();
      await w.write(b);
      await w.close();
    },
    async list(p) {
      const d = await folder(split(p), false);
      if (!d) return [];
      const out: { name: string; dir: boolean }[] = [];
      for await (const [name, h] of (d as any).entries() as AsyncIterable<[string, FileSystemHandle]>) out.push({ name, dir: h.kind === "directory" });
      return out;
    },
    async remove(p) {
      const parts = split(p);
      const d = await folder(parts.slice(0, -1), false);
      await d?.removeEntry(parts[parts.length - 1], { recursive: true }).catch((e: DOMException) => {
        if (e.name !== "NotFoundError") throw e;
      });
    },
  };
}

/** A folder on disk in the desktop app (Tauri fs plugin; access granted by the folder dialog). */
export function pathDir(base: string): Dir {
  const sep = /^[A-Za-z]:\\|\\\\/.test(base) ? "\\" : "/";
  const full = (p: string) => [base.replace(/[\\/]+$/, ""), ...p.split("/").filter(Boolean)].join(sep);
  const fs = () => import("@tauri-apps/plugin-fs");
  return {
    async read(p) {
      const { exists, readFile } = await fs();
      return (await exists(full(p))) ? readFile(full(p)) : null;
    },
    async write(p, b) {
      const { mkdir, writeFile } = await fs();
      const parts = p.split("/").filter(Boolean);
      if (parts.length > 1) await mkdir(full(parts.slice(0, -1).join("/")), { recursive: true });
      await writeFile(full(p), b);
    },
    async list(p) {
      const { exists, readDir } = await fs();
      if (!(await exists(full(p)))) return [];
      return (await readDir(full(p))).map((e) => ({ name: e.name, dir: e.isDirectory }));
    },
    async remove(p) {
      const { exists, remove } = await fs();
      if (await exists(full(p))) await remove(full(p), { recursive: true });
    },
  };
}
