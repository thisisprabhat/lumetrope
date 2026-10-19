import { useEffect, useMemo, useRef, useState } from "react";
import { act, beginGesture, endGesture, query } from "../actions";
import { geometry, hitTest, type Geometry } from "../engine";
import { useSceneInfo } from "../doc";
import { useEditor } from "../store";

type Pt = [number, number];
type Drag =
  | { kind: "move"; start: Pt; applied: Pt; base: [number, number, number, number] }
  | { kind: "scale"; id: string; anchor: Pt; start: Pt; scale0: number[]; axis: "both" | "x" | "y"; ux: Pt; uy: Pt }
  | { kind: "rotate"; id: string; anchor: Pt; start: Pt; rot0: number }
  | { kind: "draw"; start: Pt; cur: Pt }
  | { kind: "marquee"; start: Pt; cur: Pt };

interface PenPoint {
  v: Pt;
  o: Pt;
}

const SNAP_PX = 6;
const dist = (a: Pt, b: Pt) => Math.hypot(a[0] - b[0], a[1] - b[1]);

export function CanvasOverlay({ zoom }: { zoom: number }) {
  const { info, frame, selection, select, tool, setTool, revision, snapping, grid, scene: sceneId, shape: selShape } = useEditor();
  const scene = useSceneInfo();
  const ref = useRef<SVGSVGElement>(null);
  const [drag, setDragState] = useState<Drag | null>(null);
  // Pointer events can outrun React re-renders; handlers read the ref, rendering reads state.
  const dragRef = useRef<Drag | null>(null);
  const setDrag = (d: Drag | null) => {
    dragRef.current = d;
    setDragState(d);
  };
  const [hover, setHover] = useState<string | null>(null);
  const [pen, setPen] = useState<PenPoint[] | null>(null);
  const [penDrag, setPenDrag] = useState(false);
  const [guides, setGuides] = useState<{ x?: number; y?: number }>({});
  const W = info?.w ?? 0;
  const H = info?.h ?? 0;
  const f = Math.floor(frame);
  const locked = useMemo(() => new Set(scene.layers.filter((l) => l.locked).map((l) => l.id)), [scene]);
  const geoms = useMemo(() => selection.map((id) => geometry(id, f)).filter((g): g is Geometry => !!g), [selection, f, revision]);
  const hoverGeom = useMemo(() => (hover && !selection.includes(hover) ? geometry(hover, f) : null), [hover, f, revision, selection]);

  const toComp = (e: { clientX: number; clientY: number }): Pt => {
    const r = ref.current!.getBoundingClientRect();
    return [(e.clientX - r.left) / zoom, (e.clientY - r.top) / zoom];
  };

  // Motion path for a single selected layer with animated position.
  const motion = useMemo(() => {
    if (selection.length !== 1 || !info) return null;
    const st = query<any>("get_property", { layer: selection[0], property: "position", frame: f }, null);
    if (!st?.animated || scene.layers.find((l) => l.id === selection[0])?.parent) return null;
    const step = Math.max(1, Math.round((info.op - info.ip) / 90));
    const pts: Pt[] = [];
    for (let t = info.ip; t <= info.op; t += step) {
      const v = query<any>("get_property", { layer: selection[0], property: "position", frame: t }, null)?.value;
      if (Array.isArray(v)) pts.push([v[0], v[1]]);
    }
    const keys: Pt[] = (st.keyframes as number[]).map((t) => query<any>("get_property", { layer: selection[0], property: "position", frame: t }, null)?.value).filter(Array.isArray).map((v: number[]) => [v[0], v[1]]);
    return { pts, keys };
  }, [selection, revision, info?.ip, info?.op, f, scene]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!pen) return;
      if (e.key === "Escape") setPen(null);
      if (e.key === "Enter") finishPen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  useEffect(() => {
    if (tool !== "pen") setPen(null);
  }, [tool]);

  const finishPen = (closed: boolean) => {
    if (!pen || pen.length < 2) {
      setPen(null);
      return;
    }
    const xs = pen.map((p) => p.v[0]);
    const ys = pen.map((p) => p.v[1]);
    const c: Pt = [(Math.min(...xs) + Math.max(...xs)) / 2, (Math.min(...ys) + Math.max(...ys)) / 2];
    const path = {
      c: closed,
      v: pen.map((p) => [p.v[0] - c[0], p.v[1] - c[1]]),
      o: pen.map((p) => p.o),
      i: pen.map((p) => [-p.o[0], -p.o[1]]),
    };
    const r = act<{ id: string }>("create_shape", { type: "path", path, position: c, scene: sceneId, fill: closed ? "#c4c4c4" : null, stroke: closed ? undefined : "#1b1b1b", stroke_width: 3 });
    setPen(null);
    if (r) {
      select([r.id]);
      setTool("select");
    }
  };

  const snapDelta = (base: [number, number, number, number], d: Pt): Pt => {
    if (!snapping) {
      setGuides({});
      return d;
    }
    const tol = SNAP_PX / zoom;
    const xs = [0, W / 2, W];
    const ys = [0, H / 2, H];
    for (const l of scene.layers) {
      if (selection.includes(l.id)) continue;
      const g = geometry(l.id, f);
      if (g && g.bounds[2] > g.bounds[0]) {
        xs.push(g.bounds[0], (g.bounds[0] + g.bounds[2]) / 2, g.bounds[2]);
        ys.push(g.bounds[1], (g.bounds[1] + g.bounds[3]) / 2, g.bounds[3]);
      }
    }
    const best = (cands: number[], edges: number[]) => {
      let out: { d: number; at: number } | null = null;
      for (const e of edges)
        for (const c of cands) {
          const dd = c - e;
          if (Math.abs(dd) <= tol && (!out || Math.abs(dd) < Math.abs(out.d))) out = { d: dd, at: c };
        }
      return out;
    };
    const bx = [base[0] + d[0], (base[0] + base[2]) / 2 + d[0], base[2] + d[0]];
    const by = [base[1] + d[1], (base[1] + base[3]) / 2 + d[1], base[3] + d[1]];
    const sx = best(xs, bx);
    const sy = best(ys, by);
    setGuides({ x: sx?.at, y: sy?.at });
    return [d[0] + (sx?.d ?? 0), d[1] + (sy?.d ?? 0)];
  };

  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button !== 0 || !info) return;
    const p = toComp(e);
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    if (tool === "pen") {
      if (pen && pen.length > 2 && dist(p, pen[0].v) * zoom < 8) {
        finishPen(true);
        return;
      }
      setPen([...(pen ?? []), { v: p, o: [0, 0] }]);
      setPenDrag(true);
      return;
    }
    if (tool === "text") {
      const r = act<{ id: string }>("create_text", { text: "Your text", position: p, font_size: Math.max(16, Math.round(H / 10)), scene: sceneId });
      if (r) select([r.id]);
      setTool("select");
      return;
    }
    if (tool === "rect" || tool === "ellipse" || tool === "polygon" || tool === "star") {
      setDrag({ kind: "draw", start: p, cur: p });
      return;
    }
    if (tool !== "select") return;
    let id = hitTest(p[0], p[1], f);
    if (id && locked.has(id)) id = null;
    if (!id) {
      if (!e.shiftKey) select([]);
      setDrag({ kind: "marquee", start: p, cur: p });
      return;
    }
    let sel = selection;
    if (e.shiftKey) {
      sel = selection.includes(id) ? selection.filter((x) => x !== id) : [...selection, id];
      select(sel);
    } else if (!selection.includes(id)) {
      sel = [id];
      select(sel);
    }
    const gs = sel.map((x) => geometry(x, f)).filter((g): g is Geometry => !!g);
    const base: [number, number, number, number] = gs.reduce((b, g) => [Math.min(b[0], g.bounds[0]), Math.min(b[1], g.bounds[1]), Math.max(b[2], g.bounds[2]), Math.max(b[3], g.bounds[3])], [Infinity, Infinity, -Infinity, -Infinity] as [number, number, number, number]);
    beginGesture();
    setDrag({ kind: "move", start: p, applied: [0, 0], base });
  };

  const startHandle = (e: React.PointerEvent, kind: "scale" | "rotate", axis: "both" | "x" | "y" = "both") => {
    e.stopPropagation();
    if (geoms.length !== 1) return;
    (e.currentTarget as Element).setPointerCapture(e.pointerId);
    const g = geoms[0];
    const id = g.id;
    const p = toComp(e);
    const anchor = g.anchor;
    const m = g.matrix;
    const nx = Math.hypot(m[0], m[1]) || 1;
    const ny = Math.hypot(m[2], m[3]) || 1;
    beginGesture();
    if (kind === "scale") {
      const s = query<any>("get_property", { layer: id, property: "scale", frame: f }, null)?.value ?? [100, 100];
      setDrag({ kind: "scale", id, anchor, start: p, scale0: s, axis, ux: [m[0] / nx, m[1] / nx], uy: [m[2] / ny, m[3] / ny] });
    } else {
      const r = query<any>("get_property", { layer: id, property: "rotation", frame: f }, null)?.value ?? [0];
      setDrag({ kind: "rotate", id, anchor, start: p, rot0: r[0] ?? 0 });
    }
  };

  const onPointerMove = (e: React.PointerEvent) => {
    const p = toComp(e);
    const drag = dragRef.current;
    if (tool === "pen" && pen && penDrag) {
      const last = pen[pen.length - 1];
      const o: Pt = [p[0] - last.v[0], p[1] - last.v[1]];
      setPen([...pen.slice(0, -1), { v: last.v, o: Math.hypot(...o) * zoom > 3 ? o : [0, 0] }]);
      return;
    }
    if (!drag) {
      if (tool === "select") {
        const h = hitTest(p[0], p[1], f);
        if (h !== hover) setHover(h);
      }
      return;
    }
    switch (drag.kind) {
      case "move": {
        const total = snapDelta(drag.base, [p[0] - drag.start[0], p[1] - drag.start[1]]);
        const inc: Pt = [total[0] - drag.applied[0], total[1] - drag.applied[1]];
        if (Math.abs(inc[0]) + Math.abs(inc[1]) < 1e-3) return;
        const layers = selection.filter((x) => !locked.has(x));
        if (layers.length) act("nudge_layers", { layers, dx: inc[0], dy: inc[1], frame: f });
        setDrag({ ...drag, applied: total });
        break;
      }
      case "scale": {
        const v0: Pt = [drag.start[0] - drag.anchor[0], drag.start[1] - drag.anchor[1]];
        const v1: Pt = [p[0] - drag.anchor[0], p[1] - drag.anchor[1]];
        const proj = (v: Pt, u: Pt) => v[0] * u[0] + v[1] * u[1];
        let fx = Math.abs(proj(v0, drag.ux)) > 1e-6 ? proj(v1, drag.ux) / proj(v0, drag.ux) : 1;
        let fy = Math.abs(proj(v0, drag.uy)) > 1e-6 ? proj(v1, drag.uy) / proj(v0, drag.uy) : 1;
        if (drag.axis === "both") {
          const u = Math.hypot(...v1) / Math.max(1e-6, Math.hypot(...v0));
          if (e.shiftKey) {
            fx = fy = u;
          } else {
            fx = Number.isFinite(fx) ? fx : u;
            fy = Number.isFinite(fy) ? fy : u;
          }
        }
        if (drag.axis === "x") fy = 1;
        if (drag.axis === "y") fx = 1;
        act("set_property", { layer: drag.id, property: "scale", value: [drag.scale0[0] * fx, drag.scale0[1] * fy].map((v) => Math.round(v * 10) / 10), frame: f });
        break;
      }
      case "rotate": {
        const a0 = Math.atan2(drag.start[1] - drag.anchor[1], drag.start[0] - drag.anchor[0]);
        const a1 = Math.atan2(p[1] - drag.anchor[1], p[0] - drag.anchor[0]);
        let r = drag.rot0 + ((a1 - a0) * 180) / Math.PI;
        if (e.shiftKey) r = Math.round(r / 15) * 15;
        act("set_property", { layer: drag.id, property: "rotation", value: Math.round(r * 10) / 10, frame: f });
        break;
      }
      case "draw":
      case "marquee":
        setDrag({ ...drag, cur: e.shiftKey && drag.kind === "draw" ? squareEnd(drag.start, p) : p });
        break;
    }
  };

  const onPointerUp = () => {
    if (tool === "pen") {
      setPenDrag(false);
      return;
    }
    const drag = dragRef.current;
    if (!drag) return;
    if (drag.kind === "move" || drag.kind === "scale" || drag.kind === "rotate") endGesture();
    if (drag.kind === "draw") {
      const [x0, y0] = drag.start;
      const [x1, y1] = drag.cur;
      let w = Math.abs(x1 - x0);
      let h = Math.abs(y1 - y0);
      const tiny = w * zoom < 4 && h * zoom < 4;
      if (tiny) w = h = 100;
      const c: Pt = tiny ? drag.start : [(x0 + x1) / 2, (y0 + y1) / 2];
      const type = tool as "rect" | "ellipse" | "polygon" | "star";
      const r = act<{ id: string }>("create_shape", {
        type,
        position: c,
        scene: sceneId,
        ...(type === "rect" || type === "ellipse" ? { size: [Math.round(w), Math.round(h)] } : { radius: Math.round(Math.max(w, h) / 2), points: type === "star" ? 5 : 5 }),
      });
      if (r) select([r.id]);
      setTool("select");
    }
    if (drag.kind === "marquee") {
      const b = [Math.min(drag.start[0], drag.cur[0]), Math.min(drag.start[1], drag.cur[1]), Math.max(drag.start[0], drag.cur[0]), Math.max(drag.start[1], drag.cur[1])];
      if ((b[2] - b[0]) * zoom > 3 || (b[3] - b[1]) * zoom > 3) {
        const hits = scene.layers
          .filter((l) => !l.locked && !l.hidden)
          .filter((l) => {
            const g = geometry(l.id, f);
            return g && g.bounds[0] < b[2] && g.bounds[2] > b[0] && g.bounds[1] < b[3] && g.bounds[3] > b[1] && g.bounds[2] > g.bounds[0];
          })
          .map((l) => l.id);
        select(hits);
      }
    }
    setGuides({});
    setDrag(null);
  };

  const Z = (p: Pt) => `${p[0] * zoom},${p[1] * zoom}`;
  const cursor = tool === "select" ? (hover ? "move" : "default") : tool === "hand" ? "grab" : "crosshair";

  return (
    <svg
      ref={ref}
      className="overlay"
      width={W * zoom}
      height={H * zoom}
      style={{ cursor }}
      onPointerDown={onPointerDown}
      onPointerMove={onPointerMove}
      onPointerUp={onPointerUp}
      onPointerLeave={() => setHover(null)}
      onDoubleClick={() => tool === "pen" && finishPen(false)}
    >
      {grid &&
        Array.from({ length: Math.floor(W / 50) + 1 }, (_, i) => <line key={`gx${i}`} x1={i * 50 * zoom} y1={0} x2={i * 50 * zoom} y2={H * zoom} className="grid-line" />).concat(
          Array.from({ length: Math.floor(H / 50) + 1 }, (_, i) => <line key={`gy${i}`} x1={0} y1={i * 50 * zoom} x2={W * zoom} y2={i * 50 * zoom} className="grid-line" />),
        )}
      {motion && (
        <g className="motion-path">
          <polyline points={motion.pts.map(Z).join(" ")} />
          {motion.keys.map((k, i) => (
            <rect key={i} x={k[0] * zoom - 3} y={k[1] * zoom - 3} width={6} height={6} transform={`rotate(45 ${k[0] * zoom} ${k[1] * zoom})`} />
          ))}
        </g>
      )}
      {hoverGeom && <polygon className="hover-outline" points={hoverGeom.corners.map((c) => Z(c as Pt)).join(" ")} />}
      {geoms.map((g) => (
        <polygon key={g.id} className="sel-outline" points={g.corners.map((c) => Z(c as Pt)).join(" ")} />
      ))}
      {geoms.length === 1 && !selShape && (
        <Handles g={geoms[0]} zoom={zoom} onScale={(e, axis) => startHandle(e, "scale", axis)} onRotate={(e) => startHandle(e, "rotate")} />
      )}
      {guides.x !== undefined && <line className="snap-guide" x1={guides.x * zoom} y1={0} x2={guides.x * zoom} y2={H * zoom} />}
      {guides.y !== undefined && <line className="snap-guide" x1={0} y1={guides.y * zoom} x2={W * zoom} y2={guides.y * zoom} />}
      {drag?.kind === "draw" && <DrawPreview tool={tool} a={drag.start} b={drag.cur} zoom={zoom} />}
      {drag?.kind === "marquee" && (
        <rect
          className="marquee"
          x={Math.min(drag.start[0], drag.cur[0]) * zoom}
          y={Math.min(drag.start[1], drag.cur[1]) * zoom}
          width={Math.abs(drag.cur[0] - drag.start[0]) * zoom}
          height={Math.abs(drag.cur[1] - drag.start[1]) * zoom}
        />
      )}
      {pen && <PenPreview pts={pen} zoom={zoom} />}
    </svg>
  );
}

