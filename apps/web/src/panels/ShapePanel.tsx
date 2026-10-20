import { act } from "../actions";
import { useLayerJson } from "../doc";
import { useEditor } from "../store";
import { Icon } from "../components/Icon";
import { openContextMenu } from "../contextMenu";
import { shapeItemMenu } from "../menus";
import { ColorField, ColorRow, NumberField, PropRow, Section, Select, toHex } from "../ui/controls";

export function itemAt(layer: any, path: string | null): any {
  if (!layer || !path) return null;
  let list = layer.shapes ?? [];
  let item: any = null;
  for (const seg of path.split("/")) {
    item = list?.[Number(seg)];
    list = item?.it;
  }
  return item;
}

const TYPE_LABEL: Record<string, string> = {
  gr: "Group", rc: "Rectangle", el: "Ellipse", sr: "Star", sh: "Path", fl: "Fill", st: "Stroke", gf: "Gradient fill", gs: "Gradient stroke",
  tm: "Trim paths", rd: "Round corners", rp: "Repeater", tr: "Transform",
};

const parentPath = (p: string) => (p.includes("/") ? p.slice(0, p.lastIndexOf("/")) : undefined);

function ContentsTree({ id, items, prefix }: { id: string; items: any[]; prefix: string }) {
  const shape = useEditor((s) => s.shape);
  const select = useEditor((s) => s.select);
  return (
    <ul className="contents">
      {items.map((it, i) => {
        if (it.ty === "tr") return null;
        const path = prefix ? `${prefix}/${i}` : String(i);
        const label = TYPE_LABEL[it.ty] === "Star" && it.sy === 2 ? "Polygon" : TYPE_LABEL[it.ty] ?? it.ty;
        return (
          <li key={path}>
            <div
              className={`contents-row ${shape === path ? "sel" : ""} ${it.hd ? "hidden" : ""}`}
              onClick={() => select([id], path)}
              onContextMenu={(e) => {
                select([id], path);
                openContextMenu(e, shapeItemMenu(id, path, !!it.hd));
              }}
            >
              <span className={`shape-dot s-${it.ty}`} />
              <span className="contents-name">{it.nm || label}</span>
              <span className="muted tiny">{label}</span>
              <button className="icon-btn mini" title={it.hd ? "Show" : "Hide"} onClick={(e) => {
                e.stopPropagation();
                act("update_shape_item", { layer: id, shape: path, hidden: !it.hd });
              }}>
                <Icon name={it.hd ? "eyeOff" : "eye"} size={13} />
              </button>
              <button className="icon-btn mini" title="Delete" onClick={(e) => {
                e.stopPropagation();
                act("remove_shape_item", { layer: id, shape: path });
                select([id]);
              }}>
                <Icon name="trash" size={13} />
              </button>
            </div>
            {it.ty === "gr" && <ContentsTree id={id} items={it.it ?? []} prefix={path} />}
          </li>
        );
      })}
    </ul>
  );
}

function AddMenu({ id, container }: { id: string; container?: string }) {
  const base = container ? { layer: id, shape: container } : { layer: id };
  const add = (tool: string, extra: object = {}) => act(tool, { ...base, ...extra });
  return (
    <div className="chip-row">
      {!container && (
        <>
          <button className="chip-btn" onClick={() => add("add_shape", { type: "rect", size: [80, 80] })}>+ Rect</button>
          <button className="chip-btn" onClick={() => add("add_shape", { type: "ellipse", size: [80, 80] })}>+ Ellipse</button>
          <button className="chip-btn" onClick={() => add("add_shape", { type: "star" })}>+ Star</button>
        </>
      )}
      <button className="chip-btn" onClick={() => add("set_fill", { color: "#c4c4c4" })}>+ Fill</button>
      <button className="chip-btn" onClick={() => add("set_stroke", { color: "#1b1b1b", width: 3 })}>+ Stroke</button>
      <button className="chip-btn" onClick={() => add("set_gradient", { kind: "fill", type: "linear", stops: [{ offset: 0, color: "#ffb340" }, { offset: 1, color: "#ff5f6d" }] })}>+ Gradient</button>
      <button className="chip-btn" onClick={() => add("add_trim_path", { start: 0, end: 100 })}>+ Trim</button>
      <button className="chip-btn" onClick={() => add("add_round_corners", { radius: 10 })}>+ Round</button>
      <button className="chip-btn" onClick={() => add("add_repeater", { copies: 3, position: [40, 0] })}>+ Repeater</button>
    </div>
  );
}

