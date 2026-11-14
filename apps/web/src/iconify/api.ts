// Iconify's public API, called from the editor itself. Icon bodies are cached;
// a placed icon is copied into the file and does not stay bound to the network.

export const ICONIFY_HOSTS = ["https://api.iconify.design", "https://api.simplesvg.com", "https://api.unisvg.com"] as const;

export const SEARCH_TTL = 5 * 60 * 1000;
const SVG_TTL = 30 * 24 * 60 * 60 * 1000;
const PREFIX = /^[a-z0-9][a-z0-9-]*$/i;
const NAME = /^[a-z0-9][a-z0-9_.-]*$/i;

export class IconifyError extends Error {
  readonly offline: boolean;
  constructor(message: string, offline = false) {
    super(message);
    this.name = "IconifyError";
    this.offline = offline;
  }
}

export interface IconCollection {
  name: string;
  license: string;
}

export interface IconSearch {
  icons: string[];
  total: number;
  collections: Record<string, IconCollection>;
}

type CacheRow = { value: string; expires: number };

const memory = new Map<string, CacheRow>();
const inflight = new Map<string, Promise<string>>();
let clock = () => Date.now();
let dbp: Promise<IDBDatabase> | null = null;

/** Test hook so cache expiry can move without waiting. */
export function setIconifyNow(fn: () => number): void {
  clock = fn;
}

export async function clearIconifyCache(scope: "all" | "memory" = "all"): Promise<void> {
  memory.clear();
  inflight.clear();
  if (scope === "memory" || typeof indexedDB === "undefined") return;
  try {
    const db = await openCache();
    await idbReq(db.transaction("entries", "readwrite").objectStore("entries").clear());
  } catch {
    dbp = null;
  }
}

export function parseIconId(icon: string): { prefix: string; name: string } | null {
  const i = icon.indexOf(":");
  if (i <= 0 || i === icon.length - 1) return null;
  const prefix = icon.slice(0, i);
  const name = icon.slice(i + 1);
  if (!PREFIX.test(prefix) || !NAME.test(name) || name.includes("..")) return null;
  return { prefix, name };
}

