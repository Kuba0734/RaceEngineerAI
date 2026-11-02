import { useEffect, useRef } from "react";
import type { LiveFrame } from "@/lib/ipc";
import { lapTime } from "@/lib/format";

/** Delta to the best lap. Faster: green bar to the right, slower: red bar to the left. */
export function Delta({ liveRef, showLaps, showBar, rangeS }: { liveRef: React.MutableRefObject<LiveFrame | null>; showLaps: boolean; showBar: boolean; rangeS: number }) {
  const value = useRef<HTMLDivElement>(null);
  const faster = useRef<HTMLDivElement>(null);
  const slower = useRef<HTMLDivElement>(null);
  const last = useRef<HTMLSpanElement>(null);
  const best = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    let raf = 0;
    const draw = () => {
      const f = liveRef.current;
      if (f) {
        const d = f.delta_ms / 1000;
        const text = `${d > 0 ? "+" : ""}${d.toFixed(2)}`;
        const el = value.current;
        if (el && el.textContent !== text) {
          el.textContent = text;
          el.style.color = d <= 0 ? "var(--ok)" : "var(--danger)";
        }
        const k = Math.min(1, Math.abs(d) / Math.max(0.1, rangeS));
        if (faster.current) faster.current.style.transform = `scaleX(${d < 0 ? k : 0})`;
        if (slower.current) slower.current.style.transform = `scaleX(${d > 0 ? k : 0})`;
        const l = lapTime(f.last_lap_ms);
        const b = lapTime(f.best_lap_ms);
        if (last.current && last.current.textContent !== l) last.current.textContent = l;
        if (best.current && best.current.textContent !== b) best.current.textContent = b;
      }
      raf = requestAnimationFrame(draw);
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  }, [liveRef, rangeS]);
  return (
    <div className="w-60 text-center">
      <div ref={value} className="num text-3xl font-semibold">0.00</div>
      {showBar && (
        <div className="relative mt-1.5 h-2.5 overflow-hidden rounded-full bg-surface-2">
          <div ref={slower} className="absolute inset-y-0 left-0 w-1/2 origin-right bg-danger" style={{ transform: "scaleX(0)" }} />
          <div ref={faster} className="absolute inset-y-0 right-0 w-1/2 origin-left bg-ok" style={{ transform: "scaleX(0)" }} />
          <div className="absolute inset-y-0 left-1/2 w-px -translate-x-1/2 bg-fg/70" />
        </div>
      )}
      {showLaps && (
        <div className="num mt-1 flex justify-between gap-4 text-xs text-dim">
          <span ref={last} />
          <span ref={best} className="text-purple" />
        </div>
      )}
    </div>
  );
}
