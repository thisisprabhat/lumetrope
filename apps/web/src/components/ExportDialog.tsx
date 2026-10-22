import { useEffect, useMemo, useRef, useState } from "react";
import { engine, projectValue, type Info } from "../engine";
import { ExportCancelled, exportMedia, MEDIA_MIME, webmSupport, type MediaOptions, type Progress } from "../exporters";
import { download } from "../files";
import { CanvasRenderer } from "../gpu/CanvasRenderer";
import { exportFileName, formatBytes, type ExportFormat } from "../lib/export";
import { evenSize, exportSize, frameTimes, type MediaFormat } from "../lib/mediaExport";
import { markExported } from "../persist";
import { useEditor } from "../store";
import { Icon } from "./Icon";

const FORMATS: { id: ExportFormat; group: string; title: string; ext: string; note: string }[] = [
  { id: "lottie", group: "Lottie", title: "dotLottie", ext: ".lottie", note: "Everything: animations, state machines, themes, assets" },
  { id: "json", group: "Lottie", title: "Lottie JSON", ext: ".json", note: "Main animation for any Lottie player" },
  { id: "gif", group: "Animated image", title: "GIF", ext: ".gif", note: "Plays everywhere · on/off transparency" },
  { id: "webp", group: "Animated image", title: "Animated WebP", ext: ".webp", note: "Lossless · full alpha" },
  { id: "webm", group: "Video", title: "WebM", ext: ".webm", note: "VP9/VP8 video with alpha" },
];
const GROUPS = ["Lottie", "Animated image", "Video"];

const isMedia = (f: ExportFormat): f is MediaFormat => f === "gif" || f === "webp" || f === "webm";
const SCALES = [0.25, 0.5, 1, 2];
const FPS = [12, 15, 24, 25, 30, 50, 60];
const QUALITY = [
  { label: "High", bpp: 0.2 },
  { label: "Medium", bpp: 0.1 },
  { label: "Small file", bpp: 0.05 },
];

