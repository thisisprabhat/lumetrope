// Pure helpers for the dashboard: what happened last, filtering and sorting.
import type { ProjectMeta } from "./db";

export type ActivityKind = "created" | "opened" | "edited" | "previewed" | "exported";
export type ProjectFilter = "all" | "created" | "files" | "previewed" | "exported";
export type ProjectSort = "recent" | "name" | "created";

export function lastActivity(m: ProjectMeta): { kind: ActivityKind; at: number } {
  // Lowest priority first; a later entry wins ties.
  const candidates: [ActivityKind, number | undefined][] = [
    ["created", m.origin === "file" ? undefined : m.created],
    ["opened", m.origin === "file" || m.opened > m.created ? m.opened : undefined],
    ["edited", m.updated > m.created ? m.updated : undefined],
    ["previewed", m.previewed],
    ["exported", m.exported?.at],
  ];
  let best: { kind: ActivityKind; at: number } = { kind: "created", at: m.created };
  let first = true;
  for (const [kind, at] of candidates) {
    if (at === undefined) continue;
    if (first || at >= best.at) best = { kind, at };
    first = false;
  }
  return best;
}

export function filterProjects(list: ProjectMeta[], filter: ProjectFilter, query: string): ProjectMeta[] {
  const q = query.trim().toLowerCase();
  return list.filter((m) => {
    if (filter === "created" && m.origin === "file") return false;
    if (filter === "files" && !m.file) return false;
    if (filter === "previewed" && !m.previewed) return false;
    if (filter === "exported" && !m.exported) return false;
    if (!q) return true;
    const path = m.file?.link?.kind === "path" ? m.file.link.path : (m.file?.name ?? "");
    return m.name.toLowerCase().includes(q) || path.toLowerCase().includes(q);
  });
}

export function sortProjects(list: ProjectMeta[], sort: ProjectSort): ProjectMeta[] {
  const out = [...list];
  if (sort === "name") out.sort((a, b) => a.name.localeCompare(b.name, undefined, { sensitivity: "base" }));
  else if (sort === "created") out.sort((a, b) => b.created - a.created);
  else out.sort((a, b) => lastActivity(b).at - lastActivity(a).at);
  return out;
}
