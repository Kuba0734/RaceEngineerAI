import { cx } from "@/components/ui";

/** iRacing licence colours by class letter (used when the sim sends no colour). */
const LICENSE_COLOR: Record<string, string> = { R: "#b91c1c", D: "#ea580c", C: "#ca8a04", B: "#16a34a", A: "#1d4ed8", P: "#111827" };

/** "A 3.45" → compact badge "A3.4" in the licence colour. */
export function LicenseBadge({ license, color }: { license: string | undefined; color: string | undefined }) {
  if (!license) return null;
  const [cls = "", sr = ""] = license.split(" ");
  const bg = color ?? LICENSE_COLOR[cls[0] ?? ""] ?? "#374151";
  const text = `${cls}${sr ? Number.parseFloat(sr).toFixed(1) : ""}`;
  return <span className="num inline-block min-w-[2.6em] rounded px-1 text-center text-[0.8em] font-semibold leading-[1.45] text-white" style={{ background: bg }}>{text}</span>;
}

/** "8.8k ▲14" – rating and the estimated change if the race ended now. */
export function IRatingBadge({ irating, change }: { irating: number | undefined; change?: number | undefined }) {
  if (irating == null) return null;
  const k = irating >= 1000 ? `${(irating / 1000).toFixed(1)}k` : String(irating);
  return (
    <span className="num inline-flex items-center gap-1 rounded bg-fg/90 px-1 text-[0.8em] font-semibold leading-[1.45] text-bg">
      {k}
      {change != null && change !== 0 && <span className={change > 0 ? "text-ok" : "text-danger"}>{change > 0 ? "▲" : "▼"}{Math.abs(Math.round(change))}</span>}
    </span>
  );
}

/** Places gained (green ▲) or lost (red ▼) since the start. */
export function PosChange({ start, now }: { start: number | undefined; now: number }) {
  if (start == null || now <= 0) return <span className="inline-block w-7" />;
  const d = start - now;
  if (d === 0) return <span className="inline-block w-7 text-center text-dim">–</span>;
  return <span className={cx("num inline-block w-7 text-[0.85em]", d > 0 ? "text-ok" : "text-danger")}>{d > 0 ? "▲" : "▼"}{Math.abs(d)}</span>;
}

/** Car number on the class colour. */
export function CarNumber({ text, color }: { text: string; color: string | undefined }) {
  return <span className="num font-semibold" style={{ color: color ?? "var(--fg)" }}>#{text}</span>;
}

/** Signed seconds: "+1.4" / "−0.3". */
export function secs(v: number | null | undefined, digits = 1): string {
  if (v == null) return "";
  return `${v > 0 ? "+" : v < 0 ? "−" : ""}${Math.abs(v).toFixed(digits)}`;
}
