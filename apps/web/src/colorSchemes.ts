// Colour-scheme presets for recolouring an animation, plus the user's own
// presets (kept in localStorage and included in workspace backups).
import { fromOklab, normalizeHex, rgbToHex } from "./lib/palette";

export interface ColorScheme {
  id: string;
  name: string;
  category: string;
  colors: string[];
  custom?: boolean;
}

const RAW: [category: string, name: string, colors: string][] = [
  // Editor themes
  ["Editor themes", "Nord", "2e3440 5e81ac 88c0d0 a3be8c ebcb8b eceff4"],
  ["Editor themes", "Dracula", "282a36 44475a ff79c6 bd93f9 8be9fd 50fa7b f8f8f2"],
  ["Editor themes", "Solarized Dark", "002b36 073642 268bd2 2aa198 b58900 cb4b16 fdf6e3"],
  ["Editor themes", "Solarized Light", "fdf6e3 eee8d5 93a1a1 268bd2 d33682 586e75"],
  ["Editor themes", "Gruvbox Dark", "282828 3c3836 fb4934 fabd2f b8bb26 83a598 ebdbb2"],
  ["Editor themes", "Gruvbox Light", "fbf1c7 ebdbb2 9d0006 b57614 79740e 076678 3c3836"],
  ["Editor themes", "Monokai", "272822 f92672 fd971f e6db74 a6e22e 66d9ef f8f8f2"],
  ["Editor themes", "One Dark", "282c34 e06c75 e5c07b 98c379 61afef c678dd abb2bf"],
  ["Editor themes", "Catppuccin Mocha", "1e1e2e f38ba8 fab387 f9e2af a6e3a1 89b4fa cba6f7 cdd6f4"],
  ["Editor themes", "Catppuccin Latte", "eff1f5 d20f39 fe640b df8e1d 40a02b 1e66f5 8839ef 4c4f69"],
  ["Editor themes", "Tokyo Night", "1a1b26 f7768e ff9e64 e0af68 9ece6a 7aa2f7 bb9af7 c0caf5"],
  ["Editor themes", "Rosé Pine", "191724 eb6f92 f6c177 ebbcba 31748f 9ccfd8 c4a7e7 e0def4"],
  ["Editor themes", "Rosé Pine Dawn", "faf4ed b4637a ea9d34 d7827e 286983 56949f 907aa9 575279"],
  ["Editor themes", "Everforest", "2d353b e67e80 e69875 dbbc7f a7c080 83c092 7fbbb3 d3c6aa"],
  ["Editor themes", "Kanagawa", "1f1f28 c34043 ffa066 c0a36e 76946a 7e9cd8 957fb8 dcd7ba"],
  ["Editor themes", "Ayu Mirage", "1f2430 f28779 ffcc66 d5ff80 73d0ff dfbfff cccac2"],
  ["Editor themes", "Material Ocean", "0f111a f07178 ffcb6b c3e88d 82aaff c792ea 89ddff 8f93a2"],
  ["Editor themes", "GitHub Dark", "0d1117 ff7b72 ffa657 d2a8ff 79c0ff 7ee787 c9d1d9"],
  ["Editor themes", "Synthwave '84", "262335 fe4450 ff8b39 fede5d 72f1b8 36f9f6 ff7edb f4eee4"],
  ["Editor themes", "Night Owl", "011627 ef5350 f78c6c ffcb8b addb67 82aaff c792ea d6deeb"],
  // Pastel
  ["Pastel", "Cotton Candy", "ffc8dd ffafcc bde0fe a2d2ff cdb4db"],
  ["Pastel", "Sorbet", "ffadad ffd6a5 fdffb6 caffbf 9bf6ff a0c4ff bdb2ff"],
  ["Pastel", "Macaron", "f7d6e0 f2b5d4 eff7f6 b2f7ef 7bdff2"],
  ["Pastel", "Lavender Fields", "e0c3fc c8b6ff b8c0ff bbd0ff ffd6ff"],
  ["Pastel", "Peach Cream", "fff1e6 fde2e4 fad2e1 e2ece9 bee1e6"],
  ["Pastel", "Mint Tea", "d8f3dc b7e4c7 95d5b2 74c69d 52b788"],
  ["Pastel", "Baby Blue", "edf6f9 caf0f8 ade8f4 90e0ef 48cae4"],
  ["Pastel", "Spring Bloom", "fec5bb fcd5ce fae1dd e8e8e4 d8e2dc ece4db"],
  ["Pastel", "Soft Dawn", "fbf8cc fde4cf ffcfd2 f1c0e8 cfbaf0 a3c4f3 90dbf4"],
  ["Pastel", "Sherbet", "ffcdb2 ffb4a2 e5989b b5838d 6d6875"],
  ["Pastel", "Gelato", "f9dcc4 f8edeb fec89a ffd7ba fae1dd"],
  ["Pastel", "Hologram", "e0c3fc 8ec5fc a1ffce faffd1 f9f7ff"],
  // Vibrant
  ["Vibrant", "Primary Pop", "e63946 f1c40f 1d70b8 ffffff 111111"],
  ["Vibrant", "Rainbow", "ff595e ffca3a 8ac926 1982c4 6a4c93"],
  ["Vibrant", "Electric", "f72585 b5179e 7209b7 3a0ca3 4361ee 4cc9f0"],
  ["Vibrant", "Tropical", "ff006e fb5607 ffbe0b 8338ec 3a86ff"],
  ["Vibrant", "Citrus Splash", "ff9f1c ffbf69 ffffff cbf3f0 2ec4b6"],
  ["Vibrant", "Festival", "ef476f ffd166 06d6a0 118ab2 073b4c"],
  ["Vibrant", "Candy Shop", "ff4d6d ff758f ffb3c1 ffd60a 4cc9f0"],
  ["Vibrant", "Memphis", "ff3c38 fff275 6699cc a23e48 000000 ffffff"],
  ["Vibrant", "Pop Art", "ff1d58 f75990 fff685 00ddff 0049b7"],
  ["Vibrant", "Fiesta", "d62828 f77f00 fcbf49 eae2b7 003049"],
  ["Vibrant", "Jelly Beans", "e71d36 ff9f1c 2ec4b6 011627 fdfffc"],
  ["Vibrant", "Arcade", "f94144 f3722c f8961e f9c74f 90be6d 43aa8b 577590"],
  // Retro
  ["Retro", "70s Groove", "3d2b1f 8c4a2f d9822b e8b04b f2dfb4"],
  ["Retro", "80s Neon Grid", "2b1055 7597de f72585 ffd319 00f5d4"],
  ["Retro", "Mid-century", "2f4858 33658a 86bbd8 f6ae2d f26419"],
  ["Retro", "Diner", "e63946 f1faee a8dadc 457b9d 1d3557"],
  ["Retro", "Polaroid", "f4f1de e07a5f 3d405b 81b29a f2cc8f"],
  ["Retro", "Handheld", "0f380f 306230 8bac0f 9bbc0f"],
  ["Retro", "CGA", "000000 55ffff ff55ff ffffff"],
  ["Retro", "Sepia", "2b1d0e 5c4033 8b6f47 c4a77d f1e3c8"],
  ["Retro", "Vaporwave", "ff71ce 01cdfe 05ffa1 b967ff fffb96"],
  ["Retro", "Art Deco", "1b1b1b c9a227 e9d8a6 0a9396 9b2226"],
  // Nature
  ["Nature", "Forest", "081c15 1b4332 2d6a4f 40916c 74c69d d8f3dc"],
  ["Nature", "Autumn", "6a040f 9d0208 d00000 dc2f02 e85d04 f48c06 faa307"],
  ["Nature", "Desert", "edc9af d4a373 ccd5ae e9edc9 fefae0 faedcd"],
  ["Nature", "Coral Reef", "ff6f59 254441 43aa8b b2b09b ef3054"],
  ["Nature", "Meadow", "606c38 283618 fefae0 dda15e bc6c25"],
  ["Nature", "Glacier", "e0fbfc c2dfe3 9db4c0 5c6b73 253237"],
  ["Nature", "Lavender Farm", "2d1e2f 5a3d5c 9e7bb5 c9b6e4 f2e9f7"],
  ["Nature", "Cherry Blossom", "ffe5ec ffc2d1 ffb3c6 ff8fab fb6f92"],
  ["Nature", "Moss & Stone", "2f3e46 354f52 52796f 84a98c cad2c5"],
  ["Nature", "Savanna", "3a2e1f 7a5c2e c08a3e e3b96a f6e7c1"],
  ["Nature", "Rainforest", "004b23 006400 007200 38b000 9ef01a"],
  ["Nature", "Volcano", "03071e 370617 6a040f d00000 e85d04 ffba08"],
  // Warm
  ["Warm", "Sunset", "355070 6d597a b56576 e56b6f eaac8b"],
  ["Warm", "Golden Hour", "f9c80e f86624 ea3546 662e9b 43bccd"],
  ["Warm", "Ember", "370617 9d0208 dc2f02 f48c06 ffba08"],
  ["Warm", "Peach Sunset", "ff9a8b ff6a88 ff99ac fcb69f ffecd2"],
  ["Warm", "Terracotta", "5f0f40 9a031e fb8b24 e36414 0f4c5c"],
  ["Warm", "Honey", "fff3b0 e09f3e 9e2a2b 540b0e 335c67"],
  ["Warm", "Spice Market", "6f1d1b bb9457 432818 99582a ffe6a7"],
  ["Warm", "Dusk", "22223b 4a4e69 9a8c98 c9ada7 f2e9e4"],
  // Cool
  ["Cool", "Deep Ocean", "03045e 023e8a 0077b6 0096c7 48cae4 ade8f4"],
  ["Cool", "Lagoon", "05668d 028090 00a896 02c39a f0f3bd"],
  ["Cool", "Arctic", "e3f2fd 90caf9 42a5f5 1565c0 0d1b2a"],
  ["Cool", "Tidepool", "264653 2a9d8f e9c46a f4a261 e76f51"],
  ["Cool", "Seaglass", "d8e2dc a3c9a8 84b59f 69a297 50808e"],
  ["Cool", "Midnight Sea", "0d1b2a 1b263b 415a77 778da9 e0e1dd"],
  ["Cool", "Blue Lagoon", "00b4d8 0077b6 90e0ef caf0f8 ffb703"],
  ["Cool", "Ice Cave", "caf0f8 e0fbfc 98c1d9 3d5a80 293241"],
  // Neon
  ["Neon", "Cyberpunk", "0d0221 261447 ff3864 2de2e6 f6019d fcee0c"],
  ["Neon", "Neon Tokyo", "120458 7a04eb ff00a0 fe75fe 00ffff"],
  ["Neon", "Acid", "0b0b0b 39ff14 ccff00 ff00ff 00ffff"],
  ["Neon", "Laser", "000000 ff0055 00e5ff ffe600 ffffff"],
  ["Neon", "Blacklight", "10002b 3c096c 7b2cbf c77dff e0aaff"],
  ["Neon", "Matrix", "000000 003b00 008f11 00ff41 d1ffd6"],
  ["Neon", "Rave", "ff0099 ff6600 ffee00 00ff66 0066ff 9900ff"],
  ["Neon", "Ultraviolet", "0b0019 2a0a4a 6a00f4 db00b6 f8f7ff"],
  // Earthy
  ["Earthy", "Clay", "5a4033 8d5b4c c38e70 e0c9a6 f2e8cf"],
  ["Earthy", "Olive Grove", "344e41 3a5a40 588157 a3b18a dad7cd"],
  ["Earthy", "Mocha", "3e2723 5d4037 8d6e63 bcaaa4 efebe9"],
  ["Earthy", "Sage & Sand", "52796f 84a98c cad2c5 e9d8a6 f7ede2"],
  ["Earthy", "Adobe", "a44a3f d4a373 f1e3d3 6b705c 3d405b"],
  ["Earthy", "Nordic Home", "3f3f37 7f7f62 a3a380 d6ccc2 f4f1ea"],
  ["Earthy", "Linen", "7f5539 b08968 d5bdaf e3d5ca f5ebe0"],
  ["Earthy", "Rust & Slate", "2b2d42 8d99ae edf2f4 ef233c d90429"],
  // Interface
  ["Interface", "Material", "f44336 ff9800 ffeb3b 4caf50 2196f3 9c27b0"],
  ["Interface", "Slate", "0f172a 334155 64748b cbd5e1 f8fafc"],
  ["Interface", "System Colours", "ff3b30 ff9500 ffcc00 34c759 007aff af52de"],
  ["Interface", "Office", "0078d4 106ebe ffb900 e81123 107c10 f3f2f1"],
  ["Interface", "Startup", "0b0f19 6366f1 22d3ee f472b6 f9fafb"],
  ["Interface", "Fintech", "0a2540 635bff 00d4ff 80e9ff f6f9fc"],
  ["Interface", "Clinic", "005f73 0a9396 94d2bd e9d8a6 ee9b00"],
  ["Interface", "Data Viz", "003f5c 58508d bc5090 ff6361 ffa600"],
  // Seasonal
  ["Seasonal", "Christmas", "165b33 146b3a f8b229 ea4630 bb2528"],
  ["Seasonal", "Halloween", "0b0b0b 3d0c11 ff6f00 ffb300 6a1b9a"],
  ["Seasonal", "Valentine", "590d22 a4133c ff4d6d ff8fa3 fff0f3"],
  ["Seasonal", "Easter", "f8e1f4 fff0b3 c1f0d6 b5d8ff e2c6ff"],
  ["Seasonal", "Summer", "00b4d8 ffd166 ef476f 06d6a0 fffcf2"],
  ["Seasonal", "Winter", "1b3a4b 3a6b8a 6ba3be b3d9e5 e8f1f2"],
  // Duotone & contrast
  ["Duotone", "Ink & Paper", "111111 f5f5f0"],
  ["Duotone", "Blueprint", "0b3d91 ffffff"],
  ["Duotone", "Risograph", "ff48b0 0078bf ffe800 f7f2e8"],
  ["Duotone", "Newsprint", "1a1a1a e8e4d9"],
  ["Duotone", "Bauhaus", "e53935 fdd835 1e88e5 212121 fafafa"],
  ["Duotone", "Swiss", "e30613 000000 ffffff"],
  ["Duotone", "Navy & Gold", "00296b fdc500"],
  ["Duotone", "Purple & Amber", "240046 ff9e00"],
];