/** Export with live preview: Lottie formats or rendered GIF / WebP / WebM (main scene). */
export function ExportDialog({ onClose }: { onClose: () => void }) {
  const { fileName, toast, scene } = useEditor();
  // Exports always use the main scene; show it while the dialog is open.
  const info = useMemo<Info>(() => {
    if (scene !== "root") engine().set_view_scene("root");
    return JSON.parse(engine().info());
  }, [scene]);
  useEffect(() => () => void (scene !== "root" && engine().set_view_scene(scene)), [scene]);

  const project = useMemo(() => projectValue(), []);
  const interactive = Object.keys(project.stateMachines ?? {}).length > 0 || Object.keys(project.themes ?? {}).length > 0;
  const [format, setFormat] = useState<ExportFormat>(interactive ? "lottie" : "json");
  const [name, setName] = useState(fileName || "animation");
  const maxSide = Math.max(info.w, info.h);
  const gifScale = SCALES.filter((s) => s * maxSide <= 480).pop() ?? 0.25;
  const [scale, setScale] = useState<number | null>(null);
  const [fps, setFps] = useState<number>(Math.min(60, Math.round(info.fr) || 30));
  const [range, setRange] = useState<string>("all");
  const [transparent, setTransparent] = useState(true);
  const [background, setBackground] = useState("#ffffff");
  const [matte, setMatte] = useState("#ffffff");
  const [quality, setQuality] = useState(QUALITY[0].bpp);
  const [webm, setWebm] = useState<{ label: string } | null | "checking">("checking");
  const [busy, setBusy] = useState<{ progress: Progress; abort: AbortController } | null>(null);
  const canvas = useRef<HTMLCanvasElement>(null);

  useEffect(() => {
    void webmSupport(info.w, info.h).then((s) => setWebm(s ? { label: s.label } : null));
  }, [info.w, info.h]);

  const media = isMedia(format);
  const effScale = scale ?? (format === "gif" ? gifScale : 1);
  const effFps = format === "gif" ? Math.min(fps, 50) : fps;
  const size = format === "webm" ? evenSize(exportSize(info.w, info.h, effScale)) : exportSize(info.w, info.h, effScale);
  const marker = info.markers.find((m) => m.name === range);
  const [start, end] = marker ? [marker.time, marker.time + Math.max(1, marker.duration)] : [info.ip, info.op];
  const frameCount = frameTimes(start, end, info.fr, effFps).length;

  const payload = useMemo(() => {
    if (media) return null;
    try {
      return format === "json" ? new TextEncoder().encode(engine().export_lottie()) : engine().export_dotlottie();
    } catch (e) {
      return String(e);
    }
  }, [format, media]);

  useEffect(() => {
    const esc = (e: KeyboardEvent) => e.key === "Escape" && !busy && onClose();
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, [onClose, busy]);

  // Fit the preview inside the stage (contain), re-fitting when the stage resizes.
  const stage = useRef<HTMLDivElement>(null);
  const [box, setBox] = useState({ w: 360, h: 300 });
  useEffect(() => {
    const el = stage.current!;
    const ro = new ResizeObserver(() => setBox({ w: el.clientWidth, h: el.clientHeight }));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);

  useEffect(() => {
    const c = canvas.current!;
    const ctx = c.getContext("2d")!;
    const pad = 28;
    const s = Math.max(0.01, Math.min((box.w - pad * 2) / info.w, (box.h - pad * 2) / info.h));
    const dpr = devicePixelRatio || 1;
    const w = Math.max(1, Math.round(info.w * s * dpr));
    const h = Math.max(1, Math.round(info.h * s * dpr));
    c.width = w;
    c.height = h;
    c.style.width = `${w / dpr}px`;
    c.style.height = `${h / dpr}px`;
    let raf = 0;
    const t0 = performance.now();
    const gpu = new CanvasRenderer();
    gpu.setImages(JSON.parse(engine().image_assets()));
    const z = w / info.w;
    const len = Math.max(1, end - start);
    const loop = (now: number) => {
      // Preview at the export frame rate so choppiness is visible before exporting.
      const f = start + (Math.floor(((((now - t0) / 1000) * effFps) % ((len * effFps) / info.fr))) * info.fr) / effFps;
      ctx.clearRect(0, 0, w, h);
      gpu.render(ctx, JSON.parse(engine().display_list(f, z, 0, 0)), w, h, [0, 0, w, h]);
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  }, [info, start, end, effFps, box]);

  const finishSave = (bytes: Uint8Array, mime: string) => {
    const file = exportFileName(name, format);
    download(file, bytes, mime);
    toast(`Exported ${file} · ${formatBytes(bytes.byteLength)}`);
    void markExported(format);
    onClose();
  };

  const save = async () => {
    if (!media) {
      if (typeof payload === "string" || !payload) return toast(String(payload), "error");
      return finishSave(payload, format === "json" ? "application/json" : "application/zip+dotlottie");
    }
    const abort = new AbortController();
    const opts: MediaOptions = { format, scale: effScale, fps: effFps, start, end, transparent, background, matte, quality };
    setBusy({ progress: { done: 0, total: frameCount, stage: "Rendering" }, abort });
    try {
      const bytes = await exportMedia(opts, (progress) => setBusy((b) => (b ? { ...b, progress } : b)), abort.signal);
      finishSave(bytes, MEDIA_MIME[format]);
    } catch (e) {
      setBusy(null);
      if (!(e instanceof ExportCancelled)) toast(`Export failed: ${e instanceof Error ? e.message : e}`, "error");
    }
  };

  const pct = busy ? Math.round((busy.progress.done / Math.max(1, busy.progress.total)) * 100) : 0;

  const disabled = !!busy;
  const seconds = ((end - start) / info.fr).toFixed(1);

  return (
    <div className="modal-backdrop" onPointerDown={(e) => e.target === e.currentTarget && !busy && onClose()}>
      <div className="modal export-modal" role="dialog" aria-label="Export">
        <header className="modal-head">
          <span>Export</span>
          <button className="icon-btn" onClick={onClose} aria-label="Close" disabled={disabled}>
            <Icon name="close" />
          </button>
        </header>
        <div className="export-grid">
          <section className="export-stage-col">
            <div ref={stage} className={`export-stage ${media && !transparent ? "solid" : ""}`} style={media && !transparent ? { background } : undefined}>
              <canvas ref={canvas} />
            </div>
            <div className="export-facts num">
              <span>
                {media ? `${size.w}×${size.h}` : `${info.w}×${info.h}`}
              </span>
              <span>{media ? effFps : info.fr} fps</span>
              <span>{media ? `${frameCount} frames` : `${info.op - info.ip} frames`}</span>
              <span>{seconds}s</span>
            </div>
          </section>

          <section className="export-side">
            <div className="export-formats" role="radiogroup" aria-label="Format">
              {GROUPS.map((g) => (
                <div key={g} className="export-group">
                  <div className="export-group-label">{g}</div>
                  {FORMATS.filter((f) => f.group === g).map((f) => {
                    const unavailable = f.id === "webm" && webm === null;
                    const note = f.id === "webm" && webm && webm !== "checking" ? f.note.replace("VP9/VP8", webm.label) : f.note;
                    return (
                      <button
                        key={f.id}
                        role="radio"
                        aria-checked={format === f.id}
                        className={`export-format ${format === f.id ? "on" : ""}`}
                        disabled={unavailable || disabled}
                        onClick={() => setFormat(f.id)}
                        title={unavailable ? "No VP8/VP9 video encoder (WebCodecs) in this window" : undefined}
                      >
                        <span className="export-ext num">{f.ext}</span>
                        <span className="export-format-text">
                          <strong>{f.title}</strong>
                          <span>{unavailable ? "Needs a WebCodecs video encoder (Chrome, Edge, Windows app)" : note}</span>
                        </span>
                        <span className="export-radio" aria-hidden />
                      </button>
                    );
                  })}
                </div>
              ))}
            </div>

            {format === "json" && interactive && <div className="export-note warn">State machines and themes are only kept in dotLottie.</div>}

            {media && (
              <div className="export-settings">
                <label>
                  <span>Size</span>
                  <select value={effScale} onChange={(e) => setScale(Number(e.target.value))} disabled={disabled}>
                    {SCALES.map((sc) => {
                      const z = format === "webm" ? evenSize(exportSize(info.w, info.h, sc)) : exportSize(info.w, info.h, sc);
                      return (
                        <option key={sc} value={sc}>
                          {sc}× · {z.w}×{z.h}
                        </option>
                      );
                    })}
                  </select>
                </label>
                <label>
                  <span>Frame rate</span>
                  <select value={effFps} onChange={(e) => setFps(Number(e.target.value))} disabled={disabled}>
                    {FPS.filter((f) => format !== "gif" || f <= 50).map((f) => (
                      <option key={f} value={f}>
                        {f} fps{Math.round(info.fr) === f ? " · scene" : ""}
                      </option>
                    ))}
                  </select>
                </label>
                <label className="span2">
                  <span>Frames</span>
                  <select value={range} onChange={(e) => setRange(e.target.value)} disabled={disabled}>
                    <option value="all">Whole animation</option>
                    {info.markers.map((m) => (
                      <option key={m.name} value={m.name}>
                        Segment · {m.name}
                      </option>
                    ))}
                  </select>
                </label>
                <div className="span2 export-bg">
                  <span>Background</span>
                  <div className="bg-seg">
                    <button className={transparent ? "on" : ""} onClick={() => setTransparent(true)} disabled={disabled}>
                      <span className="swatch checker" /> Transparent
                    </button>
                    <button className={!transparent ? "on" : ""} onClick={() => setTransparent(false)} disabled={disabled}>
                      <span className="swatch" style={{ background }} /> Colour
                    </button>
                  </div>
                  {!transparent && <input type="color" value={background} onChange={(e) => setBackground(e.target.value)} aria-label="Background colour" disabled={disabled} />}
                </div>
                {format === "gif" && transparent && (
                  <div className="span2 export-bg">
                    <span>Edge matte</span>
                    <input type="color" value={matte} onChange={(e) => setMatte(e.target.value)} aria-label="Edge matte colour" disabled={disabled} />
                    <em>GIF pixels are clear or solid; soft edges blend into this colour. Match the page it will sit on.</em>
                  </div>
                )}
                {format === "webm" && (
                  <label className="span2">
                    <span>Quality</span>
                    <select value={quality} onChange={(e) => setQuality(Number(e.target.value))} disabled={disabled}>
                      {QUALITY.map((q) => (
                        <option key={q.label} value={q.bpp}>
                          {q.label}
                        </option>
                      ))}
                    </select>
                  </label>
                )}
              </div>
            )}
          </section>
        </div>

        <footer className="export-foot">
          <div className="export-file">
            <div className="export-name">
              <input value={name} onChange={(e) => setName(e.target.value)} disabled={disabled} aria-label="File name" spellCheck={false} />
              <span className="num">.{format}</span>
            </div>
            {busy ? (
              <div className="export-progress" role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100}>
                <div className="export-bar">
                  <span style={{ width: `${pct}%` }} />
                </div>
                <span>
                  {busy.progress.stage}
                  {busy.progress.total > 1 ? ` · ${busy.progress.done}/${busy.progress.total}` : "…"}
                </span>
              </div>
            ) : (
              <span className="export-summary">
                {media ? (transparent ? "Transparent background" : "Solid background") : typeof payload === "string" ? "Can't export this project" : formatBytes(payload?.byteLength ?? 0)}
              </span>
            )}
          </div>
          <div className="export-buttons">
            {busy ? (
              <button className="btn ghost" onClick={() => busy.abort.abort()}>
                Cancel export
              </button>
            ) : (
              <button className="btn ghost" onClick={onClose}>
                Cancel
              </button>
            )}
            <button className="btn primary" onClick={() => void save()} disabled={disabled || (format === "webm" && !webm)}>
              <Icon name="export" size={14} /> {media ? "Export" : "Save"}
            </button>
          </div>
        </footer>
      </div>
    </div>
  );
}