function GradientEditor({ id, path, item }: { id: string; path: string; item: any }) {
  const count: number = item.g?.p ?? 0;
  const k: number[] = Array.isArray(item.g?.k?.k) && typeof item.g.k.k[0] === "number" ? item.g.k.k : [];
  const stops = Array.from({ length: count }, (_, i) => ({ offset: k[i * 4] ?? 0, color: toHex(k.slice(i * 4 + 1, i * 4 + 4)), opacity: 1 }));
  const alpha = k.slice(count * 4);
  stops.forEach((s, i) => {
    if (alpha.length >= (i + 1) * 2) s.opacity = alpha[i * 2 + 1];
  });
  const kind = item.ty === "gs" ? "stroke" : "fill";
  const type = item.t === 2 ? "radial" : "linear";
  const container = parentPath(path);
  const save = (next: typeof stops, t = type) => act("set_gradient", { layer: id, ...(container ? { shape: container } : {}), kind, type: t, stops: next, start: item.s?.k, end: item.e?.k });
  const css = `linear-gradient(90deg, ${stops.map((s) => `${s.color} ${Math.round(s.offset * 100)}%`).join(", ")})`;
  return (
    <>
      <div className="prow">
        <span className="prow-label">Type</span>
        <Select value={type} options={[{ value: "linear", label: "Linear" }, { value: "radial", label: "Radial" }]} onChange={(t) => save(stops, t)} />
        <span />
      </div>
      <div className="gradient-bar" style={{ background: css }} />
      {stops.map((s, i) => (
        <div className="prow" key={i}>
          <span className="prow-label">Stop {i + 1}</span>
          <div className="prow-fields">
            <NumberField value={s.offset * 100} min={0} max={100} precision={0} suffix="%" onChange={(v) => save(stops.map((x, j) => (j === i ? { ...x, offset: v / 100 } : x)))} />
            <ColorField value={s.color} onChange={(c) => save(stops.map((x, j) => (j === i ? { ...x, color: c } : x)))} />
          </div>
          <button className="keybtn" title="Remove stop" disabled={stops.length <= 2} onClick={() => save(stops.filter((_, j) => j !== i))}>
            ✕
          </button>
        </div>
      ))}
      <button className="chip-btn" onClick={() => save([...stops, { offset: 1, color: "#ffffff", opacity: 1 }])}>+ Stop</button>
      <PropRow layer={id} shape={path} property="start" label="Start" dims={2} />
      <PropRow layer={id} shape={path} property="end" label="End" dims={2} />
      {type === "radial" && <PropRow layer={id} shape={path} property="highlight" label="Highlight" suffix="%" />}
      <PropRow layer={id} shape={path} property="opacity" label="Opacity" min={0} max={100} suffix="%" />
    </>
  );
}

const CAPS = [{ value: "butt", label: "Butt" }, { value: "round", label: "Round" }, { value: "square", label: "Square" }];
const JOINS = [{ value: "miter", label: "Miter" }, { value: "round", label: "Round" }, { value: "bevel", label: "Bevel" }];

function StrokeExtras({ id, path, item }: { id: string; path: string; item: any }) {
  const dash = item.d?.find((d: any) => d.n === "d")?.v?.k;
  const gap = item.d?.find((d: any) => d.n === "g")?.v?.k;
  const container = parentPath(path);
  return (
    <>
      <PropRow layer={id} shape={path} property="width" label="Width" min={0} />
      <div className="prow">
        <span className="prow-label">Cap / Join</span>
        <div className="prow-fields">
          <Select value={CAPS[(item.lc ?? 2) - 1]?.value ?? "round"} options={CAPS} onChange={(v) => act("update_shape_item", { layer: id, shape: path, line_cap: v })} />
          <Select value={JOINS[(item.lj ?? 2) - 1]?.value ?? "round"} options={JOINS} onChange={(v) => act("update_shape_item", { layer: id, shape: path, line_join: v })} />
        </div>
        <span />
      </div>
      {item.ty === "st" && (
        <div className="prow">
          <span className="prow-label">Dash</span>
          <div className="prow-fields">
            <NumberField label="—" value={typeof dash === "number" ? dash : 0} min={0} precision={0} onChange={(v) => act("set_stroke", { layer: id, ...(container ? { shape: container } : {}), dash: v > 0 ? [v, typeof gap === "number" ? gap : v] : null })} />
            <NumberField label="gap" value={typeof gap === "number" ? gap : 0} min={0} precision={0} onChange={(v) => act("set_stroke", { layer: id, ...(container ? { shape: container } : {}), dash: [typeof dash === "number" && dash > 0 ? dash : 8, v] })} />
          </div>
          <span />
        </div>
      )}
    </>
  );
}

