export type ExportFormat = "json" | "lottie" | "gif" | "webp" | "webm";

export function exportFileName(name: string, format: ExportFormat): string {
  const base = name.trim().replace(/\.(json|lottie|gif|webp|webm)$/i, "").replace(/[\\/:*?"<>|]+/g, "-").trim() || "animation";
  return `${base}.${format}`;
}

export function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / 1024 / 1024).toFixed(1)} MB`;
}
