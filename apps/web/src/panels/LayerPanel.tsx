import { act } from "../actions";
import { useLayerJson, useSceneInfo, type LayerNode } from "../doc";
import { useEditor } from "../store";
import { Icon } from "../components/Icon";
import { NumberField, PropRow, Section, Select, Toggle } from "../ui/controls";
import { keyLabel } from "../lib/keys";
import "./alignbar.css";

const BLEND = ["normal", "multiply", "screen", "overlay", "darken", "lighten", "color_dodge", "color_burn", "hard_light", "soft_light", "difference", "exclusion", "hue", "saturation", "color", "luminosity", "add"];
const ANCHORS = ["top_left", "top", "top_right", "left", "center", "right", "bottom_left", "bottom", "bottom_right"];
const label = (s: string) => s.replace(/_/g, " ").replace(/^\w/, (c) => c.toUpperCase());

type AlignIconName = "left" | "center" | "right" | "top" | "middle" | "bottom" | "horizontal" | "vertical";

/** Align/distribute glyphs: the reference edge in amber, the shapes solid. */
function AlignIcon({ name }: { name: AlignIconName }) {
  const edge = (x1: number, y1: number, x2: number, y2: number) => <line x1={x1} y1={y1} x2={x2} y2={y2} className="ai-edge" />;
  const box = (x: number, y: number, w: number, h: number) => <rect x={x} y={y} width={w} height={h} rx={0.9} className="ai-box" />;
  const parts: Record<AlignIconName, JSX.Element> = {
    left: <>{edge(2.5, 2, 2.5, 14)}{box(4.5, 4, 8.5, 3)}{box(4.5, 9, 5, 3)}</>,
    center: <>{edge(8, 1.5, 8, 14.5)}{box(3, 4, 10, 3)}{box(5, 9, 6, 3)}</>,
    right: <>{edge(13.5, 2, 13.5, 14)}{box(3, 4, 8.5, 3)}{box(6.5, 9, 5, 3)}</>,
    top: <>{edge(2, 2.5, 14, 2.5)}{box(4, 4.5, 3, 8.5)}{box(9, 4.5, 3, 5)}</>,
    middle: <>{edge(1.5, 8, 14.5, 8)}{box(4, 3, 3, 10)}{box(9, 5, 3, 6)}</>,
    bottom: <>{edge(2, 13.5, 14, 13.5)}{box(4, 3, 3, 8.5)}{box(9, 6.5, 3, 5)}</>,
    horizontal: <>{edge(2.5, 2, 2.5, 14)}{edge(13.5, 2, 13.5, 14)}{box(6.5, 4, 3, 8)}</>,
    vertical: <>{edge(2, 2.5, 14, 2.5)}{edge(2, 13.5, 14, 13.5)}{box(4, 6.5, 8, 3)}</>,
  };
  return (
    <svg width="16" height="16" viewBox="0 0 16 16" aria-hidden>
      {parts[name]}
    </svg>
  );
}

export function AlignBar({ ids }: { ids: string[] }) {
  const to = ids.length > 1 ? "selection" : "scene";
  const target = ids.length > 1 ? "selection" : "canvas";
  const btn = (align: AlignIconName, label: string) => (
    <button className="align-btn" data-tip={`Align ${label} (to ${target})`} aria-label={`Align ${label}`} onClick={() => act("align_layers", { layers: ids, align, to })}>
      <AlignIcon name={align} />
    </button>
  );
  const canDistribute = ids.length >= 3;
  const dist = (axis: "horizontal" | "vertical") => (
    <button
      className="align-btn"
      data-tip={canDistribute ? `Distribute ${axis}ly` : "Select 3 or more layers to distribute"}
      aria-label={`Distribute ${axis}ly`}
      disabled={!canDistribute}
      onClick={() => act("distribute_layers", { layers: ids, axis })}
    >
      <AlignIcon name={axis} />
    </button>
  );
  return (
    <div className="alignbar">
      <div className="align-group" role="group" aria-label="Align">
        {btn("left", "left")}
        {btn("center", "horizontal centres")}
        {btn("right", "right")}
        <span className="align-div" />
        {btn("top", "top")}
        {btn("middle", "vertical centres")}
        {btn("bottom", "bottom")}
      </div>
      <div className="align-group" role="group" aria-label="Distribute">
        {dist("horizontal")}
        {dist("vertical")}
      </div>
    </div>
  );
}

export function LayerHeader({ layer }: { layer: LayerNode }) {
  const select = useEditor((s) => s.select);
  return (
    <div className="layer-header">
      <span className={`type-badge t-${layer.type}`}>{layer.type}</span>
      <input className="text-input layer-name" key={layer.id + layer.name} defaultValue={layer.name} onBlur={(e) => e.target.value !== layer.name && act("rename_layer", { layer: layer.id, name: e.target.value })} />
      <button className="icon-btn" data-tip={`Duplicate (${keyLabel("⌘D")})`} onClick={() => {
        const r = act<{ id: string }>("duplicate_layer", { layer: layer.id });
        if (r) select([r.id]);
      }}>
        <Icon name="copy" />
      </button>
      <button className="icon-btn" data-tip="Delete (⌫)" onClick={() => {
        act("delete_layer", { layer: layer.id });
        select([]);
      }}>
        <Icon name="trash" />
      </button>
    </div>
  );
}

