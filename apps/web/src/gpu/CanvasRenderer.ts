// GPU-accelerated renderer: executes the engine's display list on Canvas2D,
// which Chrome/Edge (Skia) and Safari/WKWebView (Core Animation/Metal) run on
// the GPU. Groups (masks, mattes, blend modes, effects, precomp clips) render
// into pooled offscreen layers that are composited back with GPU blend ops.

type Mat = [number, number, number, number, number, number];
type Stop = [number, number, number, number, number];
type Paint = { c: [number, number, number, number] } | { lin: [number, number, number, number]; stops: Stop[] } | { rad: [number, number, number, number, number]; stops: Stop[] };

interface FillCmd {
  op: "fill";
  d: string;
  t: Mat;
  rule: CanvasFillRule;
  paint: Paint;
}
interface StrokeCmd {
  op: "stroke";
  d: string;
  t: Mat;
  w: number;
  cap: CanvasLineCap;
  join: CanvasLineJoin;
  ml: number;
  dash: number[];
  doff: number;
  paint: Paint;
}
interface ImageCmd {
  op: "image";
  id: string;
  t: Mat;
  w: number;
  h: number;
  a: number;
}
interface MaskSpec {
  d: string;
  t: Mat;
  mode: string;
  inv: boolean;
  o: number;
}
type Fx =
  | { type: "blur"; sigma: number; dims: number }
  | { type: "shadow"; color: number[]; opacity: number; dx: number; dy: number; blur: number; only: boolean }
  | { type: "fill"; color: number[]; opacity: number }
  | { type: "tint"; black: number[]; white: number[]; amount: number }
  | { type: "tritone"; hi: number[]; mid: number[]; lo: number[]; amount: number };
interface GroupCmd {
  op: "group";
  cmds: Cmd[];
  alpha: number;
  blend: GlobalCompositeOperation;
  clip?: { d: string; t: Mat };
  masks?: MaskSpec[];
  matte?: { mode: "alpha" | "alpha_inv" | "luma" | "luma_inv"; cmds: Cmd[] };
  fx?: Fx[];
}
export type Cmd = FillCmd | StrokeCmd | ImageCmd | GroupCmd;

const LUMA_FILTER_ID = "lumetrope-luma-to-alpha";

function ensureLumaFilter(): void {
  if (typeof document === "undefined" || document.getElementById(LUMA_FILTER_ID)) return;
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("width", "0");
  svg.setAttribute("height", "0");
  svg.style.position = "absolute";
  const f = document.createElementNS(ns, "filter");
  f.id = LUMA_FILTER_ID;
  f.setAttribute("color-interpolation-filters", "sRGB");
  const m = document.createElementNS(ns, "feColorMatrix");
  m.setAttribute("type", "luminanceToAlpha");
  f.appendChild(m);
  svg.appendChild(f);
  document.body.appendChild(svg);
}

const css = (c: number[], a = 1) => `rgba(${Math.round(c[0] * 255)},${Math.round(c[1] * 255)},${Math.round(c[2] * 255)},${(c[3] ?? 1) * a})`;

export class CanvasRenderer {
  private paths = new Map<string, Path2D>();
  private pool: HTMLCanvasElement[] = [];
  private poolUsed = 0;
  private images = new Map<string, { src: string; img: HTMLImageElement }>();
  private w = 0;
  private h = 0;
  onImageLoad?: () => void;

  constructor() {
    ensureLumaFilter();
  }

  /** Sync image assets (id → data URI); triggers onImageLoad once decoded. */
  setImages(map: Record<string, string>): void {
    for (const [id, src] of Object.entries(map)) {
      const cur = this.images.get(id);
      if (cur?.src === src) continue;
      const img = new Image();
      img.onload = () => this.onImageLoad?.();
      img.src = src;
      this.images.set(id, { src, img });
    }
  }

  render(ctx: CanvasRenderingContext2D, cmds: Cmd[], w: number, h: number, clip?: [number, number, number, number]): void {
    this.w = w;
    this.h = h;
    this.poolUsed = 0;
    ctx.save();
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.globalCompositeOperation = "source-over";
    ctx.globalAlpha = 1;
    ctx.clearRect(0, 0, w, h);
    if (clip) {
      ctx.beginPath();
      ctx.rect(clip[0], clip[1], clip[2], clip[3]);
      ctx.clip();
    }
    this.exec(ctx, cmds);
    ctx.restore();
    if (this.paths.size > 4000) this.paths.clear();
  }

