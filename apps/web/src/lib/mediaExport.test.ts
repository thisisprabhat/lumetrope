import { describe, expect, it } from "vitest";
import { evenSize, exportSize, frameTimes } from "./mediaExport";

describe("frameTimes", () => {
  it("samples the scene at the export frame rate", () => {
    expect(frameTimes(0, 30, 30, 30)).toHaveLength(30);
    expect(frameTimes(0, 30, 30, 15)).toEqual([0, 2, 4, 6, 8, 10, 12, 14, 16, 18, 20, 22, 24, 26, 28]);
    expect(frameTimes(0, 30, 30, 60).slice(0, 4)).toEqual([0, 0.5, 1, 1.5]);
    expect(frameTimes(0, 30, 30, 60)).toHaveLength(60);
  });
  it("covers a segment and never runs past its end", () => {
    const t = frameTimes(20, 40, 30, 24);
    expect(t[0]).toBe(20);
    expect(t).toHaveLength(16);
    expect(Math.max(...t)).toBeLessThan(40);
  });
  it("always yields at least one frame", () => {
    expect(frameTimes(5, 5, 30, 30)).toEqual([5]);
  });
});

describe("exportSize", () => {
  it("scales the scene and keeps at least 1 px", () => {
    expect(exportSize(400, 300, 0.5)).toEqual({ w: 200, h: 150 });
    expect(exportSize(3, 3, 0.1)).toEqual({ w: 1, h: 1 });
  });
  it("rounds video sizes to even numbers", () => {
    expect(evenSize({ w: 201, h: 151 })).toEqual({ w: 202, h: 152 });
  });
});
