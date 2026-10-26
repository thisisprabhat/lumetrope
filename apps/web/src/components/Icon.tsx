// Hand-drawn 16px stroke icons (no icon dependency).
const paths: Record<string, string> = {
  select: "M3 2l10 5.5-4.2 1.3L7 13z",
  rect: "M2.5 3.5h11v9h-11z",
  ellipse: "M8 3c3.3 0 5.5 2.2 5.5 5S11.3 13 8 13 2.5 10.8 2.5 8 4.7 3 8 3z",
  polygon: "M8 2l5.7 4.1-2.2 6.6h-7L2.3 6.1z",
  star: "M8 1.8l1.9 3.9 4.3.6-3.1 3 .7 4.3L8 11.6l-3.8 2 .7-4.3-3.1-3 4.3-.6z",
  pen: "M2.5 13.5l2.8-.6 7.6-7.6-2.2-2.2-7.6 7.6zM9.6 4.2l2.2 2.2",
  text: "M3 3.5h10M8 3.5v9.5M6 13h4",
  hand: "M5 8V3.8a1 1 0 012 0V7m0-3.6a1 1 0 012 0V7m0-2.6a1 1 0 012 0V8m0-1.6a1 1 0 012 0v3.4c0 2.4-1.8 4.2-4.2 4.2H8.6c-1.3 0-2.3-.6-3-1.6L3.2 8.9a1 1 0 011.6-1.1L5 8",
  play: "M5 3.2v9.6L12.6 8z",
  pause: "M4.5 3h2.5v10H4.5zM9 3h2.5v10H9z",
  first: "M4 3v10M12 3.5L6 8l6 4.5z",
  last: "M12 3v10M4 3.5L10 8l-6 4.5z",
  prev: "M10.5 3.5L5.5 8l5 4.5",
  next: "M5.5 3.5l5 4.5-5 4.5",
  loop: "M3 7V6a2 2 0 012-2h7l-2-2M13 9v1a2 2 0 01-2 2H4l2 2",
  open: "M2 4.5h4l1.5 1.5H14v6.5H2z",
  save: "M3 2.5h8l2.5 2.5v8.5h-10.5zM5 2.5v3.5h5V2.5M5 13.5v-4h6v4",
  undo: "M5 4L2 7l3 3M2.5 7H10a3.5 3.5 0 010 7H7",
  redo: "M11 4l3 3-3 3M13.5 7H6a3.5 3.5 0 000 7h3",
  zoomIn: "M7 3a4 4 0 110 8 4 4 0 010-8zM10 10l3.5 3.5M5 7h4M7 5v4",
  zoomOut: "M7 3a4 4 0 110 8 4 4 0 010-8zM10 10l3.5 3.5M5 7h4",
  fit: "M2.5 6V2.5H6M10 2.5h3.5V6M13.5 10v3.5H10M6 13.5H2.5V10",
  expand: "M9.5 2.5h4v4M13.5 2.5L9 7M6.5 13.5h-4v-4M2.5 13.5L7 9",
  plus: "M8 3v10M3 8h10",
  minus: "M3 8h10",
  close: "M4 4l8 8M12 4l-8 8",
  eye: "M1.5 8S4 3.5 8 3.5 14.5 8 14.5 8 12 12.5 8 12.5 1.5 8 1.5 8zM8 6a2 2 0 110 4 2 2 0 010-4z",
  eyeOff: "M2 2l12 12M6.5 4C7 3.7 7.5 3.5 8 3.5c4 0 6.5 4.5 6.5 4.5a12 12 0 01-1.7 2.2M10.6 11.8A5.7 5.7 0 018 12.5C4 12.5 1.5 8 1.5 8a12 12 0 012.7-3",
  lock: "M4 7.5h8v6H4zM5.5 7.5V5a2.5 2.5 0 015 0v2.5",
  unlock: "M4 7.5h8v6H4zM5.5 7.5V5a2.5 2.5 0 014.8-1",
  trash: "M3 4.5h10M6.5 4.5V3h3v1.5M4.5 4.5l.6 9h5.8l.6-9",
  diamond: "M8 2.5L13.5 8 8 13.5 2.5 8z",
  chevronDown: "M4 6l4 4 4-4",
  chevronRight: "M6 4l4 4-4 4",
  layers: "M8 2l6 3-6 3-6-3zM2 8l6 3 6-3M2 11l6 3 6-3",
  image: "M2.5 3h11v10h-11zM2.5 11l3.5-3.5 3 3 2-2 2.5 2.5M10.5 6a1 1 0 100-.1",
  palette: "M8 2a6 6 0 100 12c.8 0 1-.6 1-1.1 0-.9-.6-1.2-.6-1.9 0-.6.5-1 1.1-1H11a3 3 0 003-3C14 4.5 11.3 2 8 2zM5 7.5h.01M7 5h.01M10 5h.01",
  history: "M2.5 8a5.5 5.5 0 101.6-3.9M2.5 2.5v2.6h2.6M8 5v3l2 1.5",
  segments: "M2 4h5v8H2zM9 4h5v8H9z",
  theme: "M8 2a6 6 0 000 12zM8 2a6 6 0 010 12",
  bolt: "M9 1.5L3.5 9H8l-1 5.5L12.5 7H8z",
  settings: "M8 5.5a2.5 2.5 0 110 5 2.5 2.5 0 010-5zM8 1.5v2M8 12.5v2M1.5 8h2M12.5 8h2M3.4 3.4l1.4 1.4M11.2 11.2l1.4 1.4M3.4 12.6l1.4-1.4M11.2 4.8l1.4-1.4",
  export: "M8 10V2.5M5 5.5l3-3 3 3M3 9.5v4h10v-4",
  link: "M6.5 9.5l3-3M7 4.5l1-1a2.5 2.5 0 013.5 3.5l-1 1M9 11.5l-1 1A2.5 2.5 0 014.5 9l1-1",
  graph: "M4 4h.01M12 4h.01M8 12h.01M4 4l8 0M4 4l4 8M12 4l-4 8",
  mcp: "M5.5 2.5v3M10.5 2.5v3M4 5.5h8v3a4 4 0 01-8 0zM8 12.5V14",
  more: "M4 8h.01M8 8h.01M12 8h.01",
  shuffle: "M2 4.5h2.2c2.2 0 3.1 1.2 4 3.5s1.8 3.5 4 3.5H14M2 11.5h2.2c1 0 1.7-.3 2.3-.9M9.5 5.4c.6-.6 1.3-.9 2.3-.9H14M12 2.5l2 2-2 2M12 9.5l2 2-2 2",
  import: "M8 2.5V10M5 7l3 3 3-3M3 9.5v4h10v-4",
  folder: "M2 4.5h4l1.5 1.5H14v6.5H2z",
  archive: "M2.5 3h11v3h-11zM3.5 6v7h9V6M6.5 8.5h3",
  copy: "M5.5 5.5h8v8h-8zM2.5 10.5v-8h8",
};

export function Icon({ name, size = 16, fill = false }: { name: keyof typeof paths | string; size?: number; fill?: boolean }) {
  const d = paths[name] ?? paths.more;
  return (
    <svg width={size} height={size} viewBox="0 0 16 16" fill={fill ? "currentColor" : "none"} stroke="currentColor" strokeWidth={fill ? 0 : 1.35} strokeLinecap="round" strokeLinejoin="round" aria-hidden>
      <path d={d} />
    </svg>
  );
}
