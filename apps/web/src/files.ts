import { engine } from "./engine";
import { useEditor } from "./store";
import { pickFile as platformPick, saveFile } from "./platform";
import { newProjectId } from "./persist";

export const OPEN_ACCEPT = ".json,.lottie,.svg,.png,.jpg,.jpeg";

/** Load a dropped/picked file into the engine and refresh the editor. */
export async function openFile(file: File): Promise<void> {
  const st = useEditor.getState();
  try {
    const bytes = new Uint8Array(await file.arrayBuffer());
    engine().load_file(file.name, bytes);
    engine().vcs_reset();
    engine().vcs_commit(`Opened ${file.name}`, "You", Date.now());
    useEditor.setState({ projectId: newProjectId(), selection: [], shape: null, scene: "root" });
    st.bumpHistory();
    st.setFileName(file.name.replace(/\.(json|lottie)$/i, ""));
    st.setFrame(0);
    st.refresh();
    st.toast(`Opened ${file.name}`);
  } catch (e) {
    st.toast(String(e), "error");
  }
}

export function pickFile(accept = OPEN_ACCEPT): Promise<File | null> {
  return platformPick(accept);
}

/** Save/download a file (native save dialog on desktop). */
export function download(name: string, data: BlobPart, mime: string): void {
  saveFile(name, data, mime).catch((e) => useEditor.getState().toast(`Could not save: ${e}`, "error"));
}

/** Fetch a same-origin URL (e.g. a bundled sample) and open it. */
export async function openUrl(url: string): Promise<void> {
  const res = await fetch(url);
  if (!res.ok) {
    useEditor.getState().toast(`Could not load ${url}`, "error");
    return;
  }
  const name = url.split("/").pop() || "file.json";
  await openFile(new File([await res.arrayBuffer()], name));
}
