import init, { Engine } from "./wasm/lumetrope_wasm.js";

export interface Marker {
  name: string;
  time: number;
  duration: number;
}

export interface Info {
  w: number;
  h: number;
  fr: number;
  ip: number;
  op: number;
  name: string;
  layers: number;
  animations: string[];
  active: string;
  markers: Marker[];
  revision: number;
}

let instance: Engine | null = null;

export async function initEngine(): Promise<Engine> {
  if (!instance) {
    await init();
    instance = new Engine();
  }
  return instance;
}

export function engine(): Engine {
  if (!instance) throw new Error("engine not initialised");
  return instance;
}

export function readInfo(): Info {
  return JSON.parse(engine().info()) as Info;
}

/** Render the active animation into a canvas 2D context at the given pixel size. */
export function renderTo(ctx: CanvasRenderingContext2D, frame: number, w: number, h: number): void {
  if (w <= 0 || h <= 0) return;
  const px = engine().render(frame, w, h);
  ctx.putImageData(new ImageData(new Uint8ClampedArray(px.buffer, px.byteOffset, px.byteLength), w, h), 0, 0);
}
