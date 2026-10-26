// Workspace commands for the UI: switch, open a folder, create, remove,
// reconnect, and back up / restore everything as one zip.
import { CUSTOM_KEY, loadCustomSchemes, mergeCustomSchemes } from "./colorSchemes";
import { importProjects, pickSettings, readBackup, writeBackup } from "./library/backup";
import {
  addWorkspace,
  CURRENT_KEY,
  currentWorkspace,
  DEFAULT_WORKSPACE,
  forgetWorkspace,
  listWorkspaces,
  newWorkspaceId,
  pickFolderWorkspace,
  REGISTRY_KEY,
  renameWorkspace as renameEntry,
  setCurrentWorkspace,
  type WorkspaceInfo,
} from "./library/workspace";
import { closeProject, enterWorkspace, library, saveNow } from "./persist";
import { pickFile, saveFile } from "./platform";
import { useEditor } from "./store";
import { ZipBuilder, ZipReader } from "./wasm/lumetrope_wasm.js";
import pkg from "../package.json";

const st = () => useEditor.getState();
const err = (e: unknown) => st().toast(String(e).replace(/^Error: /, ""), "error");
/** Keys that describe this machine's workspaces, not portable settings. */
const LOCAL_KEYS = [REGISTRY_KEY, CURRENT_KEY, CUSTOM_KEY];

export async function switchWorkspace(id: string, request = false): Promise<void> {
  const w = listWorkspaces().find((x) => x.id === id);
  if (!w) return;
  await closeProject();
  setCurrentWorkspace(w.id);
  const ok = await enterWorkspace(w, request);
  if (ok) st().toast(`Workspace: ${w.name}`);
}

/** Ask for folder access again (browsers forget it between visits). Call from a click. */
export async function reconnectWorkspace(): Promise<void> {
  const ok = await enterWorkspace(currentWorkspace(), true);
  if (!ok) st().toast("Lumetrope still can't open that folder", "error");
}

export async function openFolderWorkspace(): Promise<void> {
  try {
    const w = await pickFolderWorkspace();
    if (w) await switchWorkspace(w.id, true);
  } catch (e) {
    err(e);
  }
}

export async function newBrowserWorkspace(name: string): Promise<void> {
  const w: WorkspaceInfo = { id: newWorkspaceId(), name: name.trim() || "Workspace", kind: "browser", created: Date.now() };
  addWorkspace(w);
  await switchWorkspace(w.id);
}

export function renameWorkspace(id: string, name: string): void {
  renameEntry(id, name.trim() || "Workspace");
  const cur = st().workspace;
  if (cur.info.id === id) useEditor.setState({ workspace: { ...cur, info: { ...cur.info, name: name.trim() || "Workspace" } } });
}

export async function removeWorkspace(w: WorkspaceInfo): Promise<void> {
  if (w.id === DEFAULT_WORKSPACE.id) return;
  if (st().workspace.info.id === w.id) await switchWorkspace(DEFAULT_WORKSPACE.id);
  await forgetWorkspace(w);
  useEditor.setState((s) => ({ libraryTick: s.libraryTick + 1 }));
  st().toast(w.kind === "folder" ? `Removed ${w.name} from the list; the folder is untouched` : `Deleted workspace ${w.name}`);
}

const stamp = (t: number) => new Date(t).toISOString().slice(0, 10);
const slug = (s: string) => s.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "") || "workspace";

/** Everything in the current workspace — projects, version history, settings, custom presets — as one zip. */
export async function exportAllData(): Promise<void> {
  const ws = st().workspace.info;
  try {
    await saveNow();
    const z = new ZipBuilder();
    let settings: Record<string, string> = {};
    try {
      settings = pickSettings(localStorage, LOCAL_KEYS);
    } catch {
      /* storage unavailable */
    }
    const n = await writeBackup(library, { add: (name, bytes) => z.add(name, bytes) }, { name: ws.name, app: `Lumetrope ${pkg.version}`, settings, presets: loadCustomSchemes().map(({ id, name, colors }) => ({ id, name, colors })) });
    const bytes = z.finish();
    if (await saveFile(`lumetrope-${slug(ws.name)}-${stamp(Date.now())}.zip`, bytes, "application/zip")) st().toast(`Exported ${n} project${n === 1 ? "" : "s"} with history, settings and presets`);
  } catch (e) {
    err(e);
  }
}

/** Restore a backup zip into the current workspace (newer copies win), plus its presets and settings. */
export async function importBackupFile(file?: File): Promise<void> {
  try {
    const f = file ?? (await pickFile(".zip"));
    if (!f) return;
    const reader = new ZipReader(new Uint8Array(await f.arrayBuffer()));
    let backup;
    try {
      const names: string[] = JSON.parse(reader.names());
      backup = readBackup({ names: () => names, read: (n) => reader.read(n) ?? null });
    } finally {
      reader.free();
    }
    await closeProject();
    const r = await importProjects(library, backup.projects);
    const presets = mergeCustomSchemes(backup.presets);
    restoreSettings(backup.settings);
    useEditor.setState((s) => ({ libraryTick: s.libraryTick + 1 }));
    const parts = [`${r.added} added`, r.updated && `${r.updated} updated`, r.skipped && `${r.skipped} kept (yours were newer)`, presets && `${presets} preset${presets === 1 ? "" : "s"}`, backup.broken && `${backup.broken} unreadable`].filter(Boolean);
    st().toast(`Imported “${backup.manifest.name}”: ${parts.join(", ")}`);
  } catch (e) {
    err(e);
  }
}

function restoreSettings(settings: Record<string, string>) {
  for (const [k, v] of Object.entries(settings)) {
    if (!k.startsWith("lumetrope.") || LOCAL_KEYS.includes(k)) continue;
    try {
      localStorage.setItem(k, v);
    } catch {
      return;
    }
  }
  const s = st();
  const r = settings["lumetrope.renderer"];
  if (r === "gpu" || r === "cpu") s.setRenderer(r);
  const w = settings["lumetrope.wheelMode"];
  if (w === "auto" || w === "zoom" || w === "scroll") s.setWheelMode(w);
  try {
    const p = JSON.parse(settings["lumetrope.panes"] ?? "null");
    if (p && [p.left, p.right, p.bottom].every((n: unknown) => typeof n === "number" && n > 0)) s.setPanes(p);
  } catch {
    /* keep current panes */
  }
}
