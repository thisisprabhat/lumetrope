import { describe, expect, it } from "vitest";
import { exportFileName, formatBytes } from "./export";

describe("export helpers", () => {
  it("builds safe file names per format", () => {
    expect(exportFileName("My Anim", "json")).toBe("My Anim.json");
    expect(exportFileName("a/b:c", "lottie")).toBe("a-b-c.lottie");
    expect(exportFileName("  ", "lottie")).toBe("animation.lottie");
    expect(exportFileName("x.lottie", "lottie")).toBe("x.lottie");
    expect(exportFileName("x.gif", "gif")).toBe("x.gif");
    expect(exportFileName("loop.webm", "webp")).toBe("loop.webp");
  });
  it("formats sizes", () => {
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(2048)).toBe("2.0 KB");
    expect(formatBytes(3 * 1024 * 1024)).toBe("3.0 MB");
  });
});
