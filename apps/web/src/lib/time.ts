/** Clamp a frame into the playable range [ip, op) and floor it to an integer. */
export function clampFrame(frame: number, ip: number, op: number): number {
  const last = Math.max(ip, Math.ceil(op) - 1);
  return Math.min(last, Math.max(ip, Math.floor(frame)));
}

/** "1s 15f" style time label. */
export function formatTime(frame: number, fr: number): string {
  const f = Math.max(0, Math.floor(frame));
  const rate = Math.max(1, Math.round(fr));
  return `${Math.floor(f / rate)}s ${f % rate}f`;
}

/** Advance playback by `delta` frames, wrapping when looping. */
export function advance(frame: number, delta: number, ip: number, op: number, loop: boolean): { frame: number; ended: boolean } {
  const span = op - ip;
  let next = frame + delta;
  if (next < op) return { frame: next, ended: false };
  if (!loop || span <= 0) return { frame: Math.max(ip, Math.ceil(op) - 1), ended: true };
  next = ip + ((next - ip) % span);
  return { frame: next, ended: false };
}