/** Single-hue ramps generated in OKLCh, dark → light. */
const RAMPS: [name: string, hue: number, chroma: number][] = [
  ["Greyscale", 0, 0],
  ["Red", 25, 0.17],
  ["Orange", 55, 0.15],
  ["Amber", 80, 0.14],
  ["Lime", 125, 0.15],
  ["Green", 150, 0.14],
  ["Teal", 185, 0.11],
  ["Cyan", 210, 0.12],
  ["Blue", 255, 0.16],
  ["Indigo", 275, 0.16],
  ["Violet", 300, 0.16],
  ["Pink", 350, 0.16],
];

function ramp(hue: number, chroma: number): string[] {
  const h = (hue * Math.PI) / 180;
  return [0.22, 0.38, 0.55, 0.72, 0.9].map((L) => {
    const c = chroma * (1 - Math.abs(L - 0.6) * 1.1);
    return rgbToHex(fromOklab([L, c * Math.cos(h), c * Math.sin(h)]));
  });
}

const slug = (s: string) => s.toLowerCase().normalize("NFD").replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "");

export const BUILT_IN: ColorScheme[] = [
  ...RAW.map(([category, name, colors]) => ({ id: slug(name), name, category, colors: colors.split(" ").map((c) => "#" + c) })),
  ...RAMPS.map(([name, hue, chroma]) => ({ id: `mono-${slug(name)}`, name: `${name} ramp`, category: "Monochrome", colors: ramp(hue, chroma) })),
];

