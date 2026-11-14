import { describe, expect, it } from "vitest";
import { iconPreview } from "./preview";

describe("icon previews", () => {
  it("tints only black icons", () => {
    expect(iconPreview('<svg><path stroke="#000000" fill="none"/></svg>').monochrome).toBe(true);
    expect(iconPreview('<svg><path fill="currentColor"/></svg>').monochrome).toBe(true);
  });

  it("preserves colored fills, strokes, gradients and inline styles", () => {
    for (const body of ['<path fill="#0099ff"/>', '<path stroke="blue"/>', '<stop stop-color="#123456"/>', '<path style="fill:blue"/>']) {
      const svg = `<svg>${body}</svg>`;
      const preview = iconPreview(svg);
      expect(preview.monochrome).toBe(false);
      expect(decodeURIComponent(preview.src.split(",")[1])).toBe(svg);
    }
  });
});