/** Editor for whichever shape item is selected. */
export function ItemEditor({ id, path, item }: { id: string; path: string; item: any }) {
  const p = (property: string, l: string, extra: object = {}) => <PropRow key={property} layer={id} shape={path} property={property} label={l} {...extra} />;
  switch (item.ty) {
    case "rc":
      return <>{p("size", "Size", { dims: 2 })}{p("position", "Position", { dims: 2 })}{p("roundness", "Roundness", { min: 0 })}</>;
    case "el":
      return <>{p("size", "Size", { dims: 2 })}{p("position", "Position", { dims: 2 })}</>;
    case "sr":
      return (
        <>
          {p("points", "Points", { min: 3, precision: 0 })}
          {p("outer_radius", "Outer radius", { min: 0 })}
          {item.sy !== 2 && p("inner_radius", "Inner radius", { min: 0 })}
          {p("outer_roundness", "Outer round", { suffix: "%" })}
          {item.sy !== 2 && p("inner_roundness", "Inner round", { suffix: "%" })}
          {p("rotation", "Rotation", { suffix: "°" })}
          {p("position", "Position", { dims: 2 })}
        </>
      );
    case "sh":
      return <div className="muted tiny">{item.ks?.k?.v?.length ?? item.ks?.k?.[0]?.s?.[0]?.v?.length ?? 0} vertices · {item.ks?.a ? "animated" : "static"} path. Draw new paths with the Pen tool (P).</div>;
    case "fl":
      return (
        <>
          <ColorRow layer={id} shape={path} property="color" label="Color" />
          {p("opacity", "Opacity", { min: 0, max: 100, suffix: "%" })}
          <div className="prow">
            <span className="prow-label">Fill rule</span>
            <Select value={item.r === 2 ? "even_odd" : "non_zero"} options={[{ value: "non_zero", label: "Non-zero" }, { value: "even_odd", label: "Even-odd" }]} onChange={(v) => act("update_shape_item", { layer: id, shape: path, fill_rule: v })} />
            <span />
          </div>
        </>
      );
    case "st":
      return (
        <>
          <ColorRow layer={id} shape={path} property="color" label="Color" />
          {p("opacity", "Opacity", { min: 0, max: 100, suffix: "%" })}
          <StrokeExtras id={id} path={path} item={item} />
        </>
      );
    case "gf":
      return <GradientEditor id={id} path={path} item={item} />;
    case "gs":
      return (
        <>
          <GradientEditor id={id} path={path} item={item} />
          <StrokeExtras id={id} path={path} item={item} />
        </>
      );
    case "tm":
      return (
        <>
          {p("start", "Start", { min: 0, max: 100, suffix: "%" })}
          {p("end", "End", { min: 0, max: 100, suffix: "%" })}
          {p("offset", "Offset", { suffix: "°" })}
          <div className="prow">
            <span className="prow-label">Mode</span>
            <Select value={item.m === 2 ? "individual" : "simultaneous"} options={[{ value: "simultaneous", label: "Simultaneously" }, { value: "individual", label: "Individually" }]} onChange={(v) => act("update_shape_item", { layer: id, shape: path, trim_mode: v })} />
            <span />
          </div>
        </>
      );
    case "rd":
      return p("radius", "Radius", { min: 0 });
    case "rp":
      return (
        <>
          {p("copies", "Copies", { min: 1, precision: 0 })}
          {p("offset", "Offset", { precision: 2, step: 0.1 })}
          {p("position", "Position", { dims: 2 })}
          {p("scale", "Scale", { dims: 2, suffix: "%" })}
          {p("rotation", "Rotation", { suffix: "°" })}
          {p("start_opacity", "Start opacity", { suffix: "%" })}
          {p("end_opacity", "End opacity", { suffix: "%" })}
        </>
      );
    case "gr":
      return (
        <>
          {p("position", "Position", { dims: 2 })}
          {p("scale", "Scale", { dims: 2, suffix: "%" })}
          {p("rotation", "Rotation", { suffix: "°" })}
          {p("opacity", "Opacity", { min: 0, max: 100, suffix: "%" })}
          {p("anchor", "Anchor", { dims: 2 })}
          <AddMenu id={id} container={path} />
        </>
      );
    default:
      return <div className="muted tiny">This shape item type is preserved but not editable here.</div>;
  }
}

export function ShapeSections({ id }: { id: string }) {
  const json = useLayerJson(id);
  const shape = useEditor((s) => s.shape);
  const item = itemAt(json, shape);
  const label = item ? (TYPE_LABEL[item.ty] === "Star" && item.sy === 2 ? "Polygon" : TYPE_LABEL[item.ty] ?? item.ty) : "";
  return (
    <>
      <Section title="Contents">
        <ContentsTree id={id} items={json?.shapes ?? []} prefix="" />
        <AddMenu id={id} />
      </Section>
      {item && shape && (
        <Section title={`${item.nm || label}`}>
          <ItemEditor id={id} path={shape} item={item} />
        </Section>
      )}
    </>
  );
}
