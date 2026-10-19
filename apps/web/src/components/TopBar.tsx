import { commands } from "../commands";
import { useEditor, type Mode, type Tool } from "../store";
import { Icon } from "./Icon";
import { Menu } from "./Menu";

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
  const { tool, setTool, mode, setMode, fileName, zoom, setZoom, canUndo, canRedo, selection, snapping, setSnapping, grid, setGrid } = useEditor();

  return (
    <header className="topbar">
      <div className="wordmark">
        <svg className="disc" viewBox="0 0 20 20" aria-hidden>
          <circle cx="10" cy="10" r="7" fill="none" stroke="currentColor" strokeWidth="3.2" strokeDasharray="5.5 5.5" />
        </svg>
        Lumetrope
      </div>
      <Menu
        trigger={<span className="menu-word">File</span>}
        items={[
          { label: "New project", onSelect: commands.newProject },
          { label: "Open…", shortcut: "⌘O", onSelect: () => void commands.open() },
          { label: "Import SVG, Lottie, image or font…", onSelect: () => void commands.import() },
          { separator: true, label: "" },
          { label: "Save project", shortcut: "⌘S", onSelect: commands.saveProjectJson },
          { label: "Export Lottie JSON", shortcut: "⌘E", onSelect: commands.exportJson },
        ]}
      />
      <Menu
        trigger={<span className="menu-word">Edit</span>}
        items={[
          { label: "Undo", shortcut: "⌘Z", onSelect: commands.undo, disabled: !canUndo },
          { label: "Redo", shortcut: "⇧⌘Z", onSelect: commands.redo, disabled: !canRedo },
          { separator: true, label: "" },
          { label: "Duplicate", shortcut: "⌘D", onSelect: commands.duplicate, disabled: !selection.length },
          { label: "Delete", shortcut: "⌫", onSelect: commands.remove, disabled: !selection.length },
          { label: "Select all", shortcut: "⌘A", onSelect: commands.selectAll },
          { label: "Group into nested scene", shortcut: "⌘G", onSelect: commands.precompose, disabled: !selection.length },
        ]}
      />
      <Menu
        trigger={<span className="menu-word">View</span>}
        items={[
          { label: "Zoom to fit", shortcut: "⌘0", onSelect: commands.fit },
          { label: "Snapping", checked: snapping, onSelect: () => setSnapping(!snapping) },
          { label: "Grid", checked: grid, onSelect: () => setGrid(!grid) },
        ]}
      />
      <div className="sep" />
      <button className="icon-btn" data-tip="Undo (⌘Z)" disabled={!canUndo} onClick={commands.undo}>
        <Icon name="undo" />
      </button>
      <button className="icon-btn" data-tip="Redo (⇧⌘Z)" disabled={!canRedo} onClick={commands.redo}>
        <Icon name="redo" />
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
      <button className="btn primary" onClick={commands.exportJson}>
        <Icon name="export" size={14} /> Export
      </button>
    </header>
  );
}
