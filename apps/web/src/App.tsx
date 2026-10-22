import { useEffect } from "react";
import { CanvasOverlay } from "./components/CanvasOverlay";
import { ContextMenuHost } from "./components/ContextMenu";
import { ExportDialog } from "./components/ExportDialog";
import { McpDialog } from "./components/McpDialog";
import { Stage } from "./components/Stage";
import { Toasts } from "./components/Toasts";
import { TopBar } from "./components/TopBar";
import { openUrl } from "./files";
import { markPreviewed, scheduleSave } from "./persist";
import { Dashboard } from "./components/Dashboard";
import { PaneSplitters } from "./components/PaneSplitters";
import { FullscreenPlayer } from "./components/FullscreenPlayer";
import { LeftSidebar } from "./panels/LeftSidebar";
import { PropertiesPanel } from "./panels/PropertiesPanel";
import { Timeline } from "./panels/Timeline";
import { useShortcuts } from "./shortcuts";
import { useEditor } from "./store";
import { SmGraph } from "./sm/SmGraph";
import { EventLog, MachinesPanel, PreviewPanel, SmInspector } from "./sm/SmPanels";
import { SmPointerOverlay } from "./sm/SmPointerOverlay";
import { useSmPreview } from "./sm/useSmPreview";
import { usePlayback } from "./usePlayback";

let started = false;

export function App() {
  const refresh = useEditor((s) => s.refresh);
  const mode = useEditor((s) => s.mode);
  const exportOpen = useEditor((s) => s.exportOpen);
  const mcpHelpOpen = useEditor((s) => s.mcpHelpOpen);
  const setExportOpen = useEditor((s) => s.setExportOpen);
  useEffect(() => {
    refresh();
    const open = new URLSearchParams(location.search).get("open");
    if (started) return;
    started = true; // StrictMode runs effects twice in development
    // Launch on the dashboard; ?open=/path deep-links straight into a file.
    if (open && open.startsWith("/")) void openUrl(open);
  }, [refresh]);
  usePlayback();
  const revision = useEditor((s) => s.revision);
  const historyTick = useEditor((s) => s.historyTick);
  const fileName = useEditor((s) => s.fileName);
  useEffect(() => {
    if (started) scheduleSave();
  }, [revision, historyTick, fileName]);
  useEffect(() => {
    if (mode === "preview") void markPreviewed();
  }, [mode]);
  const view = useEditor((s) => s.view);
  const panes = useEditor((s) => s.panes);
  const fullscreen = useEditor((s) => s.fullscreen);
  useSmPreview();
  useShortcuts();
  if (view === "home")
    return (
      <>
        <Dashboard />
        <Toasts />
        <ContextMenuHost />
        {mcpHelpOpen && <McpDialog />}
      </>
    );
  return (
    <div className={`app mode-${mode}`} style={{ "--left-w": `${panes.left}px`, "--right-w": `${panes.right}px`, "--timeline-h": `${panes.bottom}px` } as React.CSSProperties}>
      <TopBar />
      <aside className="panel left">{mode === "animate" ? <LeftSidebar /> : <MachinesPanel />}</aside>
      <Stage overlay={(zoom) => (mode === "animate" ? <CanvasOverlay zoom={zoom} /> : <SmPointerOverlay zoom={zoom} />)} />
      <aside className="panel right">{mode === "animate" ? <PropertiesPanel /> : mode === "state" ? <SmInspector /> : <PreviewPanel />}</aside>
      <section className="panel bottom">{mode === "animate" ? <Timeline /> : mode === "state" ? <SmGraph /> : <EventLog />}</section>
      <PaneSplitters />
      {fullscreen && <FullscreenPlayer />}
      <Toasts />
      <ContextMenuHost />
      {exportOpen && <ExportDialog onClose={() => setExportOpen(false)} />}
      {mcpHelpOpen && <McpDialog />}
    </div>
  );
}
