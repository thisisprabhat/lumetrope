import { describe, expect, it } from "vitest";
import { classifyWheel, fitCamera, screenToComp, zoomAt } from "./camera";

describe("camera", () => {
  it("zoomAt keeps the point under the cursor fixed", () => {
    const cam = { zoom: 1, x: 100, y: 50 };
    const before = screenToComp(cam, 300, 200);
    const next = zoomAt(cam, 2.5, 300, 200);
    expect(next.zoom).toBe(2.5);
    const after = screenToComp(next, 300, 200);
    expect(after[0]).toBeCloseTo(before[0]);
    expect(after[1]).toBeCloseTo(before[1]);
  });

  it("clamps zoom", () => {
    expect(zoomAt({ zoom: 60, x: 0, y: 0 }, 10, 0, 0).zoom).toBe(64);
    expect(zoomAt({ zoom: 0.03, x: 0, y: 0 }, 0.1, 0, 0).zoom).toBe(0.02);
  });

  it("fit centres the artboard inside the padded viewport", () => {
    const c = fitCamera(200, 100, 1000, 600, 50, 0);
    expect(c.zoom).toBeCloseTo(4.5);
    expect(c.x + (200 * c.zoom) / 2).toBeCloseTo(500);
    expect(c.y + (100 * c.zoom) / 2).toBeCloseTo(300);
  });

  it("classifies trackpad, pinch and mouse wheel", () => {
    expect(classifyWheel({ deltaX: 0, deltaY: 4, deltaMode: 0, ctrlKey: true, metaKey: false }, "auto")).toBe("zoom");
    expect(classifyWheel({ deltaX: 3, deltaY: 7, deltaMode: 0, ctrlKey: false, metaKey: false, wheelDeltaY: -21 }, "auto")).toBe("pan");
    expect(classifyWheel({ deltaX: 0, deltaY: 100, deltaMode: 0, ctrlKey: false, metaKey: false, wheelDeltaY: -120 }, "auto")).toBe("zoom");
    expect(classifyWheel({ deltaX: 0, deltaY: 3, deltaMode: 1, ctrlKey: false, metaKey: false }, "auto")).toBe("zoom");
    expect(classifyWheel({ deltaX: 0, deltaY: 100, deltaMode: 0, ctrlKey: false, metaKey: false, wheelDeltaY: -120 }, "scroll")).toBe("pan");
    expect(classifyWheel({ deltaX: 0, deltaY: 4, deltaMode: 0, ctrlKey: false, metaKey: false, wheelDeltaY: -12 }, "zoom")).toBe("zoom");
  });
});
