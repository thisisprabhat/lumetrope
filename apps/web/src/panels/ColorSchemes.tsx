// Colour schemes: recolour the open animation with a preset (instantly, one
// undo step), shuffle or hand-tweak the result, try the black-and-white
// outline look, and keep your own presets.
import { useEffect, useMemo, useState } from "react";
import { BUILT_IN, CATEGORIES, loadCustomSchemes, newSchemeId, PRESETS_CHANGED, saveCustomSchemes, type ColorScheme } from "../colorSchemes";
import { Icon } from "../components/Icon";
import { buildColorMap, dominant, normalizeHex, shuffled, targetOrder, type MapMode, type SourceColor } from "../lib/palette";
import { applyLook, baseColors, currentLook, revertLook } from "../recolor";
import { useEditor } from "../store";
import { Select, Toggle } from "../ui/controls";

const PREFS = "lumetrope.schemePrefs";

interface Prefs {
  mode: MapMode;
  keepNeutrals: boolean;
  width: number;
  category: string;
}

function loadPrefs(): Prefs {
  const d: Prefs = { mode: "groups", keepNeutrals: false, width: 2, category: "All" };
  try {
    return { ...d, ...JSON.parse(localStorage.getItem(PREFS) ?? "{}") };
  } catch {
    return d;
  }
}

function useCustomSchemes(): [ColorScheme[], (l: ColorScheme[]) => void] {
  const [list, setList] = useState(loadCustomSchemes);
  useEffect(() => {
    const on = () => setList(loadCustomSchemes());
    window.addEventListener(PRESETS_CHANGED, on);
    return () => window.removeEventListener(PRESETS_CHANGED, on);
  }, []);
  return [list, saveCustomSchemes];
}

function Swatches({ colors }: { colors: string[] }) {
  return (
    <span className="scheme-swatches">
      {colors.map((c, i) => (
        <span key={i} style={{ background: c }} />
      ))}
    </span>
  );
}

function SchemeEditor({ initial, onDone }: { initial: ColorScheme; onDone: (s: ColorScheme | null) => void }) {
  const [name, setName] = useState(initial.name);
  const [colors, setColors] = useState(initial.colors);
  const set = (i: number, c: string) => setColors(colors.map((x, j) => (j === i ? c : x)));
  return (
    <div className="item-card scheme-editor">
      <input className="text-input" value={name} onChange={(e) => setName(e.target.value)} placeholder="Preset name" autoFocus aria-label="Preset name" />
      <div className="scheme-edit-colors">
        {colors.map((c, i) => (
          <span key={i} className="scheme-edit-color">
            <span className="swatch" style={{ background: c }}>
              <input type="color" value={c} onChange={(e) => set(i, e.target.value)} aria-label={`Colour ${i + 1}`} />
            </span>
            <button className="icon-btn mini" title="Remove colour" disabled={colors.length < 2} onClick={() => setColors(colors.filter((_, j) => j !== i))}>
              <Icon name="close" size={11} />
            </button>
          </span>
        ))}
        {colors.length < 12 && (
          <button className="chip-btn" onClick={() => setColors([...colors, colors[colors.length - 1] ?? "#888888"])}>
            + Colour
          </button>
        )}
      </div>
      <button className="chip-btn" onClick={() => setColors(dominant(baseColors(), 6))} title="Use the animation's most used colours">
        Use this animation's colours
      </button>
      <div className="row">
        <button className="btn small primary" disabled={!colors.length} onClick={() => onDone({ ...initial, name: name.trim() || "My preset", colors: colors.map(normalizeHex) })}>
          Save preset
        </button>
        <button className="btn small ghost" onClick={() => onDone(null)}>
          Cancel
        </button>
      </div>
    </div>
  );
}

