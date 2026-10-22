// Shortcut labels are written with macOS glyphs (⌘ ⇧ ⌥) and spelled out as
// Ctrl+Shift+… on Windows and Linux. Key handling accepts both ⌘ and Ctrl.

interface NavLike {
  platform?: string;
  userAgentData?: { platform?: string };
}

export function isMacPlatform(nav: NavLike = typeof navigator !== "undefined" ? (navigator as NavLike) : {}): boolean {
  const p = nav.userAgentData?.platform || nav.platform || "";
  return /mac|iphone|ipad/i.test(p);
}

const IS_MAC = isMacPlatform();

const NAMES: [string, string][] = [
  ["⌘", "Ctrl"],
  ["⇧", "Shift"],
  ["⌥", "Alt"],
];

export function keyLabel(shortcut: string, mac: boolean = IS_MAC): string {
  if (mac) return shortcut;
  const mods = NAMES.filter(([g]) => shortcut.includes(g)).map(([, n]) => n);
  const key = NAMES.reduce((s, [g]) => s.split(g).join(""), shortcut);
  return [...mods, key].join("+");
}
