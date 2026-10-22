import { useEditor } from "./store";
import { pickFile as platformPick, saveFile } from "./platform";
import { openPicked } from "./persist";

/** Open a dropped/picked Lottie or dotLottie file as a project. */
export async function openFile(file: File, handle?: FileSystemFileHandle): Promise<void> {
  await openPicked({ file, link: handle ? { kind: "handle", handle } : undefined });
}

export function pickFile(accept: string): Promise<File | null> {
  return platformPick(accept);
}

/** Save/download a file (native save dialog on desktop). */
export function download(name: string, data: BlobPart, mime: string): void {
  saveFile(name, data, mime).catch((e) => useEditor.getState().toast(`Could not save: ${e}`, "error"));
}

/** Fetch a same-origin URL (e.g. a bundled sample) and open it. */
export async function openUrl(url: string): Promise<void> {
  const res = await fetch(url).catch(() => null);
  if (!res?.ok) {
    useEditor.getState().toast(`Could not load ${url}`, "error");
    return;
  }
  const name = url.split("/").pop() || "file.json";
  await openPicked({ file: new File([await res.arrayBuffer()], name) }, "sample");
}
