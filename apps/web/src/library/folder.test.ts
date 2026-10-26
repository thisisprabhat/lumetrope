import "fake-indexeddb/auto";
import { IDBFactory } from "fake-indexeddb";
import { describe, expect, it } from "vitest";
import { importProjects, pickSettings, readBackup, writeBackup, type ZipIn } from "./backup";
import { createLibrary, type ProjectMeta } from "./db";
import { createFolderLibrary, initFolder, MANIFEST, memoryDir, readManifest, type HandleStore } from "./folder";

const meta = (id: string, extra: Partial<ProjectMeta> = {}): ProjectMeta => ({ id, name: id, origin: "new", created: 1, updated: 1, opened: 1, ...extra });
const data = (id: string, project = `{"id":"${id}"}`) => ({ id, project, history: new Uint8Array([1, 2, 3]) });

function memZip() {
  const files = new Map<string, Uint8Array>();
  const zin: ZipIn = { names: () => [...files.keys()], read: (n) => files.get(n) ?? null };
  return { files, out: { add: (n: string, b: Uint8Array) => void files.set(n, b) }, zin };
}

describe("folder library", () => {
  it("round-trips projects through files and lists them", async () => {
    const dir = memoryDir();
    const lib = createFolderLibrary(dir);
    await lib.put(meta("a", { name: "Alpha", thumb: "data:x" }), data("a"));
    await lib.put(meta("b"), data("b"));
    expect((await lib.list()).map((m) => m.id).sort()).toEqual(["a", "b"]);
    expect(await lib.getMeta("a")).toMatchObject({ name: "Alpha", thumb: "data:x" });
    expect(await lib.getData("a")).toEqual(data("a"));
    expect([...dir.files.keys()].sort()).toContain("projects/a/history.bin");
    await lib.patch("a", { name: "Renamed" });
    expect((await lib.getMeta("a"))?.name).toBe("Renamed");
    await lib.remove("a");
    expect((await lib.list()).map((m) => m.id)).toEqual(["b"]);
    expect(await lib.getData("a")).toBeUndefined();
  });

  it("keeps path links in JSON and browser handles in the handle store", async () => {
    const handles = new Map<string, any>();
    const store: HandleStore = { get: async (p) => handles.get(p), put: async (p, h) => void handles.set(p, h), delete: async (p) => void handles.delete(p) };
    const dir = memoryDir();
    const lib = createFolderLibrary(dir, store);
    const handle = { kind: "file", name: "x.lottie", isSameEntry: async (o: unknown) => o === handle } as any;
    await lib.put(meta("p", { file: { name: "a.lottie", synced: true, link: { kind: "path", path: "/tmp/a.lottie" } } }), data("p"));
    await lib.put(meta("h", { file: { name: "x.lottie", synced: false, link: { kind: "handle", handle } } }), data("h"));
    expect(new TextDecoder().decode(dir.files.get("projects/h/meta.json")!)).not.toContain("isSameEntry");
    expect((await lib.getMeta("p"))?.file?.link).toEqual({ kind: "path", path: "/tmp/a.lottie" });
    expect((await lib.getMeta("h"))?.file?.link).toEqual({ kind: "handle", handle });
    expect((await lib.findByLink({ kind: "handle", handle }))?.id).toBe("h");
    expect((await lib.findByLink({ kind: "path", path: "/tmp/a.lottie" }))?.id).toBe("p");
  });

  it("ignores folders and ids that aren't projects", async () => {
    const dir = memoryDir();
    dir.files.set("projects/../evil/meta.json", new TextEncoder().encode("{}"));
    dir.files.set("projects/junk/meta.json", new TextEncoder().encode("not json"));
    const lib = createFolderLibrary(dir);
    expect(await lib.list()).toEqual([]);
    await expect(lib.put(meta("../x"), data("../x"))).rejects.toThrow();
  });

  it("initialises a folder once and keeps an existing manifest", async () => {
    const dir = memoryDir();
    const m = await initFolder(dir, "Studio");
    expect(m.name).toBe("Studio");
    await initFolder(dir, "Other");
    expect((await readManifest(dir))?.name).toBe("Studio");
    expect(dir.files.has(MANIFEST)).toBe(true);
  });
});

describe("backup", () => {
  it("writes a zip that reads back into the same projects, settings and presets", async () => {
    const src = createLibrary(new IDBFactory());
    await src.put(meta("a", { updated: 5 }), data("a"));
    await src.put(meta("b", { file: { name: "b.json", synced: true, link: { kind: "path", path: "/b.json" } } }), data("b"));
    const z = memZip();
    const n = await writeBackup(src, z.out, { name: "Home", app: "test", settings: { "lumetrope.renderer": "cpu" }, presets: [{ id: "c1", name: "Mine", colors: ["#123456"] }] }, 42);
    expect(n).toBe(2);
    const b = readBackup(z.zin);
    expect(b.manifest).toMatchObject({ name: "Home", projects: 2, exported: 42 });
    expect(b.projects.map((p) => p.meta.id).sort()).toEqual(["a", "b"]);
    expect(b.projects.find((p) => p.meta.id === "a")!.data).toEqual(data("a"));
    expect(b.settings).toEqual({ "lumetrope.renderer": "cpu" });
    expect(b.presets).toEqual([{ id: "c1", name: "Mine", colors: ["#123456"] }]);
    // The project part is a valid workspace folder.
    const folder = createFolderLibrary(memoryDir(z.files));
    expect((await folder.list()).length).toBe(2);
  });

  it("imports newer copies, keeps newer local ones and preserves local file links", async () => {
    const lib = createLibrary(new IDBFactory());
    await lib.put(meta("same", { updated: 10, name: "Local newer" }), data("same", "local"));
    await lib.put(meta("old", { updated: 1, file: { name: "o.lottie", synced: true, link: { kind: "path", path: "/o.lottie" } } }), data("old", "local"));
    const r = await importProjects(lib, [
      { meta: meta("same", { updated: 5 }), data: data("same", "backup") },
      { meta: meta("old", { updated: 9, file: { name: "o.lottie", synced: false } }), data: data("old", "backup") },
      { meta: meta("new"), data: data("new") },
    ]);
    expect(r).toEqual({ added: 1, updated: 1, skipped: 1 });
    expect((await lib.getData("same"))?.project).toBe("local");
    expect((await lib.getData("old"))?.project).toBe("backup");
    expect((await lib.getMeta("old"))?.file?.link).toEqual({ kind: "path", path: "/o.lottie" });
  });

  it("rejects zips that aren't backups and counts unreadable projects", () => {
    const z = memZip();
    expect(() => readBackup(z.zin)).toThrow(/isn't a Lumetrope backup/);
    z.out.add(MANIFEST, new TextEncoder().encode(JSON.stringify({ format: "lumetrope-workspace", version: 1, name: "x", created: 1 })));
    z.out.add("projects/a/meta.json", new TextEncoder().encode(JSON.stringify(meta("a"))));
    expect(readBackup(z.zin)).toMatchObject({ projects: [], broken: 1 });
  });

  it("picks only portable lumetrope settings", () => {
    const kv: [string, string][] = [["lumetrope.renderer", "gpu"], ["lumetrope.workspaces", "[]"], ["other", "x"]];
    const storage = { length: kv.length, key: (i: number) => kv[i][0], getItem: (k: string) => kv.find((e) => e[0] === k)?.[1] ?? null };
    expect(pickSettings(storage, ["lumetrope.workspaces"])).toEqual({ "lumetrope.renderer": "gpu" });
  });
});
