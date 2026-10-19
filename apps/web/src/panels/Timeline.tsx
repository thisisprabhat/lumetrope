import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { act, query } from "../actions";
import { useSceneInfo, type LayerNode } from "../doc";
import { formatTime } from "../lib/time";
import { useEditor } from "../store";
import { Icon } from "../components/Icon";
import { EasingEditor } from "./EasingEditor";
import { itemAt } from "./ShapePanel";

interface KfInfo {
  frame: number;
  value: unknown;
  easing: [number, number, number, number] | "hold" | null;
}
interface Track {
  path: string;
  keyframes: KfInfo[];
}
interface KfSel {
  layer: string;
  path: string;
  frame: number;
}

const KEY_LABEL: Record<string, string> = {
  p: "Position", a: "Anchor", s: "Scale", r: "Rotation", o: "Opacity", sk: "Skew", sa: "Skew axis", c: "Color", w: "Width", e: "End",
  ks: "Path", pt: "Points", or: "Outer radius", ir: "Inner radius", os: "Outer roundness", is: "Inner roundness", tm: "Time remap", x: "Position X", y: "Position Y",
};
const ITEM_LABEL: Record<string, string> = { rc: "Rect", el: "Ellipse", sr: "Star", sh: "Path", fl: "Fill", st: "Stroke", gf: "Gradient", gs: "Gradient", tm: "Trim", rd: "Round", rp: "Repeater", gr: "Group", tr: "Group" };

function labelFor(path: string, layerJson: any): string {
  const parts = path.split("/");
  const key = parts[parts.length - 1] === "k" ? parts[parts.length - 2] : parts[parts.length - 1];
  if (parts[0] === "ks") return KEY_LABEL[parts[1]] ?? parts[1];
  if (parts[0] === "shapes") {
    const shapePath = parts.slice(1, -1).filter((p) => p !== "it").join("/");
    const item = itemAt(layerJson, shapePath);
    const owner = ITEM_LABEL[item?.ty] ?? "Shape";
    let k = KEY_LABEL[key] ?? key;
    if (item?.ty === "tm" && key === "s") k = "Start";
    if (item?.ty === "tm" && key === "o") k = "Offset";
    if ((item?.ty === "rc" || item?.ty === "el") && key === "s") k = "Size";
    if (item?.ty === "rc" && key === "r") k = "Roundness";
    return `${item?.nm ?? owner} · ${k}`;
  }
  if (parts[0] === "masksProperties") return `Mask ${Number(parts[1]) + 1} · ${KEY_LABEL[key] ?? key}`;
  if (parts[0] === "ef") return `Effect ${Number(parts[1]) + 1} · ${layerJson?.ef?.[parts[1]]?.ef?.[parts[3]]?.nm ?? "value"}`;
  return path;
}

