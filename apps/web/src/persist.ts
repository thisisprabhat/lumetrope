// Project sessions: every project lives in the local library (IndexedDB) with
// its version history, a thumbnail and its activity (created, opened, edited,
// previewed, exported). Projects opened from or saved to disk remember their
// file so ⌘S writes back to it and the dashboard can reopen it.
import { act } from "./actions";
import { engine, projectValue } from "./engine";
import { exportFileName } from "./lib/export";
import type { FileLink, Library, Origin, ProjectMeta } from "./library/db";
import { linkLabel, pickOpen, pickSave, readLink, saveFormat, writeLink, type Picked } from "./library/files";
import { saveFile } from "./platform";
import { useEditor } from "./store";
import { looksLikeLottie, svgCanvasSize, svgDeclaredSize } from "./library/start";
import { Engine } from "./wasm/lumetrope_wasm.js";
import { currentWorkspace, openWorkspace, type WorkspaceInfo } from "./library/workspace";

/** The current workspace's library; null while it opens or when it can't be read. */
let current: Promise<Library | undefined> = Promise.resolve(undefined);

async function lib(): Promise<Library> {
  const l = await current;
  if (!l) throw new Error("This workspace isn't available right now");
  return l;
}

/** Projects of the current workspace (follows workspace switches). */
export const library: Library = {
  list: async () => (await lib()).list(),
  getMeta: async (id) => (await lib()).getMeta(id),
  getData: async (id) => (await lib()).getData(id),
  put: async (meta, data) => (await lib()).put(meta, data),
  patch: async (id, changes) => (await lib()).patch(id, changes),
  remove: async (id) => (await lib()).remove(id),
  findByLink: async (link) => (await lib()).findByLink(link),
};

/** Point the library at `w`. `request` may ask for folder access (call from a click). */
export async function enterWorkspace(w: WorkspaceInfo, request = false): Promise<boolean> {
  const previous = current;
  const opening = openWorkspace(w, request).catch(() => ({ lib: undefined, status: "error" as const }));
  current = opening.then((r) => r.lib);
  await (await previous)?.close?.();
  useEditor.setState({ workspace: { info: w, status: "loading" } });
  const r = await opening;
  useEditor.setState((s) => ({ workspace: { info: w, status: r.status }, libraryTick: s.libraryTick + 1 }));
  return r.status === "ready";
}

void enterWorkspace(currentWorkspace());

/** Close the open project (saving it first) so another workspace or an import can take over. */
export async function closeProject(): Promise<void> {
  await saveNow();
  session++;
  clearTimeout(timer);
  stored = false;
  useEditor.setState({ project: null, view: "home", mode: "animate", fullscreen: false });
}

const st = () => useEditor.getState();

/** Project JSON as last written to the library (detects real edits). */
let savedJson = "";
/** Whether the open project is in the library yet (blank new projects wait for their first edit). */
let stored = false;
/** Bumped whenever another project becomes current; a save that finishes later must not touch the new session. */
let session = 0;
let timer = 0;

export function newProjectId(): string {
  return typeof crypto !== "undefined" && "randomUUID" in crypto ? crypto.randomUUID() : `p${Date.now()}${Math.random().toString(16).slice(2)}`;
}

const stripExt = (n: string) => n.replace(/\.(json|lottie)$/i, "");

function readInfo(e: Engine = engine()): ProjectMeta["info"] {
  try {
    const i = JSON.parse(e.info());
    const p = JSON.parse(e.project_json());
    return { w: i.w, h: i.h, fr: i.fr, frames: i.op - i.ip, layers: i.layers, machines: Object.keys(p.stateMachines ?? {}).length };
  } catch {
    return undefined;
  }
}

function makeThumb(e: Engine = engine()): string | undefined {
  try {
    const i = JSON.parse(e.info());
    const s = Math.min(320 / i.w, 200 / i.h, 1);
    const w = Math.max(1, Math.round(i.w * s));
    const h = Math.max(1, Math.round(i.h * s));
    const rgba = e.render(Math.floor(i.ip + (i.op - i.ip) / 2), w, h);
    const c = document.createElement("canvas");
    c.width = w;
    c.height = h;
    c.getContext("2d")!.putImageData(new ImageData(new Uint8ClampedArray(rgba), w, h), 0, 0);
    return c.toDataURL("image/webp", 0.82);
  } catch {
    return undefined;
  }
}

