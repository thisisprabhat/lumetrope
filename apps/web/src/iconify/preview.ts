/** Keep the original SVG colors; only all-black icons need a white preview. */
export function iconPreview(svg: string): { src: string; monochrome: boolean } {
  const colors = [
    ...Array.from(svg.matchAll(/\b(?:fill|stroke|color|stop-color)\s*=\s*["']([^"']+)["']/gi), (match) => match[1]),
    ...Array.from(svg.matchAll(/(?:^|[;"'\s])(?:fill|stroke|color|stop-color)\s*:\s*([^;"'}]+)/gi), (match) => match[1]),
  ];
  const monochrome = colors.every((color) => /^(?:none|inherit|currentcolor|transparent|black|#000|#000000|rgb\(\s*0[\s,]+0[\s,]+0\s*\))$/i.test(color.trim()));
  return { src: `data:image/svg+xml;charset=utf-8,${encodeURIComponent(svg)}`, monochrome };
}
