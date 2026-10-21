// Local persistence: projects and their version history live in IndexedDB,
// autosaved after edits; the most recent project is restored on launch.
import { engine } from "./engine";
import { useEditor } from "./store";

const DB = "lumetrope";
const STORE = "projects";

export interface ProjectRecord {
  id: string;
  name: string;
  updated: number;
  project: string;
  history: Uint8Array;
}

function open(): Promise<IDBDatabase> {
  return new Promise((resolve, reject) => {
    const req = indexedDB.open(DB, 1);
    req.onupgradeneeded = () => req.result.createObjectStore(STORE, { keyPath: "id" });
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}

async function tx<T>(mode: IDBTransactionMode, fn: (s: IDBObjectStore) => IDBRequest<T>): Promise<T> {
  const db = await open();
  return new Promise((resolve, reject) => {
    const r = fn(db.transaction(STORE, mode).objectStore(STORE));
    r.onsuccess = () => resolve(r.result);
    r.onerror = () => reject(r.error);
  });
}

export const persist = {
  save: (rec: ProjectRecord) => tx("readwrite", (s) => s.put(rec)),
  get: (id: string) => tx<ProjectRecord | undefined>("readonly", (s) => s.get(id)),
  all: async () => ((await tx<ProjectRecord[]>("readonly", (s) => s.getAll())) ?? []).sort((a, b) => b.updated - a.updated),
  remove: (id: string) => tx("readwrite", (s) => s.delete(id)),
};

export function newProjectId(): string {
  return typeof crypto !== "undefined" && "randomUUID" in crypto ? crypto.randomUUID() : `p${Date.now()}${Math.random().toString(16).slice(2)}`;
}

/** Save the current document + history now. */
export async function saveNow(): Promise<void> {
  const st = useEditor.getState();
  try {
    await persist.save({ id: st.projectId, name: st.fileName || "Untitled", updated: Date.now(), project: engine().project_json(), history: engine().vcs_export() });
    useEditor.setState({ savedAt: Date.now() });
  } catch {
    /* storage unavailable (private mode / quota): editing continues in memory */
  }
}

let timer = 0;
/** Debounced autosave; call after document changes. */
export function scheduleSave(): void {
  clearTimeout(timer);
  timer = window.setTimeout(() => void saveNow(), 1200);
}

/** Load a stored project into the engine. */
export async function loadProject(id: string): Promise<boolean> {
  const rec = await persist.get(id).catch(() => undefined);
  if (!rec) return false;
  const e = engine();
  e.set_project_json(rec.project);
  try {
    e.vcs_import(rec.history);
  } catch {
    e.vcs_reset();
  }
  useEditor.setState({ projectId: rec.id, fileName: rec.name, selection: [], shape: null, scene: "root" });
  useEditor.getState().refresh();
  return true;
}