function squareEnd(a: Pt, b: Pt): Pt {
  const s = Math.max(Math.abs(b[0] - a[0]), Math.abs(b[1] - a[1]));
  return [a[0] + Math.sign(b[0] - a[0] || 1) * s, a[1] + Math.sign(b[1] - a[1] || 1) * s];
}

function Handles({ g, zoom, onScale, onRotate }: { g: Geometry; zoom: number; onScale: (e: React.PointerEvent, axis: "both" | "x" | "y") => void; onRotate: (e: React.PointerEvent) => void }) {
  const c = g.corners.map((p) => [p[0] * zoom, p[1] * zoom] as Pt);
  const mid = (a: Pt, b: Pt): Pt => [(a[0] + b[0]) / 2, (a[1] + b[1]) / 2];
  const edges: [Pt, "x" | "y"][] = [
    [mid(c[0], c[1]), "y"],
    [mid(c[1], c[2]), "x"],
    [mid(c[2], c[3]), "y"],
    [mid(c[3], c[0]), "x"],
  ];
  const top = mid(c[0], c[1]);
  const center = mid(c[0], c[2]);
  const away = Math.hypot(top[0] - center[0], top[1] - center[1]) || 1;
  const rot: Pt = [top[0] + ((top[0] - center[0]) / away) * 22, top[1] + ((top[1] - center[1]) / away) * 22];
  const anchor: Pt = [g.anchor[0] * zoom, g.anchor[1] * zoom];
  return (
    <g>
      <line x1={top[0]} y1={top[1]} x2={rot[0]} y2={rot[1]} className="rot-arm" />
      <circle cx={rot[0]} cy={rot[1]} r={5} className="rot-handle" onPointerDown={onRotate} />
      {c.map((p, i) => (
        <rect key={i} x={p[0] - 4} y={p[1] - 4} width={8} height={8} className="handle" onPointerDown={(e) => onScale(e, "both")} />
      ))}
      {edges.map(([p, axis], i) => (
        <rect key={`e${i}`} x={p[0] - 3} y={p[1] - 3} width={6} height={6} className="handle edge" onPointerDown={(e) => onScale(e, axis)} />
      ))}
      <g className="anchor-mark">
        <circle cx={anchor[0]} cy={anchor[1]} r={4} />
        <line x1={anchor[0] - 7} y1={anchor[1]} x2={anchor[0] + 7} y2={anchor[1]} />
        <line x1={anchor[0]} y1={anchor[1] - 7} x2={anchor[0]} y2={anchor[1] + 7} />
      </g>
    </g>
  );
}

