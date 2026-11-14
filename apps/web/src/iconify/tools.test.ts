import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { clearIconifyCache, setIconifyNow } from "./api";
import { iconImportArgs, sizeIconForCanvas } from "./place";
import { iconifyImport, iconifySearch } from "./tools";

const svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M0 0h24v24H0z"/></svg>';

beforeEach(async () => {
  await clearIconifyCache();
  setIconifyNow(() => 1_000);
  vi.stubGlobal(
    "fetch",
    vi.fn(async (url: string) => {
      if (String(url).includes("/search?")) {
        return new Response(JSON.stringify({ icons: ["lucide:house", "mdi:home"], total: 2, collections: { lucide: { name: "Lucide", license: { spdx: "ISC" } } } }), { status: 200 });
      }
      return new Response(svg, { status: 200 });
    }),
  );
});

afterEach(() => {
  vi.unstubAllGlobals();
  setIconifyNow(() => Date.now());
});

describe("placing an icon", () => {
  it("sizes picker icons to a quarter of the shorter canvas dimension", async () => {
    const args = await iconImportArgs("lucide:house", { scene: "comp", canvas: { w: 1920, h: 1080 } });
    expect(args.svg).toContain('width="270" height="270"');
    expect(args.svg).toContain('viewBox="0 0 24 24"');
  });

  it("preserves wide icon proportions and replaces intrinsic dimensions", () => {
    const wide = '<svg width="48" height="24" viewBox="0 0 48 24"><path/></svg>';
    expect(sizeIconForCanvas(wide, { w: 400, h: 800 })).toBe('<svg viewBox="0 0 48 24" width="100" height="50"><path/></svg>');
  });

  it("adds a viewBox when only intrinsic dimensions are available", () => {
    expect(sizeIconForCanvas('<svg width="24" height="12"><path/></svg>', { w: 200, h: 200 }))
      .toBe('<svg viewBox="0 0 24 12" width="50" height="25"><path/></svg>');
  });

  it("leaves SVGs unchanged when the canvas or SVG dimensions are unavailable", () => {
    expect(sizeIconForCanvas(svg, { w: 0, h: 100 })).toBe(svg);
    expect(sizeIconForCanvas('<svg><path/></svg>', { w: 400, h: 400 })).toBe('<svg><path/></svg>');
  });

  it("builds an import_svg call whose markup starts with <svg", async () => {
    const args = await iconImportArgs("lucide:house", { scene: "comp" });
    expect(args).toEqual({ svg, name: "lucide:house", scene: "comp" });
    expect(args.svg.startsWith("<svg")).toBe(true);
  });
});

describe("iconify host tools", () => {
  it("searches and returns icon ids", async () => {
    await expect(iconifySearch({ query: "  " })).rejects.toThrow("missing 'query'");
    const found = await iconifySearch({ query: "house", limit: 8, prefix: "lucide" });
    expect(found.icons).toEqual(["lucide:house", "mdi:home"]);
    expect(found.total).toBe(2);
    expect(String(vi.mocked(fetch).mock.calls[0][0])).toContain("prefix=lucide");
  });

  it("passes the downloaded svg to import_svg", async () => {
    const calls: unknown[] = [];
    const result = await iconifyImport({ icon: "lucide:house", color: "#111111", position: [3, 4], scene: "root" }, (args) => {
      calls.push(args);
      return { id: "root:3" };
    });
    expect(result).toEqual({ id: "root:3" });
    expect(calls).toEqual([{ svg, name: "lucide:house", scene: "root", position: [3, 4] }]);
    expect(String(vi.mocked(fetch).mock.calls[0][0])).toContain("color=%23111111");
    expect((calls[0] as { svg: string }).svg.startsWith("<svg")).toBe(true);
  });
});