export function ColorSchemes() {
  useEditor((s) => s.revision); // re-read the current look after every change
  const [prefs, setPrefsState] = useState(loadPrefs);
  const setPrefs = (p: Partial<Prefs>) => {
    const next = { ...prefs, ...p };
    setPrefsState(next);
    try {
      localStorage.setItem(PREFS, JSON.stringify(next));
    } catch {
      /* not remembered */
    }
    return next;
  };
  const [custom, saveCustom] = useCustomSchemes();
  const [search, setSearch] = useState("");
  const [order, setOrder] = useState<number[] | null>(null);
  const [overrides, setOverrides] = useState<Record<string, string>>({});
  const [editing, setEditing] = useState<ColorScheme | null>(null);
  const [showMap, setShowMap] = useState(true);
  const look = currentLook();
  const all = useMemo(() => [...custom, ...BUILT_IN], [custom]);
  const active = look?.kind === "scheme" ? all.find((s) => s.id === look.id) ?? null : null;

  const shown = useMemo(() => {
    const q = search.trim().toLowerCase();
    return all.filter((s) => (prefs.category === "All" || s.category === prefs.category) && (!q || s.name.toLowerCase().includes(q) || s.category.toLowerCase().includes(q)));
  }, [all, search, prefs.category]);

  const applyScheme = (s: ColorScheme, o: { order?: number[] | null; overrides?: Record<string, string>; mode?: MapMode; keepNeutrals?: boolean } = {}) =>
    applyLook({ kind: "scheme", id: s.id }, (base) => ({
      tool: "apply_color_map",
      args: { map: buildColorMap(base, s.colors, { mode: o.mode ?? prefs.mode, keepNeutrals: o.keepNeutrals ?? prefs.keepNeutrals, order: o.order ?? null, overrides: o.overrides ?? {} }) },
    }));

  const pick = (s: ColorScheme) => {
    setOrder(null);
    setOverrides({});
    applyScheme(s);
  };
  const reapply = (o: { order?: number[] | null; overrides?: Record<string, string>; mode?: MapMode; keepNeutrals?: boolean }) => {
    if (active) applyScheme(active, { order, overrides, ...o });
  };
  const outline = (inverted: boolean, width = prefs.width) =>
    applyLook({ kind: "outline", inverted }, () => ({ tool: "outline_only", args: { line: inverted ? "#ffffff" : "#000000", background: inverted ? "#000000" : "#ffffff", width } }));

  const base = look ? baseColors() : [];
  const mapping = active ? buildColorMap(base, active.colors, { mode: prefs.mode, keepNeutrals: prefs.keepNeutrals, order, overrides }) : {};
  const sourceRows: SourceColor[] = active ? [...base].sort((a, b) => b.count - a.count) : [];
  const targets = active ? targetOrder(active.colors, order) : [];

  const saveEdited = (s: ColorScheme | null) => {
    setEditing(null);
    if (!s) return;
    const i = custom.findIndex((x) => x.id === s.id);
    saveCustom(i >= 0 ? custom.map((x) => (x.id === s.id ? s : x)) : [s, ...custom]);
    if (active?.id === s.id) applyScheme(s, { order: null, overrides });
    if (i < 0) setPrefs({ category: prefs.category === "All" ? "All" : "Custom" });
  };

  return (
    <div className="schemes">
      <div className="subhead">
        Colour schemes
        <span className="muted tiny num">{all.length}</span>
      </div>

      <div className="segmented" role="radiogroup" aria-label="How colours are matched">
        {(
          [
            ["groups", "Keep shading", "Groups similar colours and keeps each shade's offset"],
            ["order", "Dark → light", "Darkest colour gets the preset's darkest, and so on"],
          ] as const
        ).map(([m, label, tip]) => (
          <button
            key={m}
            role="radio"
            aria-checked={prefs.mode === m}
            className={prefs.mode === m ? "on" : ""}
            title={tip}
            onClick={() => {
              setPrefs({ mode: m });
              reapply({ mode: m });
            }}
          >
            {label}
          </button>
        ))}
      </div>
      <Toggle
        label="Keep black, white & greys"
        value={prefs.keepNeutrals}
        onChange={(v) => {
          setPrefs({ keepNeutrals: v });
          reapply({ keepNeutrals: v });
        }}
      />

      {look && (
        <div className="item-card on scheme-active">
          <div className="item-card-head">
            <span className="scheme-active-name">{active ? active.name : look.kind === "outline" ? (look.inverted ? "Outline · white on black" : "Outline · black on white") : "Scheme"}</span>
            <div className="row">
              {active && (
                <button
                  className="icon-btn mini"
                  data-tip="Shuffle colours"
                  aria-label="Shuffle colours"
                  onClick={() => {
                    const next = shuffled(active.colors.length, order);
                    setOrder(next);
                    reapply({ order: next });
                  }}
                >
                  <Icon name="shuffle" size={13} />
                </button>
              )}
              {active && (
                <button className="icon-btn mini" data-tip={showMap ? "Hide colour map" : "Tweak colours"} aria-label="Tweak colours" onClick={() => setShowMap(!showMap)}>
                  <Icon name={showMap ? "chevronDown" : "chevronRight"} size={13} />
                </button>
              )}
              <button className="icon-btn mini" data-tip="Back to the original colours" aria-label="Revert to original colours" onClick={revertLook}>
                <Icon name="undo" size={13} />
              </button>
            </div>
          </div>
          {active && showMap && (
            <div className="map-strip">
              {sourceRows.map((s) => {
                const to = mapping[normalizeHex(s.hex)];
                const manual = overrides[normalizeHex(s.hex)];
                return (
                  <div className="map-row" key={s.hex}>
                    <span className="swatch" style={{ background: s.hex }} title={`${s.hex} · used ×${s.count}`} />
                    <Icon name="next" size={11} />
                    {to ? (
                      <>
                        <span className="swatch" style={{ background: to }} title="Pick any colour">
                          <input
                            type="color"
                            value={to}
                            onChange={(e) => {
                              const next = { ...overrides, [normalizeHex(s.hex)]: e.target.value };
                              setOverrides(next);
                              reapply({ overrides: next });
                            }}
                            aria-label={`New colour for ${s.hex}`}
                          />
                        </span>
                        <span className="map-choices">
                          {targets.map((t, i) => (
                            <button
                              key={i}
                              className={`map-choice ${t === to ? "on" : ""}`}
                              style={{ background: t }}
                              title={`Use ${t}`}
                              aria-label={`Use ${t} for ${s.hex}`}
                              onClick={() => {
                                const next = { ...overrides, [normalizeHex(s.hex)]: t };
                                setOverrides(next);
                                reapply({ overrides: next });
                              }}
                            />
                          ))}
                        </span>
                        {manual && (
                          <button
                            className="icon-btn mini"
                            title="Use the automatic colour"
                            aria-label="Use the automatic colour"
                            onClick={() => {
                              const { [normalizeHex(s.hex)]: _, ...rest } = overrides;
                              setOverrides(rest);
                              reapply({ overrides: rest });
                            }}
                          >
                            <Icon name="close" size={11} />
                          </button>
                        )}
                      </>
                    ) : (
                      <span className="muted tiny">kept</span>
                    )}
                  </div>
                );
              })}
              {!sourceRows.length && <div className="muted tiny">This animation has no colours to map.</div>}
            </div>
          )}
        </div>
      )}

      <div className="subhead">Outlines only</div>
      <div className="outline-looks">
        <button className={`outline-look ${look?.kind === "outline" && !look.inverted ? "on" : ""}`} onClick={() => outline(false)}>
          <span className="outline-glyph light" aria-hidden />
          Black on white
        </button>
        <button className={`outline-look ${look?.kind === "outline" && look.inverted ? "on" : ""}`} onClick={() => outline(true)}>
          <span className="outline-glyph dark" aria-hidden />
          White on black
        </button>
      </div>
      <Select
        label="Line width"
        value={String(prefs.width)}
        options={["1", "1.5", "2", "3", "4", "6", "8"].map((v) => ({ value: v, label: `${v} px` }))}
        onChange={(v) => {
          const width = Number(v);
          setPrefs({ width });
          if (look?.kind === "outline") outline(look.inverted, width);
        }}
      />

      <div className="subhead">
        Presets
        <button className="chip-btn" onClick={() => setEditing({ id: newSchemeId(), name: "My preset", category: "Custom", custom: true, colors: dominant(baseColors(), 5) })}>
          + New preset
        </button>
      </div>
      {editing && !custom.some((c) => c.id === editing.id) && <SchemeEditor initial={editing} onDone={saveEdited} />}
      <div className="row">
        <input className="text-input" type="search" placeholder="Search presets" value={search} onChange={(e) => setSearch(e.target.value)} aria-label="Search colour presets" />
      </div>
      <Select
        value={prefs.category}
        options={[{ value: "All", label: "All categories" }, ...CATEGORIES.map((c) => ({ value: c, label: c === "Custom" ? `Custom (${custom.length})` : c }))]}
        onChange={(category) => setPrefs({ category })}
      />
      <div className="scheme-list" role="list">
        {shown.map((s) =>
          editing?.id === s.id ? (
            <SchemeEditor key={s.id} initial={s} onDone={saveEdited} />
          ) : (
            <div key={s.id} role="listitem" className={`scheme-row ${active?.id === s.id ? "on" : ""}`}>
              <button className="scheme-pick" onClick={() => pick(s)} title={`Apply ${s.name}`}>
                <Swatches colors={s.colors} />
                <span className="scheme-name">{s.name}</span>
              </button>
              {s.custom && (
                <span className="scheme-tools">
                  <button className="icon-btn mini" title="Edit preset" aria-label={`Edit ${s.name}`} onClick={() => setEditing(s)}>
                    <Icon name="pen" size={12} />
                  </button>
                  <button className="icon-btn mini" title="Delete preset" aria-label={`Delete ${s.name}`} onClick={() => saveCustom(custom.filter((x) => x.id !== s.id))}>
                    <Icon name="trash" size={12} />
                  </button>
                </span>
              )}
            </div>
          ),
        )}
        {!shown.length && <div className="muted tiny">{prefs.category === "Custom" && !search ? "No custom presets yet. Make one with + New preset." : "No presets match."}</div>}
      </div>
    </div>
  );
}
