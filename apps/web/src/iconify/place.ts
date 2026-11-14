// Turns a fetched Iconify SVG into the arguments import_svg already accepts.
import { fetchIconSvg, parseIconId, type IconifyError } from "./api";

export interface SvgImportArgs {
  svg: string;
  name: string;
  scene: string;
  position?: [number, number];
}

/** Size picker imports relative to the canvas, keeping the SVG's aspect ratio. */
export function sizeIconForCanvas(svg: string, canvas: { w: number; h: number }): string {
  if (!Number.isFinite(canvas.w) || !Number.isFinite(canvas.h) || canvas.w <= 0 || canvas.h <= 0) return svg;
  return svg.replace(/<svg\b[^>]*>/i, (root) => {
    const viewBox = root.match(/\bviewBox\s*=\s*["']([^"']+)["']/i)?.[1].trim().split(/[\s,]+/).map(Number);
    const width = viewBox?.[2] ?? Number.parseFloat(root.match(/\bwidth\s*=\s*["']([^"']+)["']/i)?.[1] ?? "");
    const height = viewBox?.[3] ?? Number.parseFloat(root.match(/\bheight\s*=\s*["']([^"']+)["']/i)?.[1] ?? "");
    if (!Number.isFinite(width) || !Number.isFinite(height) || width <= 0 || height <= 0) return root;
    const scale = (Math.min(canvas.w, canvas.h) * 0.25) / Math.max(width, height);
    const viewport = viewBox ? "" : ` viewBox="0 0 ${width} ${height}"`;
    return root.replace(/\s+(?:width|height)\s*=\s*["'][^"']*["']/gi, "").replace(/>$/, `${viewport} width="${width * scale}" height="${height * scale}">`);
  });
}

export function svgImportArgs(opts: { svg: string; name: string; scene: string; position?: [number, number]; canvas?: { w: number; h: number } | null }): SvgImportArgs {
  const svg = opts.canvas ? sizeIconForCanvas(opts.svg.trim(), opts.canvas) : opts.svg.trim();
  if (!svg.includes("<svg")) throw new Error("Icon SVG is empty");
  const args: SvgImportArgs = { svg, name: opts.name || "Icon", scene: opts.scene || "root" };
  if (opts.position) args.position = opts.position;
  return args;
}

export function readPosition(value: unknown): [number, number] | undefined {
  if (value == null) return undefined;
  if (!Array.isArray(value) || value.length < 2) throw new Error("position must be [x, y]");
  const x = Number(value[0]);
  const y = Number(value[1]);
  if (!Number.isFinite(x) || !Number.isFinite(y)) throw new Error("position must be [x, y]");
  return [x, y];
}

/** Download one icon and build the import_svg call. The SVG text is what copy puts on the clipboard. */
export async function iconImportArgs(icon: string, opts: { scene: string; name?: string; color?: string; position?: [number, number]; canvas?: { w: number; h: number } | null } = { scene: "root" }): Promise<SvgImportArgs> {
  if (!parseIconId(icon)) throw new Error("Icon must look like prefix:name");
  const svg = await fetchIconSvg(icon, { color: opts.color });
  return svgImportArgs({ svg, name: opts.name?.trim() || icon, scene: opts.scene || "root", position: opts.position, canvas: opts.canvas });
}

export function iconifyMessage(e: unknown, fallback: string): string {
  return e instanceof Error && e.message ? e.message : fallback;
}

export function isOffline(e: unknown): boolean {
  return typeof e === "object" && e !== null && (e as IconifyError).offline === true;
}
