import { useEffect } from "react";
import { CanvasOverlay } from "./components/CanvasOverlay";
import { ContextMenuHost } from "./components/ContextMenu";
import { ExportDialog } from "./components/ExportDialog";
import { McpDialog } from "./components/McpDialog";
import { Stage } from "./components/Stage";
import { Toasts } from "./components/Toasts";
import { TopBar } from "./components/TopBar";
import { openUrl } from "./files";
import { loadProject, persist, scheduleSave } from "./persist";
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

let openedFromUrl = false;

export function App() {
  const refresh = useEditor((s) => s.refresh);
  const mode = useEditor((s) => s.mode);
  const exportOpen = useEditor((s) => s.exportOpen);
  const mcpHelpOpen = useEditor((s) => s.mcpHelpOpen);
  const setExportOpen = useEditor((s) => s.setExportOpen);
  useEffect(() => {
    refresh();
    const open = new URLSearchParams(location.search).get("open");
    if (openedFromUrl) return;
    openedFromUrl = true; // StrictMode runs effects twice in development
    if (open && open.startsWith("/")) void openUrl(open);
    else
      void persist
        .all()
        .then((recs) => recs[0] && loadProject(recs[0].id))
        .catch(() => undefined);
  }, [refresh]);
  usePlayback();
  const revision = useEditor((s) => s.revision);
  const historyTick = useEditor((s) => s.historyTick);
  const fileName = useEditor((s) => s.fileName);
  useEffect(() => {
    if (openedFromUrl) scheduleSave();
  }, [revision, historyTick, fileName]);
  useSmPreview();
  useShortcuts();
  return (
    <div className={`app mode-${mode}`}>
      <TopBar />
      <aside className="panel left">{mode === "animate" ? <LeftSidebar /> : <MachinesPanel />}</aside>
      <Stage overlay={(zoom) => (mode === "animate" ? <CanvasOverlay zoom={zoom} /> : <SmPointerOverlay zoom={zoom} />)} />
      <aside className="panel right">{mode === "animate" ? <PropertiesPanel /> : mode === "state" ? <SmInspector /> : <PreviewPanel />}</aside>
      <section className="panel bottom">{mode === "animate" ? <Timeline /> : mode === "state" ? <SmGraph /> : <EventLog />}</section>
      <Toasts />
      <ContextMenuHost />
      {exportOpen && <ExportDialog onClose={() => setExportOpen(false)} />}
      {mcpHelpOpen && <McpDialog />}
    </div>
  );
}
