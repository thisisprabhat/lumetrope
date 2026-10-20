import { useEffect } from "react";
import { commands } from "./commands";
import { useEditor, type Tool } from "./store";

const TOOL_KEYS: Record<string, Tool> = { v: "select", r: "rect", o: "ellipse", p: "pen", t: "text", h: "hand" };

function typing(e: KeyboardEvent): boolean {
  const t = e.target as HTMLElement;
  return t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable;
}

export function useShortcuts(): void {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (typing(e)) return;
      const st = useEditor.getState();
      const mod = e.metaKey || e.ctrlKey;
      const k = e.key.toLowerCase();
      if (mod && k === "z") {
        e.preventDefault();
        return e.shiftKey ? commands.redo() : commands.undo();
      }
      if (mod && k === "y") return (e.preventDefault(), commands.redo());
      if (mod && k === "d") return (e.preventDefault(), commands.duplicate());
      if (mod && k === "c") return (e.preventDefault(), commands.copy());
      if (mod && k === "x") return (e.preventDefault(), commands.cut());
      if (mod && k === "v") return (e.preventDefault(), void commands.paste());
      if (mod && k === "a") return (e.preventDefault(), commands.selectAll());
      if (mod && k === "o") return (e.preventDefault(), void commands.open());
      if (mod && k === "s") return (e.preventDefault(), commands.saveProject());
      if (mod && k === "e") return (e.preventDefault(), commands.openExport());
      if (mod && k === "g") return (e.preventDefault(), commands.precompose());
      if (mod && k === "0") return (e.preventDefault(), commands.fit());
      if (mod && (k === "=" || k === "+")) return (e.preventDefault(), st.zoomStep(1));
      if (mod && k === "-") return (e.preventDefault(), st.zoomStep(-1));
      if (mod) return;
      if (e.key === "Delete" || e.key === "Backspace") return (e.preventDefault(), commands.remove());
      if (e.key === "Escape") return st.select([]);
      if (e.shiftKey && e.code === "Digit1") return commands.fit();
      if (e.shiftKey && e.code === "Digit0") return st.zoomTo(1);
      if (e.shiftKey && e.code === "Digit2") return commands.zoomToSelection();
      if (e.key === ",") return st.setFrame(st.frame - (e.shiftKey ? 10 : 1));
      if (e.key === ".") return st.setFrame(st.frame + (e.shiftKey ? 10 : 1));
      if (e.key === "Home") return st.setFrame(st.info?.ip ?? 0);
      if (e.key === "End") return st.setFrame((st.info?.op ?? 1) - 1);
      const arrows: Record<string, [number, number]> = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
      if (arrows[e.key] && st.selection.length) {
        e.preventDefault();
        const m = e.shiftKey ? 10 : 1;
        return commands.nudge(arrows[e.key][0] * m, arrows[e.key][1] * m);
      }
      if (st.mode === "animate" && TOOL_KEYS[k]) st.setTool(TOOL_KEYS[k]);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);
}
