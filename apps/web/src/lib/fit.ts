/** Scale and place `content` centred inside `box` (contain), leaving `pad` px around it. */
export function fitRect(content: { w: number; h: number }, box: { w: number; h: number }, pad: number) {
  const scale = Math.max(0.01, Math.min((box.w - pad * 2) / content.w, (box.h - pad * 2) / content.h));
  const w = content.w * scale;
  const h = content.h * scale;
  return { scale, x: (box.w - w) / 2, y: (box.h - h) / 2, w, h };
}
