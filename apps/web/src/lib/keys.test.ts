import { describe, expect, it } from "vitest";
import { isMacPlatform, keyLabel } from "./keys";

describe("keyLabel", () => {
  it("keeps macOS glyphs on a Mac", () => {
    expect(keyLabel("⇧⌘Z", true)).toBe("⇧⌘Z");
  });
  it("spells shortcuts out with Ctrl on Windows and Linux", () => {
    expect(keyLabel("⌘Z", false)).toBe("Ctrl+Z");
    expect(keyLabel("⇧⌘Z", false)).toBe("Ctrl+Shift+Z");
    expect(keyLabel("⇧1", false)).toBe("Shift+1");
    expect(keyLabel("⌥⌘C", false)).toBe("Ctrl+Alt+C");
    expect(keyLabel("⌘+", false)).toBe("Ctrl++");
    expect(keyLabel("Space", false)).toBe("Space");
  });
});

describe("isMacPlatform", () => {
  it("reads the user agent platform", () => {
    expect(isMacPlatform({ userAgentData: { platform: "macOS" } })).toBe(true);
    expect(isMacPlatform({ platform: "MacIntel" })).toBe(true);
    expect(isMacPlatform({ platform: "Win32" })).toBe(false);
    expect(isMacPlatform({ userAgentData: { platform: "Linux" }, platform: "Linux x86_64" })).toBe(false);
  });
});
