import { useMemo, useState } from "react";
import { act, query } from "../actions";
import { engine, projectValue } from "../engine";
import { useSceneInfo, type LayerNode } from "../doc";
import { pickFile } from "../files";
import { useEditor, type LeftTab } from "../store";
import { Icon } from "../components/Icon";
import { ColorField, NumberField } from "../ui/controls";
import { HistoryPanel } from "./HistoryPanel";
import { openContextMenu } from "../contextMenu";
import { layerMenu } from "../menus";

const TABS: { id: LeftTab; icon: string; tip: string }[] = [
  { id: "layers", icon: "layers", tip: "Layers" },
  { id: "assets", icon: "image", tip: "Assets & import" },
  { id: "segments", icon: "segments", tip: "Segments" },
  { id: "themes", icon: "theme", tip: "Themes & slots" },
  { id: "colors", icon: "palette", tip: "Colours" },
  { id: "history", icon: "history", tip: "Version history" },
];

const TYPE_ICON: Record<string, string> = { shape: "star", text: "text", image: "image", precomp: "layers", null: "plus", solid: "rect" };

export function LeftSidebar() {
  const tab = useEditor((s) => s.leftTab);
  const setTab = useEditor((s) => s.setLeftTab);
  return (
    <div className="leftbar">
      <nav className="rail">
        {TABS.map((t) => (
          <button key={t.id} className={`icon-btn ${tab === t.id ? "active" : ""}`} data-tip={t.tip} onClick={() => setTab(t.id)}>
            <Icon name={t.icon} />
          </button>
        ))}
      </nav>
      <div className="leftbar-body">
        {tab === "layers" && <LayersPanel />}
        {tab === "assets" && <AssetsPanel />}
        {tab === "segments" && <SegmentsPanel />}
        {tab === "themes" && <ThemesPanel />}
        {tab === "colors" && <ColorsPanel />}
        {tab === "history" && <HistoryPanel />}
      </div>
    </div>
  );
}

function PanelHead({ title, children }: { title: string; children?: React.ReactNode }) {
  return (
    <div className="panel-head">
      <span>{title}</span>
      <div className="row">{children}</div>
    </div>
  );
}

function LayerRow({ l, index }: { l: LayerNode; index: number }) {
  const { selection, select, setScene, renaming: renamingId, setRenaming: setRenamingId } = useEditor();
  const [drop, setDrop] = useState<"above" | "below" | null>(null);
  const renaming = renamingId === l.id;
  const setRenaming = (on: boolean) => setRenamingId(on ? l.id : null);
  const layers = useSceneInfo().layers;
  const sel = selection.includes(l.id);
  return (
    <div
      className={`layer-row ${sel ? "sel" : ""} ${l.hidden ? "hidden" : ""} ${drop ? `drop-${drop}` : ""}`}
      draggable
      onDragStart={(e) => e.dataTransfer.setData("text/layer", l.id)}
      onDragOver={(e) => {
        e.preventDefault();
        const r = (e.currentTarget as HTMLElement).getBoundingClientRect();
        setDrop(e.clientY < r.top + r.height / 2 ? "above" : "below");
      }}
      onDragLeave={() => setDrop(null)}
      onDrop={(e) => {
        const id = e.dataTransfer.getData("text/layer");
        const where = drop;
        setDrop(null);
        if (id && id !== l.id) act("reorder_layer", { layer: id, index: where === "above" ? index : index + 1 });
      }}
      onClick={(e) => {
        if (e.shiftKey || e.metaKey || e.ctrlKey) select(sel ? selection.filter((x) => x !== l.id) : [...selection, l.id]);
        else select([l.id]);
      }}
      onDoubleClick={() => (l.type === "precomp" && l.scene ? setScene(l.scene) : setRenaming(true))}
      onContextMenu={(e) => {
        const ids = sel ? selection : [l.id];
        if (!sel) select([l.id]);
        openContextMenu(e, layerMenu(layers.filter((x) => ids.includes(x.id))));
      }}
    >
      <Icon name={TYPE_ICON[l.type] ?? "more"} size={13} />
      {renaming ? (
        <input
          className="text-input inline"
          autoFocus
          defaultValue={l.name}
          onBlur={(e) => {
            setRenaming(false);
            if (e.target.value !== l.name) act("rename_layer", { layer: l.id, name: e.target.value });
          }}
          onKeyDown={(e) => e.key === "Enter" && (e.target as HTMLInputElement).blur()}
        />
      ) : (
        <span className="layer-name-text">
          {l.name || l.id}
          {l.is_matte && <span className="badge">matte</span>}
          {l.matte_mode && <span className="badge">◐ {l.matte_mode}</span>}
          {l.parent && <span className="badge">↳</span>}
        </span>
      )}
      {l.type === "precomp" && l.scene && (
        <button className="icon-btn mini" title="Open nested scene" onClick={(e) => {
          e.stopPropagation();
          setScene(l.scene!);
        }}>
          <Icon name="chevronRight" size={13} />
        </button>
      )}
      <button className="icon-btn mini" title={l.locked ? "Unlock" : "Lock"} onClick={(e) => {
        e.stopPropagation();
        act("set_layer_lock", { layer: l.id, locked: !l.locked });
      }}>
        <Icon name={l.locked ? "lock" : "unlock"} size={13} />
      </button>
      <button className="icon-btn mini" title={l.hidden ? "Show" : "Hide"} onClick={(e) => {
        e.stopPropagation();
        act("set_layer_visibility", { layer: l.id, visible: l.hidden });
      }}>
        <Icon name={l.hidden ? "eyeOff" : "eye"} size={13} />
      </button>
    </div>
  );
}

