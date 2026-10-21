import { create } from "zustand";
import { engine, readInfo, type Info } from "./engine";
import { centerCamera, fitCamera, nextStep, zoomAt, type Camera, type WheelMode } from "./lib/camera";
import { clampFrame } from "./lib/time";

export type Mode = "animate" | "state" | "preview";
export type Tool = "select" | "rect" | "ellipse" | "polygon" | "star" | "pen" | "text" | "hand";
export interface SmSnapshot {
  state: string;
  frame: number;
  playing: boolean;
  tweening: boolean;
  stopped: boolean;
  inputs: Record<string, boolean | number | string | null>;
  theme: string | null;
  events: { type: string; [k: string]: unknown }[];
}

export type SmSelection = { kind: "state"; name: string } | { kind: "transition"; from: string; index: number } | null;

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
  renderer: "gpu" | "cpu";
  setRenderer: (r: "gpu" | "cpu") => void;
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
  machine: string | null;
  setMachine: (id: string | null) => void;
  smSnap: SmSnapshot | null;
  setSmSnap: (s: SmSnapshot | null) => void;
  smLog: { t: number; text: string; kind: string }[];
  logSm: (events: { type: string; [k: string]: unknown }[]) => void;
  clearSmLog: () => void;
  smSel: SmSelection;
  setSmSel: (s: SmSelection) => void;
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
  renderer: localStorageGet("lumetrope.renderer") === "cpu" ? "cpu" : "gpu",
  setRenderer: (renderer) => {
    localStorageSet("lumetrope.renderer", renderer);
    set({ renderer });
  },
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
  machine: null,
  setMachine: (machine) => set({ machine, smSel: null }),
  smSnap: null,
  setSmSnap: (smSnap) => set({ smSnap }),
  smLog: [],
  logSm: (events) => {
    if (!events.length) return;
    const t = performance.now();
    const add = events
      .map((e) => ({
        t,
        kind: e.type,
        text:
          e.type === "Transition" ? `${e.from} → ${e.to}` : e.type === "StateEntered" ? `entered ${e.state}` : e.type === "StateExited" ? `left ${e.state}` : e.type === "InputChanged" ? `${e.name} = ${JSON.stringify(e.value)}` : e.type === "OpenUrl" ? `open ${e.url}` : e.type === "CustomEvent" ? `event "${e.value}"` : e.type === "SetTheme" ? `theme ${e.theme}` : String(e.type),
      }));
    const log = [...get().smLog];
    for (const a of add) {
      const last = log[log.length - 1];
      if (last && last.kind === a.kind && last.text.replace(/ ×\d+$/, "") === a.text) {
        const n = Number(/ ×(\d+)$/.exec(last.text)?.[1] ?? 1) + 1;
        log[log.length - 1] = { ...last, t: a.t, text: `${a.text} ×${n}` };
      } else log.push(a);
    }
    set({ smLog: log.slice(-200) });
  },
  clearSmLog: () => set({ smLog: [] }),
  smSel: null,
  setSmSel: (smSel) => set({ smSel }),
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
