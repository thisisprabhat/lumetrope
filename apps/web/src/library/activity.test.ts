import { describe, expect, it } from "vitest";
import type { ProjectMeta } from "./db";
import { filterProjects, lastActivity, sortProjects } from "./activity";

const m = (id: string, extra: Partial<ProjectMeta> = {}): ProjectMeta => ({ id, name: id, origin: "new", created: 10, updated: 10, opened: 10, ...extra });

describe("lastActivity", () => {
  it("reports the most recent thing that happened to a project", () => {
    expect(lastActivity(m("a"))).toEqual({ kind: "created", at: 10 });
    expect(lastActivity(m("a", { origin: "file" }))).toEqual({ kind: "opened", at: 10 });
    expect(lastActivity(m("a", { updated: 30, opened: 20 }))).toEqual({ kind: "edited", at: 30 });
    expect(lastActivity(m("a", { updated: 30, opened: 40 }))).toEqual({ kind: "opened", at: 40 });
    expect(lastActivity(m("a", { updated: 30, previewed: 50 }))).toEqual({ kind: "previewed", at: 50 });
    expect(lastActivity(m("a", { previewed: 50, exported: { at: 60, format: "gif" } }))).toEqual({ kind: "exported", at: 60 });
  });
});

describe("filterProjects", () => {
  const list = [
    m("Logo intro"),
    m("Button", { origin: "file", file: { name: "button.lottie", synced: true, link: { kind: "path", path: "/work/button.lottie" } } }),
    m("Loader", { previewed: 20 }),
    m("Badge", { exported: { at: 30, format: "mp4" } }),
  ];
  const ids = (f: Parameters<typeof filterProjects>[1], q = "") => filterProjects(list, f, q).map((x) => x.id);
  it("filters by activity", () => {
    expect(ids("all")).toEqual(["Logo intro", "Button", "Loader", "Badge"]);
    expect(ids("created")).toEqual(["Logo intro", "Loader", "Badge"]);
    expect(ids("files")).toEqual(["Button"]);
    expect(ids("previewed")).toEqual(["Loader"]);
    expect(ids("exported")).toEqual(["Badge"]);
  });
  it("searches names and file paths, ignoring case", () => {
    expect(ids("all", "LOAD")).toEqual(["Loader"]);
    expect(ids("all", "work/")).toEqual(["Button"]);
  });
});

describe("sortProjects", () => {
  it("sorts by latest activity, name or creation", () => {
    const list = [m("b", { updated: 50 }), m("a", { created: 5, opened: 5, updated: 5 }), m("c", { created: 30, previewed: 90 })];
    expect(sortProjects(list, "recent").map((x) => x.id)).toEqual(["c", "b", "a"]);
    expect(sortProjects(list, "name").map((x) => x.id)).toEqual(["a", "b", "c"]);
    expect(sortProjects(list, "created").map((x) => x.id)).toEqual(["c", "b", "a"]);
  });
});