  private path(d: string): Path2D {
    let p = this.paths.get(d);
    if (!p) {
      p = new Path2D(d);
      this.paths.set(d, p);
    }
    return p;
  }

  private style(ctx: CanvasRenderingContext2D, p: Paint): string | CanvasGradient {
    if ("c" in p) return css(p.c);
    const g = "lin" in p ? ctx.createLinearGradient(p.lin[0], p.lin[1], p.lin[2], p.lin[3]) : ctx.createRadialGradient(p.rad[0], p.rad[1], 0, p.rad[2], p.rad[3], p.rad[4]);
    for (const s of p.stops) g.addColorStop(Math.min(1, Math.max(0, s[0])), css([s[1], s[2], s[3], s[4]]));
    return g;
  }

  private layerCanvas(): CanvasRenderingContext2D {
    let c = this.pool[this.poolUsed];
    if (!c) {
      c = document.createElement("canvas");
      this.pool.push(c);
    }
    this.poolUsed++;
    if (c.width !== this.w || c.height !== this.h) {
      c.width = this.w;
      c.height = this.h;
    }
    const ctx = c.getContext("2d")!;
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.globalCompositeOperation = "source-over";
    ctx.globalAlpha = 1;
    ctx.filter = "none";
    ctx.clearRect(0, 0, this.w, this.h);
    return ctx;
  }

  private release(n: number): void {
    this.poolUsed -= n;
  }

  private exec(ctx: CanvasRenderingContext2D, cmds: Cmd[]): void {
    for (const c of cmds) {
      switch (c.op) {
        case "fill":
          ctx.setTransform(...c.t);
          ctx.fillStyle = this.style(ctx, c.paint);
          ctx.fill(this.path(c.d), c.rule);
          break;
        case "stroke":
          ctx.setTransform(...c.t);
          ctx.strokeStyle = this.style(ctx, c.paint);
          ctx.lineWidth = c.w;
          ctx.lineCap = c.cap;
          ctx.lineJoin = c.join;
          ctx.miterLimit = c.ml;
          ctx.setLineDash(c.dash.length ? c.dash : []);
          ctx.lineDashOffset = c.doff;
          ctx.stroke(this.path(c.d));
          ctx.setLineDash([]);
          break;
        case "image": {
          const im = this.images.get(c.id)?.img;
          if (!im || !im.complete || !im.naturalWidth) break;
          ctx.setTransform(...c.t);
          ctx.globalAlpha = c.a;
          ctx.drawImage(im, 0, 0, c.w || im.naturalWidth, c.h || im.naturalHeight);
          ctx.globalAlpha = 1;
          break;
        }
        case "group":
          this.group(ctx, c);
          break;
      }
    }
  }

