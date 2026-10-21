export interface ToolRequest {
  id: string;
  tool: string;
  args: Record<string, unknown>;
}

export function parseRequest(raw: string): ToolRequest | null {
  try {
    const m = JSON.parse(raw);
    if (m?.type !== "tool_request" || typeof m.id !== "string" || typeof m.tool !== "string") return null;
    return { id: m.id, tool: m.tool, args: m.args && typeof m.args === "object" ? m.args : {} };
  } catch {
    return null;
  }
}

export function response(id: string, result?: unknown, error?: string): string {
  return JSON.stringify(error !== undefined ? { type: "tool_response", id, error } : { type: "tool_response", id, result: result ?? null });
}

export const backoffMs = (attempt: number) => Math.min(10000, 1000 * 2 ** attempt);
