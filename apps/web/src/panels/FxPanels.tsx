import { useMemo } from "react";
import { act, query } from "../actions";
import { useLayerJson } from "../doc";
import { useEditor } from "../store";
import { Icon } from "../components/Icon";
import { ColorField, NumberField, PropRow, Section, Select, Toggle, toHex } from "../ui/controls";

const MASK_MODES = ["add", "subtract", "intersect", "lighten", "darken", "difference", "none"];
const MODE_CODE: Record<string, string> = { a: "add", s: "subtract", i: "intersect", l: "lighten", d: "darken", f: "difference", n: "none" };

export function MasksSection({ id }: { id: string }) {
  const json = useLayerJson(id);
  const masks: any[] = json?.masksProperties ?? [];
  return (
    <Section
      title={`Masks${masks.length ? ` · ${masks.length}` : ""}`}
      defaultOpen={masks.length > 0}
      actions={
        <>
          <button className="icon-btn" data-tip="Add rectangle mask" onClick={() => act("add_mask", { layer: id, shape: "rect" })}>
            <Icon name="rect" size={13} />
          </button>
          <button className="icon-btn" data-tip="Add ellipse mask" onClick={() => act("add_mask", { layer: id, shape: "ellipse" })}>
            <Icon name="ellipse" size={13} />
          </button>
        </>
      }
    >
      {masks.length === 0 && <div className="muted tiny">Masks clip this layer. Add a rectangle or ellipse mask above.</div>}
      {masks.map((m, i) => (
        <div className="item-card" key={i}>
          <div className="item-card-head">
            {m.nm ?? `Mask ${i + 1}`}
            <button className="icon-btn mini" title="Remove mask" onClick={() => act("remove_mask", { layer: id, index: i })}>
              <Icon name="trash" size={13} />
            </button>
          </div>
          <div className="row">
            <Select value={MODE_CODE[m.mode] ?? "add"} options={MASK_MODES.map((x) => ({ value: x, label: x[0].toUpperCase() + x.slice(1) }))} onChange={(mode) => act("update_mask", { layer: id, index: i, mode })} />
            <Toggle label="Invert" value={!!m.inv} onChange={(inverted) => act("update_mask", { layer: id, index: i, inverted })} />
          </div>
          <PropRow layer={id} shape={`mask:${i}`} property="opacity" label="Opacity" min={0} max={100} suffix="%" />
          <PropRow layer={id} shape={`mask:${i}`} property="expansion" label="Expansion" />
        </div>
      ))}
    </Section>
  );
}

const EFFECTS = [
  { value: "drop_shadow", label: "Drop shadow" },
  { value: "gaussian_blur", label: "Gaussian blur" },
  { value: "fill", label: "Fill" },
  { value: "tint", label: "Tint" },
  { value: "tritone", label: "Tritone" },
];

const PARAM_UI: Record<string, { key: string; label: string; kind: "num" | "color" | "bool" | "dims"; min?: number; max?: number; suffix?: string }[]> = {
  drop_shadow: [
    { key: "color", label: "Color", kind: "color" },
    { key: "opacity", label: "Opacity", kind: "num", min: 0, max: 100, suffix: "%" },
    { key: "direction", label: "Direction", kind: "num", suffix: "°" },
    { key: "distance", label: "Distance", kind: "num", min: 0 },
    { key: "softness", label: "Softness", kind: "num", min: 0 },
    { key: "shadow_only", label: "Shadow only", kind: "bool" },
  ],
  gaussian_blur: [
    { key: "blurriness", label: "Blurriness", kind: "num", min: 0 },
    { key: "dimensions", label: "Dimensions", kind: "dims" },
  ],
  fill: [
    { key: "color", label: "Color", kind: "color" },
    { key: "opacity", label: "Opacity", kind: "num", min: 0, max: 100, suffix: "%" },
  ],
  tint: [
    { key: "black", label: "Map black", kind: "color" },
    { key: "white", label: "Map white", kind: "color" },
    { key: "amount", label: "Amount", kind: "num", min: 0, max: 100, suffix: "%" },
  ],
  tritone: [
    { key: "highlights", label: "Highlights", kind: "color" },
    { key: "midtones", label: "Midtones", kind: "color" },
    { key: "shadows", label: "Shadows", kind: "color" },
    { key: "blend", label: "Blend", kind: "num", min: 0, max: 100, suffix: "%" },
  ],
};

