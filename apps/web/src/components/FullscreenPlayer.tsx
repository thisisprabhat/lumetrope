// Full-screen player: the main scene fitted to the screen with transport
// controls that fade while idle. "Interactive" runs a state machine so clicks
// and hovers drive the animation like in a real player.
import { useEffect, useMemo, useRef, useState } from "react";
import { engine, type Info } from "../engine";
import { CanvasRenderer } from "../gpu/CanvasRenderer";
import { fitRect } from "../lib/fit";
import { advance, formatTime } from "../lib/time";
import { isTauri } from "../platform";
import { useEditor } from "../store";
import { Icon } from "./Icon";

type Bg = "checker" | "dark" | "light" | "white";
const BGS: { id: Bg; label: string }[] = [
  { id: "checker", label: "Transparency grid" },
  { id: "dark", label: "Dark" },
  { id: "light", label: "Light grey" },
  { id: "white", label: "White" },
];

async function setNativeFullscreen(on: boolean, el: HTMLElement | null) {
  try {
    if (isTauri()) {
      const { getCurrentWindow } = await import("@tauri-apps/api/window");
      await getCurrentWindow().setFullscreen(on);
    } else if (on) await el?.requestFullscreen?.();
    else if (document.fullscreenElement) await document.exitFullscreen();
  } catch {
    /* fullscreen refused (no user gesture, policy): the overlay still fills the window */
  }
}

