// Trying colour schemes on the open animation. Each pick applies instantly as
// one undo step; picking another look first undoes the previous one (when
// nothing else changed since), so looks never stack and the mapping always
// starts from the animation's own colours.
import { call, engine, ToolError } from "./engine";
import type { SourceColor } from "./lib/palette";
import { useEditor } from "./store";

export type Look = { kind: "scheme"; id: string } | { kind: "outline"; inverted: boolean };

interface Session {
  /** Colours before the look was applied. */
  base: SourceColor[];
  /** Document revision right after applying; any other change ends the session. */
  rev: number;
  /** Whether applying recorded an undo step (a map that matched nothing doesn't). */
  undoable: boolean;
  look: Look;
}

let session: Session | null = null;

const live = () => !!session && session.rev === useEditor.getState().revision;

/** The animation's colours as they were before the current look (its current colours when none is applied). */
export function baseColors(): SourceColor[] {
  if (live()) return session!.base;
  try {
    return call<SourceColor[]>("list_colors", {});
  } catch {
    return [];
  }
}

export function currentLook(): Look | null {
  return live() ? session!.look : null;
}

/** Replace the current look with `look`; `step` turns the original colours into the tool call to run. */
export function applyLook(look: Look, step: (base: SourceColor[]) => { tool: string; args: object }): boolean {
  const st = useEditor.getState();
  const wasLive = live();
  const base = baseColors();
  if (wasLive && session!.undoable) engine().undo();
  const { tool, args } = step(base);
  let undoable = false;
  try {
    const r = call<{ replaced?: number }>(tool, args);
    undoable = tool !== "apply_color_map" || (r.replaced ?? 0) > 0;
  } catch (e) {
    session = null;
    st.refresh();
    st.toast(e instanceof ToolError ? e.message : String(e), "error");
    return false;
  }
  st.refresh();
  session = { base, rev: useEditor.getState().revision, undoable, look };
  return true;
}

/** Back to the colours from before the current look. */
export function revertLook(): void {
  if (live() && session!.undoable) engine().undo();
  session = null;
  useEditor.getState().refresh();
}
