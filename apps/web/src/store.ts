import { create } from "zustand";
import { engine, readInfo, type Info } from "./engine";
import { centerCamera, fitCamera, nextStep, zoomAt, type Camera, type WheelMode } from "./lib/camera";
import { clampFrame } from "./lib/time";

export type Mode = "animate" | "state" | "preview";
export type Tool = "select" | "rect" | "ellipse" | "polygon" | "star" | "pen" | "text" | "hand";
export type LeftTab = "layers" | "assets" | "segments" | "themes" | "colors" | "history";

export interface Toast {
  id: number;
  kind: "info" | "error";
  text: string;
}

interface EditorState {
  info: Info | null;
  revision: number;
  frame: number;
  playing: boolean;
  loop: boolean;
  speed: number;
  mode: Mode;
  tool: Tool;
  /** screen = comp × camera.zoom + (camera.x, camera.y) in viewport pixels */
  camera: Camera;
  viewport: { w: number; h: number };
  wheelMode: WheelMode;
  fileName: string;
  toasts: Toast[];
  selection: string[];
  shape: string | null;
  scene: string;
  leftTab: LeftTab;
  snapping: boolean;
  grid: boolean;
  canUndo: boolean;
  canRedo: boolean;
  fonts: string[];
  renaming: string | null;
  exportOpen: boolean;
  setExportOpen: (b: boolean) => void;
  setRenaming: (id: string | null) => void;
  addFontName: (n: string) => void;
  select: (ids: string[], shape?: string | null) => void;
  setScene: (s: string) => void;
  setLeftTab: (t: LeftTab) => void;
  setSnapping: (b: boolean) => void;
  setGrid: (b: boolean) => void;
  refresh: () => void;
  setFrame: (f: number) => void;
  setPlaying: (p: boolean) => void;
  setLoop: (l: boolean) => void;
  setSpeed: (s: number) => void;
  setMode: (m: Mode) => void;
  setTool: (t: Tool) => void;
  setCamera: (c: Camera) => void;
  setViewport: (w: number, h: number) => void;
  setWheelMode: (m: WheelMode) => void;
  zoomStep: (dir: 1 | -1) => void;
  zoomTo: (z: number) => void;
  fitView: () => void;
  fitBounds: (b: [number, number, number, number]) => void;
  setFileName: (n: string) => void;
  toast: (text: string, kind?: Toast["kind"]) => void;
  dismiss: (id: number) => void;
}

let toastId = 0;

export const useEditor = create<EditorState>((set, get) => ({
  info: null,
  revision: 0,
  frame: 0,
  playing: false,
  loop: true,
  speed: 1,
  mode: "animate",
  tool: "select",
  camera: { zoom: 1, x: 0, y: 0 },
  viewport: { w: 0, h: 0 },
  wheelMode: (localStorageGet("lumetrope.wheelMode") as WheelMode) || "auto",
  fileName: "Untitled",
  toasts: [],
  selection: [],
  shape: null,
  scene: "root",
  leftTab: "layers",
  snapping: true,
  grid: false,
  canUndo: false,
  canRedo: false,
  fonts: [],
  renaming: null,
  exportOpen: false,
  setExportOpen: (exportOpen) => set({ exportOpen }),
  setRenaming: (renaming) => set({ renaming }),
  addFontName: (n) => set({ fonts: Array.from(new Set([...get().fonts, n])) }),
  refresh: () => {
    const info = readInfo();
    const e = engine();
    set({ info, revision: info.revision, frame: clampFrame(get().frame, info.ip, info.op), canUndo: e.can_undo(), canRedo: e.can_redo() });
  },
  select: (selection, shape = null) => set({ selection, shape }),
  setScene: (scene) => {
    engine().set_view_scene(scene);
    set({ scene, selection: [], shape: null });
    get().refresh();
  },
  setLeftTab: (leftTab) => set({ leftTab }),
  setSnapping: (snapping) => set({ snapping }),
  setGrid: (grid) => set({ grid }),
  setFrame: (f) => {
    const info = get().info;
    set({ frame: info ? clampFrame(f, info.ip, info.op) : Math.max(0, Math.floor(f)) });
  },
  setPlaying: (playing) => set({ playing }),
  setLoop: (loop) => set({ loop }),
  setSpeed: (speed) => set({ speed }),
  setMode: (mode) => set({ mode }),
  setTool: (tool) => set({ tool }),
  setCamera: (camera) => set({ camera }),
  setViewport: (w, h) => set({ viewport: { w, h } }),
  setWheelMode: (wheelMode) => {
    localStorageSet("lumetrope.wheelMode", wheelMode);
    set({ wheelMode });
  },
  zoomStep: (dir) => {
    const { camera, viewport } = get();
    set({ camera: zoomAt(camera, nextStep(camera.zoom, dir) / camera.zoom, viewport.w / 2, viewport.h / 2) });
  },
  zoomTo: (z) => {
    const { camera, viewport } = get();
    set({ camera: zoomAt(camera, z / camera.zoom, viewport.w / 2, viewport.h / 2) });
  },
  fitView: () => {
    const { info, viewport } = get();
    if (info && viewport.w > 0) set({ camera: fitCamera(info.w, info.h, viewport.w, viewport.h) });
  },
  fitBounds: (b) => {
    const { viewport } = get();
    const w = Math.max(1, b[2] - b[0]);
    const h = Math.max(1, b[3] - b[1]);
    const c = fitCamera(w, h, viewport.w, viewport.h, 80);
    set({ camera: { zoom: c.zoom, x: c.x - b[0] * c.zoom, y: c.y - b[1] * c.zoom } });
  },
  setFileName: (fileName) => set({ fileName }),
  toast: (text, kind = "info") => {
    const id = ++toastId;
    set({ toasts: [...get().toasts, { id, kind, text }] });
    setTimeout(() => get().dismiss(id), kind === "error" ? 6000 : 3000);
  },
  dismiss: (id) => set({ toasts: get().toasts.filter((t) => t.id !== id) }),
}));

function localStorageGet(k: string): string | null {
  try {
    return localStorage.getItem(k);
  } catch {
    return null;
  }
}

function localStorageSet(k: string, v: string): void {
  try {
    localStorage.setItem(k, v);
  } catch {
    /* storage unavailable */
  }
}

export { centerCamera };