function interactive(): boolean {
  const p = projectValue();
  return Object.keys(p.stateMachines ?? {}).length > 0 || Object.keys(p.themes ?? {}).length > 0;
}

/** Make `meta` the open project and show the editor. */
function activate(meta: ProjectMeta, inLibrary: boolean) {
  session++;
  clearTimeout(timer);
  savedJson = engine().project_json();
  stored = inLibrary;
  useEditor.setState({ project: meta, projectId: meta.id, fileName: meta.name, selection: [], shape: null, scene: "root", mode: "animate", view: "editor" });
  st().bumpHistory();
  st().setFrame(0);
  st().refresh();
  window.dispatchEvent(new Event("lumetrope:fit"));
}

/** Save the open project to the library now. `force` writes even an unedited project. */
export async function saveNow(opts: { force?: boolean } = {}): Promise<void> {
  const s = st();
  let meta = s.project;
  if (!meta) return;
  const mine = session;
  const json = engine().project_json();
  const edited = json !== savedJson;
  if (!stored && !edited && !opts.force) return;
  const now = Date.now();
  meta = { ...meta, name: s.fileName || "Untitled", info: readInfo() };
  if (edited) {
    meta.updated = now;
    if (meta.file) meta.file = { ...meta.file, synced: false };
  }
  if (edited || !meta.thumb) meta.thumb = makeThumb();
  try {
    await library.put(meta, { id: meta.id, project: json, history: engine().vcs_export() });
    if (mine !== session) return; // another project was opened meanwhile; this write belonged to the old one
    savedJson = json;
    stored = true;
    useEditor.setState({ project: meta, savedAt: now });
  } catch {
    /* storage unavailable (private mode / quota): editing continues in memory */
  }
}

/** Debounced autosave; call after document changes. */
export function scheduleSave(): void {
  clearTimeout(timer);
  timer = window.setTimeout(() => void saveNow(), 1200);
}

/** Update a project's metadata; the open project's in-memory copy too if it is still the one open. */
async function patchCurrent(changes: Partial<ProjectMeta>, id = st().project?.id) {
  const meta = st().project;
  if (!id) return;
  if (meta?.id === id) useEditor.setState({ project: { ...meta, ...changes } });
  if (meta?.id !== id || stored) await library.patch(id, changes).catch(() => undefined);
}

export const markPreviewed = () => patchCurrent({ previewed: Date.now() });
export const markExported = (format: string) => patchCurrent({ exported: { at: Date.now(), format } });

/** Keep the stored history and record outside changes as a version. */
function restoreHistory(history: Uint8Array | undefined, label: string) {
  const e = engine();
  try {
    if (history) e.vcs_import(history);
    else e.vcs_reset();
  } catch {
    e.vcs_reset();
  }
  const status = JSON.parse(e.vcs_status());
  const changed = Object.values(status).some((v) => (Array.isArray(v) ? v.length > 0 : v === true));
  if (changed) {
    try {
      e.vcs_commit(label, "You", Date.now());
    } catch {
      /* identical to the last version */
    }
  }
}

/** Put the library copy into version history (so a disk reload never loses it); returns the updated history. */
function keepCachedCopy(data: { project: string; history: Uint8Array }, label: string): Uint8Array {
  engine().set_project_json(data.project);
  restoreHistory(data.history, label);
  return engine().vcs_export();
}

/** Start a blank project (stored once it is first edited). */
export async function createProject(opts: { w?: number; h?: number; fr?: number; frames?: number; name?: string } = {}): Promise<void> {
  await saveNow();
  engine().new_project(opts.w ?? 512, opts.h ?? 512, opts.fr ?? 30, opts.frames ?? 90);
  engine().vcs_reset();
  const now = Date.now();
  activate({ id: newProjectId(), name: opts.name ?? "Untitled", origin: "new", created: now, updated: now, opened: now }, false);
}

