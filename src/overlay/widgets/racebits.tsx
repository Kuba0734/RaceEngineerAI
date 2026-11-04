import * as React from "react";
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
  return <span className="num font-semibold" style={{ color: color ?? "var(--text)" }}>#{text}</span>;
}

/** Signed seconds: "+1.4" / "−0.3". */
export function secs(v: number | null | undefined, digits = 1): string {
  if (v == null) return "";
  return `${v > 0 ? "+" : v < 0 ? "−" : ""}${Math.abs(v).toFixed(digits)}`;
}

/** Widget title row: coloured marker, upper-case label, optional content on the right. */
export function WidgetHead({ label, color = "var(--accent)", children, className }: { label: string; color?: string; children?: React.ReactNode; className?: string }) {
  return (
    <div className={cx("mb-1.5 flex items-center gap-2 text-[11px] font-bold uppercase tracking-[0.14em] text-dim", className)}>
      <span className="h-2 w-2 shrink-0 rounded-[2px]" style={{ background: color }} />
      <span className="truncate">{label}</span>
      {children != null && <span className="ml-auto flex items-center gap-3 normal-case tracking-normal">{children}</span>}
    </div>
  );
}

/** Licence + iRating + estimated change in one pill: "A 3.8 | 5900 | +88". */
export function DriverPill({ license, licenseColor, irating, change }: { license: string | undefined; licenseColor: string | undefined; irating: number | undefined; change?: number | undefined }) {
  if (!license && irating == null) return null;
  const [cls = "", sr = ""] = (license ?? "").split(" ");
  const lc = licenseColor ?? LICENSE_COLOR[cls[0] ?? ""] ?? "#374151";
  return (
    <span className="num inline-flex items-stretch overflow-hidden rounded text-[0.78em] font-bold leading-[1.5]" style={{ boxShadow: `inset 0 0 0 1px ${lc}` }}>
      {license && <span className="px-1 text-white" style={{ background: lc }}>{cls} {sr ? Number.parseFloat(sr).toFixed(1) : ""}</span>}
      {irating != null && <span className="bg-bg/80 px-1 text-fg">{irating}</span>}
      {change != null && change !== 0 && <span className={cx("px-1", change > 0 ? "bg-ok/20 text-ok" : "bg-danger/20 text-danger")}>{change > 0 ? "+" : "−"}{Math.abs(Math.round(change))}</span>}
    </span>
  );
}

/** Position cell on a class-coloured gradient. */
export function PosCell({ pos, color, player }: { pos: number; color: string | undefined; player?: boolean }) {
  const c = color ?? "#8a90a0";
  return (
    <span className="num flex h-full w-7 items-center justify-center rounded-l font-bold text-white" style={{ background: player ? `linear-gradient(90deg, ${c}, ${c}66)` : `linear-gradient(90deg, ${c}cc, ${c}22)` }}>
      {pos > 0 ? pos : "–"}
    </span>
  );
}

/** Signed change as "+3" / "−1", green or red. */
export function Change({ v }: { v: number | null | undefined }) {
  if (v == null || v === 0) return <span className="text-dim">–</span>;
  return <span className={v > 0 ? "text-ok" : "text-danger"}>{v > 0 ? "+" : "−"}{Math.abs(v)}</span>;
}

/** Updates an element's text from the live ref every animation frame (no React re-render). */
export function useLiveText<T extends HTMLElement>(liveRef: React.MutableRefObject<import("@/lib/ipc").LiveFrame | null>, text: (f: import("@/lib/ipc").LiveFrame) => string) {
  const el = React.useRef<T>(null);
  const fn = React.useRef(text);
  React.useEffect(() => {
    fn.current = text;
  });
  React.useEffect(() => {
    let raf = 0;
    const tick = () => {
      const f = liveRef.current;
      if (f && el.current) {
        const s = fn.current(f);
        if (el.current.textContent !== s) el.current.textContent = s;
      }
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [liveRef]);
  return el;
}

/** "m:ss" for seconds. */
export function mss(s: number): string {
  const v = Math.max(0, Math.floor(s));
  return `${Math.floor(v / 60)}:${String(v % 60).padStart(2, "0")}`;
}

const ICON: Record<string, string> = {
  timer: "M12 8v5l3 2M9 2h6M12 21a8 8 0 1 0 0-16a8 8 0 0 0 0 16",
  clock: "M12 7v5l3 2M12 21a9 9 0 1 0 0-18a9 9 0 0 0 0 18",
  sun: "M12 17a5 5 0 1 0 0-10a5 5 0 0 0 0 10M12 1v2M12 21v2M4.2 4.2l1.4 1.4M18.4 18.4l1.4 1.4M1 12h2M21 12h2M4.2 19.8l1.4-1.4M18.4 5.6l1.4-1.4",
  drop: "M12 3s6 7 6 11a6 6 0 0 1-12 0c0-4 6-11 6-11",
  thermo: "M14 14.8V5a2 2 0 0 0-4 0v9.8a4 4 0 1 0 4 0",
  road: "M5 21L9 3M19 21L15 3M12 6v2M12 11v2M12 16v2",
  warn: "M12 9v4M12 17h.01M10.3 3.9L1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0",
  fuel: "M6 20V6a2 2 0 0 1 2-2h6a2 2 0 0 1 2 2v14M5 20h12M16 9h2a2 2 0 0 1 2 2v5a1.5 1.5 0 0 1-3 0",
  wrench: "M14.7 6.3a4 4 0 0 0-5.4 5.4L3 18l3 3 6.3-6.3a4 4 0 0 0 5.4-5.4l-2.5 2.5-2.4-.6-.6-2.4z",
  flag: "M5 21V4h13l-3 4 3 4H5",
};

/** Small line icon in the text colour. */
export function Ico({ name, size = 13 }: { name: keyof typeof ICON | string; size?: number }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.2" strokeLinecap="round" strokeLinejoin="round" className="inline-block shrink-0">
      <path d={ICON[name] ?? ""} />
    </svg>
  );
}
