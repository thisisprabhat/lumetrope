/** Canvas camera: screen = comp × zoom + (x, y), in viewport pixels. */
export interface Camera {
  zoom: number;
  x: number;
  y: number;
}

export const MIN_ZOOM = 0.02;
export const MAX_ZOOM = 64;

export const clampZoom = (z: number) => Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, z));

export function screenToComp(c: Camera, sx: number, sy: number): [number, number] {
  return [(sx - c.x) / c.zoom, (sy - c.y) / c.zoom];
}

/** Multiply zoom by `factor`, keeping the comp point under screen (sx, sy) fixed. */
export function zoomAt(c: Camera, factor: number, sx: number, sy: number): Camera {
  const zoom = clampZoom(c.zoom * factor);
  const k = zoom / c.zoom;
  return { zoom, x: sx - (sx - c.x) * k, y: sy - (sy - c.y) * k };
}

/** Fit the artboard into the viewport with padding, leaving `bottom` px free (player bar). */
export function fitCamera(docW: number, docH: number, viewW: number, viewH: number, pad = 48, bottom = 72): Camera {
  const zoom = clampZoom(Math.min((viewW - pad * 2) / Math.max(1, docW), (viewH - pad * 2 - bottom) / Math.max(1, docH)));
  return { zoom, x: (viewW - docW * zoom) / 2, y: (viewH - bottom - docH * zoom) / 2 };
}

/** Centre the artboard at a given zoom. */
export function centerCamera(docW: number, docH: number, viewW: number, viewH: number, zoom: number, bottom = 72): Camera {
  const z = clampZoom(zoom);
  return { zoom: z, x: (viewW - docW * z) / 2, y: (viewH - bottom - docH * z) / 2 };
}

export type WheelMode = "auto" | "zoom" | "scroll";

export interface WheelLike {
  deltaX: number;
  deltaY: number;
  deltaMode: number;
  ctrlKey: boolean;
  metaKey: boolean;
  wheelDeltaY?: number;
}

/**
 * Decide whether a wheel event pans or zooms.
 * - ctrl/⌘ + wheel (incl. Chrome's trackpad pinch, which sets ctrlKey) zooms;
 * - trackpad two-finger scrolls pan;
 * - a notched mouse wheel zooms in "auto" mode (detected by line-mode deltas or
 *   legacy wheelDelta in 120 steps rather than the trackpad's 3× pixel ratio).
 */
export function classifyWheel(e: WheelLike, mode: WheelMode): "pan" | "zoom" {
  if (e.ctrlKey || e.metaKey) return "zoom";
  if (mode === "zoom") return "zoom";
  if (mode === "scroll") return "pan";
  if (e.deltaMode !== 0) return "zoom";
  if (e.wheelDeltaY !== undefined && e.wheelDeltaY !== 0) {
    const trackpad = Math.abs(e.wheelDeltaY) === Math.abs(e.deltaY) * 3;
    if (!trackpad && e.deltaX === 0 && Math.abs(e.wheelDeltaY) % 120 === 0) return "zoom";
  }
  return "pan";
}

/** Zoom factor for a wheel delta (pinch deltas are small, mouse notches large). */
export function wheelZoomFactor(e: WheelLike): number {
  const dy = e.deltaMode === 1 ? e.deltaY * 33 : e.deltaY;
  const k = e.ctrlKey && Math.abs(dy) < 50 ? 0.01 : 0.0022;
  return Math.exp(-dy * k);
}

export const ZOOM_STEPS = [0.05, 0.1, 0.25, 0.5, 0.75, 1, 1.5, 2, 3, 4, 6, 8, 12, 16, 24, 32, 48, 64];

export function nextStep(z: number, dir: 1 | -1): number {
  if (dir > 0) return ZOOM_STEPS.find((s) => s > z * 1.001) ?? MAX_ZOOM;
  return [...ZOOM_STEPS].reverse().find((s) => s < z / 1.001) ?? MIN_ZOOM;
}
