// Mutations from the UI: run a tool, refresh the editor, report errors.
import { call, engine, ToolError } from "./engine";
import { useEditor } from "./store";

/** Apply a mutating tool and refresh. Returns the result, or undefined after showing an error toast. */
export function act<T = any>(tool: string, args: object = {}): T | undefined {
  try {
    const r = call<T>(tool, args);
    useEditor.getState().refresh();
    return r;
  } catch (e) {
    useEditor.getState().toast(e instanceof ToolError ? e.message : String(e), "error");
    return undefined;
  }
}

/** Read-only query that never toasts; returns fallback on error. */
export function query<T = any>(tool: string, args: object, fallback: T): T {
  try {
    return call<T>(tool, args);
  } catch {
    return fallback;
  }
}

let batching = 0;
/** Group the tool calls of a gesture (drag) into one undo step. */
export function beginGesture(): void {
  if (batching++ === 0) engine().begin_batch();
}
export function endGesture(): void {
  if (batching > 0 && --batching === 0) {
    engine().end_batch();
    useEditor.getState().refresh();
  }
}

export function undo(): void {
  if (engine().undo()) useEditor.getState().refresh();
}
export function redo(): void {
  if (engine().redo()) useEditor.getState().refresh();
}
