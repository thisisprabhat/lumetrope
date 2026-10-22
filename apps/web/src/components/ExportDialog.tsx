import { useEffect, useMemo, useRef, useState } from "react";
import { engine, projectValue } from "../engine";
import { download } from "../files";
import { exportFileName, formatBytes, type ExportFormat } from "../lib/export";
import { useEditor } from "../store";
import { Icon } from "./Icon";
import { CanvasRenderer } from "../gpu/CanvasRenderer";
import { markExported } from "../persist";

/** Export with live preview: Lottie JSON (active animation) or dotLottie (whole project). */
export function ExportDialog({ onClose }: { onClose: () => void }) {
  const { fileName, info, toast } = useEditor();
  const project = useMemo(() => projectValue(), []);
  const interactive = Object.keys(project.stateMachines ?? {}).length > 0 || Object.keys(project.themes ?? {}).length > 0;
  const [format, setFormat] = useState<ExportFormat>(interactive ? "lottie" : "json");
  const [name, setName] = useState(fileName || "animation");
  const canvas = useRef<HTMLCanvasElement>(null);
  const payload = useMemo(() => {
    try {
      return format === "json" ? new TextEncoder().encode(engine().export_lottie()) : engine().export_dotlottie();
    } catch (e) {
      return String(e);
    }
  }, [format]);

  useEffect(() => {
    const esc = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", esc);
    return () => window.removeEventListener("keydown", esc);
  }, [onClose]);

  useEffect(() => {
    if (!info) return;
    const c = canvas.current!;
    const ctx = c.getContext("2d")!;
    const scale = Math.min(320 / info.w, 200 / info.h);
    const w = Math.max(1, Math.round(info.w * scale * devicePixelRatio));
    const h = Math.max(1, Math.round(info.h * scale * devicePixelRatio));
    c.width = w;
    c.height = h;
    c.style.width = `${w / devicePixelRatio}px`;
    c.style.height = `${h / devicePixelRatio}px`;
    let raf = 0;
    const t0 = performance.now();
    const gpu = new CanvasRenderer();
    gpu.setImages(JSON.parse(engine().image_assets()));
    const z = w / info.w;
    const loop = (now: number) => {
      const f = info.ip + ((((now - t0) / 1000) * info.fr) % Math.max(1, info.op - info.ip));
      gpu.render(ctx, JSON.parse(engine().display_list(f, z, 0, 0)), w, h, [0, 0, w, h]);
      raf = requestAnimationFrame(loop);
    };
    raf = requestAnimationFrame(loop);
    return () => cancelAnimationFrame(raf);
  }, [info]);

  const save = () => {
    if (typeof payload === "string") return toast(payload, "error");
    download(exportFileName(name, format), payload, format === "json" ? "application/json" : "application/zip+dotlottie");
    toast(`Exported ${exportFileName(name, format)}`);
    void markExported(format);
    onClose();
  };

  return (
    <div className="modal-backdrop" onPointerDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal" role="dialog" aria-label="Export">
        <header className="modal-head">
          <span>Export</span>
          <button className="icon-btn" onClick={onClose} aria-label="Close">
            <Icon name="close" />
          </button>
        </header>
        <div className="export-preview">
          <canvas ref={canvas} />
        </div>
        <div className="modal-body">
          <div className="format-cards">
            <button className={`format-card ${format === "lottie" ? "on" : ""}`} onClick={() => setFormat("lottie")}>
              <strong>dotLottie</strong>
              <span>.lottie · animations, state machines, themes, images and fonts in one compressed file</span>
            </button>
            <button className={`format-card ${format === "json" ? "on" : ""}`} onClick={() => setFormat("json")}>
              <strong>Lottie JSON</strong>
              <span>.json · the active animation only, for any Lottie player</span>
            </button>
          </div>
          {format === "json" && interactive && <div className="warn tiny">This project has state machines or themes — they are only kept in dotLottie.</div>}
          <div className="prow">
            <span className="prow-label">File name</span>
            <input className="text-input" value={name} onChange={(e) => setName(e.target.value)} />
            <span />
          </div>
          <div className="muted tiny">
            {exportFileName(name, format)} · {typeof payload === "string" ? "error" : formatBytes(payload.byteLength)} · {info?.w}×{info?.h} · {info?.fr} fps · {info ? info.op - info.ip : 0} frames
          </div>
        </div>
        <footer className="modal-foot">
          <button className="btn ghost" onClick={onClose}>
            Cancel
          </button>
          <button className="btn primary" onClick={save}>
            <Icon name="export" size={14} /> Download
          </button>
        </footer>
      </div>
    </div>
  );
}
