export interface Diff {
  layers_added: string[];
  layers_removed: string[];
  layers_changed: string[];
  settings_changed: string[];
  state_machines_changed: string[];
  themes_changed: string[];
  segments_changed: boolean;
}

export function relativeTime(ts: number, now = Date.now()): string {
  const s = Math.max(0, (now - ts) / 1000);
  if (s < 45) return "just now";
  if (s < 3600) return `${Math.max(1, Math.floor(s / 60))} min ago`;
  if (s < 86400) return `${Math.floor(s / 3600)} h ago`;
  const d = Math.floor(s / 86400);
  return d === 1 ? "yesterday" : `${d} days ago`;
}

export function changeSummary(d: Diff): string {
  const parts: string[] = [];
  const n = (k: number, w: string) => `${k} ${w}${k === 1 ? "" : "s"}`;
  if (d.layers_added.length) parts.push(`+${n(d.layers_added.length, "layer")}`);
  if (d.layers_removed.length) parts.push(`−${n(d.layers_removed.length, "layer")}`);
  if (d.layers_changed.length) parts.push(`${d.layers_changed.length} edited`);
  if (d.settings_changed.length) parts.push("scene settings");
  if (d.state_machines_changed.length) parts.push("state machines");
  if (d.themes_changed.length) parts.push("themes");
  if (d.segments_changed) parts.push("segments");
  return parts.length ? parts.join(" · ") : "no changes";
}
