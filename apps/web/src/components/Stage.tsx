import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { engine } from "../engine";
import { openFile } from "../files";
import { classifyWheel, wheelZoomFactor, zoomAt } from "../lib/camera";
import { useEditor } from "../store";
import { PlayerBar } from "./PlayerBar";
import { openContextMenu } from "../contextMenu";
import { canvasMenu } from "../menus";

/**
 * Canvas stage. One viewport-sized canvas is rendered through the camera, so
 * zoomed views stay sharp; the artboard (checkerboard) and the interaction
 * overlay are positioned with the same camera.
 *
 * Input: trackpad two-finger scroll pans; pinch (ctrl+wheel, or WebKit gesture
 * events) zooms at the cursor; a notched mouse wheel zooms (auto-detected, or
 * forced via View ▸ Mouse wheel); shift+wheel pans horizontally; space-drag,
 * middle-drag or the hand tool pan.
 */
export function Stage({ overlay }: { overlay?: (zoom: number) => React.ReactNode }) {
  const info = useEditor((s) => s.info);
  const revision = useEditor((s) => s.revision);
  const frame = useEditor((s) => s.frame);
  const camera = useEditor((s) => s.camera);
  const viewport = useEditor((s) => s.viewport);
  const tool = useEditor((s) => s.tool);
  const viewRef = useRef<HTMLDivElement>(null);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const [dragging, setDragging] = useState(false);
  const [spaceHeld, setSpaceHeld] = useState(false);
  const [panning, setPanning] = useState(false);
  const panDrag = useRef<{ x: number; y: number; cx: number; cy: number } | null>(null);
  const spaceUsed = useRef(false);

  useLayoutEffect(() => {
    const el = viewRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => useEditor.getState().setViewport(el.clientWidth, el.clientHeight));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  // Fit when the document size changes or the viewport first appears.
  const hasViewport = viewport.w > 0;
  useEffect(() => {
    useEditor.getState().fitView();
  }, [info?.w, info?.h, info?.view, hasViewport]);

  useEffect(() => {
    const onFit = () => useEditor.getState().fitView();
    window.addEventListener("lumetrope:fit", onFit);
    return () => window.removeEventListener("lumetrope:fit", onFit);
  }, []);

  // Render through the camera at device resolution.
  const dpr = window.devicePixelRatio || 1;
  const pxW = Math.max(1, Math.round(viewport.w * dpr));
  const pxH = Math.max(1, Math.round(viewport.h * dpr));
  useEffect(() => {
    const c = canvasRef.current;
    const ctx = c?.getContext("2d");
    if (!c || !ctx || !info || viewport.w === 0) return;
    const px = engine().render_view(frame, pxW, pxH, camera.zoom * dpr, camera.x * dpr, camera.y * dpr);
    ctx.putImageData(new ImageData(new Uint8ClampedArray(px.buffer, px.byteOffset, px.byteLength), pxW, pxH), 0, 0);
  }, [revision, frame, pxW, pxH, camera, info, dpr, viewport.w]);

  // Wheel / pinch (non-passive so the browser does not zoom the page).
  useEffect(() => {
    const el = viewRef.current;
    if (!el) return;
    const onWheel = (e: WheelEvent) => {
      e.preventDefault();
      const st = useEditor.getState();
      const r = el.getBoundingClientRect();
      const sx = e.clientX - r.left;
      const sy = e.clientY - r.top;
      const intent = classifyWheel(
        { deltaX: e.deltaX, deltaY: e.deltaY, deltaMode: e.deltaMode, ctrlKey: e.ctrlKey, metaKey: e.metaKey, wheelDeltaY: (e as any).wheelDeltaY },
        st.wheelMode,
      );
      if (intent === "zoom") {
        st.setCamera(zoomAt(st.camera, wheelZoomFactor(e), sx, sy));
      } else {
        const dx = e.shiftKey && e.deltaX === 0 ? e.deltaY : e.deltaX;
        const dy = e.shiftKey && e.deltaX === 0 ? 0 : e.deltaY;
        const k = e.deltaMode === 1 ? 33 : 1;
        st.setCamera({ ...st.camera, x: st.camera.x - dx * k, y: st.camera.y - dy * k });
      }
    };
    // Safari / WKWebView (Tauri on macOS) report pinch as gesture events.
    let lastScale = 1;
    const onGestureStart = (e: any) => {
      e.preventDefault();
      lastScale = 1;
    };
    const onGestureChange = (e: any) => {
      e.preventDefault();
      const st = useEditor.getState();
      const r = el.getBoundingClientRect();
      st.setCamera(zoomAt(st.camera, e.scale / lastScale, e.clientX - r.left, e.clientY - r.top));
      lastScale = e.scale;
    };
    el.addEventListener("wheel", onWheel, { passive: false });
    el.addEventListener("gesturestart", onGestureStart as EventListener, { passive: false } as AddEventListenerOptions);
    el.addEventListener("gesturechange", onGestureChange as EventListener, { passive: false } as AddEventListenerOptions);
    return () => {
      el.removeEventListener("wheel", onWheel);
      el.removeEventListener("gesturestart", onGestureStart as EventListener);
      el.removeEventListener("gesturechange", onGestureChange as EventListener);
    };
  }, []);

  // Space: hold to pan, tap to play/pause.
  useEffect(() => {
    const typing = (t: EventTarget | null) => t instanceof HTMLElement && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable);
    const down = (e: KeyboardEvent) => {
      if (e.code !== "Space" || typing(e.target)) return;
      e.preventDefault();
      if (!e.repeat) {
        spaceUsed.current = false;
        setSpaceHeld(true);
      }
    };
    const up = (e: KeyboardEvent) => {
      if (e.code !== "Space" || typing(e.target)) return;
      e.preventDefault();
      setSpaceHeld(false);
      if (!spaceUsed.current) {
        const st = useEditor.getState();
        st.setPlaying(!st.playing);
      }
    };
    window.addEventListener("keydown", down);
    window.addEventListener("keyup", up);
    return () => {
      window.removeEventListener("keydown", down);
      window.removeEventListener("keyup", up);
    };
  }, []);

  const onPointerDown = (e: React.PointerEvent) => {
    if (e.button === 1 || (e.button === 0 && (tool === "hand" || spaceHeld))) {
      e.preventDefault();
      e.stopPropagation();
      spaceUsed.current = true;
      const c = useEditor.getState().camera;
      panDrag.current = { x: e.clientX, y: e.clientY, cx: c.x, cy: c.y };
      (e.currentTarget as Element).setPointerCapture(e.pointerId);
      setPanning(true);
    }
  };
  const onPointerMove = (e: React.PointerEvent) => {
    const d = panDrag.current;
    if (!d) return;
    const st = useEditor.getState();
    st.setCamera({ ...st.camera, x: d.cx + e.clientX - d.x, y: d.cy + e.clientY - d.y });
  };
  const onPointerUp = () => {
    panDrag.current = null;
    setPanning(false);
  };

  const art = info ? { left: camera.x, top: camera.y, width: info.w * camera.zoom, height: info.h * camera.zoom } : null;
  const cursor = panning ? "grabbing" : tool === "hand" || spaceHeld ? "grab" : undefined;

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
        onPointerDownCapture={onPointerDown}
        onPointerMove={onPointerMove}
        onPointerUp={onPointerUp}
        onAuxClick={(e) => e.preventDefault()}
        onContextMenu={(e) => {
          const r = viewRef.current!.getBoundingClientRect();
          const c = useEditor.getState().camera;
          openContextMenu(e, canvasMenu([(e.clientX - r.left - c.x) / c.zoom, (e.clientY - r.top - c.y) / c.zoom]));
        }}
        style={{ cursor }}
      >
        {art && (
          <div className="artboard" style={art}>
            <span className="artboard-label num">
              {info!.name || "Scene"} · {info!.w}×{info!.h}
            </span>
          </div>
        )}
        <canvas ref={canvasRef} className="view-canvas" width={pxW} height={pxH} style={{ width: viewport.w, height: viewport.h }} />
        {art && overlay && (
          <div className="overlay-host" style={{ ...art, pointerEvents: tool === "hand" || spaceHeld ? "none" : undefined }}>
            {overlay(camera.zoom)}
          </div>
        )}
      </div>
      <PlayerBar />
      {dragging && <div className="drop-hint">Drop a .json, .lottie or .svg to open</div>}
    </div>
  );
}
