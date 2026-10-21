import "@fontsource/instrument-sans/400.css";
import "@fontsource/instrument-sans/500.css";
import "@fontsource/instrument-sans/600.css";
import "@fontsource/instrument-serif/400-italic.css";
import "@fontsource/martian-mono/400.css";
import "./styles.css";
import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { App } from "./App";
import { initEngine } from "./engine";
import { useEditor } from "./store";
import { initMcp } from "./mcp/client";

initEngine().then((engine) => {
  if (import.meta.env.DEV) (window as any).__lumetrope = { engine, store: useEditor };
  initMcp();
  createRoot(document.getElementById("root")!).render(
    <StrictMode>
      <App />
    </StrictMode>,
  );
});
