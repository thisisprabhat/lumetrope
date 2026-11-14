import { describe, expect, it } from "vitest";
import { decideDrop, decodeIconDrag, encodeIconDrag, scenePoint } from "./drop";
import { svgImportArgs } from "./place";

const svg = '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"></svg>';
const camera = { x: 10, y: 5, zoom: 2 };
const rect = { left: 50, top: 20 };

describe("icon drop", () => {
  it("places a dragged icon at the scene point under the cursor", () => {
    const decision = decideDrop({
      file: null,
      iconRaw: encodeIconDrag(svg, "lucide:house"),
      clientX: 150,
      clientY: 80,
      rect,
      camera,
    });
    expect(scenePoint(150, 80, rect, camera)).toEqual([45, 27.5]);
    expect(decision).toEqual({ kind: "icon", svg, name: "lucide:house", position: [45, 27.5] });
    if (decision.kind !== "icon") return;
    expect(svgImportArgs({ svg: decision.svg, name: decision.name, scene: "root", position: decision.position })).toEqual({
      svg,
      name: "lucide:house",
      scene: "root",
      position: [45, 27.5],
    });
  });

  it("keeps a dropped file on the file path", () => {
    const file = { name: "icon.svg" } as File;
    const decision = decideDrop({
      file,
      iconRaw: encodeIconDrag(svg, "lucide:house"),
      clientX: 0,
      clientY: 0,
      rect: { left: 0, top: 0 },
      camera: { x: 0, y: 0, zoom: 1 },
    });
    expect(decision).toEqual({ kind: "file", file });
  });

  it("ignores a payload that is not an svg", () => {
    expect(decodeIconDrag("nope")).toBeNull();
    expect(decodeIconDrag(JSON.stringify({ svg: "hello", name: "x" }))).toBeNull();
    expect(
      decideDrop({ file: null, iconRaw: "", clientX: 0, clientY: 0, rect: { left: 0, top: 0 }, camera: { x: 0, y: 0, zoom: 1 } }).kind,
    ).toBe("ignore");
  });
});