export function FullscreenPlayer() {
  const { scene, mode, fileName, project } = useEditor();
  const close = () => useEditor.setState({ fullscreen: false });
  // Always play the main scene; restore the editor's view and mode afterwards.
  const info = useMemo<Info>(() => {
    if (scene !== "root") engine().set_view_scene("root");
    return JSON.parse(engine().info());
  }, [scene]);
  const machines = useMemo(() => Object.entries(JSON.parse(engine().project_json()).stateMachines ?? {}).map(([id, m]: [string, any]) => ({ id, name: m?.lmeta?.name || id })), []);
  const prevMode = useRef(mode);

  const root = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLCanvasElement>(null);
  const [playing, setPlaying] = useState(true);
  const [loop, setLoop] = useState(true);
  const [speed, setSpeed] = useState(1);
  const [bg, setBg] = useState<Bg>("checker");
  const [machine, setMachine] = useState<string | null>(null);
  const [frame, setFrame] = useState(info.ip);
  const [smState, setSmState] = useState<string | null>(null);
  const [idle, setIdle] = useState(false);
  const live = useRef({ frame: info.ip, playing: true, loop: true, speed: 1, machine: null as string | null, layout: { scale: 1, x: 0, y: 0 } });
  live.current.playing = playing;
  live.current.loop = loop;
  live.current.speed = speed;
  live.current.machine = machine;

  // Enter native full screen; leaving it (Esc in the browser) closes the player.
  useEffect(() => {
    if (prevMode.current !== "animate") useEditor.setState({ mode: "animate" });
    void setNativeFullscreen(true, root.current);
    const onChange = () => {
      if (!isTauri() && !document.fullscreenElement) close();
    };
    document.addEventListener("fullscreenchange", onChange);
    return () => {
      document.removeEventListener("fullscreenchange", onChange);
      void setNativeFullscreen(false, root.current);
      engine().sm_stop();
      if (scene !== "root") engine().set_view_scene(scene);
      useEditor.setState({ mode: prevMode.current });
    };
  }, []);

  // State machine runtime for interactive playback.
  useEffect(() => {
    if (!machine) {
      engine().sm_stop();
      setSmState(null);
      return;
    }
    try {
      const s = JSON.parse(engine().sm_start(machine));
      live.current.frame = s.frame;
      setSmState(s.state);
    } catch (e) {
      useEditor.getState().toast(String(e), "error");
      setMachine(null);
    }
    return () => engine().sm_stop();
  }, [machine]);

  // Render loop.
  useEffect(() => {
    const c = canvas.current!;
    const ctx = c.getContext("2d")!;
    const gpu = new CanvasRenderer();
    gpu.setImages(JSON.parse(engine().image_assets()));
    let raf = 0;
    let last = performance.now();
    let shown = -1;
    let lastUi = 0;
    const draw = (now: number) => {
      const dt = Math.min(0.1, (now - last) / 1000);
      last = now;
      const L = live.current;
      const dpr = devicePixelRatio || 1;
      const W = Math.round(c.clientWidth * dpr);
      const H = Math.round(c.clientHeight * dpr);
      if (c.width !== W || c.height !== H) {
        c.width = W;
        c.height = H;
        shown = -1;
      }
      const fit = fitRect(info, { w: c.clientWidth, h: c.clientHeight }, Math.min(64, c.clientWidth * 0.05));
      L.layout = fit;
      if (L.machine) {
        const s = JSON.parse(engine().sm_tick(dt * L.speed));
        if (s) {
          L.frame = s.frame;
          if (now - lastUi > 120) setSmState(s.state);
        }
      } else if (L.playing) {
        const r = advance(L.frame, dt * info.fr * L.speed, info.ip, info.op, L.loop);
        L.frame = r.frame;
        if (r.ended) setPlaying(false);
      }
      if (L.frame !== shown) {
        ctx.clearRect(0, 0, W, H);
        gpu.render(ctx, JSON.parse(engine().display_list(L.frame, fit.scale * dpr, fit.x * dpr, fit.y * dpr)), W, H, [0, 0, W, H]);
        shown = L.frame;
      }
      if (now - lastUi > 120) {
        lastUi = now;
        setFrame(L.frame);
      }
      raf = requestAnimationFrame(draw);
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  }, [info]);

  // Controls fade after a moment without pointer movement.
  const idleTimer = useRef(0);
  const wake = () => {
    setIdle(false);
    clearTimeout(idleTimer.current);
    idleTimer.current = window.setTimeout(() => setIdle(true), 2500);
  };
  useEffect(() => {
    wake();
    return () => clearTimeout(idleTimer.current);
  }, []);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement).tagName === "SELECT") return;
      wake();
      if (e.key === "Escape" || e.key.toLowerCase() === "f") return (e.preventDefault(), close());
      if (e.key === " ") return (e.preventDefault(), setPlaying((p) => !p));
      if (e.key === "ArrowLeft" || e.key === "ArrowRight") {
        e.preventDefault();
        setPlaying(false);
        live.current.frame = Math.min(info.op - 1, Math.max(info.ip, Math.round(live.current.frame) + (e.key === "ArrowLeft" ? -1 : 1)));
      }
    };
    window.addEventListener("keydown", onKey, true);
    return () => window.removeEventListener("keydown", onKey, true);
  }, [info]);

  const toComp = (e: React.PointerEvent): [number, number] => {
    const { scale, x, y } = live.current.layout;
    const r = canvas.current!.getBoundingClientRect();
    return [(e.clientX - r.left - x) / scale, (e.clientY - r.top - y) / scale];
  };
  const pointer = (kind: string) => (e: React.PointerEvent) => {
    wake();
    if (!machine) return;
    try {
      const s = JSON.parse(engine().sm_pointer(kind, ...toComp(e)));
      if (s) setSmState(s.state);
    } catch {
      /* machine stopped */
    }
  };
  const down = useRef(false);

  return (
    <div ref={root} className={`fs-player bg-${bg} ${idle ? "idle" : ""}`} onPointerMove={wake} role="dialog" aria-label="Full-screen player">
      <canvas
        ref={canvas}
        className={machine ? "interactive" : ""}
        onPointerMove={pointer("move")}
        onPointerEnter={pointer("enter")}
        onPointerLeave={pointer("exit")}
        onPointerDown={(e) => {
          down.current = true;
          pointer("down")(e);
        }}
        onPointerUp={(e) => {
          pointer("up")(e);
          if (down.current) pointer("click")(e);
          down.current = false;
        }}
        onDoubleClick={() => !machine && setPlaying((p) => !p)}
      />

      <div className="fs-top">
        <span className="fs-title">{project?.name || fileName || "Untitled"}</span>
        <span className="fs-dims num">
          {info.w}×{info.h} · {info.fr} fps
        </span>
        <button className="fs-close" onClick={close} aria-label="Exit full screen" data-tip="Exit (Esc)">
          <Icon name="close" />
        </button>
      </div>

      <div className="fs-controls" onPointerDown={(e) => e.stopPropagation()}>
        {machine ? (
          <span className="fs-state">
            <span className="dot" /> {smState ?? "—"}
          </span>
        ) : (
          <button className="fs-play" onClick={() => setPlaying(!playing)} aria-label={playing ? "Pause" : "Play"}>
            <Icon name={playing ? "pause" : "play"} fill />
          </button>
        )}
        <span className="num fs-time">{formatTime(frame - info.ip, info.fr)}</span>
        <input
          className="fs-scrub"
          type="range"
          min={info.ip}
          max={Math.max(info.ip, info.op - 1)}
          step={0.01}
          value={frame}
          disabled={!!machine}
          onChange={(e) => {
            setPlaying(false);
            live.current.frame = Number(e.target.value);
            setFrame(live.current.frame);
          }}
          aria-label="Position"
        />
        <span className="num fs-time faint">{formatTime(info.op - info.ip, info.fr)}</span>
        <button className={`fs-btn ${loop ? "on" : ""}`} onClick={() => setLoop(!loop)} aria-label="Loop" data-tip="Loop" disabled={!!machine}>
          <Icon name="loop" />
        </button>
        <select value={speed} onChange={(e) => setSpeed(Number(e.target.value))} aria-label="Speed">
          {[0.25, 0.5, 1, 1.5, 2].map((s) => (
            <option key={s} value={s}>
              {s}×
            </option>
          ))}
        </select>
        <div className="fs-bgs" role="radiogroup" aria-label="Background">
          {BGS.map((b) => (
            <button key={b.id} className={`fs-bg ${b.id} ${bg === b.id ? "on" : ""}`} onClick={() => setBg(b.id)} aria-label={b.label} aria-checked={bg === b.id} role="radio" data-tip={b.label} />
          ))}
        </div>
        {machines.length > 0 && (
          <select value={machine ?? ""} onChange={(e) => setMachine(e.target.value || null)} aria-label="Interactivity" className={machine ? "on" : ""}>
            <option value="">Timeline playback</option>
            {machines.map((m) => (
              <option key={m.id} value={m.id}>
                Interactive · {m.name}
              </option>
            ))}
          </select>
        )}
      </div>
    </div>
  );
}
