// Pure helpers for the state machine editor.

export interface Guard {
  type: "Numeric" | "String" | "Boolean" | "Event";
  inputName: string;
  conditionType?: string;
  compareTo?: unknown;
}

export interface Action {
  type: string;
  inputName?: string;
  value?: unknown;
  url?: string;
  target?: string;
}

const OPS: Record<string, string> = {
  Equal: "=",
  NotEqual: "≠",
  GreaterThan: ">",
  GreaterThanOrEqual: "≥",
  LessThan: "<",
  LessThanOrEqual: "≤",
};

/**
 * Saved positions win. Unplaced states flow left→right in breadth-first layers
 * from the initial state (unreachable ones after the last layer); the global
 * state sits below everything.
 */
export function autoLayout(
  names: string[],
  saved: Record<string, [number, number]>,
  globalName?: string,
  initial?: string,
  edges: [string, string][] = [],
): Record<string, [number, number]> {
  const out: Record<string, [number, number]> = {};
  const depth = new Map<string, number>();
  const start = initial && names.includes(initial) ? initial : names.find((n) => n !== globalName);
  if (start) {
    depth.set(start, 0);
    const queue = [start];
    while (queue.length) {
      const cur = queue.shift()!;
      for (const [a, b] of edges) {
        if (a === cur && b !== globalName && !depth.has(b)) {
          depth.set(b, depth.get(cur)! + 1);
          queue.push(b);
        }
      }
    }
  }
  let maxDepth = Math.max(-1, ...depth.values());
  for (const n of names) if (n !== globalName && !depth.has(n)) depth.set(n, ++maxDepth);
  const rows = new Map<number, number>();
  let maxY = 0;
  for (const n of names) {
    if (saved[n]) {
      out[n] = saved[n];
      maxY = Math.max(maxY, saved[n][1]);
    }
  }
  for (const n of names) {
    if (out[n] || n === globalName) continue;
    const d = depth.get(n)!;
    const r = rows.get(d) ?? 0;
    rows.set(d, r + 1);
    out[n] = [60 + d * 330, 40 + r * 130];
    maxY = Math.max(maxY, out[n][1]);
  }
  if (globalName && !out[globalName]) out[globalName] = [60, maxY + 160];
  return out;
}

export function guardSummary(guards: Guard[] | undefined): string {
  if (!guards?.length) return "always";
  return guards
    .map((g) => (g.type === "Event" ? `on ${g.inputName}` : `${g.inputName} ${OPS[g.conditionType ?? "Equal"] ?? "="} ${typeof g.compareTo === "string" && !g.compareTo.startsWith("$") ? `"${g.compareTo}"` : String(g.compareTo)}`))
    .join(" & ");
}

export function actionSummary(a: Action): string {
  switch (a.type) {
    case "Toggle":
      return `toggle ${a.inputName}`;
    case "Increment":
      return `${a.inputName} += ${a.value ?? 1}`;
    case "Decrement":
      return `${a.inputName} −= ${a.value ?? 1}`;
    case "SetBoolean":
    case "SetNumeric":
    case "SetString":
      return `${a.inputName} = ${typeof a.value === "string" && !a.value.startsWith("$") ? `"${a.value}"` : String(a.value)}`;
    case "Fire":
      return `fire ${a.inputName}`;
    case "Reset":
      return `reset ${a.inputName}`;
    case "SetFrame":
      return `go to frame ${a.value}`;
    case "SetProgress":
      return `go to ${Math.round(Number(a.value) * 100)}%`;
    case "OpenUrl":
      return `open ${a.url}`;
    case "SetTheme":
      return `theme ${a.value}`;
    case "FireCustomEvent":
      return `emit "${a.value}"`;
    default:
      return a.type;
  }
}

const SNAKE: Record<string, string> = {
  Equal: "equal",
  NotEqual: "not_equal",
  GreaterThan: "greater_than",
  GreaterThanOrEqual: "greater_than_or_equal",
  LessThan: "less_than",
  LessThanOrEqual: "less_than_or_equal",
};

/** Spec guards → the add/update_transition tool's friendly guard args. */
export function toGuardArgs(guards: Guard[] | undefined): { input: string; condition?: string; value?: unknown }[] {
  return (guards ?? []).map((g) => (g.type === "Event" ? { input: g.inputName } : { input: g.inputName, condition: SNAKE[g.conditionType ?? "Equal"] ?? "equal", value: g.compareTo }));
}

export const ACTION_TYPES = ["Toggle", "SetBoolean", "SetNumeric", "SetString", "Increment", "Decrement", "Fire", "Reset", "SetFrame", "SetProgress", "SetTheme", "OpenUrl", "FireCustomEvent"] as const;
export const INTERACTION_TYPES = ["Click", "PointerDown", "PointerUp", "PointerEnter", "PointerExit", "PointerMove", "OnComplete", "OnLoopComplete"] as const;
export const CONDITIONS = Object.keys(OPS);