function LayersPanel() {
  const s = useSceneInfo();
  const { scene, setScene } = useEditor();
  const current = s.scenes.find((x) => x.id === scene);
  return (
    <>
      <PanelHead title="Layers">
        <button className="icon-btn" data-tip="Add null" onClick={() => act("create_null", { scene })}>
          <Icon name="plus" size={14} />
        </button>
      </PanelHead>
      <div className="breadcrumb">
        <button onClick={() => setScene("root")} className={scene === "root" ? "on" : ""}>
          Main scene
        </button>
        {scene !== "root" && (
          <>
            <Icon name="chevronRight" size={11} />
            <button className="on">{current?.name ?? scene}</button>
          </>
        )}
      </div>
      <div className="layer-list">
        {s.layers.length === 0 && <div className="panel-empty">No layers yet. Draw with the shape tools, or drop an SVG / Lottie file on the canvas.</div>}
        {s.layers.map((l, i) => (
          <LayerRow key={l.id} l={l} index={i} />
        ))}
      </div>
    </>
  );
}

function readAsDataUrl(f: File): Promise<string> {
  return new Promise((res, rej) => {
    const r = new FileReader();
    r.onload = () => res(String(r.result));
    r.onerror = rej;
    r.readAsDataURL(f);
  });
}

export async function importFile(f: File) {
  const st = useEditor.getState();
  const name = f.name.replace(/\.[^.]+$/, "");
  if (/\.svg$/i.test(f.name)) {
    const r = act<{ id: string }>("import_svg", { svg: await f.text(), name, scene: st.scene });
    if (r) st.select([r.id]);
  } else if (/\.(png|jpe?g)$/i.test(f.name)) {
    const r = act<{ id: string }>("import_image", { data: await readAsDataUrl(f), name });
    if (r) st.select([r.id]);
  } else if (/\.json$/i.test(f.name)) {
    const r = act<{ id: string }>("import_lottie", { json: await f.text(), name });
    if (r) st.select([r.id]);
  } else if (/\.(ttf|otf|woff2?)$/i.test(f.name)) {
    if (engine().add_font(name, new Uint8Array(await f.arrayBuffer()))) {
      st.addFontName(name);
      st.refresh();
      st.toast(`Font ${name} added`);
    } else st.toast("Could not read that font", "error");
  } else st.toast(`Cannot import ${f.name}`, "error");
}

