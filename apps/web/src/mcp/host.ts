// Executes MCP tool calls inside the app: engine catalogue tools plus host
// tools (editor state, frame snapshots, history, state machine testing).
import { redo, undo } from "../actions";
import { call, engine, readInfo } from "../engine";
import { CanvasRenderer } from "../gpu/CanvasRenderer";
import { commitNow } from "../panels/HistoryPanel";
import { useEditor, type SmSnapshot } from "../store";

const st = () => useEditor.getState();

async function frameImage(frame: number | undefined, width: number | undefined) {
  const info = readInfo();
  const f = frame ?? Math.floor(st().frame);
  const w = Math.max(16, Math.min(2048, Math.round(width ?? 512)));
  const h = Math.max(16, Math.round((w * info.h) / info.w));
  const c = document.createElement("canvas");
  c.width = w;
  c.height = h;
  const r = new CanvasRenderer();
  const imgs = JSON.parse(engine().image_assets()) as Record<string, string>;
  if (Object.keys(imgs).length) {
    await new Promise<void>((done) => {
      let left = Object.keys(imgs).length;
      r.onImageLoad = () => --left <= 0 && done();
      r.setImages(imgs);
      setTimeout(done, 1500);
    });
  }
  r.render(c.getContext("2d")!, JSON.parse(engine().display_list(f, w / info.w, 0, 0)), w, h, [0, 0, w, h]);
  return { png: c.toDataURL("image/png").split(",")[1], frame: f, width: w, height: h };
}

function snap(json: string): SmSnapshot | null {
  const s = JSON.parse(json) as SmSnapshot | null;
  if (s) {
    st().logSm(s.events);
    st().setSmSnap(s);
    useEditor.setState({ frame: s.frame });
  }
  return s;
}

function requirePreview() {
  if (!st().smSnap) throw new Error("No state machine preview is running. Call preview_state_machine first.");
}

export async function runTool(tool: string, args: Record<string, any>): Promise<unknown> {
  switch (tool) {
    case "get_editor_state": {
      const s = st();
      return {
        file: s.fileName,
        mode: s.mode,
        frame: Math.floor(s.frame),
        selection: s.selection,
        scene: s.scene,
        state_machine: s.machine,
        preview: s.smSnap ? { state: s.smSnap.state, inputs: s.smSnap.inputs } : null,
        info: readInfo(),
        unsaved_changes: JSON.parse(engine().vcs_status()),
      };
    }
    case "get_frame_image":
      return frameImage(args.frame, args.width);
    case "set_current_frame":
      st().setFrame(Number(args.frame) || 0);
      return { frame: Math.floor(st().frame) };
    case "undo":
      undo();
      return { ok: true };
    case "redo":
      redo();
      return { ok: true };
    case "save_version": {
      const id = commitNow(String(args.message ?? ""));
      if (!id) throw new Error("Nothing to save: no changes since the last version");
      return { id };
    }
    case "list_versions":
      return JSON.parse(engine().vcs_log());
    case "restore_version":
      engine().vcs_restore(String(args.id));
      st().refresh();
      st().bumpHistory();
      return { ok: true };
    case "preview_state_machine": {
      st().setMode("preview");
      st().setMachine(String(args.machine));
      await new Promise((r) => setTimeout(r, 60));
      const s = st().smSnap;
      if (!s) throw new Error(`Could not start state machine '${args.machine}'`);
      return s;
    }
    case "state_machine_input":
      requirePreview();
      return snap(engine().sm_set_input(String(args.name), JSON.stringify(args.value ?? null)));
    case "state_machine_pointer":
      requirePreview();
      return snap(engine().sm_pointer(String(args.kind), Number(args.x), Number(args.y)));
    case "state_machine_advance": {
      requirePreview();
      let last = "null";
      const steps = Math.ceil(Math.min(60, Number(args.seconds) || 0) * 60);
      const events: unknown[] = [];
      for (let i = 0; i < steps; i++) {
        last = engine().sm_tick(1 / 60);
        events.push(...(JSON.parse(last)?.events ?? []));
      }
      const s = snap(last);
      return s ? { ...s, events } : null;
    }
    case "stop_state_machine_preview":
      st().setMode("animate");
      return { ok: true };
    default: {
      const r = call(tool, args);
      st().refresh();
      return r;
    }
  }
}