/** Open a file the user picked or dropped. Reopening a linked file reuses its project and history. */
export async function openPicked(picked: Picked, origin: Origin = "file", displayName?: string): Promise<boolean> {
  const { file, link } = picked;
  try {
    await saveNow();
    const bytes = new Uint8Array(await file.arrayBuffer());
    const existing = link ? await library.findByLink(link).catch(() => undefined) : undefined;
    engine().load_file(file.name, bytes);
    const now = Date.now();
    let meta: ProjectMeta;
    if (existing) {
      const data = await library.getData(existing.id);
      const history = data ? keepCachedCopy(data, existing.file?.synced ? "Saved copy" : "Edits not saved to the file") : undefined;
      engine().load_file(file.name, bytes);
      restoreHistory(history, `Reloaded ${file.name} from disk`);
      meta = { ...existing, opened: now, file: { name: file.name, link, synced: true } };
    } else {
      engine().vcs_reset();
      engine().vcs_commit(`Opened ${file.name}`, "You", now);
      meta = { id: newProjectId(), name: displayName ?? stripExt(file.name), origin, created: now, updated: now, opened: now, file: origin === "file" ? { name: file.name, link, synced: true } : undefined };
    }
    activate(meta, !!existing);
    await saveNow({ force: true });
    st().toast(`Opened ${file.name}`);
    return true;
  } catch (e) {
    st().toast(String(e), "error");
    return false;
  }
}

export const OPEN_ACCEPT = ".lottie,.json";

/** File ▸ Open: native dialog / file picker. */
export async function openFromDisk(): Promise<void> {
  try {
    const picked = await pickOpen(OPEN_ACCEPT);
    if (picked) await openPicked(picked);
  } catch (e) {
    st().toast(String(e), "error");
  }
}

export async function openSample(url: string, file: string, name?: string): Promise<void> {
  const res = await fetch(url).catch(() => null);
  if (!res?.ok) return st().toast(`Could not load ${file}`, "error");
  await openPicked({ file: new File([await res.arrayBuffer()], file) }, "sample", name);
}

/** Open a project from the library. A file with no unsaved edits is re-read from disk first. */
export async function openProject(id: string): Promise<boolean> {
  await saveNow();
  const [meta, data] = await Promise.all([library.getMeta(id), library.getData(id)]).catch(() => [undefined, undefined] as const);
  if (!meta || !data) {
    st().toast("That project is no longer in the library", "error");
    return false;
  }
  const e = engine();
  const loadCached = () => {
    e.set_project_json(data.project);
    restoreHistory(data.history, "Restored");
  };
  const link = meta.file?.link;
  let readOk = false;
  if (link && meta.file?.synced) {
    try {
      const f = await readLink(link);
      const bytes = new Uint8Array(await f.arrayBuffer());
      e.load_file(f.name, bytes); // validate before touching history
      const history = keepCachedCopy(data, "Saved copy");
      e.load_file(f.name, bytes);
      restoreHistory(history, `Changed on disk: ${f.name}`);
      readOk = true;
    } catch (err) {
      loadCached();
      st().toast(`Couldn't read ${linkLabel(meta.file!)} (${String(err).replace(/^Error: /, "")}); opened the copy saved in Lumetrope`, "error");
    }
  } else loadCached();
  const missing = !!(link && meta.file?.synced && !readOk);
  activate({ ...meta, opened: Date.now(), ...(missing ? { file: { ...meta.file!, synced: false } } : {}) }, true);
  await saveNow({ force: true });
  return true;
}

function fileBytes(format: "lottie" | "json"): Uint8Array {
  return format === "lottie" ? engine().export_dotlottie() : new TextEncoder().encode(engine().export_lottie());
}

/** ⌘S: write back to the project's file, or ask where to save it. */
export async function saveToFile(): Promise<void> {
  const meta = st().project;
  if (!meta) return;
  const f = meta.file;
  if (f?.link) {
    const format = saveFormat(f.name, interactive());
    if (format !== "saveAs") {
      try {
        await writeLink(f.link, fileBytes(format));
        await saveNow({ force: true });
        await patchCurrent({ file: { ...f, synced: true } }, meta.id);
        st().toast(`Saved ${linkLabel(f)}`);
      } catch (e) {
        st().toast(`Could not save: ${e}`, "error");
      }
      return;
    }
    if (/\.json$/i.test(f.name)) st().toast("Lottie JSON can't hold state machines or themes; save it as a .lottie file");
  }
  await saveAs();
}

