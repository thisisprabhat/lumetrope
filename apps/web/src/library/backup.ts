// Whole-workspace backups: every project with its version history, the
// workspace manifest, editor settings and custom colour presets in one zip.
// The project part uses the workspace-folder layout (folder.ts), so an
// unpacked backup also opens directly as a folder workspace.
import type { Library, ProjectData, ProjectMeta } from "./db";
import { FORMAT, MANIFEST, metaFromJson, metaToJson, parseManifest, projectFile, safeId, type Manifest } from "./folder";

export const SETTINGS_FILE = "settings.json";
export const PRESETS_FILE = "presets/color-schemes.json";

export interface ZipOut {
  add(name: string, bytes: Uint8Array): void;
}

export interface ZipIn {
  names(): string[];
  read(name: string): Uint8Array | null;
}

const enc = new TextEncoder();
const dec = new TextDecoder();
const json = (v: unknown) => enc.encode(JSON.stringify(v, null, 2));

export interface BackupExtras {
  name: string;
  app: string;
  settings: Record<string, string>;
  presets: unknown;
}

/** Write every project of `lib` plus settings and presets into `out`. Returns how many projects went in. */
export async function writeBackup(lib: Library, out: ZipOut, extras: BackupExtras, now = Date.now()): Promise<number> {
  const metas = await lib.list();
  let n = 0;
  const body: [string, Uint8Array][] = [];
  for (const m of metas) {
    const data = await lib.getData(m.id);
    if (!data || !safeId(m.id)) continue;
    body.push([projectFile(m.id, "meta.json"), enc.encode(metaToJson(m))], [projectFile(m.id, "project.json"), enc.encode(data.project)], [projectFile(m.id, "history.bin"), data.history]);
    n++;
  }
  const manifest: Manifest = { format: FORMAT, version: 1, name: extras.name, created: now, exported: now, app: extras.app, projects: n };
  out.add(MANIFEST, json(manifest));
  for (const [name, bytes] of body) out.add(name, bytes);
  out.add(SETTINGS_FILE, json(extras.settings));
  out.add(PRESETS_FILE, json(extras.presets ?? []));
  return n;
}

export interface Backup {
  manifest: Manifest;
  projects: { meta: ProjectMeta; data: ProjectData }[];
  settings: Record<string, string>;
  presets: unknown;
  /** Entries that looked like projects but couldn't be read. */
  broken: number;
}

/** Parse a backup zip; throws when it isn't one. */
export function readBackup(zip: ZipIn): Backup {
  const manifest = parseManifest(zip.read(MANIFEST));
  if (!manifest) throw new Error("This zip isn't a Lumetrope backup (no lumetrope-workspace.json)");
  const ids = new Set<string>();
  for (const n of zip.names()) {
    const m = /^projects\/([^/]+)\/meta\.json$/.exec(n);
    if (m) ids.add(m[1]);
  }
  const projects: Backup["projects"] = [];
  let broken = 0;
  for (const id of ids) {
    const metaBytes = zip.read(projectFile(id, "meta.json"));
    const project = zip.read(projectFile(id, "project.json"));
    const meta = safeId(id) && metaBytes ? metaFromJson(dec.decode(metaBytes), id) : null;
    if (!meta || !project) {
      broken++;
      continue;
    }
    delete (meta as any).__handle;
    projects.push({ meta, data: { id, project: dec.decode(project), history: zip.read(projectFile(id, "history.bin")) ?? new Uint8Array() } });
  }
  const parse = (name: string) => {
    const b = zip.read(name);
    if (!b) return undefined;
    try {
      return JSON.parse(dec.decode(b));
    } catch {
      return undefined;
    }
  };
  const s = parse(SETTINGS_FILE);
  const settings: Record<string, string> = {};
  if (s && typeof s === "object") for (const [k, v] of Object.entries(s)) if (typeof v === "string") settings[k] = v;
  return { manifest, projects, settings, presets: parse(PRESETS_FILE) ?? [], broken };
}

export interface ImportResult {
  added: number;
  updated: number;
  skipped: number;
}

/** Add backup projects to `lib`. A project already there is replaced only by a copy edited at least as recently. */
export async function importProjects(lib: Library, projects: Backup["projects"]): Promise<ImportResult> {
  const r: ImportResult = { added: 0, updated: 0, skipped: 0 };
  for (const { meta, data } of projects) {
    const cur = await lib.getMeta(meta.id);
    if (cur && cur.updated > meta.updated) {
      r.skipped++;
      continue;
    }
    // Keep a link to the file on this machine if the backup only knows the name.
    const file = meta.file && !meta.file.link && cur?.file?.link ? { ...meta.file, link: cur.file.link } : meta.file;
    await lib.put({ ...meta, file }, data);
    if (cur) r.updated++;
    else r.added++;
  }
  return r;
}

/** Settings worth carrying between machines: `lumetrope.*` keys except those listed. */
export function pickSettings(storage: { length: number; key(i: number): string | null; getItem(k: string): string | null }, exclude: string[]): Record<string, string> {
  const out: Record<string, string> = {};
  for (let i = 0; i < storage.length; i++) {
    const k = storage.key(i);
    if (!k || !k.startsWith("lumetrope.") || exclude.includes(k)) continue;
    const v = storage.getItem(k);
    if (v != null) out[k] = v;
  }
  return out;
}
