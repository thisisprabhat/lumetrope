// GIF, animated WebP and WebM export with transparency. Frames come from the
// GPU display-list renderer as straight-alpha RGBA. GIF and WebP are encoded
// in the WASM engine; WebM frames are encoded by the browser's WebCodecs VP9/VP8
// encoder: one stream for colour and, when transparent, a second stream that
// carries the alpha plane as luma (the standard WebM alpha layout), muxed in WASM.
import { engine } from "./engine";
import { CanvasRenderer } from "./gpu/CanvasRenderer";
import { evenSize, exportSize, frameTimes, hexToRgb, type MediaFormat } from "./lib/mediaExport";
import { GifExporter, WebmWriter, WebpExporter } from "./wasm/lumetrope_wasm.js";

export interface MediaOptions {
  format: MediaFormat;
  scale: number;
  fps: number;
  /** Scene frames [start, end). */
  start: number;
  end: number;
  transparent: boolean;
  background: string;
  /** GIF: colour soft edges blend into. */
  matte: string;
  /** WebM: bits per pixel per frame. */
  quality: number;
}

export interface Progress {
  done: number;
  total: number;
  stage: string;
}

export class ExportCancelled extends Error {
  constructor() {
    super("Export cancelled");
  }
}

const tick = () => new Promise<void>((r) => setTimeout(r, 0));

/** Render every export frame and hand its straight-alpha RGBA to `sink`. */
async function renderFrames(o: MediaOptions, size: { w: number; h: number }, sink: (rgba: Uint8ClampedArray, i: number, time: number) => void | Promise<void>, progress: (p: Progress) => void, signal: AbortSignal) {
  const info = JSON.parse(engine().info());
  const times = frameTimes(o.start, o.end, info.fr, o.fps);
  const canvas = document.createElement("canvas");
  canvas.width = size.w;
  canvas.height = size.h;
  const ctx = canvas.getContext("2d", { willReadFrequently: true })!;
  const r = new CanvasRenderer();
  const imgs = JSON.parse(engine().image_assets()) as Record<string, string>;
  if (Object.keys(imgs).length) {
    await new Promise<void>((done) => {
      let left = Object.keys(imgs).length;
      r.onImageLoad = () => --left <= 0 && done();
      r.setImages(imgs);
      setTimeout(done, 3000);
    });
  }
  const zx = size.w / info.w;
  for (let i = 0; i < times.length; i++) {
    if (signal.aborted) throw new ExportCancelled();
    ctx.clearRect(0, 0, size.w, size.h);
    r.render(ctx, JSON.parse(engine().display_list(times[i], zx, 0, 0)), size.w, size.h, [0, 0, size.w, size.h]);
    await sink(ctx.getImageData(0, 0, size.w, size.h).data, i, i / o.fps);
    progress({ done: i + 1, total: times.length, stage: "Rendering" });
    if (i % 2 === 1) await tick();
  }
  return times.length;
}

export async function exportGif(o: MediaOptions, progress: (p: Progress) => void, signal: AbortSignal): Promise<Uint8Array> {
  const info = JSON.parse(engine().info());
  const size = exportSize(info.w, info.h, o.scale);
  const enc = new GifExporter(size.w, size.h, o.fps, o.transparent, new Uint8Array(hexToRgb(o.background)), new Uint8Array(hexToRgb(o.matte)), 128);
  try {
    await renderFrames(o, size, (rgba) => enc.push(new Uint8Array(rgba.buffer, rgba.byteOffset, rgba.byteLength)), progress, signal);
  } catch (e) {
    enc.free();
    throw e;
  }
  progress({ done: 1, total: 1, stage: "Writing GIF" });
  return enc.finish();
}

export async function exportWebp(o: MediaOptions, progress: (p: Progress) => void, signal: AbortSignal): Promise<Uint8Array> {
  const info = JSON.parse(engine().info());
  const size = exportSize(info.w, info.h, o.scale);
  const enc = new WebpExporter(size.w, size.h, o.fps, o.transparent, new Uint8Array(hexToRgb(o.background)));
  try {
    await renderFrames(o, size, (rgba) => enc.push(new Uint8Array(rgba.buffer, rgba.byteOffset, rgba.byteLength)), progress, signal);
  } catch (e) {
    enc.free();
    throw e;
  }
  progress({ done: 1, total: 1, stage: "Writing WebP" });
  return enc.finish();
}

const WEBM_CODECS = [
  { codec: "vp09.00.10.08", vp9: true, label: "VP9" },
  { codec: "vp8", vp9: false, label: "VP8" },
];

/** Which WebCodecs encoder (if any) this window can use for WebM at this size. */
export async function webmSupport(w = 512, h = 512): Promise<{ codec: string; vp9: boolean; label: string } | null> {
  if (typeof VideoEncoder === "undefined" || typeof VideoFrame === "undefined") return null;
  const s = evenSize({ w, h });
  for (const c of WEBM_CODECS) {
    try {
      const r = await VideoEncoder.isConfigSupported({ codec: c.codec, width: s.w, height: s.h, bitrate: 1_000_000, framerate: 30 });
      if (r.supported) return c;
    } catch {
      /* try the next codec */
    }
  }
  return null;
}

interface Chunk {
  data: Uint8Array;
  key: boolean;
}

