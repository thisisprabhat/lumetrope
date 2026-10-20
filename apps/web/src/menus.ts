// Context-menu item builders for each area of the app.
import { act } from "./actions";
import { commands } from "./commands";
import { SEP, type CtxItem } from "./contextMenu";
import type { LayerNode } from "./doc";
import { PRESET_GROUPS } from "./presets";
import { useEditor } from "./store";

const st = () => useEditor.getState();
const frame = () => Math.floor(st().frame);

const presetSubmenu = (): CtxItem[] =>
  PRESET_GROUPS.flatMap((g, gi) => [
    ...(gi ? [SEP] : []),
    ...g.items.map((p) => ({ label: p.label, onSelect: () => commands.preset(p.id) })),
  ]);

const alignSubmenu = (): CtxItem[] => [
  { label: "Left", onSelect: () => commands.align("left") },
  { label: "Center", onSelect: () => commands.align("center") },
  { label: "Right", onSelect: () => commands.align("right") },
  SEP,
  { label: "Top", onSelect: () => commands.align("top") },
  { label: "Middle", onSelect: () => commands.align("middle") },
  { label: "Bottom", onSelect: () => commands.align("bottom") },
];

/** Menu for one or more selected layers (canvas, layer list, timeline). */
export function layerMenu(layers: LayerNode[], extra: CtxItem[] = []): CtxItem[] {
  const one = layers.length === 1 ? layers[0] : null;
  const ids = layers.map((l) => l.id);
  return [
    ...extra,
    { label: "Cut", shortcut: "⌘X", onSelect: commands.cut },
    { label: "Copy", shortcut: "⌘C", onSelect: commands.copy },
    { label: "Paste", shortcut: "⌘V", onSelect: () => void commands.paste() },
    { label: "Duplicate", shortcut: "⌘D", onSelect: commands.duplicate },
    SEP,
    ...(one
      ? [
          {
            label: "Rename",
            onSelect: () => {
              st().setLeftTab("layers");
              st().setRenaming(one.id);
            },
          },
        ]
      : []),
    ...(one?.type === "precomp" && one.scene ? [{ label: "Open nested scene", onSelect: () => st().setScene(one.scene!) }] : []),
    { label: "Group into nested scene", shortcut: "⌘G", onSelect: commands.precompose },
    {
      label: "Arrange",
      submenu: [
        { label: "Bring to front", onSelect: () => commands.reorder("front") },
        { label: "Bring forward", onSelect: () => commands.reorder("up") },
        { label: "Send backward", onSelect: () => commands.reorder("down") },
        { label: "Send to back", onSelect: () => commands.reorder("back") },
      ],
    },
    { label: "Align", submenu: alignSubmenu() },
    ...(layers.length >= 3
      ? [
          {
            label: "Distribute",
            submenu: [
              { label: "Horizontally", onSelect: () => act("distribute_layers", { layers: ids, axis: "horizontal" }) },
              { label: "Vertically", onSelect: () => act("distribute_layers", { layers: ids, axis: "vertical" }) },
            ],
          },
        ]
      : []),
    {
      label: "Transform",
      submenu: [
        { label: "Flip horizontal", onSelect: () => ids.forEach((id) => act("flip_layer", { layer: id, axis: "horizontal" })) },
        { label: "Flip vertical", onSelect: () => ids.forEach((id) => act("flip_layer", { layer: id, axis: "vertical" })) },
        SEP,
        { label: "Anchor to center", onSelect: () => ids.forEach((id) => act("set_anchor_point", { layer: id, anchor: "center" })) },
        { label: "Reset rotation", onSelect: () => ids.forEach((id) => act("set_property", { layer: id, property: "rotation", value: 0, frame: frame() })) },
        { label: "Reset scale", onSelect: () => ids.forEach((id) => act("set_property", { layer: id, property: "scale", value: [100, 100], frame: frame() })) },
      ],
    },
    { label: "Animate", submenu: presetSubmenu() },
    ...(one
      ? [
          {
            label: "Add",
            submenu: [
              { label: "Rectangle mask", onSelect: () => act("add_mask", { layer: one.id, shape: "rect" }) },
              { label: "Ellipse mask", onSelect: () => act("add_mask", { layer: one.id, shape: "ellipse" }) },
              SEP,
              { label: "Drop shadow", onSelect: () => act("add_effect", { layer: one.id, type: "drop_shadow" }) },
              { label: "Gaussian blur", onSelect: () => act("add_effect", { layer: one.id, type: "gaussian_blur" }) },
              { label: "Tint", onSelect: () => act("add_effect", { layer: one.id, type: "tint" }) },
              ...(one.type === "shape"
                ? [
                    SEP,
                    { label: "Trim paths", onSelect: () => act("add_trim_path", { layer: one.id }) },
                    { label: "Round corners", onSelect: () => act("add_round_corners", { layer: one.id, radius: 10 }) },
                    { label: "Repeater", onSelect: () => act("add_repeater", { layer: one.id, copies: 3, position: [40, 0] }) },
                  ]
                : []),
            ],
          },
        ]
      : []),
    SEP,
    {
      label: one?.hidden ? "Show" : "Hide",
      onSelect: () => layers.forEach((l) => act("set_layer_visibility", { layer: l.id, visible: !!one?.hidden })),
    },
    { label: one?.locked ? "Unlock" : "Lock", onSelect: () => layers.forEach((l) => act("set_layer_lock", { layer: l.id, locked: !one?.locked })) },
    { label: "Zoom to selection", shortcut: "⇧2", onSelect: commands.zoomToSelection },
    SEP,
    { label: "Delete", shortcut: "⌫", danger: true, onSelect: commands.remove },
  ];
}

