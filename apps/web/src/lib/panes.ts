// Editor pane sizes (left sidebar, right inspector, bottom timeline), dragged by their edges.

export interface Panes {
  left: number;
  right: number;
  bottom: number;
}

export const DEFAULT_PANES: Panes = { left: 252, right: 292, bottom: 280 };

const HEADER = 46;
const MIN_STAGE_W = 280;
const MIN_STAGE_H = 160;

/** New size for `kind` after the pointer moved (dx, dy) from where the drag started. */
export function dragPane(kind: keyof Panes, start: number, dx: number, dy: number): number {
  return kind === "left" ? start + dx : kind === "right" ? start - dx : start - dy;
}

/** Keep panes within their limits and always leave the canvas usable room. */
export function clampPanes(p: Panes, view: { w: number; h: number }): Panes {
  const clamp = (v: number, lo: number, hi: number) => Math.round(Math.min(hi, Math.max(lo, v)));
  let left = clamp(p.left, 180, 520);
  let right = clamp(p.right, 220, 560);
  const excess = left + right - (view.w - MIN_STAGE_W);
  if (excess > 0) {
    const r = Math.max(220, right - excess);
    left = Math.max(180, left - (excess - (right - r)));
    right = r;
  }
  const bottom = clamp(p.bottom, 96, Math.max(96, view.h - HEADER - MIN_STAGE_H));
  return { left, right, bottom };
}
