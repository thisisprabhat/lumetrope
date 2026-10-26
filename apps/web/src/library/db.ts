// The local project library in IndexedDB. Listing metadata ("meta": name,
// activity times, thumbnail, linked file) is kept apart from the heavy project
// data ("projects": document JSON + version history) so the dashboard loads fast.

/** Where a project's file lives on disk: a path (desktop) or a File System Access handle (Chromium). */
export type FileLink = { kind: "path"; path: string } | { kind: "handle"; handle: FileSystemFileHandle };

export type Origin = "new" | "file" | "sample";

export interface ProjectMeta {
  id: string;
  name: string;
  origin: Origin;
  created: number;
  /** Last content edit. */
  updated: number;
  opened: number;
  previewed?: number;
  exported?: { at: number; format: string };
  /** Small preview image (data URL). */
  thumb?: string;
  /** The local file this project was opened from or saved to. `synced` = no edits since it was last read/written. */
  file?: { name: string; link?: FileLink; synced: boolean };
  info?: { w: number; h: number; fr: number; frames: number; layers: number; machines: number };
}

export interface ProjectData {
  id: string;
  project: string;
  history: Uint8Array;
}

/** Database of the default workspace; other browser workspaces get their own. */
export const DEFAULT_DB = "lumetrope";
const VERSION = 2;

/** Where projects are kept: IndexedDB here, or a folder on disk (see folder.ts). */
export interface Library {
  list(): Promise<ProjectMeta[]>;
  getMeta(id: string): Promise<ProjectMeta | undefined>;
  getData(id: string): Promise<ProjectData | undefined>;
  /** Write metadata, and project data when given. */
  put(meta: ProjectMeta, data?: ProjectData): Promise<void>;
  patch(id: string, changes: Partial<ProjectMeta>): Promise<ProjectMeta | undefined>;
  remove(id: string): Promise<void>;
  findByLink(link: FileLink): Promise<ProjectMeta | undefined>;
  /** Release the storage (lets a deleted workspace's database actually go). */
  close?(): Promise<void>;
}

function openDb(factory: IDBFactory, name: string): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = factory.open(name, VERSION);
    req.onupgradeneeded = (ev) => {
      const db = req.result;
      const old = ev.oldVersion;
      const data = old < 1 ? db.createObjectStore("projects", { keyPath: "id" }) : req.transaction!.objectStore("projects");
      const meta = db.createObjectStore("meta", { keyPath: "id" });
      if (old >= 1) {
        // v1 kept {id, name, updated, project, history} in one store.
        data.openCursor().onsuccess = (e) => {
          const cur = (e.target as IDBRequest<IDBCursorWithValue | null>).result;
          if (!cur) return;
          const r = cur.value;
          const at = typeof r.updated === "number" ? r.updated : Date.now();
          meta.put({ id: r.id, name: r.name || "Untitled", origin: "new", created: at, updated: at, opened: at } satisfies ProjectMeta);
          cur.continue();
        };
      }
    };
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

function done(t: IDBTransaction): Promise<void> {
  return new Promise((resolve, reject) => {
    t.oncomplete = () => resolve();
    t.onerror = () => reject(t.error);
    t.onabort = () => reject(t.error);
  });
}

function req<T>(r: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    r.onsuccess = () => resolve(r.result);
    r.onerror = () => reject(r.error);
  });
}

export async function sameLink(a: FileLink | undefined, b: FileLink): Promise<boolean> {
  if (!a || a.kind !== b.kind) return false;
  if (a.kind === "path" && b.kind === "path") return a.path === b.path;
  if (a.kind === "handle" && b.kind === "handle") return a.handle.isSameEntry(b.handle).catch(() => false);
  return false;
}

export function createLibrary(factory?: IDBFactory, name = DEFAULT_DB): Library {
  let dbp: Promise<IDBDatabase> | null = null;
  const db = () => (dbp ??= openDb(factory ?? indexedDB, name).catch((e) => ((dbp = null), Promise.reject(e))));
  const lib: Library = {
    async list(): Promise<ProjectMeta[]> {
      return req((await db()).transaction("meta").objectStore("meta").getAll());
    },
    async getMeta(id: string): Promise<ProjectMeta | undefined> {
      return req((await db()).transaction("meta").objectStore("meta").get(id));
    },
    async getData(id: string): Promise<ProjectData | undefined> {
      const r = await req((await db()).transaction("projects").objectStore("projects").get(id));
      return r ? { id: r.id, project: r.project, history: r.history } : undefined;
    },
    /** Write metadata, and project data when given, in one transaction. */
    async put(meta: ProjectMeta, data?: ProjectData): Promise<void> {
      const t = (await db()).transaction(["meta", "projects"], "readwrite");
      t.objectStore("meta").put(meta);
      if (data) t.objectStore("projects").put(data);
      return done(t);
    },
    async patch(id: string, changes: Partial<ProjectMeta>): Promise<ProjectMeta | undefined> {
      const t = (await db()).transaction("meta", "readwrite");
      const s = t.objectStore("meta");
      const cur: ProjectMeta | undefined = await req(s.get(id));
      if (!cur) return undefined;
      const next = { ...cur, ...changes, id };
      s.put(next);
      await done(t);
      return next;
    },
    async remove(id: string): Promise<void> {
      const t = (await db()).transaction(["meta", "projects"], "readwrite");
      t.objectStore("meta").delete(id);
      t.objectStore("projects").delete(id);
      return done(t);
    },
    async findByLink(link: FileLink): Promise<ProjectMeta | undefined> {
      for (const m of await lib.list()) if (await sameLink(m.file?.link, link)) return m;
      return undefined;
    },
    async close() {
      const p = dbp;
      dbp = null;
      (await p?.catch(() => null))?.close();
    },
  };
  return lib;
}