/** Menu on empty canvas space. `at` is the composition point clicked. */
export function canvasMenu(at: [number, number]): CtxItem[] {
  const s = st();
  const add = (type: string, extra: object = {}) => {
    const r = act<{ id: string }>("create_shape", { type, position: at, scene: s.scene, ...extra });
    if (r) s.select([r.id]);
  };
  return [
    { label: "Paste", shortcut: "⌘V", onSelect: () => void commands.paste() },
    { label: "Select all", shortcut: "⌘A", onSelect: commands.selectAll },
    SEP,
    {
      label: "Add here",
      submenu: [
        { label: "Rectangle", onSelect: () => add("rect") },
        { label: "Ellipse", onSelect: () => add("ellipse") },
        { label: "Polygon", onSelect: () => add("polygon", { points: 6 }) },
        { label: "Star", onSelect: () => add("star") },
        {
          label: "Text",
          onSelect: () => {
            const r = act<{ id: string }>("create_text", { text: "Your text", position: at, scene: s.scene });
            if (r) s.select([r.id]);
          },
        },
        { label: "Null (controller)", onSelect: () => act("create_null", { position: at, scene: s.scene }) },
      ],
    },
    { label: "Import…", onSelect: () => void commands.import() },
    SEP,
    { label: "Zoom to fit", shortcut: "⇧1", onSelect: commands.fit },
    { label: "Zoom to 100%", shortcut: "⇧0", onSelect: () => s.zoomTo(1) },
    { label: "Zoom in", shortcut: "⌘+", onSelect: () => s.zoomStep(1) },
    { label: "Zoom out", shortcut: "⌘−", onSelect: () => s.zoomStep(-1) },
    SEP,
    { label: "Snapping", checked: s.snapping, onSelect: () => s.setSnapping(!s.snapping) },
    { label: "Grid", checked: s.grid, onSelect: () => s.setGrid(!s.grid) },
    SEP,
    { label: "Scene settings", onSelect: () => s.select([]) },
  ];
}

const EASINGS = ["linear", "ease", "ease_in", "ease_out", "ease_in_out", "smooth", "ease_out_back", "snappy"];

