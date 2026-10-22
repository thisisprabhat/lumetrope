import { act } from "../actions";
import { useLayerJson, useSceneInfo, type LayerNode } from "../doc";
import { useEditor } from "../store";
import { Icon } from "../components/Icon";
import { NumberField, PropRow, Section, Select, Toggle } from "../ui/controls";
import { keyLabel } from "../lib/keys";

const BLEND = ["normal", "multiply", "screen", "overlay", "darken", "lighten", "color_dodge", "color_burn", "hard_light", "soft_light", "difference", "exclusion", "hue", "saturation", "color", "luminosity", "add"];
const ANCHORS = ["top_left", "top", "top_right", "left", "center", "right", "bottom_left", "bottom", "bottom_right"];
const label = (s: string) => s.replace(/_/g, " ").replace(/^\w/, (c) => c.toUpperCase());

export function AlignBar({ ids }: { ids: string[] }) {
  const to = ids.length > 1 ? "selection" : "scene";
  const btn = (align: string, glyph: string) => (
    <button className="icon-btn align-btn" data-tip={`Align ${align}`} onClick={() => act("align_layers", { layers: ids, align, to })}>
      <span className={`align-glyph ${align}`}>{glyph}</span>
    </button>
  );
  return (
    <div className="alignbar">
      {btn("left", "⎸")}
      {btn("center", "⎮")}
      {btn("right", "⎹")}
      {btn("top", "⎺")}
      {btn("middle", "⎯")}
      {btn("bottom", "⎽")}
      <span className="sep" />
      <button className="icon-btn" data-tip="Distribute horizontally" disabled={ids.length < 3} onClick={() => act("distribute_layers", { layers: ids, axis: "horizontal" })}>
        ⇹
      </button>
      <button className="icon-btn" data-tip="Distribute vertically" disabled={ids.length < 3} onClick={() => act("distribute_layers", { layers: ids, axis: "vertical" })}>
        ⇵
      </button>
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
