import { useEffect } from "react";
import { CanvasOverlay } from "./components/CanvasOverlay";
import { ContextMenuHost } from "./components/ContextMenu";
import { ExportDialog } from "./components/ExportDialog";
import { Stage } from "./components/Stage";
import { Toasts } from "./components/Toasts";
import { TopBar } from "./components/TopBar";
import { openUrl } from "./files";
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
  const setExportOpen = useEditor((s) => s.setExportOpen);
  useEffect(() => {
    refresh();
    const open = new URLSearchParams(location.search).get("open");
    if (open && open.startsWith("/") && !openedFromUrl) {
      openedFromUrl = true; // StrictMode runs effects twice in development
      void openUrl(open);
    }
  }, [refresh]);
  usePlayback();
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
    </div>
  );
}