function AssetsPanel() {
  const s = useSceneInfo();
  const revision = useEditor((st) => st.revision);
  const setScene = useEditor((st) => st.setScene);
  const images = useMemo(() => {
    const a = projectValue();
    const anim = a.animations[a.active];
    return (anim.assets ?? []).filter((x: any) => !x.layers && x.p);
  }, [revision]);
  const imp = async (accept: string) => {
    const f = await pickFile(accept);
    if (f) await importFile(f);
  };
  return (
    <>
      <PanelHead title="Assets" />
      <div className="panel-pad">
        <div className="import-grid">
          <button className="import-tile" onClick={() => imp(".svg")}>
            <Icon name="pen" /> SVG
          </button>
          <button className="import-tile" onClick={() => imp(".json")}>
            <Icon name="layers" /> Lottie
          </button>
          <button className="import-tile" onClick={() => imp(".png,.jpg,.jpeg")}>
            <Icon name="image" /> Image
          </button>
          <button className="import-tile" onClick={() => imp(".ttf,.otf,.woff,.woff2")}>
            <Icon name="text" /> Font
          </button>
        </div>
        <div className="subhead">Scenes</div>
        {s.scenes.map((sc) => (
          <button key={sc.id} className="asset-row" onClick={() => setScene(sc.id)}>
            <Icon name="layers" size={13} /> {sc.name} <span className="muted tiny">{sc.layers} layers</span>
          </button>
        ))}
        <button className="chip-btn" onClick={() => act("create_scene", { name: "Nested Scene", add_to_root: true })}>
          + Nested scene
        </button>
        {images.length > 0 && <div className="subhead">Images</div>}
        <div className="thumb-grid">
          {images.map((img: any) => (
            <div key={img.id} className="thumb" title={img.id}>
              <img src={img.p} alt={img.id} />
            </div>
          ))}
        </div>
      </div>
    </>
  );
}

function SegmentsPanel() {
  const s = useSceneInfo();
  const { frame, info } = useEditor();
  const [name, setName] = useState("");
  return (
    <>
      <PanelHead title="Segments" />
      <div className="panel-pad">
        <div className="muted tiny">Named frame ranges. State machine states play segments by name.</div>
        {s.segments.map((seg) => (
          <div className="item-card" key={seg.name}>
            <div className="item-card-head">
              <input className="text-input inline" defaultValue={seg.name} key={seg.name} onBlur={(e) => e.target.value !== seg.name && act("update_segment", { name: seg.name, new_name: e.target.value })} />
              <button className="icon-btn mini" onClick={() => act("remove_segment", { name: seg.name })}>
                <Icon name="trash" size={13} />
              </button>
            </div>
            <div className="row">
              <NumberField label="From" value={seg.start} precision={0} onChange={(v) => act("update_segment", { name: seg.name, start: v })} />
              <NumberField label="To" value={seg.end} precision={0} onChange={(v) => act("update_segment", { name: seg.name, end: v })} />
            </div>
          </div>
        ))}
        <div className="row">
          <input className="text-input" placeholder="Segment name" value={name} onChange={(e) => setName(e.target.value)} />
          <button
            className="btn"
            disabled={!name}
            onClick={() => {
              const start = Math.floor(frame);
              const end = Math.min(info?.op ?? start + 30, start + 30);
              if (act("add_segment", { name, start, end: end > start ? end : start + 1 })) setName("");
            }}
          >
            Add at playhead
          </button>
        </div>
      </div>
    </>
  );
}

