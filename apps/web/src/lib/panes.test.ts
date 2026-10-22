import { describe, expect, it } from "vitest";
import { clampPanes, DEFAULT_PANES, dragPane } from "./panes";

const big = { w: 1600, h: 1000 };

describe("dragPane", () => {
  it("moves each edge with the pointer", () => {
    expect(dragPane("left", 252, 40, 0)).toBe(292);
    expect(dragPane("right", 292, 40, 0)).toBe(252); // dragging the right pane's left edge rightwards narrows it
    expect(dragPane("bottom", 280, 0, -50)).toBe(330); // dragging the timeline's top edge up makes it taller
  });
});

describe("clampPanes", () => {
  it("keeps defaults on a roomy window", () => {
    expect(clampPanes(DEFAULT_PANES, big)).toEqual(DEFAULT_PANES);
  });
  it("enforces minimum and maximum sizes", () => {
    expect(clampPanes({ left: 20, right: 2000, bottom: 10 }, big)).toEqual({ left: 180, right: 560, bottom: 96 });
  });
  it("never lets the canvas get squeezed out", () => {
    const p = clampPanes({ left: 520, right: 560, bottom: 900 }, { w: 1100, h: 700 });
    expect(1100 - p.left - p.right).toBeGreaterThanOrEqual(280);
    expect(700 - 46 - p.bottom).toBeGreaterThanOrEqual(160);
  });
});
