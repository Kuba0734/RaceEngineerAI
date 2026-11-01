import { useEffect, useRef } from "react";
import type { LiveFrame } from "@/lib/ipc";

/** Spotter radar: player in the centre, cars within ±25 m, overlap highlighted. */
export function Radar({ liveRef, size = 200, onActive }: { liveRef: React.MutableRefObject<LiveFrame | null>; size?: number; onActive?: (active: boolean) => void }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  useEffect(() => {
    const c = canvas.current;
    const ctx = c?.getContext("2d");
    if (!c || !ctx) return;
    const dpr = window.devicePixelRatio || 1;
    c.width = size * dpr;
    c.height = size * dpr;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    let raf = 0;
    let active = false;
    let lastSeen = 0;
    const range = 25;
    const s = size / 2 / range;
    const draw = () => {
      const f = liveRef.current;
      // Visible while a car is around (plus a short tail so it does not flicker).
      const now = performance.now();
      if ((f?.radar.length ?? 0) > 0 || (f?.spotter ?? "clear") !== "clear") lastSeen = now;
      const nowActive = now - lastSeen < 1500;
      if (nowActive !== active) {
        active = nowActive;
        onActive?.(active);
      }
      ctx.clearRect(0, 0, size, size);
      const mid = size / 2;
      const side = f?.spotter ?? "clear";
      // Side bars (spotter state).
      const bar = (x: number, on: boolean) => {
        ctx.fillStyle = on ? "rgba(255,90,31,0.85)" : "rgba(38,42,53,0.6)";
        ctx.fillRect(x, mid - 40, 6, 80);
      };
      bar(6, side === "left" || side === "three_wide");
      bar(size - 12, side === "right" || side === "three_wide");
      ctx.strokeStyle = "rgba(138,144,160,0.25)";
      ctx.lineWidth = 1;
      for (const r of [10, 20]) {
        ctx.beginPath();
        ctx.arc(mid, mid, r * s, 0, Math.PI * 2);
        ctx.stroke();
      }
      const car = (x: number, y: number, color: string) => {
        ctx.fillStyle = color;
        ctx.beginPath();
        ctx.roundRect(x - 1.0 * s, y - 2.4 * s, 2.0 * s, 4.8 * s, 3);
        ctx.fill();
      };
      for (const r of f?.radar ?? []) {
        car(mid + r.lat * s, mid - r.lon * s, r.overlap ? "#ff5a1f" : "#22d3ee");
      }
      car(mid, mid, "#f2f4f8");
      raf = requestAnimationFrame(draw);
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  }, [liveRef, size, onActive]);
  return <canvas ref={canvas} style={{ width: size, height: size }} />;
}
