import { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { renderTo } from "../engine";
import { openFile } from "../files";
import { useEditor } from "../store";
import { PlayerBar } from "./PlayerBar";

/** Canvas stage: artboard render, zoom/pan, drag-and-drop open. */
export function Stage({ overlay }: { overlay?: (zoom: number) => React.ReactNode }) {
  const info = useEditor((s) => s.info);
  const revision = useEditor((s) => s.revision);
  const frame = useEditor((s) => s.frame);
  const zoom = useEditor((s) => s.zoom);
  const setZoom = useEditor((s) => s.setZoom);
  const tool = useEditor((s) => s.tool);
  const viewRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [view, setView] = useState({ w: 0, h: 0 });
  const [pan, setPan] = useState({ x: 0, y: 0 });
  const [dragging, setDragging] = useState(false);
  const panDrag = useRef<{ x: number; y: number; px: number; py: number } | null>(null);

  const fit = useCallback(() => {
    if (!info || !view.w) return;
    setZoom(Math.min((view.w - 80) / info.w, (view.h - 120) / info.h));
    setPan({ x: 0, y: 0 });
  }, [info?.w, info?.h, view.w, view.h, setZoom]);

  useLayoutEffect(() => {
    const el = viewRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setView({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Fit when the document size changes or the view first appears.
  useEffect(fit, [info?.w, info?.h, view.w > 0]);

  useEffect(() => {
    const onFit = () => fit();
    window.addEventListener("lumetrope:fit", onFit);
    return () => window.removeEventListener("lumetrope:fit", onFit);
  }, [fit]);

  const dpr = window.devicePixelRatio || 1;
  const cssW = info ? info.w * zoom : 0;
  const cssH = info ? info.h * zoom : 0;
  const pxW = Math.max(1, Math.round(cssW * dpr));
  const pxH = Math.max(1, Math.round(cssH * dpr));
  const left = (view.w - cssW) / 2 + pan.x;
  const top = (view.h - cssH) / 2 + pan.y;

  useEffect(() => {
    const c = canvasRef.current;
    const ctx = c?.getContext("2d");
    if (!c || !ctx || !info) return;
    renderTo(ctx, frame, pxW, pxH);
  }, [revision, frame, pxW, pxH, info]);

  const onWheel = (e: React.WheelEvent) => {
    if (e.ctrlKey || e.metaKey) {
      const rect = viewRef.current!.getBoundingClientRect();
      const mx = e.clientX - rect.left - view.w / 2 - pan.x;
      const my = e.clientY - rect.top - view.h / 2 - pan.y;
      const nz = Math.min(16, Math.max(0.05, zoom * Math.exp(-e.deltaY * 0.01)));
      const k = nz / zoom;
      setPan({ x: pan.x - mx * (k - 1), y: pan.y - my * (k - 1) });
      setZoom(nz);
    } else {
      setPan({ x: pan.x - e.deltaX, y: pan.y - e.deltaY });
    }
  };

  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button === 1 || tool === "hand") {
      panDrag.current = { x: e.clientX, y: e.clientY, px: pan.x, py: pan.y };
      (e.target as Element).setPointerCapture(e.pointerId);
      e.stopPropagation();
    }
  };
  const onPointerMove = (e: React.PointerEvent) => {
    const d = panDrag.current;
    if (d) setPan({ x: d.px + e.clientX - d.x, y: d.py + e.clientY - d.y });
  };
  const onPointerUp = () => {
    panDrag.current = null;
  };

  return (
    <div
      className="stage"
      onDragOver={(e) => {
        e.preventDefault();
        setDragging(true);
      }}
      onDragLeave={() => setDragging(false)}
      onDrop={(e) => {
        e.preventDefault();
        setDragging(false);
        const f = e.dataTransfer.files[0];
        if (f) void openFile(f);
      }}
    >
      <div
        className="viewport"
        ref={viewRef}
        onWheel={onWheel}
        onPointerDownCapture={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        style={{ cursor: tool === "hand" ? "grab" : undefined }}
      >
        {info && (
          <div className="artboard" style={{ left, top, width: cssW, height: cssH }}>
            <span className="artboard-label num">
              {info.name || "Scene"} · {info.w}×{info.h}
            </span>
            <canvas ref={canvasRef} width={pxW} height={pxH} />
          </div>
        )}
        {info && overlay && <div style={{ position: "absolute", left, top, width: cssW, height: cssH }}>{overlay(zoom)}</div>}
      </div>
      <PlayerBar />
      {dragging && <div className="drop-hint">Drop a .json, .lottie or .svg to open</div>}
    </div>
  );
}