export function Timeline() {
  const scene = useSceneInfo();
  const { frame, setFrame, revision, selection, select, info } = useEditor();
  const [expanded, setExpanded] = useState<Record<string, boolean>>({});
  const [zoom, setZoom] = useState(1);
  const [width, setWidth] = useState(600);
  const [kfSel, setKfSel] = useState<KfSel | null>(null);
  const [easing, setEasing] = useState<{ sel: KfSel; x: number; y: number; value: KfInfo["easing"] } | null>(null);
  const [dragKf, setDragKf] = useState<{ sel: KfSel; to: number } | null>(null);
  const trackRef = useRef<HTMLDivElement>(null);
  const ip = info?.ip ?? 0;
  const op = Math.max(ip + 1, info?.op ?? 60);
  const fps = info?.fr ?? 30;
  const span = op - ip;
  const inner = width * zoom;
  const ppf = inner / span;
  const fx = (f: number) => (f - ip) * ppf;

  useLayoutEffect(() => {
    const el = trackRef.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setWidth(Math.max(100, el.clientWidth - 16)));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  const tracks = useMemo(() => {
    const out: Record<string, { tracks: Track[]; json: any }> = {};
    for (const l of scene.layers) {
      const r = query<{ animated: Track[] }>("read_keyframes", { layer: l.id }, { animated: [] });
      out[l.id] = { tracks: r.animated, json: query<any>("inspect_layer", { layer: l.id }, null)?.json };
    }
    return out;
  }, [scene, revision]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (!kfSel || (e.target as HTMLElement).tagName === "INPUT" || (e.target as HTMLElement).tagName === "TEXTAREA") return;
      if (e.key === "Delete" || e.key === "Backspace") {
        e.preventDefault();
        e.stopPropagation();
        act("remove_keyframe", { layer: kfSel.layer, path: kfSel.path, frame: kfSel.frame });
        setKfSel(null);
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [kfSel]);

  const frameFromEvent = (clientX: number) => {
    const r = trackRef.current!.getBoundingClientRect();
    return Math.round(ip + (clientX - r.left + trackRef.current!.scrollLeft - 8) / ppf);
  };

  const scrub = (e: React.PointerEvent) => {
    (e.target as Element).setPointerCapture(e.pointerId);
    setFrame(frameFromEvent(e.clientX));
    const move = (ev: PointerEvent) => setFrame(frameFromEvent(ev.clientX));
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  // Ruler ticks
  const step = [1, 2, 5, 10, 15, 30, 60, 120, 300].find((s) => s * ppf >= 46) ?? 600;
  const ticks: number[] = [];
  for (let f = Math.ceil(ip / step) * step; f <= op; f += step) ticks.push(f);

  const startKfDrag = (e: React.PointerEvent, sel: KfSel) => {
    e.stopPropagation();
    setKfSel(sel);
    const x0 = e.clientX;
    let to = sel.frame;
    const move = (ev: PointerEvent) => {
      to = Math.max(0, Math.round(sel.frame + (ev.clientX - x0) / ppf));
      setDragKf({ sel, to });
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      setDragKf(null);
      if (to !== sel.frame && act("move_keyframe", { layer: sel.layer, path: sel.path, frame: sel.frame, to }) !== undefined) {
        setKfSel({ ...sel, frame: to });
      }
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };

  const diamond = (layer: string, path: string, k: KfInfo, summary = false) => {
    const sel = kfSel && kfSel.layer === layer && kfSel.path === path && kfSel.frame === k.frame;
    const dragging = dragKf && dragKf.sel.layer === layer && dragKf.sel.path === path && dragKf.sel.frame === k.frame;
    const f = dragging ? dragKf.to : k.frame;
    return (
      <span
        key={`${path}@${k.frame}`}
        className={`kf ${sel ? "sel" : ""} ${summary ? "summary" : ""} ${k.easing === "hold" ? "hold" : ""}`}
        style={{ left: fx(f) }}
        title={`${formatTime(k.frame, fps)} · frame ${k.frame}`}
        onPointerDown={summary ? undefined : (e) => startKfDrag(e, { layer, path, frame: k.frame })}
        onDoubleClick={
          summary
            ? undefined
            : (e) => {
                const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
                setEasing({ sel: { layer, path, frame: k.frame }, x: r.left, y: r.top, value: k.easing });
              }
        }
      />
    );
  };

  const layerBar = (l: LayerNode) => {
    const edge = (side: "in" | "out") => (e: React.PointerEvent) => {
      e.stopPropagation();
      const x0 = e.clientX;
      const orig = side === "in" ? l.in : l.out;
      let v = orig;
      const bar = (e.currentTarget as HTMLElement).parentElement!;
      const move = (ev: PointerEvent) => {
        v = Math.round(orig + (ev.clientX - x0) / ppf);
        if (side === "in") bar.style.left = `${fx(v)}px`;
        bar.style.width = `${Math.max(2, fx(side === "out" ? v : l.out) - fx(side === "in" ? v : l.in))}px`;
      };
      const up = () => {
        window.removeEventListener("pointermove", move);
        window.removeEventListener("pointerup", up);
        if (v !== orig) act("set_layer_time", { layer: l.id, [side]: v });
      };
      window.addEventListener("pointermove", move);
      window.addEventListener("pointerup", up);
    };
    return (
      <div className={`tl-bar t-${l.type}`} style={{ left: fx(Math.max(ip, l.in)), width: Math.max(2, fx(Math.min(op, l.out)) - fx(Math.max(ip, l.in))) }}>
        <span className="tl-bar-edge left" onPointerDown={edge("in")} />
        <span className="tl-bar-edge right" onPointerDown={edge("out")} />
      </div>
    );
  };

  return (
    <div className="timeline">
      <div className="tl-toolbar">
        <span className="num lume">{formatTime(frame - ip, fps)}</span>
        <span className="num muted">
          {Math.floor(frame)} / {op}
        </span>
        <div className="spacer" />
        <span className="muted tiny">Zoom</span>
        <input type="range" min={1} max={8} step={0.25} value={zoom} onChange={(e) => setZoom(Number(e.target.value))} className="range" />
      </div>
      <div className="tl-scroll">
        <div className="tl-grid">
          <div className="tl-names">
            <div className="tl-head-cell">Layers</div>
            {scene.segments.length > 0 && <div className="tl-seg-cell muted tiny">Segments</div>}
            {scene.layers.map((l) => {
              const t = tracks[l.id]?.tracks ?? [];
              const open = expanded[l.id];
              return (
                <div key={l.id}>
                  <div className={`tl-name ${selection.includes(l.id) ? "sel" : ""} ${l.hidden ? "hidden" : ""}`} onClick={() => select([l.id])}>
                    <button className="tl-caret" disabled={t.length === 0} onClick={(e) => {
                      e.stopPropagation();
                      setExpanded({ ...expanded, [l.id]: !open });
                    }}>
                      {t.length > 0 && <Icon name={open ? "chevronDown" : "chevronRight"} size={11} />}
                    </button>
                    <span className="tl-name-text">{l.name || l.id}</span>
                    {t.length > 0 && <span className="tl-count">{t.length}</span>}
                  </div>
                  {open &&
                    t.map((tr) => (
                      <div key={tr.path} className="tl-prop">
                        {labelFor(tr.path, tracks[l.id]?.json)}
                      </div>
                    ))}
                </div>
              );
            })}
          </div>
          <div className="tl-tracks" ref={trackRef}>
            <div style={{ width: inner + 16, position: "relative", minHeight: "100%" }}>
              <div className="tl-ruler" onPointerDown={scrub}>
                {ticks.map((f) => (
                  <span key={f} className="tick" style={{ left: fx(f) + 8 }}>
                    <span className="num">{step >= fps && f % fps === 0 ? `${f / fps}s` : f}</span>
                  </span>
                ))}
              </div>
              {scene.segments.length > 0 && (
                <div className="tl-segs">
                  {scene.segments.map((s, i) => (
                    <span key={s.name} className={`seg c${i % 4}`} style={{ left: fx(s.start) + 8, width: Math.max(4, fx(s.end) - fx(s.start)) }} title={`${s.name}: ${s.start}–${s.end}`}>
                      {s.name}
                    </span>
                  ))}
                </div>
              )}
              {scene.layers.map((l) => {
                const t = tracks[l.id]?.tracks ?? [];
                const all = Array.from(new Set(t.flatMap((x) => x.keyframes.map((k) => k.frame)))).map((f) => ({ frame: f, value: null, easing: null }) as KfInfo);
                return (
                  <div key={l.id}>
                    <div className={`tl-row ${selection.includes(l.id) ? "sel" : ""}`} onPointerDown={() => select([l.id])}>
                      <div className="tl-row-inner">
                        {layerBar(l)}
                        {all.map((k) => diamond(l.id, "*", k, true))}
                      </div>
                    </div>
                    {expanded[l.id] &&
                      t.map((tr) => (
                        <div key={tr.path} className="tl-row prop">
                          <div className="tl-row-inner">{tr.keyframes.map((k) => diamond(l.id, tr.path, k))}</div>
                        </div>
                      ))}
                  </div>
                );
              })}
              <div className="tl-playhead" style={{ left: fx(frame) + 8 }} />
            </div>
          </div>
        </div>
        {scene.layers.length === 0 && <div className="panel-empty">Keyframes appear here. Select a layer and press a ◇ next to any property to start animating.</div>}
      </div>
      {easing && (
        <div className="easing-anchor" style={{ left: Math.min(window.innerWidth - 250, easing.x), top: Math.max(60, easing.y - 330) }}>
          <EasingEditor
            value={easing.value}
            onClose={() => setEasing(null)}
            onApply={(v) => {
              act("set_keyframe_easing", { layer: easing.sel.layer, path: easing.sel.path, frame: easing.sel.frame, easing: v });
              setEasing(null);
            }}
          />
        </div>
      )}
    </div>
  );
}
