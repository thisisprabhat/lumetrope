import { describe, expect, it } from "vitest";
import { fitRect } from "./fit";

describe("fitRect", () => {
  it("fits content inside a box, centred, with padding", () => {
    expect(fitRect({ w: 400, h: 300 }, { w: 1000, h: 800 }, 50)).toEqual({ scale: 2.25, x: 50, y: 62.5, w: 900, h: 675 });
  });
  it("letterboxes tall content", () => {
    const r = fitRect({ w: 100, h: 400 }, { w: 1000, h: 500 }, 0);
    expect(r.scale).toBe(1.25);
    expect(r.x).toBe(437.5);
    expect(r.y).toBe(0);
  });
  it("never collapses to zero", () => {
    expect(fitRect({ w: 100, h: 100 }, { w: 10, h: 10 }, 50).scale).toBeGreaterThan(0);
  });
});
