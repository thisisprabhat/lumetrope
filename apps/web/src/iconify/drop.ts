// Drag payload from the icon grid onto the canvas. Files keep the existing drop path.
export const ICON_MIME = "application/x-lumetrope-icon";

export interface IconDrag {
  svg: string;
  name: string;
}

export function encodeIconDrag(svg: string, name: string): string {
  return JSON.stringify({ svg, name });
}

export function decodeIconDrag(raw: string): IconDrag | null {
  if (!raw) return null;
  try {
    const data = JSON.parse(raw) as { svg?: unknown; name?: unknown };
    if (typeof data.svg !== "string" || !data.svg.includes("<svg")) return null;
    if (typeof data.name !== "string" || !data.name.trim()) return null;
    return { svg: data.svg, name: data.name };
  } catch {
    return null;
  }
}

export function scenePoint(clientX: number, clientY: number, rect: { left: number; top: number }, camera: { x: number; y: number; zoom: number }): [number, number] {
  const zoom = camera.zoom || 1;
  return [(clientX - rect.left - camera.x) / zoom, (clientY - rect.top - camera.y) / zoom];
}

export type DropDecision = { kind: "file"; file: File } | { kind: "icon"; svg: string; name: string; position: [number, number] } | { kind: "ignore" };

export function decideDrop(input: {
  file: File | null;
  iconRaw: string;
  clientX: number;
  clientY: number;
  rect: { left: number; top: number };
  camera: { x: number; y: number; zoom: number };
}): DropDecision {
  if (input.file) return { kind: "file", file: input.file };
  const icon = decodeIconDrag(input.iconRaw);
  if (!icon) return { kind: "ignore" };
  return { kind: "icon", svg: icon.svg, name: icon.name, position: scenePoint(input.clientX, input.clientY, input.rect, input.camera) };
}
