import { describe, expect, it } from "vitest";
import { looksLikeLottie, svgCanvasSize, svgDeclaredSize } from "./start";

describe("svgCanvasSize", () => {
  it("uses width and height when present", () => {
    expect(svgCanvasSize('<svg xmlns="http://www.w3.org/2000/svg" width="300" height="150px"><rect/></svg>')).toEqual({ w: 300, h: 150 });
  });
  it("falls back to the viewBox", () => {
    expect(svgCanvasSize('<?xml version="1.0"?>\n<svg viewBox="0 0 240 120" xmlns="http://www.w3.org/2000/svg"></svg>')).toEqual({ w: 512, h: 256 });
    expect(svgCanvasSize("<svg width='100%' viewBox='-5 -5 640,480'></svg>")).toEqual({ w: 640, h: 480 });
  });
  it("scales icon-sized artwork up so it is workable, keeping its shape", () => {
    expect(svgCanvasSize('<svg viewBox="0 0 24 24"></svg>')).toEqual({ w: 512, h: 512 });
  });
  it("keeps sizes sane", () => {
    expect(svgCanvasSize("<svg></svg>")).toEqual({ w: 512, h: 512 });
    expect(svgCanvasSize('<svg width="20000" height="1"></svg>')).toEqual({ w: 4096, h: 16 });
  });
});

describe("svgDeclaredSize", () => {
  it("reports the artwork's own size, or null", () => {
    expect(svgDeclaredSize('<svg viewBox="0 0 24 24"></svg>')).toEqual({ w: 24, h: 24 });
    expect(svgDeclaredSize("<svg></svg>")).toBeNull();
  });
});

describe("looksLikeLottie", () => {
  it("accepts Lottie JSON and rejects anything else", () => {
    expect(looksLikeLottie(JSON.stringify({ v: "5.7.0", w: 100, h: 100, fr: 30, ip: 0, op: 60, layers: [] }))).toBe(true);
    expect(looksLikeLottie('{"layers": []}')).toBe(false);
    expect(looksLikeLottie("hello")).toBe(false);
    expect(looksLikeLottie("")).toBe(false);
  });
});
