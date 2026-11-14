import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { clearIconifyCache, collectionCredit, fetchIconSvg, iconPreviewUrl, iconSvgPath, normalizeHex, searchIcons, searchPath, setIconifyNow } from "./api";

const home = {
  icons: ["mdi:home", "not an id", "lucide:house"],
  total: 80,
  collections: { mdi: { name: "Material Design Icons", license: { spdx: "Apache-2.0", title: "Apache 2.0" } }, lucide: { name: "Lucide", license: { title: "ISC" } } },
};

beforeEach(async () => {
  await clearIconifyCache();
  setIconifyNow(() => 1_000_000);
  vi.stubGlobal("fetch", vi.fn());
});

afterEach(() => {
  vi.unstubAllGlobals();
  setIconifyNow(() => Date.now());
});

function json(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), { status, headers: { "content-type": "application/json" } });
}

describe("iconify urls", () => {
  it("bakes a hex colour into the svg request", () => {
    expect(normalizeHex("#0a0")).toBe("00aa00");
    expect(iconSvgPath("lucide:house", { color: "#0a0" })).toBe("/lucide/house.svg?color=%2300aa00");
    expect(iconPreviewUrl("lucide:house")).toBe("https://api.iconify.design/lucide/house.svg?color=%23ffffff&width=32&height=32");
    expect(searchPath("arrow right", { limit: 100, prefix: "lucide" })).toBe("/search?query=arrow+right&limit=64&prefix=lucide");
  });

  it("rejects a colour that is not hex", () => {
    expect(() => iconSvgPath("lucide:house", { color: "red" })).toThrow(/hex/);
  });
});

describe("iconify fetch", () => {
  it("searches the public api and keeps set name and licence", async () => {
    vi.mocked(fetch).mockResolvedValue(json(home));
    const result = await searchIcons("home");
    expect(fetch).toHaveBeenCalledTimes(1);
    expect(vi.mocked(fetch).mock.calls[0][0]).toBe("https://api.iconify.design/search?query=home&limit=32");
    expect(result.icons).toEqual(["mdi:home", "lucide:house"]);
    expect(result.total).toBe(80);
    expect(collectionCredit(result.collections)).toBe("Material Design Icons · Apache-2.0, Lucide · ISC");
  });

  it("uses the next mirror when the first host fails", async () => {
    vi.mocked(fetch).mockRejectedValueOnce(new TypeError("fetch failed")).mockResolvedValueOnce(json(home));
    await searchIcons("home");
    expect(vi.mocked(fetch).mock.calls.map((c) => c[0])).toEqual([
      "https://api.iconify.design/search?query=home&limit=32",
      "https://api.simplesvg.com/search?query=home&limit=32",
    ]);
  });

  it("does not ask the other mirrors for an icon that is missing", async () => {
    vi.mocked(fetch).mockResolvedValue(new Response("missing", { status: 404 }));
    await expect(fetchIconSvg("lucide:missing")).rejects.toThrow("That icon is not on Iconify");
    expect(fetch).toHaveBeenCalledTimes(1);
  });

  it("says to search online when every host is unreachable", async () => {
    vi.mocked(fetch).mockRejectedValue(new TypeError("fetch failed"));
    await expect(searchIcons("home")).rejects.toMatchObject({ offline: true, message: "Search when you're online" });
    expect(fetch).toHaveBeenCalledTimes(3);
  });

  it("remembers a search until it expires", async () => {
    vi.mocked(fetch).mockImplementation(async () => json(home));
    await searchIcons("home");
    await searchIcons("home");
    expect(fetch).toHaveBeenCalledTimes(1);
    setIconifyNow(() => 1_000_000 + 5 * 60 * 1000 + 1);
    await searchIcons("home");
    expect(fetch).toHaveBeenCalledTimes(2);
  });
});
