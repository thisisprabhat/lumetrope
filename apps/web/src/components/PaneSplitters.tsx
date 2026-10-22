// Drag handles on the edges of the left sidebar, right inspector and bottom
// timeline. Double-click resets a pane; arrow keys resize a focused handle.
import { useEffect, useState } from "react";
import { clampPanes, DEFAULT_PANES, dragPane, type Panes } from "../lib/panes";
import { useEditor } from "../store";

type Kind = keyof Panes;

const view = () => ({ w: window.innerWidth, h: window.innerHeight });

export function PaneSplitters() {
  const panes = useEditor((s) => s.panes);
  const setPanes = useEditor((s) => s.setPanes);
  const [drag, setDrag] = useState<Kind | null>(null);

  useEffect(() => {
    const fit = () => {
      const cur = useEditor.getState().panes;
      const next = clampPanes(cur, view());
      if (next.left !== cur.left || next.right !== cur.right || next.bottom !== cur.bottom) setPanes(next);
    };
    fit();
    window.addEventListener("resize", fit);
    return () => window.removeEventListener("resize", fit);
  }, [setPanes]);

  const resize = (kind: Kind, value: number) => setPanes(clampPanes({ ...useEditor.getState().panes, [kind]: value }, view()));

  const start = (kind: Kind) => (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.button !== 0) return;
    e.preventDefault();
    const el = e.currentTarget;
    el.setPointerCapture(e.pointerId);
    const x0 = e.clientX;
    const y0 = e.clientY;
    const from = useEditor.getState().panes[kind];
    setDrag(kind);
    document.body.classList.add(kind === "bottom" ? "resizing-row" : "resizing-col");
    const move = (ev: PointerEvent) => resize(kind, dragPane(kind, from, ev.clientX - x0, ev.clientY - y0));
    const end = () => {
      el.removeEventListener("pointermove", move);
      el.removeEventListener("pointerup", end);
      el.removeEventListener("pointercancel", end);
      document.body.classList.remove("resizing-row", "resizing-col");
      setDrag(null);
    };
    el.addEventListener("pointermove", move);
    el.addEventListener("pointerup", end);
    el.addEventListener("pointercancel", end);
  };

  const keys = (kind: Kind) => (e: React.KeyboardEvent) => {
    const step = e.shiftKey ? 48 : 16;
    const d: Record<string, [number, number]> = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
    if (!d[e.key]) return;
    e.preventDefault();
    resize(kind, dragPane(kind, panes[kind], d[e.key][0], d[e.key][1]));
  };

  const handle = (kind: Kind, label: string, style: React.CSSProperties) => (
    <div
      className={`splitter ${kind === "bottom" ? "row" : "col"} ${drag === kind ? "active" : ""}`}
      style={style}
      role="separator"
      tabIndex={0}
      aria-label={label}
      aria-orientation={kind === "bottom" ? "horizontal" : "vertical"}
      aria-valuenow={panes[kind]}
      onPointerDown={start(kind)}
      onDoubleClick={() => resize(kind, DEFAULT_PANES[kind])}
      onKeyDown={keys(kind)}
      title="Drag to resize · double-click to reset"
    />
  );

  return (
    <>
      {handle("left", "Resize left panel", { left: panes.left - 4, top: "var(--header-h)", bottom: panes.bottom })}
      {handle("right", "Resize right panel", { right: panes.right - 4, top: "var(--header-h)", bottom: panes.bottom })}
      {handle("bottom", "Resize timeline", { left: 0, right: 0, bottom: panes.bottom - 4 })}
    </>
  );
}