export const CATEGORIES = ["Custom", ...Array.from(new Set(BUILT_IN.map((s) => s.category)))];

// ------------------------------------------------------------ custom presets

export const CUSTOM_KEY = "lumetrope.colorSchemes";
export const PRESETS_CHANGED = "lumetrope:color-schemes";

function clean(v: unknown): ColorScheme[] {
  if (!Array.isArray(v)) return [];
  return v
    .filter((s) => s && typeof s.id === "string" && typeof s.name === "string" && Array.isArray(s.colors))
    .map((s) => ({ id: s.id, name: s.name, category: "Custom", custom: true, colors: s.colors.filter((c: unknown) => typeof c === "string" && /^#?[0-9a-f]{3,8}$/i.test(c)).map(normalizeHex) }))
    .filter((s) => s.colors.length > 0);
}

export function loadCustomSchemes(): ColorScheme[] {
  try {
    return clean(JSON.parse(localStorage.getItem(CUSTOM_KEY) ?? "[]"));
  } catch {
    return [];
  }
}

export function saveCustomSchemes(list: ColorScheme[]): void {
  try {
    localStorage.setItem(CUSTOM_KEY, JSON.stringify(list.map(({ id, name, colors }) => ({ id, name, colors }))));
  } catch {
    /* storage unavailable: presets last for this session only */
  }
  window.dispatchEvent(new Event(PRESETS_CHANGED));
}

/** Merge presets from a backup: same id → replaced, new ones appended. Returns how many were added or updated. */
export function mergeCustomSchemes(incoming: unknown): number {
  const add = clean(incoming);
  const list = loadCustomSchemes();
  for (const s of add) {
    const i = list.findIndex((x) => x.id === s.id);
    if (i >= 0) list[i] = s;
    else list.push(s);
  }
  if (add.length) saveCustomSchemes(list);
  return add.length;
}

export function newSchemeId(): string {
  return `custom-${Date.now().toString(36)}${Math.random().toString(36).slice(2, 6)}`;
}
