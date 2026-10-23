import { describe, expect, it } from "vitest";
import { defaultRenderer, dialogFilters, isTauri, localMcpSupported } from "./platform";

describe("platform", () => {
  it("detects the Tauri shell", () => {
    expect(isTauri({})).toBe(false);
    expect(isTauri({ __TAURI_INTERNALS__: {} })).toBe(true);
  });
  it("maps accept strings to native dialog filters", () => {
    expect(dialogFilters(".json,.lottie")).toEqual([{ name: "Supported files", extensions: ["json", "lottie"] }]);
    expect(dialogFilters("")).toEqual([]);
  });
  it("defaults to the GPU renderer only where Canvas2D filters work", () => {
    expect(defaultRenderer(null, true)).toBe("gpu");
    expect(defaultRenderer(null, false)).toBe("cpu");
    expect(defaultRenderer("cpu", true)).toBe("cpu");
    expect(defaultRenderer("gpu", false)).toBe("gpu");
    expect(defaultRenderer("bogus", false)).toBe("cpu");
  });
  it("offers local MCP only where the bridge can be reached", () => {
    expect(localMcpSupported({ __TAURI_INTERNALS__: {}, location: { hostname: "tauri.localhost" } })).toBe(true);
    for (const host of ["localhost", "127.0.0.1", "[::1]", "::1", "studio.localhost"]) expect(localMcpSupported({ location: { hostname: host } })).toBe(true);
    for (const host of ["lumetrope.example.com", "192.168.1.20", "localhost.evil.com"]) expect(localMcpSupported({ location: { hostname: host } })).toBe(false);
  });
});