/** Menu on a timeline keyframe. */
export function keyframeMenu(layer: string, path: string, f: number, onEditEasing: () => void): CtxItem[] {
  return [
    { label: "Edit easing…", onSelect: onEditEasing },
    { label: "Easing", submenu: EASINGS.map((e) => ({ label: e.replace(/_/g, " "), onSelect: () => act("set_keyframe_easing", { layer, path, frame: f, easing: e }) })) },
    { label: "Hold (step)", onSelect: () => act("set_keyframe_easing", { layer, path, frame: f, easing: "hold" }) },
    SEP,
    { label: "Go to keyframe", onSelect: () => st().setFrame(f) },
    { label: "Move to playhead", disabled: frame() === f, onSelect: () => act("move_keyframe", { layer, path, frame: f, to: frame() }) },
    SEP,
    { label: "Delete keyframe", danger: true, onSelect: () => act("remove_keyframe", { layer, path, frame: f }) },
  ];
}

/** Menu on a property row in the properties panel. */
export function propertyMenu(layer: string, property: string, shape: string | null | undefined, state: { animated: boolean; keyed: boolean; slot: string | null; value: unknown }): CtxItem[] {
  const base = { layer, property, ...(shape ? { shape } : {}) };
  return [
    state.keyed
      ? { label: "Remove keyframe", onSelect: () => act("remove_keyframe", { ...base, frame: frame() }) }
      : { label: "Add keyframe here", onSelect: () => act("set_keyframe", { ...base, frame: frame(), value: state.value }) },
    { label: "Clear animation", disabled: !state.animated, onSelect: () => act("clear_animation", { ...base, frame: frame() }) },
    SEP,
    state.slot
      ? { label: `Unlink slot “${state.slot}”`, onSelect: () => act("unlink_slot", base) }
      : { label: "Expose as theme slot", onSelect: () => act("create_slot", base) },
    SEP,
    { label: "Copy value", onSelect: () => void navigator.clipboard?.writeText(JSON.stringify(state.value)) },
    {
      label: "Paste value",
      onSelect: () =>
        void navigator.clipboard?.readText().then((t) => {
          try {
            act("set_property", { ...base, value: JSON.parse(t), frame: frame() });
          } catch {
            st().toast("Clipboard does not contain a value", "error");
          }
        }),
    },
  ];
}

/** Menu on a shape item in the contents tree. */
export function shapeItemMenu(layer: string, path: string, hidden: boolean): CtxItem[] {
  const idx = Number(path.split("/").pop());
  return [
    { label: hidden ? "Show" : "Hide", onSelect: () => act("update_shape_item", { layer, shape: path, hidden: !hidden }) },
    { label: "Move up", disabled: idx === 0, onSelect: () => act("reorder_shape", { layer, shape: path, index: idx - 1 }) },
    { label: "Move down", onSelect: () => act("reorder_shape", { layer, shape: path, index: idx + 1 }) },
    SEP,
    { label: "Delete", danger: true, onSelect: () => (act("remove_shape_item", { layer, shape: path }), st().select([layer])) },
  ];
}

/** Menu on the timeline ruler / empty track area. */
export function timelineMenu(at: number): CtxItem[] {
  const s = st();
  return [
    { label: "Move playhead here", onSelect: () => s.setFrame(at) },
    {
      label: "Add segment from here",
      onSelect: () => {
        const end = Math.min(s.info?.op ?? at + 30, at + 30);
        if (act("add_segment", { name: `segment_${at}`, start: at, end: end > at ? end : at + 1 }) !== undefined) {
          s.setLeftTab("segments");
          s.toast(`Added segment_${at} — rename it in the Segments panel`);
        }
      },
    },
    SEP,
    { label: s.playing ? "Pause" : "Play", shortcut: "Space", onSelect: () => s.setPlaying(!s.playing) },
    { label: "Loop", checked: s.loop, onSelect: () => s.setLoop(!s.loop) },
  ];
}
