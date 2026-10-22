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

const FORMATS: { id: ExportFormat; title: string; note: string }[] = [
  { id: "lottie", title: "dotLottie", note: ".lottie · animations, state machines, themes, images and fonts" },
  { id: "json", title: "Lottie JSON", note: ".json · the main animation, for any Lottie player" },
  { id: "gif", title: "GIF", note: ".gif · plays everywhere · 1-bit transparency" },
  { id: "webp", title: "Animated WebP", note: ".webp · lossless, full alpha" },
  { id: "webm", title: "WebM video", note: ".webm · VP9/VP8 with alpha channel" },
];

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

  useEffect(() => {
    const c = canvas.current!;
    const ctx = c.getContext("2d")!;
    const s = Math.min(320 / info.w, 200 / info.h);
    const w = Math.max(1, Math.round(info.w * s * devicePixelRatio));
    const h = Math.max(1, Math.round(info.h * s * devicePixelRatio));
    c.width = w;
    c.height = h;
    c.style.width = `${w / devicePixelRatio}px`;
    c.style.height = `${h / devicePixelRatio}px`;
    let raf = 0;
    const t0 = performance.now();
    const gpu = new CanvasRenderer();
    gpu.setImages(JSON.parse(engine().image_assets()));
    const z = w / info.w;
    const len = Math.max(1, end - start);
    const loop = (now: number) => {
      // Preview at the export frame rate so choppiness is visible before exporting.
      const f = start + (Math.floor(((((now - t0) / 1000) * effFps) % ((len * effFps) / info.fr))) * info.fr) / effFps;
      gpu.render(ctx, JSON.parse(engine().display_list(f, z, 0, 0)), w, h, [0, 0, w, h]);
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  }, [info, start, end, effFps]);

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

  return (
    <div className="modal-backdrop" onPointerDown={(e) => e.target === e.currentTarget && !busy && onClose()}>
      <div className="modal wide export-modal" role="dialog" aria-label="Export">
        <header className="modal-head">
          <span>Export</span>
          <button className="icon-btn" onClick={onClose} aria-label="Close" disabled={!!busy}>
            <Icon name="close" />
          </button>
        </header>
        <div className={`export-preview ${media && !transparent ? "solid" : ""}`} style={media && !transparent ? { background } : undefined}>
          <canvas ref={canvas} />
        </div>
        <div className="modal-body">
          <div className="format-cards five">
            {FORMATS.map((f) => {
              const disabled = f.id === "webm" && webm === null;
              return (
                <button key={f.id} className={`format-card ${format === f.id ? "on" : ""}`} disabled={disabled || !!busy} onClick={() => setFormat(f.id)}>
                  <strong>{f.title}</strong>
                  <span>{f.id === "webm" && webm && webm !== "checking" ? `${f.note.replace("VP9/VP8", webm.label)}` : f.note}</span>
                </button>
              );
            })}
          </div>
          {webm === null && <div className="muted tiny">WebM needs a VP8/VP9 video encoder (WebCodecs), which this window doesn't provide. Chrome, Edge and the Windows app have one.</div>}
          {format === "json" && interactive && <div className="warn tiny">This project has state machines or themes; they are only kept in dotLottie.</div>}

          {media && (
            <div className="export-options">
              <label className="prow">
                <span className="prow-label">Size</span>
                <select className="text-input" value={effScale} onChange={(e) => setScale(Number(e.target.value))} disabled={!!busy}>
                  {SCALES.map((s) => {
                    const z = format === "webm" ? evenSize(exportSize(info.w, info.h, s)) : exportSize(info.w, info.h, s);
                    return (
                      <option key={s} value={s}>
                        {s}× · {z.w}×{z.h}
                      </option>
                    );
                  })}
                </select>
                <span />
              </label>
              <label className="prow">
                <span className="prow-label">Frame rate</span>
                <select className="text-input" value={effFps} onChange={(e) => setFps(Number(e.target.value))} disabled={!!busy}>
                  {FPS.filter((f) => format !== "gif" || f <= 50).map((f) => (
                    <option key={f} value={f}>
                      {f} fps{Math.round(info.fr) === f ? " (scene)" : ""}
                    </option>
                  ))}
                </select>
                <span />
              </label>
              <label className="prow">
                <span className="prow-label">Frames</span>
                <select className="text-input" value={range} onChange={(e) => setRange(e.target.value)} disabled={!!busy}>
                  <option value="all">Whole animation</option>
                  {info.markers.map((m) => (
                    <option key={m.name} value={m.name}>
                      Segment: {m.name}
                    </option>
                  ))}
                </select>
                <span />
              </label>
              <div className="prow">
                <span className="prow-label">Background</span>
                <div className="row">
                  <button className={`chip-btn ${transparent ? "on" : ""}`} onClick={() => setTransparent(true)} disabled={!!busy}>
                    Transparent
                  </button>
                  <button className={`chip-btn ${!transparent ? "on" : ""}`} onClick={() => setTransparent(false)} disabled={!!busy}>
                    Colour
                  </button>
                  {!transparent && <input type="color" value={background} onChange={(e) => setBackground(e.target.value)} aria-label="Background colour" disabled={!!busy} />}
                </div>
                <span />
              </div>
              {format === "gif" && transparent && (
                <div className="prow">
                  <span className="prow-label">Edge matte</span>
                  <div className="row">
                    <input type="color" value={matte} onChange={(e) => setMatte(e.target.value)} aria-label="Edge matte colour" disabled={!!busy} />
                    <span className="muted tiny">GIF pixels are either clear or solid; soft edges blend into this colour (match the page it will sit on).</span>
                  </div>
                  <span />
                </div>
              )}
              {format === "webm" && (
                <label className="prow">
                  <span className="prow-label">Quality</span>
                  <select className="text-input" value={quality} onChange={(e) => setQuality(Number(e.target.value))} disabled={!!busy}>
                    {QUALITY.map((q) => (
                      <option key={q.label} value={q.bpp}>
                        {q.label}
                      </option>
                    ))}
                  </select>
                  <span />
                </label>
              )}
            </div>
          )}

          <div className="prow">
            <span className="prow-label">File name</span>
            <input className="text-input" value={name} onChange={(e) => setName(e.target.value)} disabled={!!busy} />
            <span />
          </div>
          <div className="muted tiny">
            {exportFileName(name, format)} ·{" "}
            {media
              ? `${size.w}×${size.h} · ${effFps} fps · ${frameCount} frames · ${transparent ? "transparent" : "solid background"}`
              : `${typeof payload === "string" ? "error" : formatBytes(payload?.byteLength ?? 0)} · ${info.w}×${info.h} · ${info.fr} fps · ${info.op - info.ip} frames`}
          </div>
          {busy && (
            <div className="export-progress" role="progressbar" aria-valuenow={pct} aria-valuemin={0} aria-valuemax={100}>
              <div className="export-bar">
                <span style={{ width: `${pct}%` }} />
              </div>
              <span className="muted tiny">
                {busy.progress.stage}
                {busy.progress.total > 1 ? ` · ${busy.progress.done}/${busy.progress.total}` : "…"}
              </span>
            </div>
          )}
        </div>
        <footer className="modal-foot">
          {busy ? (
            <button className="btn ghost" onClick={() => busy.abort.abort()}>
              Cancel export
            </button>
          ) : (
            <button className="btn ghost" onClick={onClose}>
              Cancel
            </button>
          )}
          <button className="btn primary" onClick={() => void save()} disabled={!!busy || (format === "webm" && !webm)}>
            <Icon name="export" size={14} /> {media ? "Export" : "Download"}
          </button>
        </footer>
      </div>
    </div>
  );
}
