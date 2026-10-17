import { describe, expect, it } from "vitest";
import { advance, clampFrame, formatTime } from "./time";

describe("time", () => {
  it("clamps frames into [ip, op)", () => {
    expect(clampFrame(-5, 0, 60)).toBe(0);
    expect(clampFrame(75, 0, 60)).toBe(59);
    expect(clampFrame(12.7, 0, 60)).toBe(12);
  });

  it("formats seconds and frames", () => {
    expect(formatTime(45, 30)).toBe("1s 15f");
    expect(formatTime(0, 24)).toBe("0s 0f");
  });

  it("advances with looping and stops at the end without loop", () => {
    expect(advance(58, 3, 0, 60, true)).toEqual({ frame: 1, ended: false });
    expect(advance(58, 3, 0, 60, false)).toEqual({ frame: 59, ended: true });
    expect(advance(10, 2.5, 0, 60, true)).toEqual({ frame: 12.5, ended: false });
  });
});