function DrawPreview({ tool, a, b, zoom }: { tool: string; a: Pt; b: Pt; zoom: number }) {
  const x = Math.min(a[0], b[0]) * zoom;
  const y = Math.min(a[1], b[1]) * zoom;
  const w = Math.abs(b[0] - a[0]) * zoom;
  const h = Math.abs(b[1] - a[1]) * zoom;
  if (tool === "ellipse") return <ellipse className="draft" cx={x + w / 2} cy={y + h / 2} rx={w / 2} ry={h / 2} />;
  if (tool === "rect") return <rect className="draft" x={x} y={y} width={w} height={h} />;
  const r = Math.max(w, h) / 2;
  const n = tool === "star" ? 10 : 5;
  const pts = Array.from({ length: n }, (_, i) => {
    const ang = -Math.PI / 2 + (i * 2 * Math.PI) / n;
    const rr = tool === "star" && i % 2 === 1 ? r / 2 : r;
    return `${x + w / 2 + rr * Math.cos(ang)},${y + h / 2 + rr * Math.sin(ang)}`;
  });
  return <polygon className="draft" points={pts.join(" ")} />;
}

function PenPreview({ pts, zoom }: { pts: PenPoint[]; zoom: number }) {
  const s = (p: Pt) => `${p[0] * zoom} ${p[1] * zoom}`;
  let d = `M${s(pts[0].v)}`;
  for (let i = 1; i < pts.length; i++) {
    const a = pts[i - 1];
    const b = pts[i];
    d += ` C${s([a.v[0] + a.o[0], a.v[1] + a.o[1]])} ${s([b.v[0] - b.o[0], b.v[1] - b.o[1]])} ${s(b.v)}`;
  }
  return (
    <g className="pen">
      <path d={d} />
      {pts.map((p, i) => (
        <g key={i}>
          {(p.o[0] !== 0 || p.o[1] !== 0) && (
            <line x1={(p.v[0] - p.o[0]) * zoom} y1={(p.v[1] - p.o[1]) * zoom} x2={(p.v[0] + p.o[0]) * zoom} y2={(p.v[1] + p.o[1]) * zoom} className="pen-tan" />
          )}
          <rect x={p.v[0] * zoom - 3.5} y={p.v[1] * zoom - 3.5} width={7} height={7} className={i === 0 ? "pen-first" : ""} />
        </g>
      ))}
    </g>
  );
}