/** ⇧⌘S: choose a new file (dotLottie keeps everything; plain JSON when you pick .json). */
export async function saveAs(): Promise<void> {
  const meta = st().project;
  if (!meta) return;
  try {
    const target = await pickSave(exportFileName(st().fileName || "project", "lottie"));
    if (!target) return;
    if (target === "download") {
      await saveFile(exportFileName(st().fileName || "project", "lottie"), engine().export_dotlottie(), "application/zip+dotlottie");
      st().toast("Downloaded the project (.lottie)");
      return;
    }
    const format = /\.json$/i.test(target.name) && !interactive() ? "json" : "lottie";
    await writeLink(target.link, fileBytes(format));
    await saveNow({ force: true });
    await patchCurrent({ file: { name: target.name, link: target.link, synced: true } }, meta.id);
    st().toast(`Saved ${linkLabel({ name: target.name, link: target.link, synced: true })}`);
  } catch (e) {
    st().toast(`Could not save: ${e}`, "error");
  }
}

/** New project sized to an SVG, with the artwork centred (scaled up when it is icon-sized). */
export async function createFromSvg(file: File): Promise<void> {
  const svg = await file.text();
  const size = svgCanvasSize(svg);
  await createProject({ ...size, name: file.name.replace(/\.svg$/i, "") });
  const r = act<{ id: string }>("import_svg", { svg, name: file.name.replace(/\.svg$/i, "") });
  const declared = svgDeclaredSize(svg);
  if (r && declared) {
    const k = Math.min(size.w / declared.w, size.h / declared.h) * 100;
    if (Math.abs(k - 100) > 0.01) act("set_transform", { layer: r.id, scale: [k, k] });
  }
  if (r) st().select([r.id]);
}

/** Open Lottie JSON copied to the clipboard as a new project. */
export async function openFromClipboard(): Promise<void> {
  let text = "";
  try {
    text = (await navigator.clipboard?.readText()) ?? "";
  } catch {
    return st().toast("Clipboard access was blocked; paste with ⌘V inside the editor instead", "error");
  }
  if (!looksLikeLottie(text)) return st().toast("The clipboard doesn't hold Lottie JSON", "error");
  let name = "Pasted animation";
  try {
    name = JSON.parse(text).nm || name;
  } catch {
    /* checked above */
  }
  await openPicked({ file: new File([text], "pasted.json") }, "new", name);
}

/** Give projects saved before thumbnails existed a preview (rendered off to the side). */
export async function backfillThumbnails(list: ProjectMeta[], onEach: () => void): Promise<void> {
  for (const m of list) {
    if (m.thumb) continue;
    const data = await library.getData(m.id).catch(() => undefined);
    if (!data) continue;
    const e = new Engine();
    try {
      e.set_project_json(data.project);
      const thumb = makeThumb(e);
      if (thumb) {
        await library.patch(m.id, { thumb, info: m.info ?? readInfo(e) });
        onEach();
      }
    } catch {
      /* unreadable project: keep the placeholder */
    } finally {
      e.free();
    }
    await new Promise((r) => setTimeout(r, 0));
  }
}

/** Approximate local storage used by the library (browser estimate). */
export async function storageUsage(): Promise<{ used: number; quota: number } | null> {
  try {
    const e = await navigator.storage?.estimate?.();
    return e?.usage != null && e.quota != null ? { used: e.usage, quota: e.quota } : null;
  } catch {
    return null;
  }
}

/** Back to the dashboard (saving first). */
export async function goHome(): Promise<void> {
  await saveNow();
  useEditor.setState({ view: "home", mode: "animate" });
}

export async function renameProject(id: string, name: string): Promise<void> {
  const clean = name.trim() || "Untitled";
  if (st().project?.id === id) {
    st().setFileName(clean);
    useEditor.setState({ project: { ...st().project!, name: clean } });
  }
  await library.patch(id, { name: clean });
}

export async function duplicateProject(id: string): Promise<string | null> {
  const [meta, data] = await Promise.all([library.getMeta(id), library.getData(id)]);
  if (!meta || !data) return null;
  const now = Date.now();
  const copy: ProjectMeta = { ...meta, id: newProjectId(), name: `${meta.name} copy`, origin: "new", created: now, updated: now, opened: now, previewed: undefined, exported: undefined, file: undefined };
  await library.put(copy, { ...data, id: copy.id });
  return copy.id;
}

/** Remove a project from Lumetrope's library. Files on disk are never touched. */
export async function removeProject(id: string): Promise<void> {
  await library.remove(id);
  if (st().project?.id === id) {
    stored = false;
    useEditor.setState({ project: null });
  }
}

export type { FileLink, ProjectMeta };