  private group(ctx: CanvasRenderingContext2D, g: GroupCmd): void {
    const off = this.layerCanvas();
    let used = 1;
    if (g.clip) {
      off.save();
      off.setTransform(...g.clip.t);
      off.clip(this.path(g.clip.d));
    }
    this.exec(off, g.cmds);
    if (g.clip) off.restore();
    off.setTransform(1, 0, 0, 1, 0, 0);

    if (g.masks?.length) {
      const mask = this.layerCanvas();
      const tmp = this.layerCanvas();
      used += 2;
      const first = g.masks[0].mode;
      if (!["a", "l", "f"].includes(first)) mask.fillRect(0, 0, this.w, this.h);
      for (const m of g.masks) {
        tmp.setTransform(1, 0, 0, 1, 0, 0);
        tmp.globalCompositeOperation = "source-over";
        tmp.clearRect(0, 0, this.w, this.h);
        tmp.fillStyle = `rgba(0,0,0,${m.o})`;
        if (m.inv) {
          tmp.fillRect(0, 0, this.w, this.h);
          tmp.globalCompositeOperation = "destination-out";
          tmp.fillStyle = "#000";
        }
        tmp.setTransform(...m.t);
        tmp.fill(this.path(m.d));
        mask.globalCompositeOperation =
          m.mode === "s" ? "destination-out" : m.mode === "i" ? "destination-in" : m.mode === "d" ? "destination-in" : m.mode === "f" ? "xor" : "source-over";
        mask.drawImage(tmp.canvas, 0, 0);
      }
      off.globalCompositeOperation = "destination-in";
      off.drawImage(mask.canvas, 0, 0);
      off.globalCompositeOperation = "source-over";
    }

    if (g.matte) {
      let m = this.layerCanvas();
      used += 1;
      this.exec(m, g.matte.cmds);
      m.setTransform(1, 0, 0, 1, 0, 0);
      if (g.matte.mode.startsWith("luma")) {
        const l = this.layerCanvas();
        used += 1;
        if ("filter" in l) {
          l.filter = `url(#${LUMA_FILTER_ID})`;
          l.drawImage(m.canvas, 0, 0);
          l.filter = "none";
        } else {
          lumaToAlphaCpu(m, l, this.w, this.h);
        }
        m = l;
      }
      off.globalCompositeOperation = g.matte.mode.endsWith("_inv") ? "destination-out" : "destination-in";
      off.drawImage(m.canvas, 0, 0);
      off.globalCompositeOperation = "source-over";
    }

    let filter = "none";
    let shadow: Extract<Fx, { type: "shadow" }> | null = null;
    for (const fx of g.fx ?? []) {
      if (fx.type === "blur" && fx.sigma > 0) filter = `blur(${fx.sigma}px)`;
      else if (fx.type === "shadow") shadow = fx;
      else if (fx.type === "fill") {
        off.globalCompositeOperation = "source-atop";
        off.fillStyle = css(fx.color, fx.opacity);
        off.fillRect(0, 0, this.w, this.h);
        off.globalCompositeOperation = "source-over";
      } else if (fx.type === "tint" || fx.type === "tritone") {
        recolorCpu(off, this.w, this.h, fx);
      }
    }

    ctx.save();
    ctx.setTransform(1, 0, 0, 1, 0, 0);
    ctx.globalAlpha = g.alpha;
    ctx.globalCompositeOperation = g.blend;
    if (shadow) {
      ctx.save();
      ctx.shadowColor = css(shadow.color, shadow.opacity);
      ctx.shadowBlur = shadow.blur;
      // Draw far off-screen and offset the shadow back, so only the shadow lands.
      ctx.shadowOffsetX = shadow.dx + 100000;
      ctx.shadowOffsetY = shadow.dy;
      ctx.drawImage(off.canvas, -100000, 0);
      ctx.restore();
      if (shadow.only) {
        ctx.restore();
        this.release(used);
        return;
      }
    }
    ctx.filter = filter;
    ctx.drawImage(off.canvas, 0, 0);
    ctx.restore();
    this.release(used);
  }
}

function lumaToAlphaCpu(src: CanvasRenderingContext2D, dst: CanvasRenderingContext2D, w: number, h: number): void {
  const img = src.getImageData(0, 0, w, h);
  const d = img.data;
  for (let i = 0; i < d.length; i += 4) {
    const a = d[i + 3] / 255;
    d[i + 3] = Math.round((0.2126 * d[i] + 0.7152 * d[i + 1] + 0.0722 * d[i + 2]) * a);
  }
  dst.putImageData(img, 0, 0);
}

function recolorCpu(ctx: CanvasRenderingContext2D, w: number, h: number, fx: Extract<Fx, { type: "tint" | "tritone" }>): void {
  const img = ctx.getImageData(0, 0, w, h);
  const d = img.data;
  const mix = (a: number[], b: number[], t: number) => [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
  for (let i = 0; i < d.length; i += 4) {
    if (d[i + 3] === 0) continue;
    const r = d[i] / 255;
    const g = d[i + 1] / 255;
    const b = d[i + 2] / 255;
    const l = 0.2126 * r + 0.7152 * g + 0.0722 * b;
    const c = fx.type === "tint" ? mix(fx.black, fx.white, l) : l < 0.5 ? mix(fx.lo, fx.mid, l * 2) : mix(fx.mid, fx.hi, (l - 0.5) * 2);
    const out = mix([r, g, b], c, fx.amount);
    d[i] = out[0] * 255;
    d[i + 1] = out[1] * 255;
    d[i + 2] = out[2] * 255;
  }
  ctx.putImageData(img, 0, 0);
}
