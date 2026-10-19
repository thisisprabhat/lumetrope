import { useEffect } from "react";
import { CanvasOverlay } from "./components/CanvasOverlay";
import { Stage } from "./components/Stage";
import { Toasts } from "./components/Toasts";
import { TopBar } from "./components/TopBar";
import { openUrl } from "./files";
import { LeftSidebar } from "./panels/LeftSidebar";
import { PropertiesPanel } from "./panels/PropertiesPanel";
import { Timeline } from "./panels/Timeline";
import { useShortcuts } from "./shortcuts";
import { useEditor } from "./store";
import { usePlayback } from "./usePlayback";

let openedFromUrl = false;

export function App() {
  const refresh = useEditor((s) => s.refresh);
  const mode = useEditor((s) => s.mode);
  useEffect(() => {
    refresh();
    const open = new URLSearchParams(location.search).get("open");
    if (open && open.startsWith("/") && !openedFromUrl) {
      openedFromUrl = true; // StrictMode runs effects twice in development
      void openUrl(open);
    }
  }, [refresh]);
  usePlayback();
  useShortcuts();
  return (
    <div className={`app mode-${mode}`}>
      <TopBar />
      <aside className="panel left">
        <LeftSidebar />
      </aside>
      <Stage overlay={mode === "animate" ? (zoom) => <CanvasOverlay zoom={zoom} /> : undefined} />
      <aside className="panel right">
        <PropertiesPanel />
      </aside>
      <section className="panel bottom">
        <Timeline />
      </section>
      <Toasts />
    </div>
  );
}
