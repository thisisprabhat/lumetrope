import { useEffect, useRef, useState, type ReactNode } from "react";
import { act, beginGesture, endGesture } from "../actions";
import { useProp } from "../doc";
import { useEditor } from "../store";
import { Icon } from "../components/Icon";

const fmt = (v: number, precision: number) => {
  if (!Number.isFinite(v)) return "0";
  const r = Number(v.toFixed(precision));
  return String(Object.is(r, -0) ? 0 : r);
};

/** Numeric input whose label scrubs the value when dragged. */
export function NumberField({
  value,
  onChange,
  label,
  step = 1,
  min,
  max,
  precision = 1,
  suffix,
  width,
}: {
  value: number;
  onChange: (v: number) => void;
  label?: ReactNode;
  step?: number;
  min?: number;
  max?: number;
  precision?: number;
  suffix?: string;
  width?: number;
}) {
  const [text, setText] = useState(fmt(value, precision));
  const [editing, setEditing] = useState(false);
  const drag = useRef<{ x: number; v: number } | null>(null);
  useEffect(() => {
    if (!editing) setText(fmt(value, precision));
  }, [value, precision, editing]);
  const clamp = (v: number) => Math.min(max ?? Infinity, Math.max(min ?? -Infinity, v));
  const commit = () => {
    setEditing(false);
    const v = Number(text);
    if (Number.isFinite(v) && v !== value) onChange(clamp(v));
    else setText(fmt(value, precision));
  };
  return (
    <label className="numfield" style={width ? { width } : undefined}>
      {label !== undefined && (
        <span
          className="numfield-label"
          onPointerDown={(e) => {
            (e.target as Element).setPointerCapture(e.pointerId);
            drag.current = { x: e.clientX, v: value };
            beginGesture();
          }}
          onPointerMove={(e) => {
            const d = drag.current;
            if (!d) return;
            const mult = e.shiftKey ? 10 : e.altKey ? 0.1 : 1;
            const nv = clamp(Number((d.v + (e.clientX - d.x) * step * mult).toFixed(precision)));
            if (nv !== value) onChange(nv);
          }}
          onPointerUp={() => {
            if (drag.current) endGesture();
            drag.current = null;
          }}
        >
          {label}
        </span>
      )}
      <input
        className="num"
        value={editing ? text : fmt(value, precision)}
        onFocus={(e) => {
          setEditing(true);
          setText(fmt(value, precision));
          e.target.select();
        }}
        onChange={(e) => setText(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") (e.target as HTMLInputElement).blur();
          if (e.key === "Escape") {
            setText(fmt(value, precision));
            setEditing(false);
            (e.target as HTMLInputElement).blur();
          }
          if (e.key === "ArrowUp" || e.key === "ArrowDown") {
            e.preventDefault();
            onChange(clamp(value + (e.key === "ArrowUp" ? 1 : -1) * step * (e.shiftKey ? 10 : 1)));
          }
        }}
      />
      {suffix && <span className="numfield-suffix">{suffix}</span>}
    </label>
  );
}

export function toHex(c: number[] | null | undefined): string {
  if (!c || c.length < 3) return "#000000";
  const big = c.slice(0, 3).some((x) => x > 1);
  return "#" + c.slice(0, 3).map((x) => Math.round(Math.min(1, Math.max(0, big ? x / 255 : x)) * 255).toString(16).padStart(2, "0")).join("");
}

