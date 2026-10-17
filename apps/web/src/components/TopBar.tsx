import { engine } from "../engine";
import { download, openFile, pickFile } from "../files";
import { useEditor, type Mode, type Tool } from "../store";
import { Icon } from "./Icon";

const TOOLS: { id: Tool; icon: string; tip: string }[] = [
  { id: "select", icon: "select", tip: "Select (V)" },
  { id: "rect", icon: "rect", tip: "Rectangle (R)" },
  { id: "ellipse", icon: "ellipse", tip: "Ellipse (O)" },
  { id: "polygon", icon: "polygon", tip: "Polygon" },
  { id: "star", icon: "star", tip: "Star" },
  { id: "pen", icon: "pen", tip: "Pen (P)" },
  { id: "text", icon: "text", tip: "Text (T)" },
  { id: "hand", icon: "hand", tip: "Hand (H)" },
];

const MODES: { id: Mode; label: string }[] = [
  { id: "animate", label: "Animate" },
  { id: "state", label: "State Machine" },
  { id: "preview", label: "Preview" },
];

export function TopBar() {
  const { tool, setTool, mode, setMode, fileName, zoom, setZoom, toast } = useEditor();

  const open = async () => {
    const f = await pickFile();
    if (f) await openFile(f);
  };

  const exportJson = () => {
    download(`${fileName || "animation"}.json`, engine().export_lottie(), "application/json");
    toast("Exported Lottie JSON");
  };

  return (
    <header className="topbar">
      <div className="wordmark">
        <svg className="disc" viewBox="0 0 20 20" aria-hidden>
          <circle cx="10" cy="10" r="7" fill="none" stroke="currentColor" strokeWidth="3.2" strokeDasharray="5.5 5.5" />
        </svg>
        Lumetrope
      </div>
      <button className="icon-btn" data-tip="Open (⌘O)" onClick={open}>
        <Icon name="open" />
      </button>
      <div className="sep" />
      {TOOLS.map((t) => (
        <button key={t.id} className={`icon-btn ${tool === t.id ? "active" : ""}`} data-tip={t.tip} onClick={() => setTool(t.id)} disabled={mode !== "animate" && t.id !== "hand"}>
          <Icon name={t.icon} />
        </button>
      ))}
      <div className="sep" />
      <button className="icon-btn" data-tip="Zoom out" onClick={() => setZoom(zoom / 1.25)}>
        <Icon name="zoomOut" />
      </button>
      <span className="num" style={{ width: 44, textAlign: "center", color: "var(--text-1)" }}>
        {Math.round(zoom * 100)}%
      </span>
      <button className="icon-btn" data-tip="Zoom in" onClick={() => setZoom(zoom * 1.25)}>
        <Icon name="zoomIn" />
      </button>
      <button className="icon-btn" data-tip="Zoom to fit (⌘0)" onClick={() => window.dispatchEvent(new Event("lumetrope:fit"))}>
        <Icon name="fit" />
      </button>
      <div className="spacer" />
      <span className="filename">{fileName}</span>
      <div className="spacer" />
      <div className="mode-switch" role="tablist">
        {MODES.map((m) => (
          <button key={m.id} role="tab" aria-selected={mode === m.id} className={`${m.id} ${mode === m.id ? "on" : ""}`} onClick={() => setMode(m.id)}>
            {m.label}
          </button>
        ))}
      </div>
      <div className="sep" />
      <button className="btn primary" onClick={exportJson}>
        <Icon name="export" size={14} /> Export
      </button>
    </header>
  );
}