function makeEncoder(config: VideoEncoderConfig, out: Chunk[], fail: (e: Error) => void): VideoEncoder {
  const enc = new VideoEncoder({
    output: (chunk) => {
      const data = new Uint8Array(chunk.byteLength);
      chunk.copyTo(data);
      out.push({ data, key: chunk.type === "key" });
    },
    error: (e) => fail(e instanceof Error ? e : new Error(String(e))),
  });
  enc.configure(config);
  return enc;
}

export async function exportWebm(o: MediaOptions, progress: (p: Progress) => void, signal: AbortSignal): Promise<Uint8Array> {
  const info = JSON.parse(engine().info());
  const size = evenSize(exportSize(info.w, info.h, o.scale));
  const support = await webmSupport(size.w, size.h);
  if (!support) throw new Error("This window has no VP8/VP9 video encoder (WebCodecs), so WebM can't be exported here.");
  const bitrate = Math.max(200_000, Math.round(size.w * size.h * o.fps * o.quality));
  const config: VideoEncoderConfig = { codec: support.codec, width: size.w, height: size.h, bitrate, framerate: o.fps, latencyMode: "quality" };
  let failure: Error | null = null;
  const fail = (e: Error) => (failure ??= e);
  const color: Chunk[] = [];
  const alpha: Chunk[] = [];
  const colorEnc = makeEncoder(config, color, fail);
  const alphaEnc = o.transparent ? makeEncoder(config, alpha, fail) : null;
  const bg = hexToRgb(o.background);
  const px = new Uint8Array(size.w * size.h * 4);
  // Alpha goes in as the Y plane of an I420 frame (U/V neutral): no RGB→YUV conversion, so no
  // limited-range (16–235) squashing by encoders that use video range (macOS WebKit does).
  const plane = size.w * size.h;
  const ax = new Uint8Array(plane + 2 * ((size.w / 2) * (size.h / 2)));
  ax.fill(128, plane);
  const keyEvery = Math.max(1, Math.round(o.fps * 2));
  const close = () => {
    for (const e of [colorEnc, alphaEnc]) if (e && e.state !== "closed") e.close();
  };
  try {
    const frames = await renderFrames(
      o,
      size,
      async (rgba, i, time) => {
        if (failure) throw failure;
        for (let p = 0; p < rgba.length; p += 4) {
          const a = rgba[p + 3];
          if (o.transparent) {
            // Colour stream: straight colour (black where fully clear); alpha stream: alpha as grey.
            px[p] = a ? rgba[p] : 0;
            px[p + 1] = a ? rgba[p + 1] : 0;
            px[p + 2] = a ? rgba[p + 2] : 0;
            ax[p >> 2] = a;
          } else {
            px[p] = (rgba[p] * a + bg[0] * (255 - a) + 127) / 255;
            px[p + 1] = (rgba[p + 1] * a + bg[1] * (255 - a) + 127) / 255;
            px[p + 2] = (rgba[p + 2] * a + bg[2] * (255 - a) + 127) / 255;
          }
          px[p + 3] = 255;
        }
        const timestamp = Math.round(time * 1e6);
        const keyFrame = i % keyEvery === 0;
        const f = new VideoFrame(px, { format: "RGBX", codedWidth: size.w, codedHeight: size.h, timestamp, duration: Math.round(1e6 / o.fps) });
        colorEnc.encode(f, { keyFrame });
        f.close();
        if (alphaEnc) {
          const fa = new VideoFrame(ax, { format: "I420", codedWidth: size.w, codedHeight: size.h, timestamp, duration: Math.round(1e6 / o.fps), colorSpace: { fullRange: true, matrix: "bt709", primaries: "bt709", transfer: "bt709" } });
          alphaEnc.encode(fa, { keyFrame });
          fa.close();
        }
        while (colorEnc.encodeQueueSize > 6 || (alphaEnc?.encodeQueueSize ?? 0) > 6) await new Promise((r) => setTimeout(r, 4));
      },
      progress,
      signal,
    );
    progress({ done: 0, total: 1, stage: "Encoding video" });
    await colorEnc.flush();
    await alphaEnc?.flush();
    if (failure) throw failure;
    if (color.length !== frames || (alphaEnc && alpha.length !== frames)) throw new Error(`Video encoder returned ${color.length}/${alpha.length} chunks for ${frames} frames`);
    const mux = new WebmWriter(size.w, size.h, support.vp9, !!alphaEnc);
    for (let i = 0; i < frames; i++) {
      // Both streams were asked for keyframes on the same frames; a block is a keyframe only if both are.
      const key = color[i].key && (!alphaEnc || alpha[i].key);
      mux.push(color[i].data, alphaEnc ? alpha[i].data : new Uint8Array(), key, (i * 1000) / o.fps);
    }
    progress({ done: 1, total: 1, stage: "Writing WebM" });
    return mux.finish((frames * 1000) / o.fps);
  } finally {
    close();
  }
}

export function exportMedia(o: MediaOptions, progress: (p: Progress) => void, signal: AbortSignal): Promise<Uint8Array> {
  return o.format === "gif" ? exportGif(o, progress, signal) : o.format === "webp" ? exportWebp(o, progress, signal) : exportWebm(o, progress, signal);
}

export const MEDIA_MIME: Record<MediaFormat, string> = { gif: "image/gif", webp: "image/webp", webm: "video/webm" };