export function ColorField({ value, onChange, label }: { value: string; onChange: (hex: string) => void; label?: string }) {
  const [text, setText] = useState(value);
  useEffect(() => setText(value), [value]);
  return (
    <div className="colorfield">
      {label && <span className="field-label">{label}</span>}
      <span className="swatch" style={{ background: value }}>
        <input
          type="color"
          value={value}
          onPointerDown={() => beginGesture()}
          onBlur={() => endGesture()}
          onChange={(e) => onChange(e.target.value)}
          aria-label={label ?? "colour"}
        />
      </span>
      <input
        className="num hex"
        value={text.replace("#", "").toUpperCase()}
        onChange={(e) => setText("#" + e.target.value)}
        onBlur={() => (/^#[0-9a-fA-F]{6}$/.test(text) ? onChange(text.toLowerCase()) : setText(value))}
        onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
      />
    </div>
  );
}

export function Select<T extends string>({ value, options, onChange, label }: { value: T; options: { value: T; label: string }[]; onChange: (v: T) => void; label?: string }) {
  return (
    <label className="selectfield">
      {label && <span className="field-label">{label}</span>}
      <select value={value} onChange={(e) => onChange(e.target.value as T)}>
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
      <Icon name="chevronDown" size={12} />
    </label>
  );
}

export function Toggle({ value, onChange, label }: { value: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <label className="toggle">
      <input type="checkbox" checked={value} onChange={(e) => onChange(e.target.checked)} />
      <span className="toggle-track" />
      <span>{label}</span>
    </label>
  );
}

export function Section({ title, children, actions, defaultOpen = true }: { title: string; children: ReactNode; actions?: ReactNode; defaultOpen?: boolean }) {
  const [open, setOpen] = useState(defaultOpen);
  return (
    <section className={`psection ${open ? "open" : ""}`}>
      <header>
        <button className="psection-title" onClick={() => setOpen(!open)}>
          <Icon name={open ? "chevronDown" : "chevronRight"} size={12} />
          {title}
        </button>
        <div className="psection-actions">{actions}</div>
      </header>
      {open && <div className="psection-body">{children}</div>}
    </section>
  );
}

/** Keyframe diamond: empty (static), hollow lume (animated), filled (key on this frame). */
export function KeyButton({ layer, property, shape, value }: { layer: string; property: string; shape?: string | null; value: unknown }) {
  const st = useProp(layer, property, shape);
  const frame = useEditor((s) => Math.floor(s.frame));
  const cls = st.keyed ? "keyed" : st.animated ? "animated" : "";
  return (
    <button
      className={`keybtn ${cls}`}
      title={st.keyed ? "Remove keyframe" : "Add keyframe"}
      onClick={() =>
        st.keyed
          ? act("remove_keyframe", { layer, property, frame, ...(shape ? { shape } : {}) })
          : act("set_keyframe", { layer, property, frame, value, ...(shape ? { shape } : {}) })
      }
    >
      <Icon name="diamond" size={11} fill={st.keyed} />
    </button>
  );
}

export function writeProp(layer: string, property: string, value: unknown, shape?: string | null) {
  const frame = Math.floor(useEditor.getState().frame);
  act("set_property", { layer, property, value, frame, ...(shape ? { shape } : {}) });
}

/** A labelled numeric (1–3 component) property row with keyframe button. */
export function PropRow({
  layer,
  property,
  shape,
  label,
  dims = 1,
  step = 1,
  precision = 1,
  min,
  max,
  suffix,
  labels,
  keyable = true,
}: {
  layer: string;
  property: string;
  shape?: string | null;
  label: string;
  dims?: 1 | 2;
  step?: number;
  precision?: number;
  min?: number;
  max?: number;
  suffix?: string;
  labels?: [string, string];
  keyable?: boolean;
}) {
  const st = useProp(layer, property, shape);
  if (!st.exists) return null;
  const v: number[] = Array.isArray(st.value) ? st.value : [Number(st.value) || 0];
  const set = (i: number, x: number) => {
    const next = v.slice(0, dims === 2 ? 2 : 1);
    next[i] = x;
    writeProp(layer, property, dims === 1 ? next[0] : next, shape);
  };
  return (
    <div className={`prow ${st.animated ? "is-animated" : ""}`}>
      <span className="prow-label">
        {label}
        {st.slot && <span className="slot-tag" title={`Bound to slot ${st.slot}`}>◆ {st.slot}</span>}
      </span>
      <div className="prow-fields">
        <NumberField label={labels?.[0] ?? (dims === 2 ? "X" : undefined)} value={v[0] ?? 0} onChange={(x) => set(0, x)} step={step} precision={precision} min={min} max={max} suffix={suffix} />
        {dims === 2 && <NumberField label={labels?.[1] ?? "Y"} value={v[1] ?? 0} onChange={(x) => set(1, x)} step={step} precision={precision} min={min} max={max} suffix={suffix} />}
      </div>
      {keyable && <KeyButton layer={layer} property={property} shape={shape} value={dims === 1 ? v[0] : v.slice(0, 2)} />}
    </div>
  );
}

/** Colour property row with keyframe button. */
export function ColorRow({ layer, property, shape, label }: { layer: string; property: string; shape?: string | null; label: string }) {
  const st = useProp(layer, property, shape);
  if (!st.exists) return null;
  const hex = toHex(st.value);
  return (
    <div className={`prow ${st.animated ? "is-animated" : ""}`}>
      <span className="prow-label">
        {label}
        {st.slot && <span className="slot-tag">◆ {st.slot}</span>}
      </span>
      <div className="prow-fields">
        <ColorField value={hex} onChange={(h) => writeProp(layer, property, h, shape)} />
      </div>
      <KeyButton layer={layer} property={property} shape={shape} value={hex} />
    </div>
  );
}
