import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import type { LiveFrame } from "@/lib/ipc";
import { cx } from "@/components/ui";

function gearText(g: number): string {
  if (g < 0) return "R";
  if (g === 0) return "N";
  return String(g);
}

function setText(el: HTMLElement | null, text: string) {
  if (el && el.textContent !== text) el.textContent = text;
}

/** Dash: updated directly from the live ref each animation frame, no React re-render. */
export function Dash({ liveRef, mph, showAssists }: { liveRef: React.MutableRefObject<LiveFrame | null>; mph: boolean; showAssists: boolean }) {
  const { t } = useTranslation();
  const gear = useRef<HTMLDivElement>(null);
  const speed = useRef<HTMLDivElement>(null);
  const rpm = useRef<HTMLDivElement>(null);
  const bar = useRef<HTMLDivElement>(null);
  const tc = useRef<HTMLSpanElement>(null);
  const abs = useRef<HTMLSpanElement>(null);
  useEffect(() => {
    let raf = 0;
    let zone = "";
    let seenMax = 1;
    const draw = () => {
      const f = liveRef.current;
      if (f) {
        setText(gear.current, gearText(f.gear));
        setText(speed.current, String(Math.round(mph ? f.speed * 0.621371 : f.speed)));
        setText(rpm.current, String(f.rpm));
        seenMax = Math.max(seenMax, f.rpm);
        const k = Math.min(1, f.rpm / (f.max_rpm > 0 ? f.max_rpm : seenMax));
        if (bar.current) {
          bar.current.style.transform = `scaleX(${k})`;
          const z = k > 0.95 ? "var(--danger)" : k > 0.85 ? "var(--warn)" : "var(--ok)";
          if (z !== zone) {
            zone = z;
            bar.current.style.background = z;
          }
        }
        setText(tc.current, `${t("wd.tc")} ${f.tc_level}`);
        setText(abs.current, `${t("wd.abs")} ${f.abs_level}`);
        tc.current?.classList.toggle("text-warn", f.tc_active);
        abs.current?.classList.toggle("text-warn", f.abs_active);
      }
      raf = requestAnimationFrame(draw);
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  }, [liveRef, mph, t]);
  return (
    <div className="w-[260px]">
      <div className="h-2 overflow-hidden rounded-full bg-surface-2">
        <div ref={bar} className="h-full origin-left" style={{ transform: "scaleX(0)" }} />
      </div>
      <div className="mt-2 grid grid-cols-3 items-end text-center">
        <div>
          <div ref={speed} className="num text-2xl font-semibold">0</div>
          <div className="text-[10px] uppercase tracking-widest text-dim">{mph ? t("wd.mph") : t("wd.kmh")}</div>
        </div>
        <div ref={gear} className="num text-5xl font-bold leading-none text-accent">N</div>
        <div>
          <div ref={rpm} className="num text-2xl font-semibold">0</div>
          <div className="text-[10px] uppercase tracking-widest text-dim">{t("wd.rpm")}</div>
        </div>
      </div>
      {showAssists && (
        <div className={cx("num mt-2 flex justify-center gap-4 text-xs text-dim")}>
          <span ref={tc} />
          <span ref={abs} />
        </div>
      )}
    </div>
  );
}