export function EffectsSection({ id }: { id: string }) {
  const revision = useEditor((s) => s.revision);
  const list = useMemo(() => query<any[]>("list_effects", { layer: id }, []), [id, revision]);
  return (
    <Section
      title={`Effects${list.length ? ` · ${list.length}` : ""}`}
      defaultOpen={list.length > 0}
      actions={
        <select className="mini-select" value="" onChange={(e) => e.target.value && act("add_effect", { layer: id, type: e.target.value })}>
          <option value="">+ Add</option>
          {EFFECTS.map((f) => (
            <option key={f.value} value={f.value}>
              {f.label}
            </option>
          ))}
        </select>
      }
    >
      {list.length === 0 && <div className="muted tiny">Drop shadow, blur, fill, tint and tritone render live and export to dotLottie.</div>}
      {list.map((fx) => (
        <div className="item-card" key={fx.index}>
          <div className="item-card-head">
            <Toggle label={EFFECTS.find((e) => e.value === fx.type)?.label ?? fx.name} value={fx.enabled} onChange={(enabled) => act("update_effect", { layer: id, index: fx.index, enabled })} />
            <button className="icon-btn mini" title="Remove effect" onClick={() => act("remove_effect", { layer: id, index: fx.index })}>
              <Icon name="trash" size={13} />
            </button>
          </div>
          {(PARAM_UI[fx.type] ?? []).map((p) => {
            const v = fx.params[p.key];
            const set = (val: unknown) => act("update_effect", { layer: id, index: fx.index, params: { [p.key]: val }, frame: Math.floor(useEditor.getState().frame) });
            return (
              <div className="prow" key={p.key}>
                <span className="prow-label">{p.label}</span>
                <div className="prow-fields">
                  {v === "animated" ? (
                    <span className="muted tiny">animated — edit in timeline</span>
                  ) : p.kind === "color" ? (
                    <ColorField value={toHex(v)} onChange={set} />
                  ) : p.kind === "bool" ? (
                    <Toggle label="" value={!!v} onChange={set} />
                  ) : p.kind === "dims" ? (
                    <Select value={["both", "horizontal", "vertical"][(v ?? 1) - 1] ?? "both"} options={["both", "horizontal", "vertical"].map((d) => ({ value: d, label: d }))} onChange={set} />
                  ) : (
                    <NumberField value={Number(v) || 0} min={p.min} max={p.max} suffix={p.suffix} onChange={set} />
                  )}
                </div>
                <span />
              </div>
            );
          })}
        </div>
      ))}
    </Section>
  );
}

const JUSTIFY = [{ value: "left", label: "Left" }, { value: "center", label: "Center" }, { value: "right", label: "Right" }];

export function TextSection({ id }: { id: string }) {
  const json = useLayerJson(id);
  const doc = json?.t?.d?.k?.[0]?.s ?? {};
  const loaded = useEditor((s) => s.fonts);
  const fonts: string[] = useMemo(() => Array.from(new Set(["Poppins-Regular", ...loaded, doc.f].filter(Boolean))), [loaded, doc.f]);
  const style = (args: object) => act("set_text_style", { layer: id, ...args });
  return (
    <Section title="Text">
      <textarea
        className="text-area"
        key={doc.t}
        defaultValue={(doc.t ?? "").replace(/\r/g, "\n")}
        rows={3}
        onBlur={(e) => e.target.value !== (doc.t ?? "").replace(/\r/g, "\n") && act("set_text", { layer: id, text: e.target.value })}
      />
      <div className="prow">
        <span className="prow-label">Font</span>
        <Select value={doc.f ?? "Poppins-Regular"} options={fonts.map((f) => ({ value: f, label: f }))} onChange={(font) => style({ font })} />
        <span />
      </div>
      <div className="prow">
        <span className="prow-label">Size</span>
        <div className="prow-fields">
          <NumberField value={doc.s ?? 48} min={1} onChange={(font_size) => style({ font_size })} />
          <Select value={JUSTIFY[[0, 2, 1][doc.j ?? 0] ?? 0]?.value ?? "left"} options={JUSTIFY} onChange={(justify) => style({ justify })} />
        </div>
        <span />
      </div>
      <div className="prow">
        <span className="prow-label">Color</span>
        <div className="prow-fields">
          <ColorField value={toHex(doc.fc)} onChange={(color) => style({ color })} />
        </div>
        <span />
      </div>
      <div className="prow">
        <span className="prow-label">Spacing</span>
        <div className="prow-fields">
          <NumberField label="Tr" value={doc.tr ?? 0} onChange={(tracking) => style({ tracking })} />
          <NumberField label="Lh" value={doc.lh ?? (doc.s ?? 48) * 1.2} min={0} onChange={(line_height) => style({ line_height })} />
        </div>
        <span />
      </div>
      <div className="prow">
        <span className="prow-label">Stroke</span>
        <div className="prow-fields">
          {doc.sc ? (
            <>
              <ColorField value={toHex(doc.sc)} onChange={(stroke_color) => style({ stroke_color })} />
              <NumberField value={doc.sw ?? 1} min={0} width={64} onChange={(stroke_width) => style({ stroke_width })} />
            </>
          ) : (
            <button className="chip-btn" onClick={() => style({ stroke_color: "#000000", stroke_width: 2 })}>+ Stroke</button>
          )}
        </div>
        {doc.sc ? (
          <button className="keybtn" onClick={() => style({ stroke_color: null })}>
            ✕
          </button>
        ) : (
          <span />
        )}
      </div>
      <div className="prow">
        <span className="prow-label">Caps</span>
        <Select value={["regular", "all_caps", "small_caps"][doc.ca ?? 0]} options={[{ value: "regular", label: "Regular" }, { value: "all_caps", label: "ALL CAPS" }, { value: "small_caps", label: "Small caps" }]} onChange={(caps) => style({ caps })} />
        <span />
      </div>
    </Section>
  );
}
