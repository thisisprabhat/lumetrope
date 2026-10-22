// Pure helpers for the dashboard's "start" actions.

const clampSide = (n: number) => Math.min(4096, Math.max(16, Math.round(n)));

/** The SVG's own size: width/height in px, else its viewBox; null when it declares neither. */
export function svgDeclaredSize(svg: string): { w: number; h: number } | null {
  const tag = /<svg\b[^>]*>/i.exec(svg)?.[0] ?? "";
  const attr = (name: string) => new RegExp(`(?:^|\\s)${name}\\s*=\\s*["']([^"']*)["']`, "i").exec(tag)?.[1];
  const px = (v: string | undefined) => {
    const m = v && /^\s*([\d.]+)\s*(px)?\s*$/i.exec(v);
    return m ? parseFloat(m[1]) : NaN;
  };
  let w = px(attr("width"));
  let h = px(attr("height"));
  if (!(w > 0 && h > 0)) {
    const vb = (attr("viewBox") ?? "").split(/[\s,]+/).map(Number);
    if (vb.length === 4 && vb[2] > 0 && vb[3] > 0) [w, h] = [vb[2], vb[3]];
  }
  return w > 0 && h > 0 ? { w, h } : null;
}

/** Canvas size for a new project made from an SVG. */
export function svgCanvasSize(svg: string): { w: number; h: number } {
  const d = svgDeclaredSize(svg);
  if (!d) return { w: 512, h: 512 };
  const { w, h } = d;
  // Icon-sized art is scaled up (longer side 512), huge art down (longer side 4096).
  const long = Math.max(w, h);
  const s = long < 256 ? 512 / long : long > 4096 ? 4096 / long : 1;
  return { w: clampSide(w * s), h: clampSide(h * s) };
}

/** Is this text a Lottie animation (as pasted from the clipboard)? */
export function looksLikeLottie(text: string): boolean {
  try {
    const v = JSON.parse(text);
    return !!v && typeof v === "object" && Array.isArray(v.layers) && typeof v.w === "number" && typeof v.h === "number";
  } catch {
    return false;
  }
}
