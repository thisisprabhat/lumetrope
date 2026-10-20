import { act } from "../actions";
import { engine, projectValue } from "../engine";
import { formatBytes } from "../lib/export";
import { useSceneInfo } from "../doc";
import { useEditor } from "../store";
import { ColorField, NumberField, Section, Select } from "../ui/controls";
import { useMemo } from "react";

const PRESETS = [
  { value: "", label: "Custom" },
  { value: "512x512", label: "512 × 512" },
  { value: "1080x1080", label: "1080 × 1080 · Square" },
  { value: "1920x1080", label: "1920 × 1080 · Landscape" },
  { value: "1080x1920", label: "1080 × 1920 · Portrait" },
  { value: "800x600", label: "800 × 600" },
  { value: "256x256", label: "256 × 256 · Icon" },
];

export function ScenePanel() {
  const s = useSceneInfo();
  const revision = useEditor((st) => st.revision);
  const active = useEditor((st) => st.info?.active ?? "main");
  const bg = useMemo(() => projectValue()?.settings?.background?.[active] ?? null, [revision, active]);
  const preset = PRESETS.find((p) => p.value === `${s.width}x${s.height}`)?.value ?? "";
  return (
    <>
      <Section title="Scene">
        <div className="prow">
          <span className="prow-label">Name</span>
          <input className="text-input" defaultValue={s.name} key={s.name} onBlur={(e) => e.target.value !== s.name && act("set_scene_settings", { name: e.target.value })} />
          <span />
        </div>
        <div className="prow">
          <span className="prow-label">Preset</span>
          <Select
            value={preset}
            options={PRESETS}
            onChange={(v) => {
              if (!v) return;
              const [w, h] = v.split("x").map(Number);
              act("set_scene_settings", { width: w, height: h });
            }}
          />
          <span />
        </div>
        <div className="prow">
          <span className="prow-label">Size</span>
          <div className="prow-fields">
            <NumberField label="W" value={s.width} min={1} precision={0} onChange={(v) => act("set_scene_settings", { width: v })} />
            <NumberField label="H" value={s.height} min={1} precision={0} onChange={(v) => act("set_scene_settings", { height: v })} />
          </div>
          <span />
        </div>
        <div className="prow">
          <span className="prow-label">Background</span>
          <div className="prow-fields">
            {bg ? (
              <ColorField value={bg} onChange={(c) => act("set_background", { color: c })} />
            ) : (
              <button className="chip-btn" onClick={() => act("set_background", { color: "#ffffff" })}>
                Transparent — add colour
              </button>
            )}
          </div>
          {bg ? (
            <button className="keybtn" title="Make transparent" onClick={() => act("set_background", { color: null })}>
              ✕
            </button>
          ) : (
            <span />
          )}
        </div>
      </Section>
      <Section title="Timing">
        <div className="prow">
          <span className="prow-label">Frame rate</span>
          <div className="prow-fields">
            <NumberField value={s.fps} min={1} max={120} precision={2} suffix="fps" onChange={(v) => act("set_scene_settings", { fps: v })} />
          </div>
          <span />
        </div>
        <div className="prow">
          <span className="prow-label">Duration</span>
          <div className="prow-fields">
            <NumberField value={s.op} min={1} precision={0} suffix="f" onChange={(v) => act("set_scene_settings", { duration_frames: v })} />
            <NumberField value={s.op / (s.fps || 30)} min={0.04} precision={2} suffix="s" step={0.05} onChange={(v) => act("set_scene_settings", { duration_frames: Math.max(1, Math.round(v * s.fps)) })} />
          </div>
          <span />
        </div>
      </Section>
      <Section title="File info" defaultOpen={false}>
        <FileInfo />
      </Section>
    </>
  );
}

function FileInfo() {
  const revision = useEditor((st) => st.revision);
  const s = useSceneInfo();
  const d = useMemo(() => {
    const p = projectValue();
    const a = p.animations[p.active] ?? {};
    const json = engine().export_lottie();
    return {
      size: json.length,
      version: a.v ?? "—",
      generator: a.meta?.g ?? "—",
      machines: Object.keys(p.stateMachines ?? {}).length,
      themes: Object.keys(p.themes ?? {}).length,
      images: (a.assets ?? []).filter((x: any) => !x.layers && x.p).length,
    };
  }, [revision]);
  const row = (k: string, v: string | number) => (
    <div className="info-row">
      <span className="muted">{k}</span>
      <span className="num">{v}</span>
    </div>
  );
  return (
    <>
      {row("Lottie JSON size", formatBytes(d.size))}
      {row("Bodymovin version", d.version)}
      {row("Generator", d.generator)}
      {row("Layers", s.layers.length)}
      {row("Nested scenes", Math.max(0, s.scenes.length - 1))}
      {row("Segments", s.segments.length)}
      {row("Images", d.images)}
      {row("State machines", d.machines)}
      {row("Themes", d.themes)}
    </>
  );
}
