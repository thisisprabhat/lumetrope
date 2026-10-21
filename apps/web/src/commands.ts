// Editor commands shared by menus and keyboard shortcuts.
import { act, query, redo, undo } from "./actions";
import { engine, geometry, type Geometry } from "./engine";
import { download, openFile, pickFile } from "./files";
import { importFile } from "./panels/LeftSidebar";
import { useEditor } from "./store";
import { exportFileName } from "./lib/export";
import { loadProject, newProjectId } from "./persist";
import { commitNow } from "./panels/HistoryPanel";

const st = () => useEditor.getState();

let clipboard: any[] = [];

export const commands = {
  copy() {
    const ids = st().selection;
    if (!ids.length) return;
    clipboard = ids.map((id) => query<any>("inspect_layer", { layer: id }, null)?.json).filter(Boolean);
    try {
      void navigator.clipboard?.writeText(JSON.stringify({ lumetrope: "layers", layers: clipboard }));
    } catch {
      /* clipboard permission denied: the in-app copy still works */
    }
    st().toast(`Copied ${clipboard.length} layer${clipboard.length > 1 ? "s" : ""}`);
  },
  cut() {
    commands.copy();
    commands.remove();
  },
  async paste() {
    let layers = clipboard;
    try {
      const text = await navigator.clipboard?.readText();
      const parsed = text ? JSON.parse(text) : null;
      if (parsed?.lumetrope === "layers" && Array.isArray(parsed.layers)) layers = parsed.layers;
      else if (parsed && (parsed.layers || parsed.w)) {
        const r = act<{ id: string }>("import_lottie", { json: parsed });
        if (r) st().select([r.id]);
        return;
      } else if (text && text.trim().startsWith("<svg")) {
        const r = act<{ id: string }>("import_svg", { svg: text, scene: st().scene });
        if (r) st().select([r.id]);
        return;
      }
    } catch {
      /* not JSON / no clipboard access: fall back to the in-app clipboard */
    }
    if (!layers.length) return;
    const r = act<{ ids: string[] }>("paste_layers", { layers, scene: st().scene, offset: [10, 10] });
    if (r) st().select(r.ids);
  },
  reorder(to: "front" | "back" | "up" | "down") {
    for (const id of st().selection) act("reorder_layer", { layer: id, to });
  },
  align(align: string) {
    const ids = st().selection;
    if (ids.length) act("align_layers", { layers: ids, align, to: ids.length > 1 ? "selection" : "scene" });
  },
  preset(preset: string) {
    const ids = st().selection;
    const frame = Math.floor(st().frame);
    if (ids.length === 1) act("apply_motion_preset", { layer: ids[0], preset, start_frame: frame });
    else if (ids.length > 1) act("stagger_layers", { layers: ids, preset, start_frame: frame, delay_frames: 4 });
  },
  newProject() {
    engine().new_project(512, 512, 30, 90);
    engine().vcs_reset();
    useEditor.setState({ projectId: newProjectId() });
    st().bumpHistory();
    st().select([]);
    st().setScene("root");
    st().setFileName("Untitled");
    st().setFrame(0);
    st().refresh();
    window.dispatchEvent(new Event("lumetrope:fit"));
  },
  async open() {
    const f = await pickFile();
    if (f) await openFile(f);
  },
  async import() {
    const f = await pickFile(".svg,.json,.png,.jpg,.jpeg,.ttf,.otf,.woff,.woff2");
    if (f) await importFile(f);
  },
  exportJson() {
    download(`${st().fileName || "animation"}.json`, engine().export_lottie(), "application/json");
    st().toast("Exported Lottie JSON");
  },
  /** Save the whole project as a dotLottie file (the native project format). */
  saveProject() {
    try {
      download(exportFileName(st().fileName || "project", "lottie"), engine().export_dotlottie(), "application/zip+dotlottie");
      st().toast("Saved project (.lottie)");
    } catch (e) {
      st().toast(String(e), "error");
    }
  },
  saveVersion() {
    st().setLeftTab("history");
    commitNow("");
  },
  async openRecent(id: string) {
    if (await loadProject(id)) st().toast("Opened from local library");
  },
  openExport() {
    st().setExportOpen(true);
  },
  undo,
  redo,
  duplicate() {
    const ids = st().selection;
    const out: string[] = [];
    for (const id of ids) {
      const r = act<{ id: string }>("duplicate_layer", { layer: id });
      if (r) out.push(r.id);
    }
    if (out.length) st().select(out);
  },
  remove() {
    const { selection, shape } = st();
    if (shape && selection.length === 1) {
      act("remove_shape_item", { layer: selection[0], shape });
      st().select(selection);
      return;
    }
    for (const id of selection) act("delete_layer", { layer: id });
    st().select([]);
  },
  selectAll() {
    try {
      const s = JSON.parse(engine().apply("read_scene", JSON.stringify({ scene: st().scene })));
      st().select((s.ok?.layers ?? []).filter((l: any) => !l.locked).map((l: any) => l.id));
    } catch {
      /* ignore */
    }
  },
  nudge(dx: number, dy: number) {
    const ids = st().selection;
    if (ids.length) act("nudge_layers", { layers: ids, dx, dy, frame: Math.floor(st().frame) });
  },
  precompose() {
    const ids = st().selection;
    if (!ids.length) return;
    const r = act<{ id: string }>("precompose_layers", { layers: ids, name: "Nested Scene" });
    if (r) st().select([r.id]);
  },
  fit() {
    st().fitView();
  },
  zoomToSelection() {
    const f = Math.floor(st().frame);
    const gs = st()
      .selection.map((id) => geometry(id, f))
      .filter((g): g is Geometry => !!g && g.bounds[2] > g.bounds[0]);
    if (!gs.length) return st().fitView();
    const b: [number, number, number, number] = [Infinity, Infinity, -Infinity, -Infinity];
    for (const g of gs) {
      b[0] = Math.min(b[0], g.bounds[0]);
      b[1] = Math.min(b[1], g.bounds[1]);
      b[2] = Math.max(b[2], g.bounds[2]);
      b[3] = Math.max(b[3], g.bounds[3]);
    }
    st().fitBounds(b);
  },
};
