// Editor commands shared by menus and keyboard shortcuts.
import { act, redo, undo } from "./actions";
import { engine } from "./engine";
import { download, openFile, pickFile } from "./files";
import { importFile } from "./panels/LeftSidebar";
import { useEditor } from "./store";

const st = () => useEditor.getState();

export const commands = {
  newProject() {
    engine().new_project(512, 512, 30, 90);
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
  saveProjectJson() {
    download(`${st().fileName || "project"}.lumetrope.json`, engine().project_json(), "application/json");
    st().toast("Saved project");
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
    window.dispatchEvent(new Event("lumetrope:fit"));
  },
};