export function TransformSection({ id }: { id: string }) {
  return (
    <Section title="Transform">
      <PropRow layer={id} property="position" label="Position" dims={2} />
      <PropRow layer={id} property="scale" label="Scale" dims={2} suffix="%" />
      <PropRow layer={id} property="rotation" label="Rotation" suffix="°" />
      <PropRow layer={id} property="opacity" label="Opacity" min={0} max={100} suffix="%" />
      <PropRow layer={id} property="skew" label="Skew" suffix="°" />
      <PropRow layer={id} property="skew_axis" label="Skew axis" suffix="°" />
      <PropRow layer={id} property="anchor" label="Anchor" dims={2} />
      <div className="prow">
        <span className="prow-label">Anchor to</span>
        <div className="anchor-grid">
          {ANCHORS.map((a) => (
            <button key={a} title={label(a)} onClick={() => act("set_anchor_point", { layer: id, anchor: a })} className={a === "center" ? "center" : ""} />
          ))}
        </div>
        <span />
      </div>
    </Section>
  );
}

export function LayerSettingsSection({ layer }: { layer: LayerNode }) {
  const scene = useSceneInfo();
  const json = useLayerJson(layer.id);
  const others = scene.layers.filter((l) => l.id !== layer.id);
  const bm = BLEND[json?.bm ?? 0] ?? "normal";
  const tp = json?.tp;
  const matteLayer = tp !== undefined ? others.find((l) => l.id.endsWith(`:${tp}`)) : undefined;
  return (
    <Section title="Layer">
      <div className="prow">
        <span className="prow-label">Blend</span>
        <Select value={bm} options={BLEND.map((b) => ({ value: b, label: label(b) }))} onChange={(mode) => act("set_blend_mode", { layer: layer.id, mode })} />
        <span />
      </div>
      <div className="prow">
        <span className="prow-label">Parent</span>
        <Select
          value={layer.parent ?? ""}
          options={[{ value: "", label: "None" }, ...others.map((l) => ({ value: l.id, label: l.name || l.id }))]}
          onChange={(p) => act("set_parent", { layer: layer.id, parent: p || null })}
        />
        <span />
      </div>
      <div className="prow">
        <span className="prow-label">Track matte</span>
        <div className="prow-fields">
          <Select
            value={matteLayer?.id ?? ""}
            options={[{ value: "", label: "None" }, ...others.map((l) => ({ value: l.id, label: l.name || l.id }))]}
            onChange={(m) => (m ? act("set_matte", { layer: layer.id, matte_layer: m, mode: layer.matte_mode ?? "alpha" }) : act("remove_matte", { layer: layer.id }))}
          />
          {matteLayer && (
            <Select
              value={layer.matte_mode ?? "alpha"}
              options={["alpha", "alpha_inverted", "luma", "luma_inverted"].map((m) => ({ value: m, label: label(m) }))}
              onChange={(mode) => act("set_matte", { layer: layer.id, matte_layer: matteLayer.id, mode })}
            />
          )}
        </div>
        <span />
      </div>
      <div className="prow">
        <span className="prow-label">In / Out</span>
        <div className="prow-fields">
          <NumberField label="In" value={layer.in} precision={0} onChange={(v) => act("set_layer_time", { layer: layer.id, in: v })} />
          <NumberField label="Out" value={layer.out} precision={0} onChange={(v) => act("set_layer_time", { layer: layer.id, out: v })} />
        </div>
        <span />
      </div>
      <div className="prow">
        <span className="prow-label">Start offset</span>
        <div className="prow-fields">
          <NumberField value={json?.st ?? 0} precision={0} suffix="f" onChange={(v) => act("set_layer_time", { layer: layer.id, start: v })} />
          <NumberField label="×" value={json?.sr ?? 1} precision={2} step={0.05} min={0.01} onChange={(v) => act("set_layer_time", { layer: layer.id, stretch: v })} />
        </div>
        <span />
      </div>
      <div className="row" style={{ gap: 16, paddingTop: 2 }}>
        <Toggle label="Visible" value={!layer.hidden} onChange={(v) => act("set_layer_visibility", { layer: layer.id, visible: v })} />
        <Toggle label="Locked" value={layer.locked} onChange={(v) => act("set_layer_lock", { layer: layer.id, locked: v })} />
      </div>
      <div className="row">
        <button className="chip-btn" onClick={() => act("flip_layer", { layer: layer.id, axis: "horizontal" })}>Flip H</button>
        <button className="chip-btn" onClick={() => act("flip_layer", { layer: layer.id, axis: "vertical" })}>Flip V</button>
      </div>
    </Section>
  );
}
