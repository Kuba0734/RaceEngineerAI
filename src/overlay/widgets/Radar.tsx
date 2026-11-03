import { useEffect, useRef } from "react";
import type { LiveFrame } from "@/lib/ipc";

/** Spotter radar: player in the centre, cars within ±25 m, overlap highlighted. */
export function Radar({ liveRef, size = 200, onActive, showDistance = true }: { liveRef: React.MutableRefObject<LiveFrame | null>; size?: number; onActive?: (active: boolean) => void; showDistance?: boolean }) {
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
      if (f?.native_spotter) {
        // The game reports only the side and the distance along the track: two side lanes
        // with the overlapping car placed by how far it is alongside, and the nearest car
        // ahead/behind in your lane. No rings, no invented lateral positions.
        ctx.clearRect(0, 0, size, size);
        const laneW = size * 0.2;
        const lanes = { left: size * 0.08, right: size - size * 0.08 - laneW };
        const block = (x: number, lon: number, color: string) => {
          ctx.fillStyle = color;
          ctx.beginPath();
          ctx.roundRect(x + laneW * 0.15, mid - lon * s - 2.4 * s, laneW * 0.7, 4.8 * s, 4);
          ctx.fill();
        };
        const leftOn = side === "left" || side === "three_wide";
        const rightOn = side === "right" || side === "three_wide";
        for (const [x, on] of [[lanes.left, leftOn], [lanes.right, rightOn]] as const) {
          ctx.fillStyle = on ? "rgba(255,90,31,0.22)" : "rgba(138,144,160,0.10)";
          ctx.beginPath();
          ctx.roundRect(x, 6, laneW, size - 12, 6);
          ctx.fill();
        }
        let ahead: number | null = null;
        let behind: number | null = null;
        for (const r of f.radar) {
          if (r.overlap) {
            block(r.lat < 0 ? lanes.left : lanes.right, r.lon, "#ff5a1f");
            continue;
          }
          if (r.lon > 0) ahead = ahead == null ? r.lon : Math.min(ahead, r.lon);
          else behind = behind == null ? r.lon : Math.max(behind, r.lon);
        }
        // A side reported without a matching car (sim and track distance disagree): light the lane.
        if (leftOn && !f.radar.some((r) => r.overlap && r.lat < 0)) block(lanes.left, 0, "#ff5a1f");
        if (rightOn && !f.radar.some((r) => r.overlap && r.lat > 0)) block(lanes.right, 0, "#ff5a1f");
        const centreX = mid - laneW * 0.35;
        const centreW = laneW * 0.7;
        const near = (lon: number) => {
          ctx.fillStyle = Math.abs(lon) < 10 ? "#f59e0b" : "#22d3ee";
          ctx.beginPath();
          ctx.roundRect(centreX, mid - lon * s - 2.4 * s, centreW, 4.8 * s, 4);
          ctx.fill();
        };
        if (ahead != null) near(ahead);
        if (behind != null) near(behind);
        ctx.fillStyle = "#f2f4f8";
        ctx.beginPath();
        ctx.roundRect(centreX, mid - 2.4 * s, centreW, 4.8 * s, 4);
        ctx.fill();
        ctx.textAlign = "center";
        if (showDistance) {
          ctx.font = "600 12px 'JetBrains Mono Variable', monospace";
          ctx.fillStyle = "#f2f4f8";
          if (ahead != null) ctx.fillText(`${Math.round(ahead)} m`, mid, 16);
          if (behind != null) ctx.fillText(`${Math.round(-behind)} m`, mid, size - 8);
        }
        if (side === "three_wide") {
          ctx.font = "700 13px Inter Variable, sans-serif";
          ctx.fillStyle = "#ff5a1f";
          ctx.fillText("3-WIDE", mid, mid + 4.8 * s + 16);
        }
        raf = requestAnimationFrame(draw);
        return;
      } else {
        for (const r of f?.radar ?? []) {
          car(mid + r.lat * s, mid - r.lon * s, r.overlap ? "#ff5a1f" : "#22d3ee");
        }
      }
      car(mid, mid, "#f2f4f8");
      raf = requestAnimationFrame(draw);
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  }, [liveRef, size, onActive, showDistance]);
  return <canvas ref={canvas} style={{ width: size, height: size }} />;
}
