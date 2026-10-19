import { useRef, useState } from "react";

const PRESETS: { name: string; v: [number, number, number, number] }[] = [
  { name: "linear", v: [0, 0, 1, 1] },
  { name: "ease", v: [0.25, 0.1, 0.25, 1] },
  { name: "ease_in", v: [0.42, 0, 1, 1] },
  { name: "ease_out", v: [0, 0, 0.58, 1] },
  { name: "ease_in_out", v: [0.42, 0, 0.58, 1] },
  { name: "smooth", v: [0.33, 0, 0.67, 1] },
  { name: "ease_out_back", v: [0.34, 1.56, 0.64, 1] },
  { name: "ease_in_back", v: [0.36, 0, 0.66, -0.56] },
  { name: "snappy", v: [0.2, 0.9, 0.1, 1] },
];

const S = 168;
const PAD = 26;

/** Cubic-bezier easing editor with draggable handles and presets. */
export function EasingEditor({ value, onApply, onClose }: { value: [number, number, number, number] | "hold" | null; onApply: (v: [number, number, number, number] | "hold") => void; onClose: () => void }) {
  const [v, setV] = useState<[number, number, number, number]>(Array.isArray(value) ? value : [0.42, 0, 0.58, 1]);
  const drag = useRef<0 | 1 | null>(null);
  const svg = useRef<SVGSVGElement>(null);
  const toX = (x: number) => PAD + x * S;
  const toY = (y: number) => PAD + (1 - y) * S;
  const move = (e: React.PointerEvent) => {
    if (drag.current === null || !svg.current) return;
    const r = svg.current.getBoundingClientRect();
    const x = Math.min(1, Math.max(0, (e.clientX - r.left - PAD) / S));
    const y = Math.min(2, Math.max(-1, 1 - (e.clientY - r.top - PAD) / S));
    const n = [...v] as [number, number, number, number];
    n[drag.current * 2] = Math.round(x * 100) / 100;
    n[drag.current * 2 + 1] = Math.round(y * 100) / 100;
    setV(n);
  };
  const path = `M${toX(0)},${toY(0)} C${toX(v[0])},${toY(v[1])} ${toX(v[2])},${toY(v[3])} ${toX(1)},${toY(1)}`;
  return (
    <div className="easing-pop" onPointerDown={(e) => e.stopPropagation()}>
      <div className="easing-head">
        <span>Easing</span>
        <span className="num muted">{v.map((x) => x.toFixed(2)).join(", ")}</span>
      </div>
      <svg ref={svg} width={S + PAD * 2} height={S + PAD * 2} onPointerMove={move} onPointerUp={() => (drag.current = null)} onPointerLeave={() => (drag.current = null)}>
        <rect x={PAD} y={PAD} width={S} height={S} className="ease-box" />
        <line x1={toX(0)} y1={toY(0)} x2={toX(v[0])} y2={toY(v[1])} className="ease-arm" />
        <line x1={toX(1)} y1={toY(1)} x2={toX(v[2])} y2={toY(v[3])} className="ease-arm" />
        <path d={path} className="ease-curve" />
        {[0, 1].map((h) => (
          <circle
            key={h}
            cx={toX(v[h * 2])}
            cy={toY(v[h * 2 + 1])}
            r={7}
            className="ease-handle"
            onPointerDown={(e) => {
              (e.target as Element).setPointerCapture(e.pointerId);
              drag.current = h as 0 | 1;
            }}
          />
        ))}
      </svg>
      <div className="easing-presets">
        {PRESETS.map((p) => (
          <button key={p.name} onClick={() => setV(p.v)} className={p.v.every((x, i) => x === v[i]) ? "on" : ""}>
            {p.name.replace(/_/g, " ")}
          </button>
        ))}
      </div>
      <div className="row" style={{ justifyContent: "flex-end", gap: 6 }}>
        <button className="btn ghost" onClick={() => onApply("hold")}>
          Hold
        </button>
        <button className="btn ghost" onClick={onClose}>
          Cancel
        </button>
        <button className="btn primary" onClick={() => onApply(v)}>
          Apply
        </button>
      </div>
    </div>
  );
}
