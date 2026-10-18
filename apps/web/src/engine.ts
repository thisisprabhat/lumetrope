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

export class ToolError extends Error {}

/** Run a catalogue tool; throws ToolError with the engine's message on failure. */
export function call<T = any>(tool: string, args: object = {}): T {
  const r = JSON.parse(engine().apply(tool, JSON.stringify(args)));
  if (r.error !== undefined) throw new ToolError(String(r.error));
  return r.ok as T;
}

export interface Geometry {
  id: string;
  local: [number, number, number, number];
  bounds: [number, number, number, number];
  corners: [number, number][];
  matrix: [number, number, number, number, number, number];
  anchor: [number, number];
}

export function geometry(id: string, frame: number): Geometry | null {
  return JSON.parse(engine().layer_geometry(id, frame));
}

export function hitTest(x: number, y: number, frame: number): string | null {
  return engine().hit_test(x, y, frame) || null;
}

export function projectValue(): any {
  return JSON.parse(engine().project_json());
}
