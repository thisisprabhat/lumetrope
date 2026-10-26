import { beforeEach, describe, expect, it } from "vitest";
import { BUILT_IN, CATEGORIES, CUSTOM_KEY, loadCustomSchemes, mergeCustomSchemes } from "./colorSchemes";

describe("built-in colour schemes", () => {
  it("offers at least 100 presets with unique ids and valid colours", () => {
    expect(BUILT_IN.length).toBeGreaterThanOrEqual(100);
    expect(new Set(BUILT_IN.map((s) => s.id)).size).toBe(BUILT_IN.length);
    for (const s of BUILT_IN) {
      expect(s.colors.length, s.name).toBeGreaterThanOrEqual(2);
      for (const c of s.colors) expect(c, s.name).toMatch(/^#[0-9a-f]{6}$/);
    }
  });
  it("lists Custom first among the categories", () => {
    expect(CATEGORIES[0]).toBe("Custom");
    expect(CATEGORIES).toContain("Monochrome");
  });
});

describe("custom presets", () => {
  beforeEach(() => {
    const store = new Map<string, string>();
    (globalThis as any).localStorage = { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => store.set(k, v), removeItem: (k: string) => store.delete(k) };
    (globalThis as any).window = { dispatchEvent: () => true };
  });

  it("merges a backup by id and drops malformed entries", () => {
    localStorage.setItem(CUSTOM_KEY, JSON.stringify([{ id: "a", name: "Old", colors: ["#000000"] }]));
    const n = mergeCustomSchemes([{ id: "a", name: "New", colors: ["#FFF", "#123456"] }, { id: "b", name: "B", colors: ["#ff0000"] }, { id: "bad", colors: "x" }, { id: "c", name: "No colours", colors: ["nope"] }]);
    expect(n).toBe(2);
    const list = loadCustomSchemes();
    expect(list.map((s) => [s.id, s.name, s.colors])).toEqual([
      ["a", "New", ["#ffffff", "#123456"]],
      ["b", "B", ["#ff0000"]],
    ]);
    expect(list.every((s) => s.custom && s.category === "Custom")).toBe(true);
  });
});
