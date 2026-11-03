import type { ReactNode } from "react";

export interface Series {
  label: string;
  color: string;
  /** One value per lap (null = gap in the line). */
  values: (number | null)[];
}

/** Small line chart over the laps (SVG, no library). `invert` puts low values on top (positions). */
export function LineChart({ series, laps, unit = "", invert = false, height = 140, format = (v) => v.toFixed(1) }: {
  series: Series[];
  laps: number[];
  unit?: string;
  invert?: boolean;
  height?: number;
  format?: (v: number) => string;
}): ReactNode {
  const all = series.flatMap((s) => s.values).filter((v): v is number => v != null);
  if (all.length < 2 || laps.length < 2) return null;
  let min = Math.min(...all);
  let max = Math.max(...all);
  if (max - min < 1e-6) {
    min -= 1;
    max += 1;
  }
  const W = 600;
  const H = height;
  const pad = { l: 44, r: 8, t: 8, b: 20 };
  const x = (i: number) => pad.l + (i / (laps.length - 1)) * (W - pad.l - pad.r);
  const y = (v: number) => {
    const k = (v - min) / (max - min);
    return pad.t + (invert ? k : 1 - k) * (H - pad.t - pad.b);
  };
  const path = (vals: (number | null)[]) =>
    vals.map((v, i) => (v == null ? "" : `${i > 0 && vals[i - 1] != null ? "L" : "M"}${x(i).toFixed(1)},${y(v).toFixed(1)}`)).join("");
  const ticks = [min, (min + max) / 2, max];
  return (
    <div>
      <svg viewBox={`0 0 ${W} ${H}`} className="h-auto w-full">
        {ticks.map((v) => (
          <g key={v}>
            <line x1={pad.l} x2={W - pad.r} y1={y(v)} y2={y(v)} stroke="var(--border)" strokeWidth="1" />
            <text x={pad.l - 6} y={y(v) + 4} textAnchor="end" className="num" fontSize="11" fill="var(--text-dim)">{format(v)}{unit}</text>
          </g>
        ))}
        {laps.map((l, i) => (i % Math.ceil(laps.length / 12) === 0 ? <text key={l} x={x(i)} y={H - 4} textAnchor="middle" className="num" fontSize="11" fill="var(--text-dim)">{l}</text> : null))}
        {series.map((s) => (
          <path key={s.label} d={path(s.values)} fill="none" stroke={s.color} strokeWidth="2" strokeLinejoin="round" />
        ))}
      </svg>
      {series.length > 1 && (
        <div className="mt-1 flex gap-4 text-xs text-dim">
          {series.map((s) => (
            <span key={s.label} className="flex items-center gap-1.5"><span className="h-2 w-3 rounded-sm" style={{ background: s.color }} />{s.label}</span>
          ))}
        </div>
      )}
    </div>
  );
}
