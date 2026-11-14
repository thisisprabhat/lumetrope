// Host MCP tools. They fetch from Iconify, then hand markup to import_svg.
import { searchIcons } from "./api";
import { iconImportArgs, readPosition, type SvgImportArgs } from "./place";

export async function iconifySearch(args: Record<string, unknown>): Promise<{ icons: string[]; total: number; collections: Record<string, { name: string; license: string }> }> {
  const query = String(args.query ?? "").trim();
  if (!query) throw new Error("missing 'query'");
  const prefix = args.prefix != null && String(args.prefix).trim() !== "" ? String(args.prefix).trim() : undefined;
  const result = await searchIcons(query, { limit: args.limit, prefix });
  return { icons: result.icons, total: result.total, collections: result.collections };
}

export async function iconifyImport(args: Record<string, unknown>, importSvg: (args: SvgImportArgs) => { id?: string }): Promise<{ id?: string }> {
  const icon = String(args.icon ?? "").trim();
  return importSvg(
    await iconImportArgs(icon, {
      scene: args.scene != null && String(args.scene).trim() !== "" ? String(args.scene) : "root",
      name: args.name != null ? String(args.name) : undefined,
      color: args.color != null ? String(args.color) : undefined,
      position: readPosition(args.position),
    }),
  );
}
