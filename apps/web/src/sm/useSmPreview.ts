import { useEffect } from "react";
import { engine } from "../engine";
import { useEditor, type SmSnapshot } from "../store";

/**
 * While in State Machine / Preview mode, run the selected machine in the
 * engine: restart it whenever the document changes, tick it every animation
 * frame and drive the canvas frame from its snapshot.
 */
export function useSmPreview(): void {
  const mode = useEditor((s) => s.mode);
  const machine = useEditor((s) => s.machine);
  const revision = useEditor((s) => s.revision);
  const active = mode !== "animate" && !!machine;
  useEffect(() => {
    const st = useEditor.getState();
    if (!active || !machine) {
      engine().sm_stop();
      st.setSmSnap(null);
      return;
    }
    let snap: SmSnapshot;
    try {
      snap = JSON.parse(engine().sm_start(machine));
    } catch (e) {
      st.toast(String(e), "error");
      st.setSmSnap(null);
      return;
    }
    st.clearSmLog();
    st.logSm(snap.events);
    st.setSmSnap(snap);
    st.setFrame(snap.frame);
    st.refresh();
    let raf = 0;
    let last = performance.now();
    const tick = (now: number) => {
      const dt = Math.min(0.1, (now - last) / 1000);
      last = now;
      const s = JSON.parse(engine().sm_tick(dt)) as SmSnapshot | null;
      if (s) {
        const cur = useEditor.getState();
        if (s.events.length) {
          cur.logSm(s.events);
          handleOutputs(s);
        }
        if (s.frame !== cur.frame || s.state !== cur.smSnap?.state || s.events.length) {
          cur.setSmSnap(s);
          useEditor.setState({ frame: s.frame });
        }
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(raf);
      engine().sm_stop();
    };
  }, [active, machine, revision]);
}

/** React to machine outputs that affect the host (themes re-render; URLs are surfaced, not followed). */
export function handleOutputs(s: SmSnapshot): void {
  const st = useEditor.getState();
  for (const e of s.events) {
    if (e.type === "SetTheme") useEditor.setState({ revision: st.revision }); // force canvas refresh
    if (e.type === "OpenUrl") st.toast(`OpenUrl → ${String(e.url)} (${String(e.target ?? "_blank")})`);
  }
}

/** Route a pointer event to the running machine. */
export function smPointer(kind: string, x: number, y: number): void {
  const st = useEditor.getState();
  if (!st.smSnap) return;
  const s = JSON.parse(engine().sm_pointer(kind, x, y)) as SmSnapshot | null;
  if (!s) return;
  if (s.events.length) {
    st.logSm(s.events);
    handleOutputs(s);
  }
  st.setSmSnap(s);
  useEditor.setState({ frame: s.frame });
}

export function smSetInput(name: string, value: unknown): void {
  const st = useEditor.getState();
  if (!st.smSnap) return;
  const s = JSON.parse(engine().sm_set_input(name, JSON.stringify(value ?? null))) as SmSnapshot | null;
  if (!s) return;
  st.logSm(s.events);
  handleOutputs(s);
  st.setSmSnap(s);
  useEditor.setState({ frame: s.frame });
}
