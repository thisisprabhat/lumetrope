// Pure helpers for GIF / WebP / WebM export.

export type MediaFormat = "gif" | "webp" | "webm";

/** Scene frame numbers to render when exporting [ip, op) at `outFps`. */
export function frameTimes(ip: number, op: number, sceneFps: number, outFps: number): number[] {
  const step = sceneFps / outFps;
  const count = Math.max(1, Math.ceil((op - ip) / step - 1e-9));
  return Array.from({ length: count }, (_, k) => ip + k * step);
}

export function exportSize(w: number, h: number, scale: number): { w: number; h: number } {
  return { w: Math.max(1, Math.round(w * scale)), h: Math.max(1, Math.round(h * scale)) };
}

/** VP8/VP9 with 4:2:0 chroma want even dimensions. */
export function evenSize(s: { w: number; h: number }): { w: number; h: number } {
  return { w: s.w + (s.w % 2), h: s.h + (s.h % 2) };
}

export function hexToRgb(hex: string): [number, number, number] {
  const m = /^#?([0-9a-f]{6})$/i.exec(hex.trim());
  const n = m ? parseInt(m[1], 16) : 0xffffff;
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255];
}
