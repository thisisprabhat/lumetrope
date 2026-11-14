import "fake-indexeddb/auto";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { SEARCH_TTL, clearIconifyCache, searchIcons, setIconifyNow } from "./api";

const body = { icons: ["lucide:house"], total: 1, collections: { lucide: { name: "Lucide", license: { spdx: "ISC" } } } };

beforeEach(async () => {
  await clearIconifyCache();
  setIconifyNow(() => 5_000);
  vi.stubGlobal("fetch", vi.fn(async () => new Response(JSON.stringify(body), { status: 200, headers: { "content-type": "application/json" } })));
});

afterEach(() => {
  vi.unstubAllGlobals();
  setIconifyNow(() => Date.now());
});

describe("iconify indexedDB cache", () => {
  it("reloads a search from IndexedDB after the memory cache is dropped", async () => {
    await searchIcons("house");
    expect(fetch).toHaveBeenCalledTimes(1);
    await clearIconifyCache("memory");
    vi.mocked(fetch).mockClear();
    const again = await searchIcons("house");
    expect(fetch).not.toHaveBeenCalled();
    expect(again.icons).toEqual(["lucide:house"]);
    expect(again.collections.lucide).toEqual({ name: "Lucide", license: "ISC" });

    setIconifyNow(() => 5_000 + SEARCH_TTL + 1);
    await clearIconifyCache("memory");
    await searchIcons("house");
    expect(fetch).toHaveBeenCalledTimes(1);
  });
});
