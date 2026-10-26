// Colour-scheme mapping: decide which new colour each colour of an animation
// becomes when a preset is applied. Works in OKLab so "darker / lighter" and
// "similar" match what people see.

export type Rgb = [number, number, number];
type Lab = [number, number, number];

export interface SourceColor {
  hex: string;
  count: number;
}

/** order: darkest → lightest, rank for rank. groups: cluster similar colours, keep each shade's offset from its group. */
export type MapMode = "order" | "groups";

export interface MapOptions {
  mode: MapMode;
  /** Permutation of the preset's colours (sorted dark → light) for shuffling; identity when omitted. */
  order?: number[] | null;
  /** Hand-picked targets by source hex; win over the computed ones. */
  overrides?: Record<string, string>;
  /** Leave black, white and greys alone. */
  keepNeutrals?: boolean;
}

export function hexToRgb(hex: string): Rgb {
  let h = hex.trim().replace(/^#/, "");
  if (h.length === 3) h = [...h].map((c) => c + c).join("");
  const n = parseInt(h.slice(0, 6), 16) || 0;
  return [((n >> 16) & 255) / 255, ((n >> 8) & 255) / 255, (n & 255) / 255];
}

export function rgbToHex(c: Rgb): string {
  return "#" + c.map((x) => Math.round(Math.min(1, Math.max(0, x)) * 255).toString(16).padStart(2, "0")).join("");
}

export function normalizeHex(hex: string): string {
  return rgbToHex(hexToRgb(hex));
}

const lin = (x: number) => (x <= 0.04045 ? x / 12.92 : ((x + 0.055) / 1.055) ** 2.4);
const gam = (x: number) => (x <= 0.0031308 ? 12.92 * x : 1.055 * x ** (1 / 2.4) - 0.055);

export function toOklab([r0, g0, b0]: Rgb): Lab {
  const [r, g, b] = [lin(r0), lin(g0), lin(b0)];
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  return [0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s, 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s];
}

export function fromOklab([L, A, B]: Lab): Rgb {
  const l = (L + 0.3963377774 * A + 0.2158037573 * B) ** 3;
  const m = (L - 0.1055613458 * A - 0.0638541728 * B) ** 3;
  const s = (L - 0.0894841775 * A - 1.291485548 * B) ** 3;
  const c = (x: number) => Math.min(1, Math.max(0, gam(Math.min(1, Math.max(0, x)))));
  return [c(4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s), c(-1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s), c(-0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s)];
}

export const lightness = (hex: string) => toOklab(hexToRgb(hex))[0];

export function isNeutral(hex: string): boolean {
  const [, a, b] = toOklab(hexToRgb(hex));
  return Math.hypot(a, b) < 0.02;
}

const dist2 = (p: Lab, q: Lab) => (p[0] - q[0]) ** 2 + (p[1] - q[1]) ** 2 + (p[2] - q[2]) ** 2;

/** Preset colours dark → light, then reordered by a shuffle permutation. */
export function targetOrder(preset: string[], order?: number[] | null): string[] {
  const sorted = preset.map(normalizeHex).sort((a, b) => lightness(a) - lightness(b) || a.localeCompare(b));
  if (!order || order.length !== sorted.length) return sorted;
  return order.map((i) => sorted[i]);
}

/** Spread `n` ranks evenly over `k` slots (centred when there are fewer ranks than slots). */
export const slot = (i: number, n: number, k: number) => Math.min(k - 1, Math.floor(((i + 0.5) * k) / n));

/** Weighted k-means in OKLab, seeded deterministically from lightness quantiles. */
export function cluster(colors: SourceColor[], k: number): number[] {
  const pts = colors.map((c) => toOklab(hexToRgb(c.hex)));
  const w = colors.map((c) => Math.sqrt(Math.max(1, c.count)));
  const byL = pts.map((_, i) => i).sort((a, b) => pts[a][0] - pts[b][0]);
  const assign = new Array<number>(pts.length).fill(0);
  byL.forEach((p, rank) => (assign[p] = slot(rank, pts.length, k)));
  let cents: Lab[] = [];
  for (let iter = 0; iter < 12; iter++) {
    const sums = Array.from({ length: k }, () => [0, 0, 0, 0]);
    pts.forEach((p, i) => {
      const s = sums[assign[i]];
      s[0] += p[0] * w[i];
      s[1] += p[1] * w[i];
      s[2] += p[2] * w[i];
      s[3] += w[i];
    });
    cents = sums.map((s, j) => (s[3] > 0 ? [s[0] / s[3], s[1] / s[3], s[2] / s[3]] : cents[j] ?? [0.5, 0, 0]));
    let moved = false;
    pts.forEach((p, i) => {
      let best = assign[i];
      for (let j = 0; j < k; j++) if (dist2(p, cents[j]) < dist2(p, cents[best]) - 1e-12) best = j;
      if (best !== assign[i]) {
        assign[i] = best;
        moved = true;
      }
    });
    if (!moved) break;
  }
  // Renumber groups dark → light so group j lines up with the j-th darkest target.
  const used = [...new Set(assign)].sort((a, b) => cents[a][0] - cents[b][0]);
  return assign.map((g) => used.indexOf(g));
}

/** Source hex → new hex for applying `preset` to an animation's colours. */
export function buildColorMap(source: SourceColor[], preset: string[], opts: MapOptions): Record<string, string> {
  const map: Record<string, string> = {};
  const targets = targetOrder(preset, opts.order);
  const srcs = source.map((s) => ({ ...s, hex: normalizeHex(s.hex) })).filter((s) => !(opts.keepNeutrals && isNeutral(s.hex)));
  if (targets.length && srcs.length) {
    const k = targets.length;
    if (opts.mode === "order") {
      const sorted = [...srcs].sort((a, b) => lightness(a.hex) - lightness(b.hex) || a.hex.localeCompare(b.hex));
      sorted.forEach((s, i) => (map[s.hex] = targets[slot(i, sorted.length, k)]));
    } else {
      const groups = Math.min(k, srcs.length);
      const assign = cluster(srcs, groups);
      const n = Math.max(...assign) + 1;
      const labs = srcs.map((s) => toOklab(hexToRgb(s.hex)));
      for (let g = 0; g < n; g++) {
        const members = srcs.map((_, i) => i).filter((i) => assign[i] === g);
        const wsum = members.reduce((t, i) => t + Math.sqrt(Math.max(1, srcs[i].count)), 0);
        const cent = [0, 1, 2].map((d) => members.reduce((t, i) => t + labs[i][d] * Math.sqrt(Math.max(1, srcs[i].count)), 0) / wsum) as Lab;
        const target = toOklab(hexToRgb(targets[slot(g, n, k)]));
        for (const i of members) {
          const shade: Lab = [target[0] + labs[i][0] - cent[0], target[1] + labs[i][1] - cent[1], target[2] + labs[i][2] - cent[2]];
          map[srcs[i].hex] = rgbToHex(fromOklab(shade));
        }
      }
    }
  }
  for (const [from, to] of Object.entries(opts.overrides ?? {})) map[normalizeHex(from)] = normalizeHex(to);
  return map;
}

/** A random permutation of 0..n-1 that differs from `prev` when possible. */
export function shuffled(n: number, prev?: number[] | null, rand: () => number = Math.random): number[] {
  const base = prev && prev.length === n ? prev : [...Array(n).keys()];
  for (let attempt = 0; attempt < 8; attempt++) {
    const p = [...Array(n).keys()];
    for (let i = n - 1; i > 0; i--) {
      const j = Math.floor(rand() * (i + 1));
      [p[i], p[j]] = [p[j], p[i]];
    }
    if (n < 2 || p.some((x, i) => x !== base[i])) return p;
  }
  return [...base.slice(1), base[0]];
}

/** Up to `max` colours of an animation, most used first, for seeding a custom preset. */
export function dominant(source: SourceColor[], max = 6): string[] {
  return [...source].sort((a, b) => b.count - a.count).slice(0, max).map((s) => normalizeHex(s.hex));
}
