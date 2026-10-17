import { useEffect } from "react";
import { Stage } from "./components/Stage";
import { Toasts } from "./components/Toasts";
import { TopBar } from "./components/TopBar";
import { openUrl } from "./files";
import { useEditor } from "./store";
import { usePlayback } from "./usePlayback";

let openedFromUrl = false;

export function App() {
  const refresh = useEditor((s) => s.refresh);
  useEffect(() => {
    refresh();
    const open = new URLSearchParams(location.search).get("open");
    if (open && open.startsWith("/") && !openedFromUrl) {
      openedFromUrl = true; // StrictMode runs effects twice in development
      void openUrl(open);
    }
  }, [refresh]);
  usePlayback();
  return (
    <div className="app">
      <TopBar />
      <aside className="panel left">
        <div className="panel-empty">Layers</div>
      </aside>
      <Stage />
      <aside className="panel right">
        <div className="panel-empty">Properties</div>
      </aside>
      <section className="panel bottom">
        <div className="panel-empty">Timeline</div>
      </section>
      <Toasts />
    </div>
  );
}
