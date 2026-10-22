import { describe, expect, it } from "vitest";
import { linkLabel, saveFormat } from "./files";

describe("saveFormat", () => {
  it("writes .lottie projects back as dotLottie", () => {
    expect(saveFormat("hero.lottie", false)).toBe("lottie");
    expect(saveFormat("HERO.LOTTIE", true)).toBe("lottie");
  });
  it("writes plain Lottie JSON only when nothing would be lost", () => {
    expect(saveFormat("anim.json", false)).toBe("json");
    expect(saveFormat("anim.json", true)).toBe("saveAs");
  });
  it("asks where to save for anything else", () => {
    expect(saveFormat("logo.svg", false)).toBe("saveAs");
    expect(saveFormat("photo.png", false)).toBe("saveAs");
  });
});

describe("linkLabel", () => {
  it("shows the full path for desktop files and the name for browser handles", () => {
    expect(linkLabel({ name: "a.lottie", synced: true, link: { kind: "path", path: "C:\\work\\a.lottie" } })).toBe("C:\\work\\a.lottie");
    expect(linkLabel({ name: "a.lottie", synced: true, link: { kind: "handle", handle: {} as FileSystemFileHandle } })).toBe("a.lottie");
    expect(linkLabel({ name: "a.lottie", synced: true })).toBe("a.lottie (copy kept in Lumetrope)");
  });
});
