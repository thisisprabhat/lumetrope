import "fake-indexeddb/auto";
import { IDBFactory } from "fake-indexeddb";
import { beforeEach, describe, expect, it } from "vitest";
import { createLibrary, type Library, type ProjectMeta } from "./db";

let factory: IDBFactory;
let lib: Library;

const meta = (id: string, extra: Partial<ProjectMeta> = {}): ProjectMeta => ({ id, name: id, origin: "new", created: 1, updated: 1, opened: 1, ...extra });
const data = (id: string) => ({ id, project: `{"id":"${id}"}`, history: new Uint8Array([1, 2, 3]) });

beforeEach(() => {
  factory = new IDBFactory();
  lib = createLibrary(factory);
});

describe("library", () => {
  it("stores listing metadata separately from the heavy project data", async () => {
    await lib.put(meta("a", { updated: 5 }), data("a"));
    await lib.put(meta("b", { updated: 9 }), data("b"));
    expect((await lib.list()).map((m) => m.id).sort()).toEqual(["a", "b"]);
    expect((await lib.getData("a"))?.project).toBe('{"id":"a"}');
    expect(Array.from((await lib.getData("a"))!.history)).toEqual([1, 2, 3]);
  });

  it("patches metadata without touching project data", async () => {
    await lib.put(meta("a"), data("a"));
    await lib.patch("a", { previewed: 42, name: "Renamed" });
    const m = await lib.getMeta("a");
    expect(m?.previewed).toBe(42);
    expect(m?.name).toBe("Renamed");
    expect((await lib.getData("a"))?.project).toBe('{"id":"a"}');
  });

  it("finds a project by its linked file path", async () => {
    await lib.put(meta("a", { file: { name: "x.lottie", synced: true, link: { kind: "path", path: "/tmp/x.lottie" } } }), data("a"));
    await lib.put(meta("b"), data("b"));
    expect((await lib.findByLink({ kind: "path", path: "/tmp/x.lottie" }))?.id).toBe("a");
    expect(await lib.findByLink({ kind: "path", path: "/tmp/other.lottie" })).toBeUndefined();
  });

  it("removes both records", async () => {
    await lib.put(meta("a"), data("a"));
    await lib.remove("a");
    expect(await lib.getMeta("a")).toBeUndefined();
    expect(await lib.getData("a")).toBeUndefined();
  });

  it("migrates version-1 projects into the listing", async () => {
    await new Promise<void>((resolve, reject) => {
      const req = factory.open("lumetrope", 1);
      req.onupgradeneeded = () => req.result.createObjectStore("projects", { keyPath: "id" }).put({ id: "old", name: "Old one", updated: 77, project: "{}", history: new Uint8Array() });
      req.onsuccess = () => (req.result.close(), resolve());
      req.onerror = () => reject(req.error);
    });
    const [m] = await createLibrary(factory).list();
    expect(m).toMatchObject({ id: "old", name: "Old one", origin: "new", created: 77, updated: 77, opened: 77 });
    expect((await lib.getData("old"))?.project).toBe("{}");
  });
});

describe("closing", () => {
  it("lets the database be deleted and reopens on next use", async () => {
    await lib.put(meta("a"), data("a"));
    await lib.close?.();
    const deleted = await new Promise<string>((resolve) => {
      const r = factory.deleteDatabase("lumetrope");
      r.onsuccess = () => resolve("deleted");
      r.onblocked = () => resolve("blocked");
    });
    expect(deleted).toBe("deleted");
    expect(await lib.list()).toEqual([]);
  });
});