function ThemesPanel() {
  const revision = useEditor((s) => s.revision);
  const slots = useMemo(() => query<any[]>("list_slots", {}, []), [revision]);
  const themes = useMemo(() => query<any[]>("list_themes", {}, []), [revision]);
  const project = useMemo(() => projectValue(), [revision]);
  const preview = project.settings?.preview_theme ?? null;
  const [open, setOpen] = useState<string | null>(null);
  return (
    <>
      <PanelHead title="Themes & slots" />
      <div className="panel-pad">
        <div className="subhead">Slots</div>
        {slots.length === 0 && <div className="muted tiny">Bind properties to slots from the properties panel (◇ on colour rows), or ask an agent via MCP: create_slot.</div>}
        {slots.map((sl) => (
          <div key={sl.id} className="item-card">
            <div className="item-card-head">
              <span>◆ {sl.id}</span>
              <span className="muted tiny">
                {sl.type} · {sl.bound} bound
              </span>
            </div>
            {sl.type === "Color" && Array.isArray(sl.value) && (
              <ColorField value={"#" + sl.value.slice(0, 3).map((x: number) => Math.round(x * 255).toString(16).padStart(2, "0")).join("")} onChange={(c) => act("update_slot_value", { slot_id: sl.id, value: c })} />
            )}
            {sl.type !== "Color" && typeof sl.value === "number" && <NumberField value={sl.value} onChange={(v) => act("update_slot_value", { slot_id: sl.id, value: v })} />}
          </div>
        ))}
        <div className="subhead">
          Themes
          <button className="chip-btn" onClick={() => act("add_theme", { name: `Theme ${themes.length + 1}` })}>
            + Theme
          </button>
        </div>
        <button className={`asset-row ${!preview ? "on" : ""}`} onClick={() => act("select_theme", { theme: null })}>
          Default (no theme)
        </button>
        {themes.map((t) => {
          const rules: any[] = project.themes?.[t.id]?.rules ?? [];
          return (
            <div key={t.id} className={`item-card ${preview === t.id ? "on" : ""}`}>
              <div className="item-card-head">
                <button className="link" onClick={() => act("select_theme", { theme: t.id })}>
                  {preview === t.id ? "● " : "○ "}
                  {t.name}
                </button>
                <div className="row">
                  <button className="icon-btn mini" onClick={() => setOpen(open === t.id ? null : t.id)}>
                    <Icon name={open === t.id ? "chevronDown" : "chevronRight"} size={13} />
                  </button>
                  <button className="icon-btn mini" onClick={() => act("remove_theme", { theme: t.id })}>
                    <Icon name="trash" size={13} />
                  </button>
                </div>
              </div>
              {open === t.id &&
                slots.map((sl) => {
                  const rule = rules.find((r) => r.id === sl.id);
                  return (
                    <div className="prow" key={sl.id}>
                      <span className="prow-label">{sl.id}</span>
                      <div className="prow-fields">
                        {sl.type === "Color" ? (
                          <ColorField
                            value={"#" + ((rule?.value ?? sl.value) as number[]).slice(0, 3).map((x) => Math.round(x * 255).toString(16).padStart(2, "0")).join("")}
                            onChange={(c) => act("update_theme_rule", { theme: t.id, slot_id: sl.id, value: c })}
                          />
                        ) : (
                          <NumberField value={Number(rule?.value ?? sl.value) || 0} onChange={(v) => act("update_theme_rule", { theme: t.id, slot_id: sl.id, value: v })} />
                        )}
                      </div>
                      {rule ? (
                        <button className="keybtn" title="Use slot default" onClick={() => act("update_theme_rule", { theme: t.id, slot_id: sl.id, remove: true })}>
                          ✕
                        </button>
                      ) : (
                        <span />
                      )}
                    </div>
                  );
                })}
            </div>
          );
        })}
      </div>
    </>
  );
}

function ColorsPanel() {
  const revision = useEditor((s) => s.revision);
  const colors = useMemo(() => query<{ hex: string; count: number }[]>("list_colors", {}, []), [revision]);
  return (
    <>
      <PanelHead title="Colours" />
      <div className="panel-pad">
        <div className="muted tiny">Every colour in the animation. Change one to recolour it everywhere — fills, strokes, keyframes, solids and text.</div>
        <div className="palette">
          {colors.map((c) => (
            <div key={c.hex} className="palette-row">
              <ColorField value={c.hex} onChange={(to) => act("replace_color", { from: c.hex, to })} />
              <span className="muted tiny">×{c.count}</span>
            </div>
          ))}
        </div>
      </div>
    </>
  );
}
