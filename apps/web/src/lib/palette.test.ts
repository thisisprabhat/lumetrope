import { describe, expect, it } from "vitest";
import { buildColorMap, cluster, fromOklab, hexToRgb, lightness, normalizeHex, rgbToHex, shuffled, slot, targetOrder, toOklab } from "./palette";

const P = ["#ffffff", "#000000", "#ff0000", "#808080", "#ffff00"];

describe("colour math", () => {
  it("round-trips hex through OKLab", () => {
    for (const h of ["#000000", "#ffffff", "#ff8800", "#123456", "#7f7f7f"]) expect(rgbToHex(fromOklab(toOklab(hexToRgb(h))))).toBe(h);
  });
  it("normalises short and upper-case hex", () => {
    expect(normalizeHex("#F80")).toBe("#ff8800");
  });
  it("orders lightness as people see it", () => {
    expect(lightness("#000000")).toBeLessThan(lightness("#0000ff"));
    expect(lightness("#0000ff")).toBeLessThan(lightness("#ffff00"));
  });
});

describe("slot", () => {
  it("spreads few ranks over many slots, centred", () => {
    expect([0].map((i) => slot(i, 1, 5))).toEqual([2]);
    expect([0, 1].map((i) => slot(i, 2, 5))).toEqual([1, 3]);
    expect([0, 1, 2, 3, 4].map((i) => slot(i, 5, 5))).toEqual([0, 1, 2, 3, 4]);
  });
  it("buckets many ranks into few slots", () => {
    expect([0, 1, 2, 3].map((i) => slot(i, 4, 2))).toEqual([0, 0, 1, 1]);
  });
});

describe("buildColorMap", () => {
  const src = [
    { hex: "#111111", count: 3 },
    { hex: "#eeeeee", count: 1 },
    { hex: "#3366cc", count: 2 },
  ];
  const preset = ["#ffeedd", "#221100", "#aa5533"];

  it("order mode maps darkest to darkest and lightest to lightest", () => {
    const m = buildColorMap(src, preset, { mode: "order" });
    expect(m["#111111"]).toBe("#221100");
    expect(m["#3366cc"]).toBe("#aa5533");
    expect(m["#eeeeee"]).toBe("#ffeedd");
  });

  it("a shuffle permutation reorders targets", () => {
    expect(targetOrder(preset, [2, 1, 0])).toEqual(["#ffeedd", "#aa5533", "#221100"]);
    const m = buildColorMap(src, preset, { mode: "order", order: [2, 1, 0] });
    expect(m["#111111"]).toBe("#ffeedd");
  });

  it("groups mode keeps shades apart inside one group", () => {
    const blues = [
      { hex: "#1a3a8a", count: 1 },
      { hex: "#2a4a9a", count: 1 },
      { hex: "#3a5aaa", count: 1 },
    ];
    const m = buildColorMap(blues, ["#cc3300"], { mode: "groups" });
    const out = blues.map((b) => m[b.hex]);
    expect(new Set(out).size).toBe(3);
    // Lightness order survives the recolour.
    expect(lightness(out[0])).toBeLessThan(lightness(out[1]));
    expect(lightness(out[1])).toBeLessThan(lightness(out[2]));
    // The middle shade sits on the preset colour.
    expect(m["#2a4a9a"]).toBe(normalizeHex(rgbToHex(fromOklab(toOklab(hexToRgb("#cc3300"))))));
  });

  it("groups mode sends a lone colour exactly to its target", () => {
    const m = buildColorMap([{ hex: "#ff0000", count: 1 }], ["#00ff00"], { mode: "groups" });
    expect(m["#ff0000"]).toBe("#00ff00");
  });

  it("keeps neutrals when asked and applies overrides last", () => {
    const m = buildColorMap(src, preset, { mode: "order", keepNeutrals: true, overrides: { "#3366CC": "#ABCDEF" } });
    expect(m["#111111"]).toBeUndefined();
    expect(m["#eeeeee"]).toBeUndefined();
    expect(m["#3366cc"]).toBe("#abcdef");
  });

  it("handles empty inputs", () => {
    expect(buildColorMap([], preset, { mode: "order" })).toEqual({});
    expect(buildColorMap(src, [], { mode: "groups" })).toEqual({});
  });
});

describe("cluster", () => {
  it("separates clearly different colours and numbers groups dark → light", () => {
    const g = cluster(
      [
        { hex: "#fafafa", count: 1 },
        { hex: "#050505", count: 1 },
        { hex: "#f0f0f0", count: 1 },
        { hex: "#0a0a0a", count: 1 },
      ],
      2,
    );
    expect(g).toEqual([1, 0, 1, 0]);
  });
});

describe("shuffled", () => {
  it("returns a different permutation of every index", () => {
    let seed = 1;
    const rand = () => ((seed = (seed * 16807) % 2147483647) / 2147483647);
    const p = shuffled(P.length, null, rand);
    expect([...p].sort()).toEqual([0, 1, 2, 3, 4]);
    expect(p).not.toEqual([0, 1, 2, 3, 4]);
    expect(shuffled(1)).toEqual([0]);
  });
});
