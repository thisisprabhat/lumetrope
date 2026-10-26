import { useEffect, useState } from "react";
import { commands } from "../commands";
import { relativeTime } from "../lib/history";
import { library } from "../persist";
import { lastActivity, sortProjects } from "../library/activity";
import { useEditor, type Mode, type Tool } from "../store";
import { Icon } from "./Icon";
import { Menu } from "./Menu";
import { McpBadge } from "./McpDialog";
import { keyLabel } from "../lib/keys";
import { exportAllData, importBackupFile } from "../workspaces";

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
  const { tool, setTool, mode, setMode, fileName, camera, zoomStep, zoomTo, canUndo, canRedo, selection, snapping, setSnapping, grid, setGrid, wheelMode, setWheelMode, renderer, setRenderer } = useEditor();

  return (
    <header className="topbar">
      <button className="wordmark home-btn" onClick={commands.home} data-tip="All projects (Home)">
        <svg className="disc" viewBox="0 0 20 20" aria-hidden>
          <circle className="ring" cx="10" cy="10" r="7" fill="none" stroke="currentColor" strokeWidth="3.2" strokeDasharray="8.5 2.5" strokeDashoffset="-1.25" />
          <circle className="dot" cx="10" cy="10" r="1.8" fill="currentColor" />
        </svg>
        Lumetrope
      </button>
      <Menu
        trigger={<span className="menu-word">File</span>}
        items={[
          { label: "All projects (Home)", onSelect: commands.home },
          { separator: true, label: "" },
          { label: "New project", shortcut: "⌘N", onSelect: commands.newProject },
          { label: "Open…", shortcut: "⌘O", onSelect: () => void commands.open() },
          { label: "Import SVG, Lottie, image or font…", onSelect: () => void commands.import() },
          { separator: true, label: "" },
          { label: "Save", shortcut: "⌘S", onSelect: commands.save },
          { label: "Save as…", shortcut: "⇧⌘S", onSelect: commands.saveAs },
          { label: "Save version", shortcut: "⌥⌘S", onSelect: commands.saveVersion },
          { separator: true, label: "" },
          { label: "Export…", shortcut: "⌘E", onSelect: commands.openExport },
          { label: "Quick export Lottie JSON", onSelect: commands.exportJson },
          { separator: true, label: "" },
          { label: "Export all data (.zip)…", onSelect: () => void exportAllData() },
          { label: "Import backup (.zip)…", onSelect: () => void importBackupFile() },
        ]}
      />
      <RecentMenu />
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
          { label: "Play full screen", shortcut: "F", onSelect: () => useEditor.setState({ fullscreen: true }) },
          { separator: true, label: "" },
          { label: "Zoom to fit", shortcut: "⌘0", onSelect: commands.fit },
          { label: "Snapping", checked: snapping, onSelect: () => setSnapping(!snapping) },
          { label: "Grid", checked: grid, onSelect: () => setGrid(!grid) },
          { separator: true, label: "" },
          { label: "Mouse wheel: auto-detect", checked: wheelMode === "auto", onSelect: () => setWheelMode("auto") },
          { label: "Mouse wheel zooms", checked: wheelMode === "zoom", onSelect: () => setWheelMode("zoom") },
          { label: "Mouse wheel scrolls", checked: wheelMode === "scroll", onSelect: () => setWheelMode("scroll") },
          { separator: true, label: "" },
          { label: "Renderer: GPU (Canvas2D)", checked: renderer === "gpu", onSelect: () => setRenderer("gpu") },
          { label: "Renderer: CPU (tiny-skia)", checked: renderer === "cpu", onSelect: () => setRenderer("cpu") },
        ]}
      />
      <div className="sep" />
      <button className="icon-btn" data-tip={`Undo (${keyLabel("⌘Z")})`} disabled={!canUndo} onClick={commands.undo}>
        <Icon name="undo" />
      </button>
      <button className="icon-btn" data-tip={`Redo (${keyLabel("⇧⌘Z")})`} disabled={!canRedo} onClick={commands.redo}>
        <Icon name="redo" />
      </button>
      <div className="sep" />
      {TOOLS.map((t) => (
        <button key={t.id} className={`icon-btn ${tool === t.id ? "active" : ""}`} data-tip={t.tip} onClick={() => setTool(t.id)} disabled={mode !== "animate" && t.id !== "hand"}>
          <Icon name={t.icon} />
        </button>
      ))}
      <div className="sep" />
      <button className="icon-btn" data-tip={`Zoom out (${keyLabel("⌘−")})`} onClick={() => zoomStep(-1)}>
        <Icon name="zoomOut" />
      </button>
      <Menu
        trigger={<span className="num zoom-readout">{camera.zoom < 0.1 ? (camera.zoom * 100).toFixed(1) : Math.round(camera.zoom * 100)}%</span>}
        items={[
          { label: "Zoom in", shortcut: "⌘+", onSelect: () => zoomStep(1) },
          { label: "Zoom out", shortcut: "⌘−", onSelect: () => zoomStep(-1) },
          { label: "Zoom to fit", shortcut: "⇧1", onSelect: commands.fit },
          { label: "Zoom to selection", shortcut: "⇧2", onSelect: commands.zoomToSelection, disabled: !selection.length },
          { separator: true, label: "" },
          ...[0.5, 1, 2, 4, 8].map((z) => ({ label: `${z * 100}%`, shortcut: z === 1 ? "⇧0" : undefined, onSelect: () => zoomTo(z), checked: Math.abs(camera.zoom - z) < 1e-3 })),
        ]}
      />
      <button className="icon-btn" data-tip={`Zoom in (${keyLabel("⌘+")})`} onClick={() => zoomStep(1)}>
        <Icon name="zoomIn" />
      </button>
      <button className="icon-btn" data-tip={`Zoom to fit (${keyLabel("⇧1")})`} onClick={commands.fit}>
        <Icon name="fit" />
      </button>
      <div className="spacer" />
      <span className="filename">{fileName}</span>
      <McpBadge />
      <div className="spacer" />
      <div className="mode-switch" role="tablist">
        {MODES.map((m) => (
          <button key={m.id} role="tab" aria-selected={mode === m.id} className={`${m.id} ${mode === m.id ? "on" : ""}`} onClick={() => setMode(m.id)}>
            {m.label}
          </button>
        ))}
      </div>
      <div className="sep" />
      <button className="btn primary" onClick={commands.openExport}>
        <Icon name="export" size={14} /> Export
      </button>
    </header>
  );
}

function RecentMenu() {
  const [items, setItems] = useState<{ id: string; name: string; at: number }[]>([]);
  const savedAt = useEditor((s) => s.savedAt);
  useEffect(() => {
    library
      .list()
      .then((r) => setItems(sortProjects(r, "recent").slice(0, 12).map((m) => ({ id: m.id, name: m.name, at: lastActivity(m).at }))))
      .catch(() => setItems([]));
  }, [savedAt]);
  return (
    <Menu
      trigger={<span className="menu-word">Recent</span>}
      items={
        items.length
          ? [
              ...items.map((r) => ({ label: `${r.name} · ${relativeTime(r.at)}`, onSelect: () => commands.openRecent(r.id) })),
              { separator: true, label: "" },
              { label: "Show all projects…", onSelect: commands.home },
            ]
          : [{ label: "No saved projects yet", disabled: true }]
      }
    />
  );
}
