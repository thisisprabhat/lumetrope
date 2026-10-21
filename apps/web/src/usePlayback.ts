import { useEffect } from "react";
import { useEditor } from "./store";
import { advance } from "./lib/time";

/** Drive the playhead with requestAnimationFrame while playing. */
export function usePlayback(): void {
  const playing = useEditor((s) => s.playing);
  const mode = useEditor((s) => s.mode);
  useEffect(() => {
    if (!playing || mode !== "animate") return;
    let raf = 0;
    let last = performance.now();
    let exact = useEditor.getState().frame;
    const tick = (now: number) => {
      const st = useEditor.getState();
      const info = st.info;
      if (!info) return;
      if (Math.floor(exact) !== st.frame) exact = st.frame; // user scrubbed
      const delta = ((now - last) / 1000) * info.fr * st.speed;
      last = now;
      const r = advance(exact, delta, info.ip, info.op, st.loop);
      exact = r.frame;
      st.setFrame(r.frame);
      if (r.ended) st.setPlaying(false);
      else raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [playing, mode]);
}
