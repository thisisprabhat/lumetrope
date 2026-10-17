import { create } from "zustand";
import { readInfo, type Info } from "./engine";
import { clampFrame } from "./lib/time";

export type Mode = "animate" | "state" | "preview";
export type Tool = "select" | "rect" | "ellipse" | "polygon" | "star" | "pen" | "text" | "hand";

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
  zoom: number;
  fileName: string;
  toasts: Toast[];
  refresh: () => void;
  setFrame: (f: number) => void;
  setPlaying: (p: boolean) => void;
  setLoop: (l: boolean) => void;
  setSpeed: (s: number) => void;
  setMode: (m: Mode) => void;
  setTool: (t: Tool) => void;
  setZoom: (z: number) => void;
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
  zoom: 1,
  fileName: "Untitled",
  toasts: [],
  refresh: () => {
    const info = readInfo();
    set({ info, revision: info.revision, frame: clampFrame(get().frame, info.ip, info.op) });
  },
  setFrame: (f) => {
    const info = get().info;
    set({ frame: info ? clampFrame(f, info.ip, info.op) : Math.max(0, Math.floor(f)) });
  },
  setPlaying: (playing) => set({ playing }),
  setLoop: (loop) => set({ loop }),
  setSpeed: (speed) => set({ speed }),
  setMode: (mode) => set({ mode }),
  setTool: (tool) => set({ tool }),
  setZoom: (zoom) => set({ zoom: Math.min(16, Math.max(0.05, zoom)) }),
  setFileName: (fileName) => set({ fileName }),
  toast: (text, kind = "info") => {
    const id = ++toastId;
    set({ toasts: [...get().toasts, { id, kind, text }] });
    setTimeout(() => get().dismiss(id), kind === "error" ? 6000 : 3000);
  },
  dismiss: (id) => set({ toasts: get().toasts.filter((t) => t.id !== id) }),
}));