export function normalizeHex(color: string | undefined): string {
  if (color == null || color.trim() === "") return "000000";
  const raw = color.trim().replace(/^#/, "");
  if (/^[0-9a-fA-F]{3}$/.test(raw)) return raw.split("").map((c) => c + c).join("").toLowerCase();
  if (/^[0-9a-fA-F]{6}$/.test(raw)) return raw.toLowerCase();
  throw new IconifyError("Color must be a hex value like #000000");
}

export function clampLimit(limit: unknown): number {
  const n = Number(limit);
  if (!Number.isFinite(n)) return 32;
  return Math.max(1, Math.min(64, Math.round(n)));
}

export function searchPath(query: string, opts: { limit?: unknown; prefix?: string } = {}): string {
  const q = new URLSearchParams({ query: query.trim(), limit: String(clampLimit(opts.limit)) });
  if (opts.prefix) {
    if (!PREFIX.test(opts.prefix)) throw new IconifyError("Unknown icon set");
    q.set("prefix", opts.prefix);
  }
  return `/search?${q}`;
}

/** SVG path with a real fill baked in, so currentColor survives SVG import. */
export function iconSvgPath(icon: string, opts: { color?: string; size?: number } = {}): string {
  const id = parseIconId(icon);
  if (!id) throw new IconifyError("Icon must look like prefix:name");
  const q = new URLSearchParams({ color: `#${normalizeHex(opts.color)}` });
  if (opts.size) {
    q.set("width", String(opts.size));
    q.set("height", String(opts.size));
  }
  return `/${encodeURIComponent(id.prefix)}/${encodeURIComponent(id.name)}.svg?${q}`;
}

export function iconPreviewUrl(icon: string): string {
  // Tint currentColor icons for the dark picker; explicit SVG colors stay intact.
  return `${ICONIFY_HOSTS[0]}${iconSvgPath(icon, { color: "#ffffff", size: 32 })}`;
}

export function collectionCredit(collections: Record<string, IconCollection>): string {
  return Object.values(collections)
    .map((c) => (c.license ? `${c.name} · ${c.license}` : c.name))
    .filter(Boolean)
    .join(", ");
}

export async function searchIcons(query: string, opts: { limit?: unknown; prefix?: string } = {}): Promise<IconSearch> {
  const q = query.trim();
  if (!q) return { icons: [], total: 0, collections: {} };
  const path = searchPath(q, opts);
  const key = `search:${q.toLowerCase()}:${clampLimit(opts.limit)}:${opts.prefix ?? ""}`;
  const raw = await cached(key, SEARCH_TTL, async () => {
    const res = await fetchMirrors(path, false);
    return JSON.stringify(parseSearch(await res.json()));
  });
  return JSON.parse(raw) as IconSearch;
}

export async function fetchIconSvg(icon: string, opts: { color?: string } = {}): Promise<string> {
  const color = normalizeHex(opts.color);
  const path = iconSvgPath(icon, { color });
  const key = `svg:${parseIconId(icon)!.prefix}:${parseIconId(icon)!.name}:${color}`;
  return cached(key, SVG_TTL, async () => {
    const res = await fetchMirrors(path, true);
    const text = (await res.text()).trim();
    if (!text.includes("<svg")) throw new IconifyError("Iconify did not return an SVG");
    return text;
  });
}

async function cached(key: string, ttl: number, load: () => Promise<string>): Promise<string> {
  const hit = await readCache(key);
  if (hit !== undefined) return hit;
  const pending = inflight.get(key);
  if (pending) return pending;
  const p = load()
    .then(async (value) => {
      await writeCache(key, value, ttl);
      return value;
    })
    .finally(() => inflight.delete(key));
  inflight.set(key, p);
  return p;
}

async function fetchMirrors(path: string, missingIsFinal: boolean): Promise<Response> {
  let last: unknown;
  for (const host of ICONIFY_HOSTS) {
    try {
      const res = await fetch(`${host}${path}`, { signal: AbortSignal.timeout(8000) });
      if (res.ok) return res;
      if (res.status === 404 && missingIsFinal) throw new IconifyError("That icon is not on Iconify");
      last = new IconifyError(`Iconify returned ${res.status}`);
    } catch (e) {
      if (e instanceof IconifyError) throw e;
      last = e;
    }
  }
  if (last instanceof IconifyError) throw last;
  if (last instanceof DOMException && last.name === "AbortError") throw new IconifyError("Icon search timed out");
  throw new IconifyError("Search when you're online", true);
}

function parseSearch(data: unknown): IconSearch {
  const d = (data ?? {}) as { icons?: unknown; total?: unknown; collections?: unknown };
  const icons = Array.isArray(d.icons) ? d.icons.filter((x): x is string => typeof x === "string" && parseIconId(x) !== null) : [];
  const collections: Record<string, IconCollection> = {};
  if (d.collections && typeof d.collections === "object") {
    for (const [prefix, info] of Object.entries(d.collections as Record<string, unknown>)) {
      const row = (info ?? {}) as { name?: unknown; license?: unknown };
      const license = row.license;
      const licenseText = typeof license === "string" ? license : license && typeof license === "object" ? String((license as { spdx?: unknown; title?: unknown }).spdx || (license as { title?: unknown }).title || "") : "";
      collections[prefix] = { name: typeof row.name === "string" && row.name ? row.name : prefix, license: licenseText };
    }
  }
  return { icons, total: Number(d.total) || icons.length, collections };
}

async function readCache(key: string): Promise<string | undefined> {
  const now = clock();
  const mem = memory.get(key);
  if (mem && mem.expires > now) return mem.value;
  if (mem) memory.delete(key);
  const row = await idbGet(key);
  if (!row) return undefined;
  if (row.expires <= now) {
    void idbDelete(key);
    return undefined;
  }
  memory.set(key, row);
  return row.value;
}

async function writeCache(key: string, value: string, ttl: number): Promise<void> {
  const row = { value, expires: clock() + ttl };
  memory.set(key, row);
  await idbSet(key, row);
}

function openCache(): Promise<IDBDatabase> {
  if (typeof indexedDB === "undefined") return Promise.reject(new Error("no indexedDB"));
  if (!dbp) {
    dbp = new Promise((resolve, reject) => {
      const req = indexedDB.open("lumetrope-iconify", 1);
      req.onupgradeneeded = () => {
        if (!req.result.objectStoreNames.contains("entries")) req.result.createObjectStore("entries");
      };
      req.onsuccess = () => resolve(req.result);
      req.onerror = () => reject(req.error);
    });
    dbp.catch(() => {
      dbp = null;
    });
  }
  return dbp;
}

async function idbGet(key: string): Promise<CacheRow | undefined> {
  try {
    const db = await openCache();
    const row = await idbReq<CacheRow | undefined>(db.transaction("entries", "readonly").objectStore("entries").get(key));
    if (!row || typeof row.value !== "string" || typeof row.expires !== "number") return undefined;
    return row;
  } catch {
    return undefined;
  }
}

async function idbSet(key: string, row: CacheRow): Promise<void> {
  try {
    const db = await openCache();
    await idbReq(db.transaction("entries", "readwrite").objectStore("entries").put(row, key));
  } catch {
    /* memory cache still serves this session */
  }
}

async function idbDelete(key: string): Promise<void> {
  try {
    const db = await openCache();
    await idbReq(db.transaction("entries", "readwrite").objectStore("entries").delete(key));
  } catch {
    /* ignore */
  }
}

function idbReq<T>(req: IDBRequest<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    req.onsuccess = () => resolve(req.result);
    req.onerror = () => reject(req.error);
  });
}
