// Memoised document queries keyed by revision so React panels stay cheap.
import { useMemo } from "react";
import { query } from "./actions";
import { useEditor } from "./store";

export interface ShapeNode {
  path: string;
  type: string;
  name: string | null;
  hidden: boolean;
}

export interface LayerNode {
  id: string;
  name: string;
  type: "precomp" | "solid" | "image" | "null" | "shape" | "text" | "unknown";
  parent: string | null;
  in: number;
  out: number;
  hidden: boolean;
  locked: boolean;
  is_matte?: boolean;
  matte_mode?: string;
  scene?: string;
  shapes?: ShapeNode[];
  text?: string;
  effects?: number;
  masks?: number;
}

export interface SceneInfo {
  name: string;
  width: number;
  height: number;
  fps: number;
  ip: number;
  op: number;
  scene: string;
  segments: { name: string; start: number; end: number }[];
  layers: LayerNode[];
  scenes: { id: string; name: string; layers: number }[];
}

const EMPTY: SceneInfo = { name: "", width: 0, height: 0, fps: 30, ip: 0, op: 0, scene: "root", segments: [], layers: [], scenes: [] };

export function useSceneInfo(): SceneInfo {
  const revision = useEditor((s) => s.revision);
  const scene = useEditor((s) => s.scene);
  return useMemo(() => query<SceneInfo>("read_scene", { scene }, EMPTY), [revision, scene]);
}

export interface PropState {
  exists: boolean;
  value: any;
  animated: boolean;
  keyed: boolean;
  keyframes: number[];
  slot: string | null;
}

const NO_PROP: PropState = { exists: false, value: null, animated: false, keyed: false, keyframes: [], slot: null };

export function useProp(layer: string | undefined, property: string, shape?: string | null): PropState {
  const revision = useEditor((s) => s.revision);
  const frame = useEditor((s) => Math.floor(s.frame));
  return useMemo(
    () => (layer ? query<PropState>("get_property", { layer, property, frame, ...(shape ? { shape } : {}) }, NO_PROP) : NO_PROP),
    [layer, property, shape, revision, frame],
  );
}

/** Raw JSON of one layer (re-read per revision). */
export function useLayerJson(layer: string | undefined): any {
  const revision = useEditor((s) => s.revision);
  return useMemo(() => (layer ? query<any>("inspect_layer", { layer }, null)?.json ?? null : null), [layer, revision]);
}
